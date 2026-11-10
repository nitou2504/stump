use std::time::Instant;

use axum::http::StatusCode;
use serde_json::json;

use super::{
	insert_user, is_locked, lock, one_password_check, sessions_of, sign_in, tracked,
	FAILED_ATTEMPTS_TO_LOCK,
};
use crate::common::TestApp;

async fn login(app: &TestApp, username: &str, password: &str) -> StatusCode {
	app.server
		.post("/api/v2/auth/login")
		.json(&json!({ "username": username, "password": password }))
		.await
		.status_code()
}

/// A sign-in that came through the public edge, which marks the sign-ins it passes on with this
/// header (and strips it from every other request)
async fn login_through_the_edge(
	app: &TestApp,
	username: &str,
	password: &str,
) -> StatusCode {
	app.server
		.post("/api/v2/auth/login")
		.add_header("x-edge-public", "1")
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

/// a locked account must not tell a right password from a wrong one, or guesses could be checked
/// against it; every attempt is on record, none as a success
#[tokio::test]
async fn a_locked_account_answers_the_same_to_every_password() {
	let app = TestApp::new().await;
	let reader = insert_user(&app, "reader", "secret", false).await;
	lock(&app, &reader).await;

	for password in ["secret", "wrong", "secret", "wrong"] {
		assert_eq!(
			login(&app, "reader", password).await,
			StatusCode::FORBIDDEN,
			"locked, tried {password:?}"
		);
	}
	assert_eq!(tracked(&app, &reader, false).await, 4);
	assert_eq!(tracked(&app, &reader, true).await, 0);
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

/// the owner's username is easy to guess, so the owner doesn't sign in through the public edge:
/// the right password is refused there like a wrong one, and nothing is signed in
#[tokio::test]
async fn the_owner_cannot_sign_in_through_the_public_edge() {
	let app = TestApp::new().await;
	let owner = insert_user(&app, "owner", "secret", true).await;

	for password in ["secret", "wrong"] {
		assert_eq!(
			login_through_the_edge(&app, "owner", password).await,
			StatusCode::UNAUTHORIZED,
			"through the edge, tried {password:?}"
		);
	}

	assert_eq!(
		sessions_of(&app, &owner.id).await,
		0,
		"nobody was signed in"
	);
	// both are on record as failures, none as a success
	assert_eq!(tracked(&app, &owner, false).await, 2);
	assert_eq!(tracked(&app, &owner, true).await, 0);
}

/// the same sign-in from home (no header) works, and however many were refused at the edge, they
/// never lock the owner
#[tokio::test]
async fn the_owner_signs_in_from_home_and_the_edge_never_locks_them() {
	let app = TestApp::new().await;
	let owner = insert_user(&app, "owner", "secret", true).await;

	assert_eq!(login(&app, "owner", "secret").await, StatusCode::OK);

	for _ in 0..FAILED_ATTEMPTS_TO_LOCK + 3 {
		assert_eq!(
			login_through_the_edge(&app, "owner", "secret").await,
			StatusCode::UNAUTHORIZED
		);
	}

	assert!(!is_locked(&app, &owner).await);
	assert_eq!(login(&app, "owner", "secret").await, StatusCode::OK);
}

/// friends and everyone else sign in through the edge as usual
#[tokio::test]
async fn other_users_sign_in_through_the_public_edge() {
	let app = TestApp::new().await;
	insert_user(&app, "owner", "secret", true).await;
	let reader = insert_user(&app, "reader", "secret", false).await;

	assert_eq!(
		login_through_the_edge(&app, "reader", "secret").await,
		StatusCode::OK
	);
	assert_eq!(tracked(&app, &reader, true).await, 1);

	// a wrong password is still a wrong password for them, and counts towards their lock
	assert_eq!(
		login_through_the_edge(&app, "reader", "wrong").await,
		StatusCode::UNAUTHORIZED
	);
	assert_eq!(tracked(&app, &reader, false).await, 1);
}

/// the check comes before the shortcut for someone who already holds a session: an owner who
/// signed in at home is refused through the edge, and not at home
#[tokio::test]
async fn an_owner_with_a_session_is_refused_through_the_public_edge_too() {
	let app = TestApp::new().await;
	insert_user(&app, "owner", "secret", true).await;
	let signed_in = sign_in(&app, "owner", "secret").await;
	let body = json!({ "username": "owner", "password": "secret" });

	let at_home = app
		.server
		.post("/api/v2/auth/login")
		.add_header("cookie", signed_in.cookie.clone())
		.json(&body)
		.await;
	assert_eq!(at_home.status_code(), StatusCode::OK);

	let through_the_edge = app
		.server
		.post("/api/v2/auth/login")
		.add_header("cookie", signed_in.cookie.clone())
		.add_header("x-edge-public", "1")
		.json(&body)
		.await;
	assert_eq!(through_the_edge.status_code(), StatusCode::UNAUTHORIZED);
}

/// it is refused after the same work as a wrong password, so how long it takes doesn't say that
/// this is the owner
#[tokio::test]
async fn the_owner_takes_as_long_to_refuse_through_the_public_edge_as_a_wrong_password() {
	let app = TestApp::new().await;
	insert_user(&app, "owner", "secret", true).await;
	let one_check = one_password_check(&app);

	let start = Instant::now();
	let status = login_through_the_edge(&app, "owner", "secret").await;
	let elapsed = start.elapsed();

	assert_eq!(status, StatusCode::UNAUTHORIZED);
	assert!(
		elapsed >= one_check / 2,
		"refused in {elapsed:?}, a password check takes {one_check:?}"
	);
}
