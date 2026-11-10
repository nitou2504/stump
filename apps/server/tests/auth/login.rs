use std::time::Instant;

use axum::http::StatusCode;
use serde_json::json;

use super::{
	insert_user, is_locked, one_password_check, tracked, FAILED_ATTEMPTS_TO_LOCK,
};
use crate::common::TestApp;

async fn login(app: &TestApp, username: &str, password: &str) -> StatusCode {
	app.server
		.post("/api/v2/auth/login")
		.json(&json!({ "username": username, "password": password }))
		.await
		.status_code()
}

/// anybody who knows the owner's username could otherwise lock the owner out by guessing
#[tokio::test]
async fn failed_logins_never_lock_the_server_owner() {
	let app = TestApp::new().await;
	let owner = insert_user(&app, "owner", "secret", true).await;

	for _ in 0..FAILED_ATTEMPTS_TO_LOCK + 3 {
		assert_eq!(
			login(&app, "owner", "wrong").await,
			StatusCode::UNAUTHORIZED
		);
	}

	assert!(!is_locked(&app, &owner).await);
	assert_eq!(login(&app, "owner", "secret").await, StatusCode::OK);
	// the attempts are still on record
	assert_eq!(
		tracked(&app, &owner, false).await,
		(FAILED_ATTEMPTS_TO_LOCK + 3) as u64
	);
}

/// the lock stays for everyone else: it comes with the 10th failure in 24 hours
#[tokio::test]
async fn failed_logins_lock_other_users() {
	let app = TestApp::new().await;
	let reader = insert_user(&app, "reader", "secret", false).await;

	for _ in 1..FAILED_ATTEMPTS_TO_LOCK {
		assert_eq!(
			login(&app, "reader", "wrong").await,
			StatusCode::UNAUTHORIZED
		);
	}
	assert!(!is_locked(&app, &reader).await, "locked too early");

	assert_eq!(
		login(&app, "reader", "wrong").await,
		StatusCode::UNAUTHORIZED
	);
	assert!(is_locked(&app, &reader).await);
	assert_eq!(
		tracked(&app, &reader, false).await,
		FAILED_ATTEMPTS_TO_LOCK as u64
	);

	// the right password doesn't get in any more
	assert_eq!(login(&app, "reader", "secret").await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_successful_login_is_tracked() {
	let app = TestApp::new().await;
	let reader = insert_user(&app, "reader", "secret", false).await;

	assert_eq!(login(&app, "reader", "secret").await, StatusCode::OK);
	assert_eq!(tracked(&app, &reader, true).await, 1);
	assert_eq!(tracked(&app, &reader, false).await, 0);
}

/// refusing an unknown username takes as long as refusing a wrong password, so the time doesn't
/// show which usernames exist
#[tokio::test]
async fn an_unknown_username_takes_as_long_to_refuse_as_a_wrong_password() {
	let app = TestApp::new().await;
	let one_check = one_password_check(&app);

	let start = Instant::now();
	let status = login(&app, "nobody", "wrong").await;
	let elapsed = start.elapsed();

	assert_eq!(status, StatusCode::UNAUTHORIZED);
	assert!(
		elapsed >= one_check / 2,
		"refused in {elapsed:?}, a password check takes {one_check:?}"
	);
}
