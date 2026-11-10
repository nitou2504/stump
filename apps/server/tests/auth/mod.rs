//! Password sign-ins (the web login and OPDS HTTP Basic auth): tracking, locking, unknown users;
//! and what a user holds after signing in, and what takes it away

mod basic_auth;
mod lifecycle;
mod login;

use std::time::{Duration, Instant};

use axum_test::TestResponse;
use base64::{
	engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
	Engine,
};
use chrono::Utc;
use models::entity::{api_key, refresh_token, session, user, user_login_activity};
use sea_orm::{
	prelude::DateTimeWithTimeZone, sea_query::Expr, ActiveModelTrait, ColumnTrait,
	EntityTrait, PaginatorTrait, QueryFilter, Set,
};
use serde_json::{json, Value};

use crate::common::TestApp;

/// How many failed attempts in a row it takes to lock an account: the 10th one, after 9 others
const FAILED_ATTEMPTS_TO_LOCK: usize = 10;

/// Puts a user straight into the database. Their bcrypt cost is the lowest there is, so signing
/// in over and over stays fast.
async fn insert_user(
	app: &TestApp,
	username: &str,
	password: &str,
	is_server_owner: bool,
) -> user::Model {
	user::ActiveModel {
		username: Set(username.to_string()),
		hashed_password: Set(bcrypt::hash(password, 4).expect("could not hash")),
		is_server_owner: Set(is_server_owner),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("could not insert user")
}

async fn is_locked(app: &TestApp, user: &user::Model) -> bool {
	user::Entity::find_by_id(user.id.clone())
		.one(app.conn())
		.await
		.expect("could not read user")
		.expect("user is gone")
		.is_locked
}

/// The login activity rows of the user, successful or failed ones
async fn tracked(app: &TestApp, user: &user::Model, successful: bool) -> u64 {
	user_login_activity::Entity::find()
		.filter(user_login_activity::Column::UserId.eq(user.id.clone()))
		.filter(user_login_activity::Column::AuthenticationSuccessful.eq(successful))
		.count(app.conn())
		.await
		.expect("could not count login activity")
}

/// How long one password check takes on this machine
fn one_password_check(app: &TestApp) -> Duration {
	let hash = bcrypt::hash("password", app.ctx.config.password_hash_cost)
		.expect("could not hash");
	let start = Instant::now();
	let _ = bcrypt::verify("not the password", &hash);
	start.elapsed()
}

/// Like [`insert_user`], for a user who holds permissions (comma-separated, as they are stored)
async fn insert_user_with_permissions(
	app: &TestApp,
	username: &str,
	password: &str,
	permissions: &str,
) -> user::Model {
	user::ActiveModel {
		username: Set(username.to_string()),
		hashed_password: Set(bcrypt::hash(password, 4).expect("could not hash")),
		is_server_owner: Set(false),
		permissions: Set(Some(permissions.to_string())),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("could not insert user")
}

/// Locks the account, as the owner or too many failed logins do (without signing anybody out)
async fn lock(app: &TestApp, user: &user::Model) {
	user::Entity::update_many()
		.col_expr(user::Column::IsLocked, Expr::value(true))
		.filter(user::Column::Id.eq(user.id.clone()))
		.exec(app.conn())
		.await
		.expect("could not lock user");
}

/// Marks the user as deleted, as the web app's default delete does (without signing anybody out)
async fn soft_delete(app: &TestApp, user: &user::Model) {
	let deleted_at: DateTimeWithTimeZone = Utc::now().into();
	user::Entity::update_many()
		.col_expr(user::Column::DeletedAt, Expr::value(deleted_at))
		.filter(user::Column::Id.eq(user.id.clone()))
		.exec(app.conn())
		.await
		.expect("could not delete user");
}

/// The `Authorization` header value of an HTTP Basic sign-in
fn basic(username: &str, password: &str) -> String {
	format!(
		"Basic {}",
		STANDARD.encode(format!("{username}:{password}"))
	)
}

/// an OPDS 1.2 request signed in with HTTP Basic auth
async fn catalog(app: &TestApp, username: &str, password: &str) -> TestResponse {
	app.server
		.get("/opds/v1.2/catalog")
		.add_header("Authorization", basic(username, password))
		.await
}

/// What a user holds after one sign-in that asks for tokens: a session cookie, and an access
/// and a refresh token
struct SignedIn {
	/// the value of the `Cookie` header that carries the session
	cookie: String,
	access_token: String,
	refresh_token: String,
}

async fn sign_in(app: &TestApp, username: &str, password: &str) -> SignedIn {
	let response = app
		.server
		.post("/api/v2/auth/login?generate_token=true")
		.json(&json!({ "username": username, "password": password }))
		.await;
	response.assert_status_ok();

	let cookie = format!("stump_session={}", response.cookie("stump_session").value());
	let body: Value = response.json();
	SignedIn {
		cookie,
		access_token: body["accessToken"]
			.as_str()
			.expect("no access token")
			.to_string(),
		refresh_token: body["refreshToken"]
			.as_str()
			.expect("no refresh token")
			.to_string(),
	}
}

/// `GET /api/v2/auth/me` with the session cookie of a sign-in
async fn me_with_cookie(app: &TestApp, cookie: &str) -> TestResponse {
	app.server
		.get("/api/v2/auth/me")
		.add_header("cookie", cookie)
		.await
}

/// `GET /api/v2/auth/me` with a bearer token: an access token or an API key
async fn me_with_bearer(app: &TestApp, token: &str) -> TestResponse {
	app.server
		.get("/api/v2/auth/me")
		.add_header("Authorization", format!("Bearer {token}"))
		.await
}

/// Exchanges a refresh token for a new pair of tokens
async fn refresh(app: &TestApp, refresh_token: &str) -> TestResponse {
	app.server
		.post("/api/v2/auth/refresh-token")
		.add_header("Authorization", format!("Bearer {refresh_token}"))
		.await
}

/// The id a refresh token is stored under (its `jti` claim)
fn refresh_token_id(refresh_token: &str) -> String {
	let payload = refresh_token.split('.').nth(1).expect("not a JWT");
	let claims: Value =
		serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).expect("not base64"))
			.expect("claims are not JSON");
	claims["jti"].as_str().expect("no jti").to_string()
}

async fn sessions_of(app: &TestApp, user_id: &str) -> u64 {
	session::Entity::find()
		.filter(session::Column::UserId.eq(user_id))
		.count(app.conn())
		.await
		.expect("could not count sessions")
}

async fn refresh_tokens_of(app: &TestApp, user_id: &str) -> u64 {
	refresh_token::Entity::find()
		.filter(refresh_token::Column::UserId.eq(user_id))
		.count(app.conn())
		.await
		.expect("could not count refresh tokens")
}

async fn api_keys_of(app: &TestApp, user_id: &str) -> u64 {
	api_key::Entity::find()
		.filter(api_key::Column::UserId.eq(user_id))
		.count(app.conn())
		.await
		.expect("could not count API keys")
}
