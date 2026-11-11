//! The edge middleware: the marker, the pass, the device, and what is refused

use axum::http::StatusCode;
use serde_json::json;

use super::*;

/// The lead's R2: the marker is believed whatever the configuration. A server with no edge
/// settings at all still refuses the owner's password through the edge
#[tokio::test]
async fn the_marker_is_believed_without_any_edge_settings() {
	let app = TestApp::new().await;
	insert_user(&app, "owner", "secret", true).await;

	let through_the_edge = public_login(&app, "owner", "secret", None).await;
	assert_eq!(through_the_edge.status_code(), StatusCode::UNAUTHORIZED);

	let at_home = app
		.server
		.post("/api/v2/auth/login")
		.json(&json!({ "username": "owner", "password": "secret" }))
		.await;
	assert_eq!(at_home.status_code(), StatusCode::OK);
}

/// Without a key nothing can be verified: the invite pages answer 404, and a public request that
/// must carry a pass is refused (the app fails closed)
#[tokio::test]
async fn without_a_key_the_public_side_fails_closed() {
	let env = edge_app_with(|dir| {
		let mut edge = edge_settings(dir, true);
		edge.key = None;
		Some(edge)
	})
	.await;

	let link_like = "A".repeat(22);
	let invite_page = public_get(&env.app, &format!("/api/_i/{link_like}"), None).await;
	assert_eq!(invite_page.status_code(), StatusCode::NOT_FOUND);
	assert!(invite_page.text().is_empty());

	// even a pass made with the right key means nothing without one to check it with
	let pass = pass_for("ana-phone-aaaaa");
	let ping = public_get(&env.app, "/api/v2/ping", Some(&pass)).await;
	assert_eq!(ping.status_code(), StatusCode::NOT_FOUND);

	// and at home everything is as usual
	env.app.get("/api/v2/ping").await.assert_status_ok();
}

#[tokio::test]
async fn a_home_request_is_never_touched_by_the_headers_of_a_pass() {
	let env = edge_app(true).await;
	let app = &env.app;
	// a pass header with no marker: nothing happens (no device is made, nothing is refused)
	let pass = pass_for("ana-phone-aaaaa");
	let response = app
		.server
		.get("/api/v2/ping")
		.add_header("x-edge-pass", pass)
		.await;
	response.assert_status_ok();
	assert!(device_row(app, "ana-phone-aaaaa").await.is_none());
}

#[tokio::test]
async fn a_valid_pass_opens_the_public_side_and_a_forged_one_does_not() {
	let env = edge_app(true).await;
	let app = &env.app;

	let no_pass = public_get(app, "/api/v2/ping", None).await;
	assert_eq!(no_pass.status_code(), StatusCode::NOT_FOUND);

	let forged = format!(
		"ana-phone-aaaaa.{}.{}",
		Utc::now().timestamp(),
		"A".repeat(43)
	);
	let forged = public_get(app, "/api/v2/ping", Some(&forged)).await;
	assert_eq!(forged.status_code(), StatusCode::NOT_FOUND);

	// a pass made with another key
	let other_key =
		pass::mint_pass(&"x".repeat(64), "ana-phone-aaaaa", Utc::now().timestamp());
	let other_key = public_get(app, "/api/v2/ping", Some(&other_key)).await;
	assert_eq!(other_key.status_code(), StatusCode::NOT_FOUND);

	let valid = public_get(app, "/api/v2/ping", Some(&pass_for("ana-phone-aaaaa"))).await;
	valid.assert_status_ok();
}

/// A pass the app has never seen (the terminal script made it, or the old edge did) is registered
/// as a legacy device, so it shows up in the list and can be revoked
#[tokio::test]
async fn an_unknown_pass_is_registered_as_a_legacy_device() {
	let env = edge_app(false).await;
	let app = &env.app;
	assert!(device_row(app, "hermana-phone").await.is_none());

	public_get(app, "/api/v2/ping", Some(&pass_for("hermana-phone")))
		.await
		.assert_status_ok();

	let device = device_row(app, "hermana-phone").await.expect("a device");
	assert_eq!(device.person_id, None, "a legacy device has no person");
	assert_eq!(device.label, "hermana-phone");
	assert!(device.revoked_at.is_none());
	assert!(device.first_seen_at.is_some());
}

/// The app refuses a revoked device **at once**, whatever the state file says (there is none here)
#[tokio::test]
async fn a_revoked_device_is_refused_at_once() {
	let env = edge_app(false).await;
	let app = &env.app;
	let pass = pass_for("hermana-phone");
	public_get(app, "/api/v2/ping", Some(&pass))
		.await
		.assert_status_ok();

	models::services::access::revoke_device(
		app.conn(),
		Utc::now(),
		"hermana-phone",
		"owner",
	)
	.await
	.unwrap();

	let refused = public_get(app, "/api/v2/ping", Some(&pass)).await;
	assert_eq!(refused.status_code(), StatusCode::FORBIDDEN);
	// and restoring it brings it back
	models::services::access::restore_device(
		app.conn(),
		Utc::now(),
		"hermana-phone",
		"owner",
	)
	.await
	.unwrap();
	public_get(app, "/api/v2/ping", Some(&pass))
		.await
		.assert_status_ok();
}

/// With `require_pass`, the invite pages are the one thing a browser without a pass reaches
#[tokio::test]
async fn the_invite_pages_are_open_without_a_pass() {
	let env = edge_app(true).await;
	let app = &env.app;

	let page = public_get(app, &format!("/api/_i/{}", "A".repeat(22)), None).await;
	page.assert_status_ok();
	assert!(page.text().contains("Open Stump"));
	let ping = public_get(app, "/api/v2/ping", None).await;
	assert_eq!(ping.status_code(), StatusCode::NOT_FOUND);
}

/// R3: the internet is never the owner, in every way of signing in, and without deleting the
/// session cookie the owner's LAN browser holds
#[tokio::test]
async fn the_owner_is_refused_through_the_edge_with_a_token_a_cookie_or_basic_auth() {
	let env = edge_app(false).await;
	let app = &env.app;
	let owner_token = app.access_token.read().await.clone().unwrap();

	// a bearer token
	let bearer = app
		.server
		.get("/api/v2/auth/viewer")
		.add_header("x-edge-public", "1")
		.add_header("authorization", format!("Bearer {owner_token}"))
		.await;
	assert_eq!(bearer.status_code(), StatusCode::UNAUTHORIZED);
	// the refusal does not delete the session cookie the owner has for the LAN
	assert!(bearer.headers().get("set-cookie").is_none());
	// the same token at home is fine
	let at_home = app
		.server
		.get("/api/v2/auth/viewer")
		.add_header("authorization", format!("Bearer {owner_token}"))
		.await;
	at_home.assert_status_ok();

	// GraphQL
	let graphql = app
		.server
		.post("/api/graphql")
		.add_header("x-edge-public", "1")
		.add_header("authorization", format!("Bearer {owner_token}"))
		.json(&json!({"query": "{ me { id } }"}))
		.await;
	assert_eq!(graphql.status_code(), StatusCode::UNAUTHORIZED);

	// a session cookie
	let login = app
		.server
		.post("/api/v2/auth/login")
		.json(&json!({"username": "initial-server-admin", "password": "password"}))
		.await;
	login.assert_status_ok();
	let cookie = login
		.headers()
		.get_all("set-cookie")
		.iter()
		.filter_map(|value| value.to_str().ok())
		.find(|value| value.starts_with("stump_session="))
		.and_then(|value| value.split(';').next())
		.expect("a session cookie")
		.to_string();
	let with_cookie = app
		.server
		.get("/api/v2/auth/viewer")
		.add_header("x-edge-public", "1")
		.add_header("cookie", cookie.clone())
		.await;
	assert_eq!(with_cookie.status_code(), StatusCode::UNAUTHORIZED);
	assert!(with_cookie.headers().get("set-cookie").is_none());
	app.server
		.get("/api/v2/auth/viewer")
		.add_header("cookie", cookie)
		.await
		.assert_status_ok();

	// HTTP Basic (OPDS)
	use base64::{engine::general_purpose::STANDARD, Engine};
	let basic = app
		.server
		.get("/opds/v1.2/catalog")
		.add_header("x-edge-public", "1")
		.add_header(
			"authorization",
			format!("Basic {}", STANDARD.encode("initial-server-admin:password")),
		)
		.await;
	assert_eq!(basic.status_code(), StatusCode::UNAUTHORIZED);
}

/// The lead's review: the owner's API key limited to some permissions is built without the owner
/// flag (it gets what it was given and no more), and is still the owner's credential. It is refused
/// through the edge in every form (a Bearer header, GraphQL, the key in an OPDS address) without
/// touching the session cookie, and is as good as ever at home (book-curator connects that way). A
/// friend's key is not the owner's, and works on the public side
#[tokio::test]
async fn the_owners_api_keys_are_refused_through_the_edge_scoped_or_not() {
	use models::shared::{api_key::APIKeyPermissions, enums::UserPermission};
	use sea_orm::{ColumnTrait, QueryFilter};

	use crate::common::api_key::create_api_key_for_user;

	let env = edge_app(false).await;
	let app = &env.app;
	let owner = user::Entity::find()
		.filter(user::Column::IsServerOwner.eq(true))
		.one(app.conn())
		.await
		.expect("the query should run")
		.expect("there should be an owner");
	let (scoped_key, _) = create_api_key_for_user(
		app,
		&owner,
		Some(APIKeyPermissions::Custom(vec![UserPermission::EditMetadata])),
	)
	.await;
	let (inherit_key, _) = create_api_key_for_user(app, &owner, None).await;

	for (kind, key) in [("scoped", &scoped_key), ("inherit", &inherit_key)] {
		// a Bearer header
		let bearer = app
			.server
			.get("/api/v2/auth/viewer")
			.add_header("x-edge-public", "1")
			.add_header("authorization", format!("Bearer {key}"))
			.await;
		assert_eq!(bearer.status_code(), StatusCode::UNAUTHORIZED, "{kind} key, bearer");
		// the refusal does not delete the session cookie the owner has for the LAN
		assert!(
			bearer.headers().get("set-cookie").is_none(),
			"{kind} key: the refusal must not touch the session cookie"
		);

		// GraphQL
		let graphql = app
			.server
			.post("/api/graphql")
			.add_header("x-edge-public", "1")
			.add_header("authorization", format!("Bearer {key}"))
			.json(&json!({ "query": "{ me { id } }" }))
			.await;
		assert_eq!(graphql.status_code(), StatusCode::UNAUTHORIZED, "{kind} key, graphql");

		// the key in an OPDS address
		let opds = app
			.server
			.get(&format!("/opds/{key}/v1.2/catalog"))
			.add_header("x-edge-public", "1")
			.await;
		assert_eq!(opds.status_code(), StatusCode::UNAUTHORIZED, "{kind} key, opds address");
		assert!(opds.headers().get("set-cookie").is_none());

		// the same key at home is as good as before
		app.server
			.get("/api/v2/auth/viewer")
			.add_header("authorization", format!("Bearer {key}"))
			.await
			.assert_status_ok();
		let gql_at_home = app
			.execute_gql_with_token("{ me { id } }", None, key)
			.await;
		assert!(gql_at_home.get("errors").is_none(), "{kind} key at home: {gql_at_home:#}");
		app.server
			.get(&format!("/opds/{key}/v1.2/catalog"))
			.await
			.assert_status_ok();
	}

	// a friend's key is not the owner's: the public side works for it
	let friend = insert_user(app, "ana", "secret-pass", false).await;
	let mut with_keys: user::ActiveModel = friend.into();
	with_keys.permissions = Set(Some("ACCESS_API_KEYS".to_string()));
	let friend = with_keys.update(app.conn()).await.expect("should update");
	let (friend_key, _) = create_api_key_for_user(
		app,
		&friend,
		Some(APIKeyPermissions::Custom(vec![UserPermission::EditMetadata])),
	)
	.await;
	let through_the_edge = app
		.server
		.get("/api/v2/auth/viewer")
		.add_header("x-edge-public", "1")
		.add_header("authorization", format!("Bearer {friend_key}"))
		.await;
	through_the_edge.assert_status_ok();
}

/// Another user is not the owner: they can use the public side with their token
#[tokio::test]
async fn a_friend_with_a_token_is_not_refused_through_the_edge() {
	let env = edge_app(false).await;
	let app = &env.app;
	insert_user(app, "ana", "secret-pass", false).await;
	let token = app.login_as("ana", "secret-pass").await;

	let viewer = app
		.server
		.get("/api/v2/auth/viewer")
		.add_header("x-edge-public", "1")
		.add_header("authorization", format!("Bearer {token}"))
		.await;
	viewer.assert_status_ok();
}

/// The lead's R3 and R7 for the websocket: GraphQL runs over `/api/graphql/ws` too (queries and
/// mutations as well as subscriptions), so the owner is refused there through the edge like
/// everywhere else, and the subscription handler gets the `EdgeRequest` its guards read.
///
/// The test server has no real websocket handshake, so what is shown is what happens before the
/// upgrade: the authentication layers (which refuse the owner) and the handler's extractors (which
/// would answer `500`, "missing extension", if the route had no `EdgeRequest`)
#[tokio::test]
async fn the_websocket_is_guarded_like_the_rest_of_graphql() {
	let env = edge_app(false).await;
	let app = &env.app;
	let owner_token = app.access_token.read().await.clone().unwrap();
	insert_user(app, "ana", "secret-pass", false).await;
	let friend_token = app.login_as("ana", "secret-pass").await;

	// the owner through the edge: refused, and nothing deletes the owner's cookie for the LAN
	let owner = app
		.server
		.get("/api/graphql/ws")
		.add_header("x-edge-public", "1")
		.add_header("authorization", format!("Bearer {owner_token}"))
		.await;
	assert_eq!(owner.status_code(), StatusCode::UNAUTHORIZED);
	assert!(owner.headers().get("set-cookie").is_none());

	// the same request at home, and a friend through the edge, get as far as the handler, which
	// wants an upgrade this plain request doesn't ask for: a client error of its own, not an
	// authentication one, and not a server error (which is what a missing `EdgeRequest` would be)
	for (who, token, marker) in [
		("the owner at home", &owner_token, false),
		("a friend through the edge", &friend_token, true),
	] {
		let mut request = app
			.server
			.get("/api/graphql/ws")
			.add_header("authorization", format!("Bearer {token}"));
		if marker {
			request = request.add_header("x-edge-public", "1");
		}
		let status = request.await.status_code();
		assert!(
			status.is_client_error()
				&& ![
					StatusCode::UNAUTHORIZED,
					StatusCode::FORBIDDEN,
					StatusCode::NOT_FOUND
				]
				.contains(&status),
			"{who}: {status}"
		);
	}
}

/// Accounts are made from home: not even the very first one through the internet
#[tokio::test]
async fn registration_is_refused_through_the_edge() {
	let app = TestApp::new().await;
	let response = app
		.server
		.post("/api/v2/auth/register")
		.add_header("x-edge-public", "1")
		.json(&json!({"username": "owner", "password": "password"}))
		.await;
	assert_eq!(response.status_code(), StatusCode::FORBIDDEN);
	// and it works at home
	app.server
		.post("/api/v2/auth/register")
		.json(&json!({"username": "owner", "password": "password"}))
		.await
		.assert_status_ok();
}
