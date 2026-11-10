use std::time::Instant;

use axum::http::StatusCode;
use axum_test::TestResponse;
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{Duration, Utc};
use models::entity::{user, user_login_activity};
use sea_orm::{
	prelude::DateTimeWithTimeZone, sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter,
};
use tests::fake_data;

use super::{
	insert_user, is_locked, one_password_check, tracked, FAILED_ATTEMPTS_TO_LOCK,
};
use crate::common::TestApp;

/// an OPDS 1.2 request signed in with HTTP Basic auth
async fn catalog(app: &TestApp, username: &str, password: &str) -> TestResponse {
	app.server
		.get("/opds/v1.2/catalog")
		.add_header(
			"Authorization",
			format!(
				"Basic {}",
				STANDARD.encode(format!("{username}:{password}"))
			),
		)
		.await
}

#[tokio::test]
async fn basic_auth_signs_in() {
	let app = TestApp::new().await;
	insert_user(&app, "reader", "secret", false).await;

	catalog(&app, "reader", "secret").await.assert_status_ok();
	catalog(&app, "reader", "wrong")
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
}

/// it had no lock and no record at all: guessing a password was free
#[tokio::test]
async fn failed_basic_auth_is_tracked_and_locks_other_users() {
	let app = TestApp::new().await;
	let reader = insert_user(&app, "reader", "secret", false).await;

	for _ in 1..FAILED_ATTEMPTS_TO_LOCK {
		catalog(&app, "reader", "wrong")
			.await
			.assert_status(StatusCode::UNAUTHORIZED);
	}
	assert!(!is_locked(&app, &reader).await, "locked too early");

	catalog(&app, "reader", "wrong")
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	assert!(is_locked(&app, &reader).await);
	assert_eq!(
		tracked(&app, &reader, false).await,
		FAILED_ATTEMPTS_TO_LOCK as u64
	);

	// locked: the right password is refused, and nothing more is tracked
	catalog(&app, "reader", "secret")
		.await
		.assert_status(StatusCode::FORBIDDEN);
	catalog(&app, "reader", "wrong")
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	assert_eq!(
		tracked(&app, &reader, false).await,
		FAILED_ATTEMPTS_TO_LOCK as u64
	);
}

#[tokio::test]
async fn failed_basic_auth_never_locks_the_server_owner() {
	let app = TestApp::new().await;
	let owner = insert_user(&app, "owner", "secret", true).await;

	for _ in 0..FAILED_ATTEMPTS_TO_LOCK + 3 {
		catalog(&app, "owner", "wrong")
			.await
			.assert_status(StatusCode::UNAUTHORIZED);
	}

	assert!(!is_locked(&app, &owner).await);
	catalog(&app, "owner", "secret").await.assert_status_ok();
	assert_eq!(
		tracked(&app, &owner, false).await,
		(FAILED_ATTEMPTS_TO_LOCK + 3) as u64
	);
}

#[tokio::test]
async fn basic_auth_ignores_deleted_users() {
	let app = TestApp::new().await;
	let reader = insert_user(&app, "reader", "secret", false).await;
	let deleted_at: DateTimeWithTimeZone = Utc::now().into();
	user::Entity::update_many()
		.col_expr(user::Column::DeletedAt, Expr::value(deleted_at))
		.filter(user::Column::Id.eq(reader.id.clone()))
		.exec(app.conn())
		.await
		.unwrap();

	catalog(&app, "reader", "secret")
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	assert_eq!(tracked(&app, &reader, true).await, 0);
	assert_eq!(tracked(&app, &reader, false).await, 0);
}

/// an account with no password (an OIDC one) is refused like a wrong password, not with a 500
#[tokio::test]
async fn basic_auth_refuses_accounts_without_a_password() {
	let app = TestApp::new().await;
	fake_data::User::new("sso-user").insert(app.conn()).await;

	catalog(&app, "sso-user", "anything")
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
}

/// an OPDS reader sends its credentials with every request: one row for the client, not one for
/// each request
#[tokio::test]
async fn basic_auth_tracks_a_success_once_an_hour_for_each_client() {
	let app = TestApp::new().await;
	let reader = insert_user(&app, "reader", "secret", false).await;

	for _ in 0..4 {
		catalog(&app, "reader", "secret").await.assert_status_ok();
	}
	assert_eq!(tracked(&app, &reader, true).await, 1);

	// another client (a different user agent) is a new sign-in, right away
	user_login_activity::Entity::update_many()
		.col_expr(
			user_login_activity::Column::UserAgent,
			Expr::value("KOReader"),
		)
		.exec(app.conn())
		.await
		.unwrap();
	catalog(&app, "reader", "secret").await.assert_status_ok();
	assert_eq!(tracked(&app, &reader, true).await, 2);
	catalog(&app, "reader", "secret").await.assert_status_ok();
	assert_eq!(tracked(&app, &reader, true).await, 2);

	// and the same client is tracked again once its last sign-in is more than an hour old
	let ninety_minutes_ago: DateTimeWithTimeZone =
		(Utc::now() - Duration::minutes(90)).into();
	user_login_activity::Entity::update_many()
		.col_expr(
			user_login_activity::Column::Timestamp,
			Expr::value(ninety_minutes_ago),
		)
		.exec(app.conn())
		.await
		.unwrap();
	catalog(&app, "reader", "secret").await.assert_status_ok();
	assert_eq!(tracked(&app, &reader, true).await, 3);
}

#[tokio::test]
async fn an_unknown_basic_auth_username_takes_as_long_to_refuse_as_a_wrong_password() {
	let app = TestApp::new().await;
	let one_check = one_password_check(&app);

	let start = Instant::now();
	let response = catalog(&app, "nobody", "wrong").await;
	let elapsed = start.elapsed();

	response.assert_status(StatusCode::UNAUTHORIZED);
	assert!(
		elapsed >= one_check / 2,
		"refused in {elapsed:?}, a password check takes {one_check:?}"
	);
}
