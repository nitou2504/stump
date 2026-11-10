//! The password the owner sets for somebody, and what that person may do until they choose
//! their own (the lead's R5)

use axum::http::StatusCode;
use models::{
	entity::{access_audit, access_password_reset, session},
	shared::enums::UserPermission,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

use super::*;
use crate::common::account::CreateTestUser;

async fn set_password(app: &TestApp, id: &str, password: &str, require_change: bool) -> Value {
	owner_gql(
		app,
		"mutation($id: ID!, $p: String!, $r: Boolean!) { setUserPassword(id: $id, password: $p, requireChange: $r) }",
		Some(json!({"id": id, "p": password, "r": require_change})),
	)
	.await
}

async fn friend(app: &TestApp, permissions: Vec<UserPermission>) -> crate::common::account::TestUser {
	CreateTestUser {
		username: "ana".to_string(),
		password: "old-password".to_string(),
		permissions,
		..Default::default()
	}
	.insert(app)
	.await
}

#[tokio::test]
async fn a_password_the_owner_sets_must_be_replaced_before_anything_else_works() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = friend(app, vec![UserPermission::ChangePassword]).await;

	// she is signed in somewhere, with a cookie and a token
	let cookie_login = app
		.server
		.post("/api/v2/auth/login")
		.json(&json!({"username": "ana", "password": "old-password"}))
		.await;
	cookie_login.assert_status_ok();

	let response = set_password(app, &ana.id, "temporary-pass", true).await;
	assert_eq!(response["data"]["setUserPassword"], true, "{response:#}");

	// it ended her sessions, and the owner's own session is another matter
	let sessions = session::Entity::find()
		.filter(session::Column::UserId.eq(ana.id.clone()))
		.count(app.conn())
		.await
		.unwrap();
	assert_eq!(sessions, 0);
	assert!(access_password_reset::Entity::find_by_id(ana.id.clone())
		.one(app.conn())
		.await
		.unwrap()
		.is_some());

	// the old password no longer works; the temporary one does, and the sign-in says what to do
	assert_eq!(
		app.server
			.post("/api/v2/auth/login")
			.json(&json!({"username": "ana", "password": "old-password"}))
			.await
			.status_code(),
		StatusCode::UNAUTHORIZED
	);
	let login = app
		.server
		.post("/api/v2/auth/login?generate_token=true")
		.json(&json!({"username": "ana", "password": "temporary-pass"}))
		.await;
	login.assert_status_ok();
	let body: Value = login.json();
	assert_eq!(body["forUser"]["mustChangePassword"], true, "{body:#}");
	let token = body["accessToken"].as_str().unwrap().to_string();

	// who she is: allowed (and says so)
	let viewer = app.get_with_token("/api/v2/auth/viewer", &token).await;
	viewer.assert_status_ok();
	assert_eq!(viewer.json::<Value>()["mustChangePassword"], true);

	// everything else is refused: REST, GraphQL, and the websocket
	for path in [
		"/api/v2/media/00000000-0000-0000-0000-000000000000/thumbnail",
		"/api/graphql/ws",
	] {
		let refused = app.get_with_token(path, &token).await;
		assert_eq!(refused.status_code(), StatusCode::FORBIDDEN, "{path}");
	}
	let other = app
		.execute_gql_with_token("{ libraries(pagination: {none: {unpaginated: true}}) { nodes { id } } }", None, &token)
		.await;
	assert!(errors_of(&other).contains("choose a new password"), "{other:#}");
	let me = app.execute_gql_with_token("{ me { username } }", None, &token).await;
	assert_eq!(me["data"]["me"]["username"], "ana");

	// the temporary password can't be "changed" to itself
	let same = app
		.execute_gql_with_token(
			"mutation($i: UpdateUserInput!) { updateViewer(input: $i) { id } }",
			Some(json!({"i": {"username": "ana", "password": "temporary-pass", "permissions": ["CHANGE_PASSWORD"]}})),
			&token,
		)
		.await;
	assert!(errors_of(&same).contains("different"), "{same:#}");

	// a different one ends the requirement
	let changed = app
		.execute_gql_with_token(
			"mutation($i: UpdateUserInput!) { updateViewer(input: $i) { id } }",
			Some(json!({"i": {"username": "ana", "password": "my-own-password", "permissions": ["CHANGE_PASSWORD"]}})),
			&token,
		)
		.await;
	assert!(changed.get("errors").is_none(), "{changed:#}");
	assert!(access_password_reset::Entity::find_by_id(ana.id.clone())
		.one(app.conn())
		.await
		.unwrap()
		.is_none());

	let viewer = app.get_with_token("/api/v2/auth/viewer", &token).await;
	assert_eq!(viewer.json::<Value>()["mustChangePassword"], false);
	let libraries = app
		.execute_gql_with_token("{ libraries(pagination: {none: {unpaginated: true}}) { nodes { id } } }", None, &token)
		.await;
	assert!(libraries.get("errors").is_none(), "{libraries:#}");

	// the audit log has both: the owner set it, she chose her own
	let actions: Vec<String> = access_audit::Entity::find()
		.all(app.conn())
		.await
		.unwrap()
		.into_iter()
		.map(|row| row.action)
		.collect();
	assert!(actions.contains(&"user.password_set".to_string()));
	assert!(actions.contains(&"user.password_changed".to_string()));
}

/// The password itself is never in the audit log, and the owner's password is not set from here
#[tokio::test]
async fn the_owners_password_is_not_set_here_and_nothing_secret_is_audited() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = friend(app, vec![UserPermission::ChangePassword]).await;
	let owner = user::Entity::find()
		.filter(user::Column::IsServerOwner.eq(true))
		.one(app.conn())
		.await
		.unwrap()
		.unwrap();

	let refused = set_password(app, &owner.id, "temporary-pass", true).await;
	assert!(errors_of(&refused).contains("server owner"), "{refused:#}");

	set_password(app, &ana.id, "temporary-pass-xyz", false).await;
	let audit = access_audit::Entity::find().all(app.conn()).await.unwrap();
	assert!(!format!("{audit:?}").contains("temporary-pass-xyz"));
	// without the requirement there is nothing to replace
	assert!(access_password_reset::Entity::find_by_id(ana.id.clone())
		.one(app.conn())
		.await
		.unwrap()
		.is_none());
}

/// Somebody who may not change their password could never replace the one they were given
#[tokio::test]
async fn a_person_who_may_not_change_their_password_gets_no_requirement() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = friend(app, vec![]).await;

	let refused = set_password(app, &ana.id, "temporary-pass", true).await;
	assert!(errors_of(&refused).contains("may not change"), "{refused:#}");
	// it works without the requirement
	let allowed = set_password(app, &ana.id, "temporary-pass", false).await;
	assert_eq!(allowed["data"]["setUserPassword"], true);

	// too short a password is refused
	let short = set_password(app, &ana.id, "short", false).await;
	assert!(errors_of(&short).contains("characters"), "{short:#}");
}

/// A friend can't set anybody's password through this, nor with a token that isn't the owner's
#[tokio::test]
async fn only_the_owner_can_set_a_password() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = friend(app, vec![UserPermission::ManageUsers, UserPermission::ManageServer]).await;
	let ben = CreateTestUser {
		username: "ben".to_string(),
		..Default::default()
	}
	.insert(app)
	.await;

	let response = app
		.execute_gql_with_token(
			"mutation($id: ID!) { setUserPassword(id: $id, password: \"temporary-pass\", requireChange: false) }",
			Some(json!({"id": ben.id})),
			&ana.token,
		)
		.await;
	assert!(!errors_of(&response).is_empty(), "{response:#}");
}
