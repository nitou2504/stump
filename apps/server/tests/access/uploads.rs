//! Friend uploads and the public edge: who gets in with a book from the internet, who never does,
//! and what stays at home.
//!
//! The way in from the internet is the "Add books" inbox (`POST /api/v2/intake/upload`) and
//! nothing else: the owner is refused at the door like everywhere through the edge, somebody who
//! manages libraries files books at home, and the approving and the filing straight into a library
//! are not reachable from the internet at all.

use std::fs;

use axum::http::StatusCode;
use axum_test::multipart::{MultipartForm, Part};
use models::{entity::intake_item, shared::enums::UserPermission};
use stump_core::config::StumpConfig;
use tests::{fake_data, fixtures};

use super::*;
use crate::common::{
	account::{CreateTestUser, TestUser},
	upload::{message, post_upload, post_upload_with, UploadBody},
};

/// An app with the public edge set up (a pass is required when `require_pass`), a library with a
/// folder of its own, and the people of a friend's server: **ana** may add books and nothing else,
/// the **manager** manages libraries (and adds books). The owner's token is the app's.
struct UploadWorld {
	env: EdgeApp,
	library_id: String,
	library_dir: std::path::PathBuf,
	ana: TestUser,
	manager: TestUser,
	tmp: tempfile::TempDir,
}

async fn world(require_pass: bool) -> UploadWorld {
	let tmp = tempfile::tempdir().expect("tempdir");
	let config_dir = tmp.path().join("config");
	fs::create_dir_all(&config_dir).expect("config folder");
	let state_dir = tempfile::tempdir().expect("a state folder");
	let mut config = StumpConfig::debug();
	// the cheapest bcrypt there is: these tests sign in a lot
	config.password_hash_cost = 4;
	config.config_dir = config_dir.to_string_lossy().to_string();
	config.edge = Some(edge_settings(&state_dir, require_pass));
	// the debug configuration reads the books of people who need approval inside the server: the
	// point here is who gets in, not the checker
	let app = TestApp::with_config(config).await;
	let token = app.create_initial_account().await;
	*app.access_token.write().await = Some(token);

	let library_dir = tmp.path().join("library");
	fs::create_dir_all(&library_dir).expect("library folder");
	let library = fake_data::Library {
		path: Some(library_dir.to_string_lossy().to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let ana = CreateTestUser {
		username: "ana".to_string(),
		permissions: vec![UserPermission::AddBooks],
		..Default::default()
	}
	.insert(&app)
	.await;
	let manager = CreateTestUser {
		username: "manager".to_string(),
		permissions: vec![UserPermission::AddBooks, UserPermission::ManageLibrary],
		..Default::default()
	}
	.insert(&app)
	.await;
	UploadWorld {
		env: EdgeApp { app, state_dir },
		library_id: library.id,
		library_dir,
		ana,
		manager,
		tmp,
	}
}

impl UploadWorld {
	fn app(&self) -> &TestApp {
		&self.env.app
	}

	fn epub(&self, title: &str) -> Vec<u8> {
		let path = self
			.tmp
			.path()
			.join(format!("{}.epub", title.replace(' ', "_")));
		fixtures::write_test_epub(
			&path,
			title,
			"<dc:creator>Somebody, Some</dc:creator>",
		);
		fs::read(path).expect("epub bytes")
	}

	fn headers(pass: Option<&str>) -> Vec<(&'static str, String)> {
		let mut headers = vec![("x-edge-public", "1".to_string())];
		if let Some(pass) = pass {
			headers.push(("x-edge-pass", pass.to_string()));
		}
		headers
	}

	/// The upload route as the edge forwards a request from the internet
	async fn public_upload(
		&self,
		token: &str,
		pass: Option<&str>,
		body: &UploadBody,
	) -> axum_test::TestResponse {
		post_upload_with(
			self.app(),
			Some(token),
			&self.library_id,
			None,
			body,
			&Self::headers(pass),
		)
		.await
	}

	/// The same at home (the LAN or the tailnet), where no marker is added
	async fn home_upload(
		&self,
		token: &str,
		body: &UploadBody,
	) -> axum_test::TestResponse {
		post_upload(self.app(), Some(token), &self.library_id, None, body).await
	}

	/// How many inbox items there are, whose they are
	async fn item_owners(&self) -> Vec<String> {
		intake_item::Entity::find()
			.all(self.app().conn())
			.await
			.expect("the items should load")
			.into_iter()
			.map(|item| item.user_id)
			.collect()
	}

	/// A GraphQL request as the edge forwards it from the internet
	async fn public_gql(
		&self,
		token: &str,
		pass: Option<&str>,
		query: &str,
		variables: Value,
	) -> Value {
		let mut request = self
			.app()
			.server
			.post("/api/graphql")
			.add_header("authorization", format!("Bearer {token}"));
		for (name, value) in Self::headers(pass) {
			request = request.add_header(name, value);
		}
		let response = request
			.json(&json!({ "query": query, "variables": variables }))
			.await;
		response.assert_status_ok();
		response.json()
	}

	/// A GraphQL multipart request (a file in a variable), at home or through the edge
	#[allow(clippy::too_many_arguments)]
	async fn multipart_gql(
		&self,
		token: &str,
		pass: Option<&str>,
		through_the_edge: bool,
		query: &str,
		variables: Value,
		path_of_file: &str,
		file_name: &str,
		bytes: Vec<u8>,
	) -> Value {
		let form = MultipartForm::new()
			.add_part(
				"operations",
				Part::text(json!({ "query": query, "variables": variables }).to_string())
					.mime_type("application/json"),
			)
			.add_part(
				"map",
				Part::text(json!({ "0": [path_of_file] }).to_string())
					.mime_type("application/json"),
			)
			.add_part(
				"0",
				Part::bytes(bytes)
					.file_name(file_name)
					.mime_type("application/octet-stream"),
			);
		let mut request = self
			.app()
			.server
			.post("/api/graphql")
			.add_header("authorization", format!("Bearer {token}"))
			.multipart(form);
		if through_the_edge {
			for (name, value) in Self::headers(pass) {
				request = request.add_header(name, value);
			}
		}
		let response = request.await;
		response.assert_status_ok();
		response.json()
	}
}

fn refused_for_lack_of_permission(response: &Value, what: &str) {
	assert!(
		errors_of(response).contains("permission"),
		"{what} should be refused for lack of permission: {response:#}"
	);
	assert!(
		response["data"].is_null()
			|| response["data"]
				.as_object()
				.is_some_and(|data| data.values().all(Value::is_null)),
		"{what}: {response:#}"
	);
}

/// The upload route against public access: no pass, nothing (the app answers as if there were
/// nothing here); the owner's credentials are refused without touching the session cookie they
/// hold for home; a person whose password the owner set has to choose their own first; and a
/// friend with a pass gets in, and the book is theirs
#[tokio::test(flavor = "multi_thread")]
async fn the_upload_route_asks_for_a_pass_and_for_who_you_are() {
	let world = world(true).await;
	let app = world.app();
	let body = UploadBody::new().file("Una.epub", world.epub("Una"));
	let pass = pass_for("ana-phone-aaaaa");

	// no pass: nothing is here, whoever asks
	let response = world.public_upload(&world.ana.token, None, &body).await;
	assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
	// and a pass made without the key is no pass
	let forged = format!(
		"ana-phone-aaaaa.{}.{}",
		Utc::now().timestamp(),
		"A".repeat(43)
	);
	let response = world
		.public_upload(&world.ana.token, Some(&forged), &body)
		.await;
	assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
	assert!(world.item_owners().await.is_empty());

	// the owner is not the internet's: 401, and the session cookie of home is left alone
	let owner_token = app.access_token.read().await.clone().unwrap();
	let response = world.public_upload(&owner_token, Some(&pass), &body).await;
	assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
	assert!(response.headers().get("set-cookie").is_none());
	assert!(world.item_owners().await.is_empty());

	// a person whose password the owner set has to choose their own first: nothing else works,
	// uploads included, from the internet or from home
	let ben = CreateTestUser {
		username: "ben".to_string(),
		password: "old-password".to_string(),
		permissions: vec![UserPermission::AddBooks, UserPermission::ChangePassword],
		..Default::default()
	}
	.insert(app)
	.await;
	let response = owner_gql(
		app,
		"mutation($id: ID!) { setUserPassword(id: $id, password: \"temporary-pass\", requireChange: true) }",
		Some(json!({ "id": ben.id })),
	)
	.await;
	assert_eq!(response["data"]["setUserPassword"], true, "{response:#}");
	let token = app.login_as("ben", "temporary-pass").await;
	let response = world.public_upload(&token, Some(&pass), &body).await;
	assert_eq!(response.status_code(), StatusCode::FORBIDDEN);
	assert!(
		response.text().contains("choose a new password"),
		"{}",
		response.text()
	);
	let response = world.home_upload(&token, &body).await;
	assert_eq!(response.status_code(), StatusCode::FORBIDDEN);
	assert!(world.item_owners().await.is_empty());

	// ana, with the pass, gets in; the book is hers, and the device was noted
	let response = world
		.public_upload(&world.ana.token, Some(&pass), &body)
		.await;
	assert_eq!(
		response.status_code(),
		StatusCode::OK,
		"{}",
		response.text()
	);
	assert_eq!(world.item_owners().await, vec![world.ana.id.clone()]);
	assert!(device_row(app, "ana-phone-aaaaa").await.is_some());

	// at home the same route is open to her as it has always been
	let response = world.home_upload(&world.ana.token, &body).await;
	assert_eq!(
		response.status_code(),
		StatusCode::OK,
		"{}",
		response.text()
	);
}

/// Somebody who manages libraries files books themselves, from home. Through the edge the route
/// answers them no: the books of the internet go to the approval queue, within the limits that
/// come with it
#[tokio::test(flavor = "multi_thread")]
async fn a_library_manager_uploads_from_home_and_not_through_the_edge() {
	let world = world(true).await;
	let body = UploadBody::new().file("Una.epub", world.epub("Una"));
	let pass = pass_for("ana-phone-aaaaa");

	let response = world
		.public_upload(&world.manager.token, Some(&pass), &body)
		.await;
	assert_eq!(response.status_code(), StatusCode::FORBIDDEN);
	assert!(
		message(&response).contains("from home"),
		"{}",
		response.text()
	);
	assert!(world.item_owners().await.is_empty());

	let response = world.home_upload(&world.manager.token, &body).await;
	assert_eq!(
		response.status_code(),
		StatusCode::OK,
		"{}",
		response.text()
	);
	assert_eq!(world.item_owners().await, vec![world.manager.id.clone()]);
}

const QUEUE: &str = "{ pendingIntakeItems { totalCount } }";
const QUEUE_COUNT: &str = "{ pendingIntakeCount }";
const APPROVE: &str = r#"mutation { intakeApprove(ids: ["x"]) { id } }"#;
const REJECT: &str = r#"mutation { intakeReject(id: "x", note: "no") { id } }"#;
const ACCEPT: &str = r#"mutation { intakeAccept(ids: ["x"]) { id } }"#;

/// Approving is the server owner's, at home. A library manager with a pass, through the edge, gets
/// none of it: not the queue, not its count, not approving, not sending back. (At home they get
/// none of it either: that is `test_a_library_manager_is_not_an_approver`, in the intake tests.)
#[tokio::test(flavor = "multi_thread")]
async fn approving_is_not_reachable_from_the_internet() {
	let world = world(true).await;
	let pass = pass_for("ana-phone-aaaaa");

	for (query, what) in [
		(QUEUE, "the approval queue"),
		(QUEUE_COUNT, "its count"),
		(APPROVE, "approving"),
		(REJECT, "sending back"),
	] {
		for token in [&world.manager.token, &world.ana.token] {
			let response = world.public_gql(token, Some(&pass), query, json!({})).await;
			refused_for_lack_of_permission(&response, what);
		}
		// the owner, from home, is told it as it is (an item that doesn't exist is not found, but
		// the way in is open)
		let response = owner_gql(world.app(), query, None).await;
		assert!(
			!errors_of(&response).contains("permission"),
			"{what} should be open to the owner at home: {response:#}"
		);
	}
}

/// Filing straight into a library, with nobody's approval, is not reachable from the internet
/// either, whatever the person was given: not `intakeAccept`, not the GraphQL upload into an
/// inbox, not `uploadBooks` and `uploadSeries`. A manager does all of it from home
#[tokio::test(flavor = "multi_thread")]
async fn filing_straight_into_a_library_is_not_reachable_from_the_internet() {
	let world = world(true).await;
	let pass = pass_for("ana-phone-aaaaa");
	let epub = world.epub("Una");
	let place_at = world.library_dir.to_string_lossy().to_string();
	// the manager is given what the direct upload asks for, so only the edge can be what stops it
	let uploader = CreateTestUser {
		username: "uploader".to_string(),
		permissions: vec![
			UserPermission::AddBooks,
			UserPermission::ManageLibrary,
			UserPermission::UploadFile,
		],
		..Default::default()
	}
	.insert(world.app())
	.await;

	let response = world
		.public_gql(&uploader.token, Some(&pass), ACCEPT, json!({}))
		.await;
	refused_for_lack_of_permission(&response, "intakeAccept");

	let calls = [
		(
			"intakeUpload",
			r#"mutation($libraryId: ID!, $files: [Upload!]!) {
				intakeUpload(libraryId: $libraryId, files: $files) { id }
			}"#,
			json!({ "libraryId": world.library_id, "files": [null] }),
			"variables.files.0",
		),
		(
			"uploadBooks",
			r#"mutation($input: UploadBooksInput!) { uploadBooks(input: $input) }"#,
			json!({ "input": {
				"libraryId": world.library_id, "placeAt": place_at, "uploads": [null]
			}}),
			"variables.input.uploads.0",
		),
		(
			"uploadSeries",
			r#"mutation($input: UploadSeriesInput!) { uploadSeries(input: $input) }"#,
			json!({ "input": {
				"libraryId": world.library_id, "placeAt": place_at,
				"seriesDirName": "series", "upload": null
			}}),
			"variables.input.upload",
		),
	];
	for (name, query, variables, path_of_file) in calls {
		let file_name = if name == "uploadSeries" {
			"series.zip"
		} else {
			"una.epub"
		};
		let response = world
			.multipart_gql(
				&uploader.token,
				Some(&pass),
				true,
				query,
				variables,
				path_of_file,
				file_name,
				epub.clone(),
			)
			.await;
		refused_for_lack_of_permission(&response, name);
	}
	assert_eq!(
		fs::read_dir(&world.library_dir)
			.expect("the library folder")
			.count(),
		0,
		"nothing was put in the library"
	);
	assert!(world.item_owners().await.is_empty());

	// the same person, from home, files books themselves: the guard is the edge, not the person
	let response = world
		.multipart_gql(
			&uploader.token,
			None,
			false,
			r#"mutation($input: UploadBooksInput!) { uploadBooks(input: $input) }"#,
			json!({ "input": {
				"libraryId": world.library_id, "placeAt": place_at, "uploads": [null]
			}}),
			"variables.input.uploads.0",
			"una.epub",
			epub.clone(),
		)
		.await;
	assert_eq!(response["data"]["uploadBooks"], json!(true), "{response:#}");
}
