use std::{fs, path::PathBuf, time::Duration};

use axum_test::multipart::{MultipartForm, Part};
use models::shared::enums::UserPermission;
use serde_json::{json, Value};
use tempfile::TempDir;
use tests::{fake_data, fixtures};

use crate::common::{account::CreateTestUser, TestApp};

const UPLOAD: &str = r#"mutation Upload($libraryId: ID!, $files: [Upload!]!) {
	intakeUpload(libraryId: $libraryId, files: $files) { id state }
}"#;
const SETTINGS: &str = r#"mutation Settings($libraryId: ID!) {
	updateLibraryIntakeSettings(libraryId: $libraryId, input: {
		mangaDir: "_Manga", lightNovelDir: "_Novela ligera", onlineLookup: false
	}) { onlineLookup }
}"#;
const ITEMS: &str = r#"query Items {
	intakeItems {
		id state kind name error problems
		proposal {
			confidence warnings
			metadata { title authors language sources { field source } }
			authorFolders { folder isNew confidence reason }
			genreSuggestions { value selected confidence reasons }
		}
		decision { folder fileNames metadata { title authors genres } }
		destination { relativeDir files { relativePath exists } }
		result { paths csvPath notes }
	}
}"#;
const ITEM: &str = r#"query Item($id: ID!) { intakeItem(id: $id) { id } }"#;
const REVIEW: &str = r#"mutation Review($id: ID!, $input: IntakeReviewInput!) {
	intakeReview(id: $id, input: $input) {
		state problems decision { metadata { genres } }
	}
}"#;
const ACCEPT: &str = r#"mutation Accept($ids: [ID!]!) {
	intakeAccept(ids: $ids) { id state error result { paths csvPath } }
}"#;
const RECENT: &str = r#"query Recent {
	recentlyAddedBooks { title authors paths username libraryName }
}"#;

struct Fixture {
	app: TestApp,
	library_id: String,
	library_dir: PathBuf,
	_tempdir: TempDir,
}

async fn setup() -> Fixture {
	let app = TestApp::new_with_default_user().await;
	let tempdir = tempfile::tempdir().expect("tempdir");
	let library_dir = tempdir.path().join("biblioteca");
	// an author already in the library, written the curated way
	fs::create_dir_all(library_dir.join("Umberto_Eco")).expect("author folder");
	let library = fake_data::Library {
		path: Some(library_dir.to_string_lossy().to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let response = app
		.execute_gql(SETTINGS, Some(json!({ "libraryId": library.id })))
		.await;
	assert_no_errors(&response);
	Fixture {
		app,
		library_id: library.id,
		library_dir,
		_tempdir: tempdir,
	}
}

fn assert_no_errors(response: &Value) {
	assert!(response.get("errors").is_none(), "unexpected errors: {response:#}");
}

fn epub_bytes(dir: &std::path::Path) -> Vec<u8> {
	let path = dir.join("upload.epub");
	fixtures::write_test_epub(
		&path,
		"El nombre de la rosa",
		r#"<dc:creator>Eco, Umberto</dc:creator><dc:subject>Novela</dc:subject>"#,
	);
	fs::read(path).expect("epub bytes")
}

fn upload_form(library_id: &str, bytes: Vec<u8>) -> MultipartForm {
	let operations = json!({
		"query": UPLOAD,
		"variables": { "libraryId": library_id, "files": [null] }
	});
	let map = json!({ "0": ["variables.files.0"] });
	MultipartForm::new()
		.add_part("operations", Part::text(operations.to_string()).mime_type("application/json"))
		.add_part("map", Part::text(map.to_string()).mime_type("application/json"))
		.add_part(
			"0",
			Part::bytes(bytes)
				.file_name("rosa.epub")
				.mime_type("application/epub+zip"),
		)
}

async fn post_multipart(app: &TestApp, token: &str, form: MultipartForm) -> Value {
	let response = app
		.server
		.post("/api/graphql")
		.add_header("Authorization", format!("Bearer {token}"))
		.multipart(form)
		.await;
	response.assert_status_ok();
	response.json()
}

/// waits for the background analysis of the inbox to finish
async fn analysed_items(app: &TestApp, token: &str) -> Value {
	for _ in 0..100 {
		let response = app.execute_gql_with_token(ITEMS, None, token).await;
		assert_no_errors(&response);
		let items = response["data"]["intakeItems"].clone();
		if items
			.as_array()
			.is_some_and(|items| items.iter().all(|i| i["state"] != "ANALYSING"))
		{
			return items;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	panic!("analysis did not finish");
}

/// upload -> proposal (existing author folder found from 'Eco, Umberto') -> review -> file:
/// the book lands in Umberto_Eco/ with its tags written into the OPF and a CSV row
#[tokio::test(flavor = "multi_thread")]
async fn test_upload_review_and_file_a_book() {
	let fixture = setup().await;
	let app = &fixture.app;
	let uploader = CreateTestUser {
		username: "uploader".to_string(),
		permissions: vec![UserPermission::AddBooks, UserPermission::UploadFile],
		..Default::default()
	}
	.insert(app)
	.await;

	let response = post_multipart(
		app,
		&uploader.token,
		upload_form(&fixture.library_id, epub_bytes(fixture._tempdir.path())),
	)
	.await;
	assert_no_errors(&response);
	let id = response["data"]["intakeUpload"][0]["id"]
		.as_str()
		.expect("item id")
		.to_string();

	let items = analysed_items(app, &uploader.token).await;
	let item = &items[0];
	assert_eq!(item["kind"], "BOOK", "{item:#}");
	let proposal = &item["proposal"];
	assert_eq!(proposal["metadata"]["title"], "El nombre de la rosa");
	assert_eq!(proposal["authorFolders"][0]["folder"], "Umberto_Eco", "{item:#}");
	assert_eq!(proposal["authorFolders"][0]["isNew"], false);
	assert_eq!(
		item["destination"]["files"][0]["relativePath"],
		"Umberto_Eco/El_nombre_de_la_rosa.epub"
	);
	// an empty library has no tags to suggest: the uploader must choose
	assert_eq!(item["state"], "NEEDS_REVIEW", "{item:#}");

	let response = app
		.execute_gql_with_token(
			REVIEW,
			Some(json!({ "id": id, "input": { "genres": ["Historical Fiction", "Origin: Italian"] } })),
			&uploader.token,
		)
		.await;
	assert_no_errors(&response);
	assert_eq!(response["data"]["intakeReview"]["state"], "READY", "{response:#}");

	let response = app
		.execute_gql_with_token(ACCEPT, Some(json!({ "ids": [id] })), &uploader.token)
		.await;
	assert_no_errors(&response);
	let filed = &response["data"]["intakeAccept"][0];
	assert_eq!(filed["state"], "FILED", "{filed:#}");

	let target = fixture.library_dir.join("Umberto_Eco/El_nombre_de_la_rosa.epub");
	assert!(target.exists(), "the book should be in its author folder");
	let mut archive = zip::ZipArchive::new(fs::File::open(&target).unwrap()).unwrap();
	let mut opf = String::new();
	std::io::Read::read_to_string(&mut archive.by_name("OEBPS/content.opf").unwrap(), &mut opf)
		.unwrap();
	assert!(opf.contains("<dc:subject>Historical Fiction</dc:subject>"), "{opf}");
	assert!(opf.contains("<dc:subject>Origin: Italian</dc:subject>"), "{opf}");
	assert!(!opf.contains("Novela"), "{opf}");
	// the creator is written first name first, with its sort name
	assert!(opf.contains(">Umberto Eco</dc:creator>"), "{opf}");
	assert!(opf.contains(r#"property="file-as">Eco, Umberto</meta>"#), "{opf}");
	assert!(opf.contains(r#"<meta name="bcurator:shelf" content="Historical Fiction"/>"#));
	assert_eq!(archive.by_index(0).unwrap().name(), "mimetype");

	let csv = fixture.library_dir.join("stump_intake.csv");
	let text = fs::read_to_string(&csv).expect("intake csv");
	assert!(
		text.contains("Umberto_Eco/El_nombre_de_la_rosa.epub,El nombre de la rosa,Umberto Eco,"),
		"{text}"
	);
	assert!(text.contains(",uploader,"));

	// the owner sees who added it
	let response = app.execute_gql(RECENT, None).await;
	assert_no_errors(&response);
	let recent = &response["data"]["recentlyAddedBooks"][0];
	assert_eq!(recent["username"], "uploader");
	assert_eq!(recent["paths"][0], "Umberto_Eco/El_nombre_de_la_rosa.epub");
}

/// the inbox needs ADD_BOOKS, and nobody sees another user's items
#[tokio::test(flavor = "multi_thread")]
async fn test_intake_requires_the_permission_and_is_private() {
	let fixture = setup().await;
	let app = &fixture.app;
	let reader = CreateTestUser {
		username: "reader".to_string(),
		permissions: vec![UserPermission::UploadFile],
		..Default::default()
	}
	.insert(app)
	.await;
	let response = app.execute_gql_with_token(ITEMS, None, &reader.token).await;
	assert!(response.get("errors").is_some(), "should be forbidden: {response:#}");
	let response = post_multipart(
		app,
		&reader.token,
		upload_form(&fixture.library_id, epub_bytes(fixture._tempdir.path())),
	)
	.await;
	assert!(response.get("errors").is_some(), "upload should be forbidden");

	let alice = CreateTestUser {
		username: "alice".to_string(),
		permissions: vec![UserPermission::AddBooks],
		..Default::default()
	}
	.insert(app)
	.await;
	let bob = CreateTestUser {
		username: "bob".to_string(),
		permissions: vec![UserPermission::AddBooks],
		..Default::default()
	}
	.insert(app)
	.await;
	let response = post_multipart(
		app,
		&alice.token,
		upload_form(&fixture.library_id, epub_bytes(fixture._tempdir.path())),
	)
	.await;
	assert_no_errors(&response);
	let id = response["data"]["intakeUpload"][0]["id"].as_str().unwrap().to_string();
	analysed_items(app, &alice.token).await;

	let response = app
		.execute_gql_with_token(ITEM, Some(json!({ "id": id })), &bob.token)
		.await;
	assert_no_errors(&response);
	assert!(response["data"]["intakeItem"].is_null());
	let response = app
		.execute_gql_with_token(ACCEPT, Some(json!({ "ids": [id] })), &bob.token)
		.await;
	assert!(response.get("errors").is_some(), "bob can't file alice's item");
	let response = app.execute_gql_with_token(RECENT, None, &alice.token).await;
	assert!(response.get("errors").is_some(), "recently added is for admins");
}
