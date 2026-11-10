//! What takes a user's sign-ins away: deleting them, locking them, a new password, signing them
//! out. A user holds a session cookie, access and refresh tokens, API keys and an OPDS login;
//! all of them must stop working, not only the password.

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use models::entity::refresh_token;
use sea_orm::{
	prelude::DateTimeWithTimeZone, sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter,
};
use serde_json::{json, Value};

use super::{
	api_keys_of, catalog, insert_user_with_permissions, lock, me_with_bearer,
	me_with_cookie, refresh, refresh_token_id, refresh_tokens_of, sessions_of, sign_in,
	soft_delete, FAILED_ATTEMPTS_TO_LOCK,
};
use crate::common::{api_key::create_api_key_for_user, TestApp};

/// runs a GraphQL mutation as the server owner and expects it to work
async fn owner_runs(app: &TestApp, query: &str, variables: Value) -> Value {
	let response = app.execute_gql(query, Some(variables)).await;
	assert!(
		response.get("errors").is_none(),
		"the mutation failed: {response:#}"
	);
	response
}

async fn login_status(app: &TestApp, username: &str, password: &str) -> StatusCode {
	app.server
		.post("/api/v2/auth/login")
		.json(&json!({ "username": username, "password": password }))
		.await
		.status_code()
}

/// the web app's default delete only marked the user as deleted: their cookie, access token,
/// refresh token (it could be exchanged for new ones, for ever) and OPDS login kept working
#[tokio::test]
async fn deleting_a_user_cuts_off_everything_they_hold() {
	let app = TestApp::new_with_default_user().await;
	let friend =
		insert_user_with_permissions(&app, "friend", "secret", "ACCESS_API_KEYS").await;
	let signed_in = sign_in(&app, "friend", "secret").await;
	let (api_key, _) = create_api_key_for_user(&app, &friend, None).await;

	me_with_cookie(&app, &signed_in.cookie)
		.await
		.assert_status_ok();
	me_with_bearer(&app, &signed_in.access_token)
		.await
		.assert_status_ok();
	me_with_bearer(&app, &api_key).await.assert_status_ok();
	catalog(&app, "friend", "secret").await.assert_status_ok();

	owner_runs(
		&app,
		"mutation($id: ID!) { deleteUser(id: $id, hardDelete: false) { id } }",
		json!({ "id": friend.id }),
	)
	.await;

	// nothing they held is left behind
	assert_eq!(sessions_of(&app, &friend.id).await, 0);
	assert_eq!(refresh_tokens_of(&app, &friend.id).await, 0);
	assert_eq!(api_keys_of(&app, &friend.id).await, 0);

	// and none of it works
	me_with_cookie(&app, &signed_in.cookie)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	me_with_bearer(&app, &signed_in.access_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	me_with_bearer(&app, &api_key)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	refresh(&app, &signed_in.refresh_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	catalog(&app, "friend", "secret")
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	assert_eq!(
		login_status(&app, "friend", "secret").await,
		StatusCode::UNAUTHORIZED
	);
}

/// even if nothing was cleaned up (a user deleted before this was fixed, a failed cleanup), a
/// deleted user is refused on every path
#[tokio::test]
async fn a_deleted_user_is_refused_even_if_nothing_was_revoked() {
	let app = TestApp::new().await;
	let friend = insert_user_with_permissions(&app, "friend", "secret", "").await;
	let signed_in = sign_in(&app, "friend", "secret").await;

	soft_delete(&app, &friend).await;
	assert_eq!(
		sessions_of(&app, &friend.id).await,
		1,
		"the session is still there"
	);
	assert_eq!(refresh_tokens_of(&app, &friend.id).await, 1);

	me_with_cookie(&app, &signed_in.cookie)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	me_with_bearer(&app, &signed_in.access_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	refresh(&app, &signed_in.refresh_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	catalog(&app, "friend", "secret")
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	// the refused refresh token is forgotten
	assert_eq!(refresh_tokens_of(&app, &friend.id).await, 0);
}

/// locking signed the user out of their sessions only: their refresh token still handed out new
/// access tokens, and their API keys kept working
#[tokio::test]
async fn locking_a_user_cuts_off_everything_they_hold() {
	let app = TestApp::new_with_default_user().await;
	let friend =
		insert_user_with_permissions(&app, "friend", "secret", "ACCESS_API_KEYS").await;
	let signed_in = sign_in(&app, "friend", "secret").await;
	let (api_key, _) = create_api_key_for_user(&app, &friend, None).await;

	owner_runs(
		&app,
		"mutation($id: ID!) { updateUserLockStatus(id: $id, lock: true) { id } }",
		json!({ "id": friend.id }),
	)
	.await;

	assert_eq!(sessions_of(&app, &friend.id).await, 0);
	assert_eq!(refresh_tokens_of(&app, &friend.id).await, 0);
	// the API key stays (it comes back with an unlock), but it is refused meanwhile
	assert_eq!(api_keys_of(&app, &friend.id).await, 1);

	me_with_cookie(&app, &signed_in.cookie)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	me_with_bearer(&app, &signed_in.access_token)
		.await
		.assert_status(StatusCode::FORBIDDEN);
	me_with_bearer(&app, &api_key)
		.await
		.assert_status(StatusCode::FORBIDDEN);
	refresh(&app, &signed_in.refresh_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	catalog(&app, "friend", "secret")
		.await
		.assert_status(StatusCode::FORBIDDEN);
	assert_eq!(
		login_status(&app, "friend", "secret").await,
		StatusCode::FORBIDDEN
	);

	// unlocked: the user signs in again, with the same API key
	owner_runs(
		&app,
		"mutation($id: ID!) { updateUserLockStatus(id: $id, lock: false) { id } }",
		json!({ "id": friend.id }),
	)
	.await;
	assert_eq!(login_status(&app, "friend", "secret").await, StatusCode::OK);
	me_with_bearer(&app, &api_key).await.assert_status_ok();
}

/// a locked user is refused on the API key and the refresh paths even when they were not cleaned
/// up (a lock made before this was fixed, or straight in the database)
#[tokio::test]
async fn a_locked_user_is_refused_even_if_nothing_was_revoked() {
	let app = TestApp::new().await;
	let friend =
		insert_user_with_permissions(&app, "friend", "secret", "ACCESS_API_KEYS").await;
	let signed_in = sign_in(&app, "friend", "secret").await;
	let (api_key, _) = create_api_key_for_user(&app, &friend, None).await;
	me_with_bearer(&app, &api_key).await.assert_status_ok();

	lock(&app, &friend).await;
	assert_eq!(refresh_tokens_of(&app, &friend.id).await, 1);

	me_with_bearer(&app, &api_key)
		.await
		.assert_status(StatusCode::FORBIDDEN);
	me_with_bearer(&app, &signed_in.access_token)
		.await
		.assert_status(StatusCode::FORBIDDEN);
	refresh(&app, &signed_in.refresh_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	assert_eq!(refresh_tokens_of(&app, &friend.id).await, 0);
}

/// the failed logins that lock an account sign it out of everything, like a lock by the owner
#[tokio::test]
async fn too_many_failed_logins_sign_the_user_out_everywhere() {
	let app = TestApp::new().await;
	let friend = insert_user_with_permissions(&app, "friend", "secret", "").await;
	let signed_in = sign_in(&app, "friend", "secret").await;
	assert_eq!(sessions_of(&app, &friend.id).await, 1);
	assert_eq!(refresh_tokens_of(&app, &friend.id).await, 1);

	for _ in 0..FAILED_ATTEMPTS_TO_LOCK {
		assert_eq!(
			login_status(&app, "friend", "wrong").await,
			StatusCode::UNAUTHORIZED
		);
	}

	assert_eq!(sessions_of(&app, &friend.id).await, 0);
	assert_eq!(refresh_tokens_of(&app, &friend.id).await, 0);
	refresh(&app, &signed_in.refresh_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
}

/// a refresh token could be exchanged again and again, each time for a new 30-day one
#[tokio::test]
async fn a_refresh_token_is_used_up() {
	let app = TestApp::new().await;
	let friend = insert_user_with_permissions(&app, "friend", "secret", "").await;
	let signed_in = sign_in(&app, "friend", "secret").await;
	let old_id = refresh_token_id(&signed_in.refresh_token);

	let response = refresh(&app, &signed_in.refresh_token).await;
	response.assert_status_ok();
	let new_pair: Value = response.json();
	let new_refresh_token = new_pair["refreshToken"].as_str().expect("no refresh token");
	assert_ne!(new_refresh_token, signed_in.refresh_token);
	me_with_bearer(
		&app,
		new_pair["accessToken"].as_str().expect("no access token"),
	)
	.await
	.assert_status_ok();

	// the old one runs out within seconds, not in 30 days
	let old = refresh_token::Entity::find_by_id(old_id.clone())
		.one(app.conn())
		.await
		.unwrap()
		.expect("the old refresh token is gone already");
	assert!(
		old.expires_at < Utc::now() + Duration::seconds(60),
		"the old refresh token still lasts until {}",
		old.expires_at
	);

	// a client that sent several requests at once exchanges the same token from each of them:
	// that works while the grace period lasts
	refresh(&app, &signed_in.refresh_token)
		.await
		.assert_status_ok();

	// once it is over, the old token is refused, and forgotten
	let a_minute_ago: DateTimeWithTimeZone = (Utc::now() - Duration::minutes(1)).into();
	refresh_token::Entity::update_many()
		.col_expr(refresh_token::Column::ExpiresAt, Expr::value(a_minute_ago))
		.filter(refresh_token::Column::Id.eq(old_id.clone()))
		.exec(app.conn())
		.await
		.unwrap();
	refresh(&app, &signed_in.refresh_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	assert!(refresh_token::Entity::find_by_id(old_id)
		.one(app.conn())
		.await
		.unwrap()
		.is_none());

	// the new one keeps working
	refresh(&app, new_refresh_token).await.assert_status_ok();
	assert!(refresh_tokens_of(&app, &friend.id).await >= 2);
}

/// a refresh token that isn't ours at all
#[tokio::test]
async fn an_unknown_refresh_token_is_refused() {
	let app = TestApp::new().await;
	insert_user_with_permissions(&app, "friend", "secret", "").await;
	let signed_in = sign_in(&app, "friend", "secret").await;

	refresh_token::Entity::delete_many()
		.exec(app.conn())
		.await
		.unwrap();
	refresh(&app, &signed_in.refresh_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	// an access token is not a refresh token
	refresh(&app, &signed_in.access_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
}

/// a password the owner resets signed nobody out of their refresh tokens, which went on handing
/// out new access tokens with the old password's sign-ins
#[tokio::test]
async fn a_password_reset_by_the_owner_signs_the_user_out_everywhere() {
	let app = TestApp::new_with_default_user().await;
	let friend =
		insert_user_with_permissions(&app, "friend", "secret", "ACCESS_API_KEYS").await;
	let signed_in = sign_in(&app, "friend", "secret").await;

	owner_runs(
		&app,
		r#"mutation($id: ID!, $input: UpdateUserInput!) {
			updateUser(id: $id, input: $input) { id }
		}"#,
		json!({
			"id": friend.id,
			"input": {
				"username": "friend",
				"password": "new-secret",
				"permissions": ["ACCESS_API_KEYS"],
				"maxSessionsAllowed": null,
				"ageRestriction": null
			}
		}),
	)
	.await;

	assert_eq!(sessions_of(&app, &friend.id).await, 0);
	assert_eq!(refresh_tokens_of(&app, &friend.id).await, 0);
	me_with_cookie(&app, &signed_in.cookie)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	refresh(&app, &signed_in.refresh_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	assert_eq!(
		login_status(&app, "friend", "secret").await,
		StatusCode::UNAUTHORIZED
	);
	assert_eq!(
		login_status(&app, "friend", "new-secret").await,
		StatusCode::OK
	);
}

/// a user who changes their own password is signed out everywhere else, but not of the browser
/// they changed it in
#[tokio::test]
async fn a_new_password_signs_the_user_out_everywhere_else() {
	let app = TestApp::new().await;
	let friend =
		insert_user_with_permissions(&app, "friend", "secret", "CHANGE_PASSWORD").await;
	let here = sign_in(&app, "friend", "secret").await;
	let elsewhere = sign_in(&app, "friend", "secret").await;
	assert_eq!(sessions_of(&app, &friend.id).await, 2);

	let response = app
		.server
		.post("/api/graphql")
		.add_header("cookie", &here.cookie)
		.json(&json!({
			"query": r#"mutation($input: UpdateUserInput!) {
				updateViewer(input: $input) { id }
			}"#,
			"variables": { "input": {
				"username": "friend",
				"password": "new-secret",
				"permissions": [],
				"maxSessionsAllowed": null,
				"ageRestriction": null
			}}
		}))
		.await;
	response.assert_status_ok();
	let body: Value = response.json();
	assert!(
		body.get("errors").is_none(),
		"updateViewer failed: {body:#}"
	);

	me_with_cookie(&app, &here.cookie).await.assert_status_ok();
	me_with_cookie(&app, &elsewhere.cookie)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	assert_eq!(sessions_of(&app, &friend.id).await, 1);
	// every refresh token goes, the ones of apps signed in with the old password included
	assert_eq!(refresh_tokens_of(&app, &friend.id).await, 0);
	refresh(&app, &here.refresh_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	refresh(&app, &elsewhere.refresh_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);

	assert_eq!(
		login_status(&app, "friend", "secret").await,
		StatusCode::UNAUTHORIZED
	);
	assert_eq!(
		login_status(&app, "friend", "new-secret").await,
		StatusCode::OK
	);
}

/// signing a user out ended their sessions and left their refresh tokens
#[tokio::test]
async fn deleting_the_sessions_of_a_user_deletes_their_refresh_tokens() {
	let app = TestApp::new_with_default_user().await;
	let friend = insert_user_with_permissions(&app, "friend", "secret", "").await;
	let other = insert_user_with_permissions(&app, "other", "secret", "").await;
	let signed_in = sign_in(&app, "friend", "secret").await;
	let other_signed_in = sign_in(&app, "other", "secret").await;

	owner_runs(
		&app,
		"mutation($id: ID!) { deleteUserSessions(id: $id) }",
		json!({ "id": friend.id }),
	)
	.await;

	assert_eq!(sessions_of(&app, &friend.id).await, 0);
	assert_eq!(refresh_tokens_of(&app, &friend.id).await, 0);
	me_with_cookie(&app, &signed_in.cookie)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
	refresh(&app, &signed_in.refresh_token)
		.await
		.assert_status(StatusCode::UNAUTHORIZED);

	// somebody else is not signed out with them
	assert_eq!(sessions_of(&app, &other.id).await, 1);
	me_with_cookie(&app, &other_signed_in.cookie)
		.await
		.assert_status_ok();
	refresh(&app, &other_signed_in.refresh_token)
		.await
		.assert_status_ok();
}
