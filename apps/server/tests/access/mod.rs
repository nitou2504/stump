//! Public access: the edge middleware (who reached the app, with which pass), the invite pages,
//! the sign-in rules that depend on a pass, and the owner's GraphQL API for all of it

mod devices;
mod edge;
mod friend_set;
mod graphql;
mod invites;
mod libraries;
mod opds_keys;
mod password;
mod public_signin;
mod retention;
mod signin;
mod signup;
mod uploads;
mod warnings;

use axum_test::TestResponse;
use chrono::Utc;
use models::entity::{access_device, access_invite, user};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{json, Value};
use stump_core::{
	access::pass,
	config::{EdgeConfig, StumpConfig},
};
use tempfile::TempDir;

use crate::common::TestApp;

/// The key the edge and the app share in these tests (64 characters)
pub const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
pub const PUBLIC_URL: &str = "https://g3.test:8443";
pub const COOKIE: &str = "__Host-g3-stump";

/// An app with the edge configured, with its state folder, and the owner's token set
pub struct EdgeApp {
	pub app: TestApp,
	pub state_dir: TempDir,
}

/// An app whose edge settings are `edge(dir)`: the state folder is a fresh temporary one
pub async fn edge_app_with(edge: impl FnOnce(&TempDir) -> Option<EdgeConfig>) -> EdgeApp {
	let state_dir = tempfile::tempdir().expect("should create a temp dir");
	let mut config = StumpConfig::debug();
	// the cheapest bcrypt there is: these tests sign in a lot
	config.password_hash_cost = 4;
	config.edge = edge(&state_dir);

	let app = TestApp::with_config(config).await;
	let token = app.create_initial_account().await;
	*app.access_token.write().await = Some(token);
	EdgeApp { app, state_dir }
}

pub fn edge_settings(dir: &TempDir, require_pass: bool) -> EdgeConfig {
	EdgeConfig {
		key: Some(KEY.to_string()),
		cookie: COOKIE.to_string(),
		public_url: Some(PUBLIC_URL.to_string()),
		state_dir: Some(dir.path().to_string_lossy().to_string()),
		require_pass,
		binds_person: true,
		public_signin: false,
	}
}

pub async fn edge_app(require_pass: bool) -> EdgeApp {
	edge_app_with(|dir| Some(edge_settings(dir, require_pass))).await
}

/// A user put straight into the database (the lowest bcrypt cost there is)
pub async fn insert_user(
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

/// A pass for `device_id`, as the app mints one now
pub fn pass_for(device_id: &str) -> String {
	pass::mint_pass(KEY, device_id, Utc::now().timestamp())
}

/// A GET that came through the public edge, with a pass when given
pub async fn public_get(app: &TestApp, path: &str, pass: Option<&str>) -> TestResponse {
	let mut request = app.server.get(path).add_header("x-edge-public", "1");
	if let Some(pass) = pass {
		request = request.add_header("x-edge-pass", pass.to_string());
	}
	request.await
}

/// A sign-in that came through the public edge, with a pass when given
pub async fn public_login(
	app: &TestApp,
	username: &str,
	password: &str,
	pass: Option<&str>,
) -> TestResponse {
	let mut request = app
		.server
		.post("/api/v2/auth/login")
		.add_header("x-edge-public", "1");
	if let Some(pass) = pass {
		request = request.add_header("x-edge-pass", pass.to_string());
	}
	request
		.json(&json!({ "username": username, "password": password }))
		.await
}

/// Asks the owner's GraphQL (at home, with the owner's token)
pub async fn owner_gql(app: &TestApp, query: &str, variables: Option<Value>) -> Value {
	app.execute_gql(query, variables).await
}

pub fn errors_of(response: &Value) -> String {
	response
		.get("errors")
		.map(|errors| errors.to_string())
		.unwrap_or_default()
}

/// Makes an invite through GraphQL as the owner. Returns its id and its link
pub async fn make_invite(
	app: &TestApp,
	person_id: &str,
	label: &str,
	max_uses: i32,
) -> (String, String) {
	let response = owner_gql(
		app,
		r#"mutation($input: CreateAccessInviteInput!) {
			createAccessInvite(input: $input) { invite { id } link }
		}"#,
		Some(json!({"input": {
			"personId": person_id, "label": label, "validForHours": 72, "maxUses": max_uses
		}})),
	)
	.await;
	assert!(
		response.get("errors").is_none(),
		"createAccessInvite failed: {response:#}"
	);
	let created = &response["data"]["createAccessInvite"];
	(
		created["invite"]["id"].as_str().unwrap().to_string(),
		created["link"].as_str().unwrap().to_string(),
	)
}

/// The token in an invite link
pub fn token_of(link: &str) -> &str {
	link.rsplit("/api/_i/").next().expect("a link has a token")
}

/// Opens the confirmation page's button: a `POST` of the link, as a form on this site sends it
pub async fn redeem(app: &TestApp, link: &str, pass: Option<&str>) -> TestResponse {
	let mut request = app
		.server
		.post("/api/_i/")
		.add_header("x-edge-public", "1")
		.add_header("sec-fetch-site", "same-origin")
		.add_header("origin", "null")
		.form(&[("t", link)]);
	if let Some(pass) = pass {
		request = request.add_header("x-edge-pass", pass.to_string());
	}
	request.await
}

/// The pass in the `Set-Cookie` of a response, when there is one
pub fn pass_in(response: &TestResponse) -> Option<String> {
	response
		.headers()
		.get_all("set-cookie")
		.iter()
		.filter_map(|value| value.to_str().ok())
		.find_map(|value| {
			value
				.strip_prefix(&format!("{COOKIE}="))
				.and_then(|rest| rest.split(';').next())
				.map(str::to_string)
		})
}

pub async fn invite_row(app: &TestApp, id: &str) -> access_invite::Model {
	access_invite::Entity::find_by_id(id.to_string())
		.one(app.conn())
		.await
		.unwrap()
		.expect("the invite is there")
}

pub async fn device_row(app: &TestApp, id: &str) -> Option<access_device::Model> {
	access_device::Entity::find_by_id(id.to_string())
		.one(app.conn())
		.await
		.unwrap()
}

// -------------------------------------------------------------------------------------------
// friend onboarding: public sign-in on, links that make accounts, friends signing in
// -------------------------------------------------------------------------------------------

use models::entity::{
	access_audit, access_library_decision, access_person, access_signup_link, library,
	library_exclusion, session, user_login_activity,
};
use sea_orm::{ColumnTrait, PaginatorTrait, QueryFilter};

/// An app with the edge configured **and public sign-in on**, as the live one will be. It trusts
/// `X-Real-IP`, so one test can be several clients; every account password costs the least bcrypt
/// there is
pub async fn signin_app() -> EdgeApp {
	let state_dir = tempfile::tempdir().expect("should create a temp dir");
	let mut config = StumpConfig::debug();
	config.password_hash_cost = 4;
	config.trust_proxy_headers = true;
	let mut edge = edge_settings(&state_dir, true);
	edge.public_signin = true;
	config.edge = Some(edge);

	let app = TestApp::with_config(config).await;
	let token = app.create_initial_account().await;
	*app.access_token.write().await = Some(token);
	EdgeApp { app, state_dir }
}

/// The libraries of a world with a "Biblioteca" and a "Manga"
pub struct Shelves {
	pub biblioteca: library::Model,
	pub manga: library::Model,
}

pub async fn shelves(app: &TestApp) -> Shelves {
	Shelves {
		biblioteca: crate::common::visibility::add_library(app, "Biblioteca").await,
		manga: crate::common::visibility::add_library(app, "Manga").await,
	}
}

/// Makes a signup link as the owner (at home). `kind` is `SINGLE` or `OPEN`. Returns its id and
/// its address
pub async fn make_signup_link(
	app: &TestApp,
	kind: &str,
	max_uses: i32,
	library_ids: &[&str],
) -> (String, String) {
	let response = make_signup_link_with(
		app,
		json!({
			"label": "book club",
			"kind": kind,
			"validForHours": 72,
			"maxUses": max_uses,
			"preset": {
				"permissions": ["CHANGE_PASSWORD", "CHANGE_AVATAR", "DOWNLOAD_FILE", "ADD_BOOKS"],
				"libraryIds": library_ids,
				"maxDevices": 3
			}
		}),
	)
	.await;
	assert!(
		response.get("errors").is_none(),
		"createAccessSignupLink failed: {response:#}"
	);
	let created = &response["data"]["createAccessSignupLink"];
	(
		created["link"]["id"].as_str().unwrap().to_string(),
		created["url"].as_str().unwrap().to_string(),
	)
}

pub async fn make_signup_link_with(app: &TestApp, input: Value) -> Value {
	owner_gql(
		app,
		r#"mutation($input: CreateSignupLinkInput!) {
			createAccessSignupLink(input: $input) { url link { id status uses maxUses } }
		}"#,
		Some(json!({ "input": input })),
	)
	.await
}

pub async fn link_row(app: &TestApp, id: &str) -> access_signup_link::Model {
	access_signup_link::Entity::find_by_id(id.to_string())
		.one(app.conn())
		.await
		.unwrap()
		.expect("the signup link is there")
}

/// A `GET` of a link, as a browser through the edge (no pass, no session)
pub async fn open_link(app: &TestApp, token: &str) -> TestResponse {
	app.server
		.get(&format!("/api/_i/{token}"))
		.add_header("x-edge-public", "1")
		.await
}

/// The headers of a `POST` of a form from a page of this site, through the edge
fn form_post<'a>(
	app: &'a TestApp,
	path: &str,
	ip: &str,
	pass: Option<&str>,
) -> axum_test::TestRequest {
	let mut request = app
		.server
		.post(path)
		.add_header("x-edge-public", "1")
		.add_header("x-real-ip", ip.to_string())
		.add_header("sec-fetch-site", "same-origin")
		.add_header("origin", "null");
	if let Some(pass) = pass {
		request = request.add_header("x-edge-pass", pass.to_string());
	}
	request
}

/// What somebody types at the signup form
pub async fn join(
	app: &TestApp,
	token: &str,
	username: &str,
	password: &str,
	again: &str,
	pass: Option<&str>,
	ip: &str,
) -> TestResponse {
	form_post(app, "/api/_i/join", ip, pass)
		.form(&[
			("t", token),
			("username", username),
			("password", password),
			("password2", again),
		])
		.await
}

/// What somebody types at the sign-in page
pub async fn page_signin(
	app: &TestApp,
	username: &str,
	password: &str,
	pass: Option<&str>,
	ip: &str,
) -> TestResponse {
	form_post(app, "/api/_i/signin", ip, pass)
		.form(&[("username", username), ("password", password)])
		.await
}

/// The value of a cookie set by a response
pub fn cookie_in(response: &TestResponse, name: &str) -> Option<String> {
	response
		.headers()
		.get_all("set-cookie")
		.iter()
		.filter_map(|value| value.to_str().ok())
		.find_map(|value| {
			value
				.strip_prefix(&format!("{name}="))
				.and_then(|rest| rest.split(';').next())
				.map(str::to_string)
		})
}

/// A GraphQL request as a friend through the edge, with the pass and the session they were given
pub async fn friend_gql(
	app: &TestApp,
	pass: &str,
	session_cookie: &str,
	query: &str,
	variables: Option<Value>,
) -> TestResponse {
	let mut body = json!({ "query": query });
	if let Some(variables) = variables {
		body["variables"] = variables;
	}
	app.server
		.post("/api/graphql")
		.add_header("x-edge-public", "1")
		.add_header("x-edge-pass", pass.to_string())
		.add_header("cookie", format!("stump_session={session_cookie}"))
		.add_header("sec-fetch-site", "same-origin")
		.json(&body)
		.await
}

/// A friend who signed up with a link: what they were given
pub struct Friend {
	pub id: String,
	pub username: String,
	pub password: String,
	pub pass: String,
	pub session: String,
}

/// Makes a single-use link for `libraries` and signs up `username` with it, from `ip`
pub async fn sign_up_friend(
	app: &TestApp,
	username: &str,
	library_ids: &[&str],
	ip: &str,
) -> Friend {
	let (_, url) = make_signup_link(app, "SINGLE", 1, library_ids).await;
	// no friend's name is in it (the rules refuse a password that contains the username)
	let password = "tulip-Harbor-47".to_string();
	let response = join(
		app,
		token_of(&url),
		username,
		&password,
		&password,
		None,
		ip,
	)
	.await;
	assert_eq!(response.status_code(), 303, "{}", response.text());
	let account = user::Entity::find()
		.filter(user::Column::Username.eq(username.to_lowercase()))
		.one(app.conn())
		.await
		.unwrap()
		.expect("the account was made");
	Friend {
		id: account.id,
		username: account.username,
		password,
		pass: cookie_in(&response, COOKIE).expect("a pass was given"),
		session: cookie_in(&response, "stump_session").expect("a session was given"),
	}
}

pub async fn audit_actions(app: &TestApp) -> Vec<String> {
	access_audit::Entity::find()
		.all(app.conn())
		.await
		.unwrap()
		.into_iter()
		.map(|row| row.action)
		.collect()
}

pub async fn person_row_of(app: &TestApp, user_id: &str) -> Option<access_person::Model> {
	access_person::Entity::find_by_id(user_id.to_string())
		.one(app.conn())
		.await
		.unwrap()
}

pub async fn hidden_libraries_of(app: &TestApp, user_id: &str) -> Vec<String> {
	let mut ids: Vec<String> = library_exclusion::Entity::find()
		.filter(library_exclusion::Column::UserId.eq(user_id))
		.all(app.conn())
		.await
		.unwrap()
		.into_iter()
		.map(|row| row.library_id)
		.collect();
	ids.sort();
	ids
}

pub async fn decided_libraries_of(app: &TestApp, user_id: &str) -> usize {
	access_library_decision::Entity::find()
		.filter(access_library_decision::Column::UserId.eq(user_id))
		.count(app.conn())
		.await
		.unwrap() as usize
}

pub async fn sessions_of(app: &TestApp, user_id: &str) -> u64 {
	session::Entity::find()
		.filter(session::Column::UserId.eq(user_id))
		.count(app.conn())
		.await
		.unwrap()
}

pub async fn devices_of(app: &TestApp, user_id: &str) -> Vec<access_device::Model> {
	access_device::Entity::find()
		.filter(access_device::Column::PersonId.eq(user_id))
		.all(app.conn())
		.await
		.unwrap()
}

pub async fn user_row(app: &TestApp, username: &str) -> user::Model {
	user::Entity::find()
		.filter(user::Column::Username.eq(username))
		.one(app.conn())
		.await
		.unwrap()
		.expect("the user is there")
}

/// Sets the permissions of an account straight in the database (the owner's edit of a user)
pub async fn set_permissions(app: &TestApp, username: &str, permissions: &str) {
	use sea_orm::IntoActiveModel;
	let mut row = user_row(app, username).await.into_active_model();
	row.permissions = Set(Some(permissions.to_string()));
	row.update(app.conn()).await.unwrap();
}

/// Waits (up to three seconds) for something the server writes after its answer is on the way
pub async fn eventually<F, Fut>(mut check: F) -> bool
where
	F: FnMut() -> Fut,
	Fut: std::future::Future<Output = bool>,
{
	for _ in 0..60 {
		if check().await {
			return true;
		}
		tokio::time::sleep(std::time::Duration::from_millis(50)).await;
	}
	false
}

/// The sign-in log as the owner sees it: `(username, successful, reason)` of every attempt
pub async fn sign_in_log(app: &TestApp) -> Vec<(String, bool, Option<String>)> {
	let response = owner_gql(
		app,
		"{ accessSignIns(pagination: {page: 1, pageSize: 200}) { nodes { person { username } successful reason } } }",
		None,
	)
	.await;
	assert!(response.get("errors").is_none(), "{response:#}");
	response["data"]["accessSignIns"]["nodes"]
		.as_array()
		.unwrap()
		.iter()
		.map(|node| {
			(
				node["person"]["username"]
					.as_str()
					.unwrap_or_default()
					.to_string(),
				node["successful"].as_bool().unwrap(),
				node["reason"].as_str().map(str::to_string),
			)
		})
		.collect()
}

/// The page of a failed sign-in with the name that was typed taken out of it: two failures that
/// say the same thing are the same text
pub fn normalised(page: &str, typed: &str) -> String {
	page.replace(&format!("value=\"{typed}\""), "value=\"\"")
}

/// The warnings the owner sees, as `(kind, person's username or "")`
pub async fn warning_kinds(app: &TestApp) -> Vec<(String, String)> {
	let response = owner_gql(
		app,
		"{ accessWarnings { kind severity person { username } } }",
		None,
	)
	.await;
	assert!(response.get("errors").is_none(), "{response:#}");
	response["data"]["accessWarnings"]
		.as_array()
		.unwrap()
		.iter()
		.map(|warning| {
			(
				warning["kind"].as_str().unwrap().to_string(),
				warning["person"]["username"]
					.as_str()
					.unwrap_or_default()
					.to_string(),
			)
		})
		.collect()
}

/// Waits until every failed sign-in the server is still writing down has its mark (the failures
/// of the sign-in page are recorded after the answer, in a task of their own)
pub async fn settled(app: &TestApp) {
	use models::entity::access_signin_mark;
	let ready = eventually(|| async {
		let failed = user_login_activity::Entity::find()
			.filter(user_login_activity::Column::AuthenticationSuccessful.eq(false))
			.count(app.conn())
			.await
			.unwrap();
		let marked = access_signin_mark::Entity::find()
			.filter(access_signin_mark::Column::FailureReason.is_not_null())
			.count(app.conn())
			.await
			.unwrap();
		failed == marked
	})
	.await;
	assert!(ready, "the failed sign-ins were not all marked");
}
