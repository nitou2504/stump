//! Public access: the edge middleware (who reached the app, with which pass), the invite pages,
//! the sign-in rules that depend on a pass, and the owner's GraphQL API for all of it

mod edge;
mod graphql;
mod invites;
mod password;
mod signin;

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
pub const KEY: &str =
	"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
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
