use std::{fs, path::PathBuf, time::Duration};

use axum_test::multipart::{MultipartForm, Part};
use models::{
	entity::{intake_item, library, library_intake_settings},
	shared::{
		enums::{LibraryType, UserPermission},
		intake::IntakeState,
	},
};
use sea_orm::{prelude::*, ActiveValue::Unchanged, Set};
use serde_json::{json, Value};
use tempfile::TempDir;
use tests::{fake_data, fixtures};

use crate::common::{account::CreateTestUser, visibility::hide_library, TestApp};

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
		result { paths notes }
	}
}"#;
const ITEM: &str = r#"query Item($id: ID!) { intakeItem(id: $id) { id } }"#;
const REVIEW: &str = r#"mutation Review($id: ID!, $input: IntakeReviewInput!) {
	intakeReview(id: $id, input: $input) {
		state problems decision { metadata { genres } }
	}
}"#;
const ACCEPT: &str = r#"mutation Accept($ids: [ID!]!) {
	intakeAccept(ids: $ids) { id state error result { paths } }
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
	// an author already in the library, written the library's way
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
	assert!(
		response.get("errors").is_none(),
		"unexpected errors: {response:#}"
	);
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
	upload_form_as(library_id, bytes, "rosa.epub", "application/epub+zip")
}

fn upload_form_as(
	library_id: &str,
	bytes: Vec<u8>,
	file_name: &str,
	mime_type: &str,
) -> MultipartForm {
	let operations = json!({
		"query": UPLOAD,
		"variables": { "libraryId": library_id, "files": [null] }
	});
	let map = json!({ "0": ["variables.files.0"] });
	MultipartForm::new()
		.add_part(
			"operations",
			Part::text(operations.to_string()).mime_type("application/json"),
		)
		.add_part(
			"map",
			Part::text(map.to_string()).mime_type("application/json"),
		)
		.add_part(
			"0",
			Part::bytes(bytes)
				.file_name(file_name.to_string())
				.mime_type(mime_type.to_string()),
		)
}

/// A zip of the entries (name, bytes, deflated)
fn zip_bytes(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
	use std::io::Write;
	use zip::{write::SimpleFileOptions, CompressionMethod};

	let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
	for (name, bytes, deflated) in entries {
		let method = if *deflated {
			CompressionMethod::Deflated
		} else {
			CompressionMethod::Stored
		};
		writer
			.start_file(
				*name,
				SimpleFileOptions::default().compression_method(method),
			)
			.unwrap();
		writer.write_all(bytes).unwrap();
	}
	writer.finish().unwrap().into_inner()
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
	analysed_items_with(app, token, ITEMS).await
}

/// the same, reading the inbox with the given query
async fn analysed_items_with(app: &TestApp, token: &str, query: &str) -> Value {
	for _ in 0..100 {
		let response = app.execute_gql_with_token(query, None, token).await;
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
	assert_eq!(
		proposal["authorFolders"][0]["folder"], "Umberto_Eco",
		"{item:#}"
	);
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
			Some(
				json!({ "id": id, "input": { "genres": ["Historical Fiction", "Origin: Italian"] } }),
			),
			&uploader.token,
		)
		.await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["intakeReview"]["state"], "READY",
		"{response:#}"
	);

	let response = app
		.execute_gql_with_token(ACCEPT, Some(json!({ "ids": [id] })), &uploader.token)
		.await;
	assert_no_errors(&response);
	let filed = &response["data"]["intakeAccept"][0];
	assert_eq!(filed["state"], "FILED", "{filed:#}");

	let target = fixture
		.library_dir
		.join("Umberto_Eco/El_nombre_de_la_rosa.epub");
	assert!(target.exists(), "the book should be in its author folder");
	let mut archive = zip::ZipArchive::new(fs::File::open(&target).unwrap()).unwrap();
	let mut opf = String::new();
	std::io::Read::read_to_string(
		&mut archive.by_name("OEBPS/content.opf").unwrap(),
		&mut opf,
	)
	.unwrap();
	assert!(
		opf.contains("<dc:subject>Historical Fiction</dc:subject>"),
		"{opf}"
	);
	assert!(
		opf.contains("<dc:subject>Origin: Italian</dc:subject>"),
		"{opf}"
	);
	assert!(!opf.contains("Novela"), "{opf}");
	// the creator is written first name first, with its sort name
	assert!(opf.contains(">Umberto Eco</dc:creator>"), "{opf}");
	assert!(
		opf.contains(r#"property="file-as">Eco, Umberto</meta>"#),
		"{opf}"
	);
	assert!(opf.contains(r#"<meta name="bcurator:shelf" content="Historical Fiction"/>"#));
	assert_eq!(archive.by_index(0).unwrap().name(), "mimetype");

	// no CSV log anywhere: the filed item in the database is the record
	assert!(!fixture.library_dir.join("stump_intake.csv").exists());
	assert!(!fixture
		.library_dir
		.parent()
		.unwrap()
		.join("stump_intake.csv")
		.exists());

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
	assert!(
		response.get("errors").is_some(),
		"should be forbidden: {response:#}"
	);
	let response = post_multipart(
		app,
		&reader.token,
		upload_form(&fixture.library_id, epub_bytes(fixture._tempdir.path())),
	)
	.await;
	assert!(
		response.get("errors").is_some(),
		"upload should be forbidden"
	);

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
	let id = response["data"]["intakeUpload"][0]["id"]
		.as_str()
		.unwrap()
		.to_string();
	analysed_items(app, &alice.token).await;

	let response = app
		.execute_gql_with_token(ITEM, Some(json!({ "id": id })), &bob.token)
		.await;
	assert_no_errors(&response);
	assert!(response["data"]["intakeItem"].is_null());
	let response = app
		.execute_gql_with_token(ACCEPT, Some(json!({ "ids": [id] })), &bob.token)
		.await;
	assert!(
		response.get("errors").is_some(),
		"bob can't file alice's item"
	);
	let response = app.execute_gql_with_token(RECENT, None, &alice.token).await;
	assert!(
		response.get("errors").is_some(),
		"recently added is for admins"
	);
}

/// a `.zip` of books is unpacked into temporary files (not into memory), staged like any upload and
/// analysed like it: the book inside is found with its metadata
#[tokio::test(flavor = "multi_thread")]
async fn test_upload_a_zip_pack_of_books() {
	let fixture = setup().await;
	let app = &fixture.app;
	let uploader = CreateTestUser {
		username: "uploader".to_string(),
		permissions: vec![UserPermission::AddBooks, UserPermission::UploadFile],
		..Default::default()
	}
	.insert(app)
	.await;

	let epub = epub_bytes(fixture._tempdir.path());
	let pack = zip_bytes(&[
		("Rosa/rosa.epub", &epub, false),
		("Rosa/notes.txt", b"not a book", true),
	]);
	let response = post_multipart(
		app,
		&uploader.token,
		upload_form_as(&fixture.library_id, pack, "pack.zip", "application/zip"),
	)
	.await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["intakeUpload"].as_array().unwrap().len(),
		1
	);

	let items = analysed_items(app, &uploader.token).await;
	assert_eq!(items.as_array().unwrap().len(), 1, "{items:#}");
	let item = &items[0];
	assert_eq!(item["kind"], "BOOK", "{item:#}");
	assert_eq!(
		item["proposal"]["metadata"]["title"], "El nombre de la rosa",
		"{item:#}"
	);
}

/// a pack whose book inflates a few thousand to one is refused before anything is staged
#[tokio::test(flavor = "multi_thread")]
async fn test_a_zip_bomb_pack_is_refused() {
	let fixture = setup().await;
	let app = &fixture.app;
	let uploader = CreateTestUser {
		username: "uploader".to_string(),
		permissions: vec![UserPermission::AddBooks, UserPermission::UploadFile],
		..Default::default()
	}
	.insert(app)
	.await;

	// 5 MiB of zeros: a few KiB of deflate
	let bomb = zip_bytes(&[("bomb.epub", &vec![0u8; 5 * 1024 * 1024], true)]);
	assert!(
		bomb.len() < 64 * 1024,
		"the pack is small: {} bytes",
		bomb.len()
	);
	let response = post_multipart(
		app,
		&uploader.token,
		upload_form_as(&fixture.library_id, bomb, "bomb.zip", "application/zip"),
	)
	.await;
	let message = response["errors"][0]["message"]
		.as_str()
		.unwrap_or_default();
	assert!(
		message.contains("inflates far more"),
		"the bomb should be refused: {response:#}"
	);

	let response = app
		.execute_gql_with_token(ITEMS, None, &uploader.token)
		.await;
	assert_no_errors(&response);
	assert!(
		response["data"]["intakeItems"]
			.as_array()
			.unwrap()
			.is_empty(),
		"nothing is staged: {response:#}"
	);
}

// ---- manga and light novel libraries: one folder per series, in the library root ----

const SETTINGS_AT_ROOT: &str = r#"mutation Settings($libraryId: ID!, $dir: String!) {
	updateLibraryIntakeSettings(libraryId: $libraryId, input: {
		mangaDir: $dir, lightNovelDir: $dir, onlineLookup: false
	}) { mangaDir lightNovelDir onlineLookup }
}"#;
const TYPED_ITEMS: &str = r#"query Items {
	intakeItems {
		id state kind name error problems libraryId libraryType
		suggestedLibrary { id name }
		proposal { seriesKind warnings }
		decision { folder seriesKind }
		destination { relativeDir files { relativePath exists } }
	}
}"#;
const CHANGE_LIBRARY: &str = r#"mutation Change($id: ID!, $libraryId: ID!) {
	intakeChangeLibrary(id: $id, libraryId: $libraryId) { id state libraryId }
}"#;
const REVIEW_KIND: &str = r#"mutation Review($id: ID!, $input: IntakeReviewInput!) {
	intakeReview(id: $id, input: $input) {
		state problems decision { seriesKind folder } destination { relativeDir }
	}
}"#;

/// what the proposal says of a manga that is not in a manga library
const LOOKS_LIKE_MANGA: &str = "This looks like manga, but this isn't a manga library";

struct TypedFixture {
	app: TestApp,
	book_library: library::Model,
	manga_library: library::Model,
	book_dir: PathBuf,
	manga_dir: PathBuf,
	_tempdir: TempDir,
}

/// A book library (series in `_Manga` and `_Novela ligera`) and a manga library (series in its
/// root), neither looking titles up online
async fn typed_setup(manga_name: &str) -> TypedFixture {
	let app = TestApp::new_with_default_user().await;
	let tempdir = tempfile::tempdir().expect("tempdir");
	let book_dir = tempdir.path().join("biblioteca");
	let manga_dir = tempdir.path().join("manga");
	fs::create_dir_all(&book_dir).expect("book library folder");
	fs::create_dir_all(&manga_dir).expect("manga library folder");
	let book_library = fake_data::Library {
		name: Some("Biblioteca".to_string()),
		path: Some(book_dir.to_string_lossy().to_string()),
		library_type: Some(LibraryType::Book),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let manga_library = fake_data::Library {
		name: Some(manga_name.to_string()),
		path: Some(manga_dir.to_string_lossy().to_string()),
		library_type: Some(LibraryType::Manga),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let response = app
		.execute_gql(SETTINGS, Some(json!({ "libraryId": book_library.id })))
		.await;
	assert_no_errors(&response);
	// the manga library has no settings row, as in a live one: what its uploads run on is the
	// typed default (series folders at its root), not a row somebody saved
	let row = library_intake_settings::Entity::find()
		.filter(library_intake_settings::Column::LibraryId.eq(manga_library.id.clone()))
		.one(app.conn())
		.await
		.unwrap();
	assert!(row.is_none(), "{row:?}");
	TypedFixture {
		app,
		book_library,
		manga_library,
		book_dir,
		manga_dir,
		_tempdir: tempdir,
	}
}

/// a comic book archive of one page whose ComicInfo names the series and the volume
fn cbz_of(series: &str, volume: u32) -> Vec<u8> {
	// a 1x1 PNG
	const PIXEL: &[u8] = &[
		0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49,
		0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06,
		0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44,
		0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d,
		0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42,
		0x60, 0x82,
	];
	let info = format!(
		"<?xml version=\"1.0\"?><ComicInfo><Series>{series}</Series><Number>{volume}</Number></ComicInfo>"
	);
	zip_bytes(&[
		("001.png", PIXEL, false),
		("ComicInfo.xml", info.as_bytes(), true),
	])
}

/// `Berserk.zip`: two volumes of one manga series
fn manga_pack() -> Vec<u8> {
	let (one, two) = (cbz_of("Berserk", 1), cbz_of("Berserk", 2));
	zip_bytes(&[
		("Berserk v01.cbz", &one, false),
		("Berserk v02.cbz", &two, false),
	])
}

async fn make_uploader(app: &TestApp) -> crate::common::account::TestUser {
	CreateTestUser {
		username: "uploader".to_string(),
		permissions: vec![UserPermission::AddBooks, UserPermission::UploadFile],
		..Default::default()
	}
	.insert(app)
	.await
}

/// uploads the pack into a library and waits for its analysis: the one item of the inbox
async fn upload_pack(app: &TestApp, token: &str, library_id: &str) -> Value {
	let response = post_multipart(
		app,
		token,
		upload_form_as(library_id, manga_pack(), "Berserk.zip", "application/zip"),
	)
	.await;
	assert_no_errors(&response);
	let items = analysed_items_with(app, token, TYPED_ITEMS).await;
	assert_eq!(items.as_array().unwrap().len(), 1, "{items:#}");
	items[0].clone()
}

/// a series uploaded to a manga library goes to `<library>/<Series>/`: no `_Manga` folder, and
/// no path that starts with `/` (which would leave the library); the library holds series of
/// its own kind only, whatever the review asks for
#[tokio::test(flavor = "multi_thread")]
async fn test_a_manga_library_files_a_series_in_its_root() {
	let fixture = typed_setup("Manga").await;
	let app = &fixture.app;
	let uploader = make_uploader(app).await;

	let item = upload_pack(app, &uploader.token, &fixture.manga_library.id).await;
	assert_eq!(item["kind"], "SERIES", "{item:#}");
	assert_eq!(item["libraryType"], "MANGA", "{item:#}");
	assert_eq!(item["proposal"]["seriesKind"], "MANGA", "{item:#}");
	assert_eq!(item["decision"]["folder"], "Berserk", "{item:#}");
	assert_eq!(item["destination"]["relativeDir"], "Berserk", "{item:#}");
	assert_eq!(
		item["destination"]["files"][0]["relativePath"], "Berserk/Berserk v01.cbz",
		"{item:#}"
	);
	assert_eq!(
		item["destination"]["files"][1]["relativePath"], "Berserk/Berserk v02.cbz",
		"{item:#}"
	);
	// it is a manga in a manga library: nothing to say about the fit, and no other library to offer
	assert!(item["proposal"]["warnings"]
		.as_array()
		.unwrap()
		.iter()
		.all(|w| !w.as_str().unwrap_or_default().contains("look")));
	assert!(item["suggestedLibrary"].is_null(), "{item:#}");
	assert_eq!(item["state"], "READY", "{item:#}");
	let id = item["id"].as_str().expect("item id").to_string();

	// the review can't take it out of its library's kind (a light novel, or a book by an author)
	for input in [
		json!({ "clearSeriesKind": true }),
		json!({ "seriesKind": "LIGHT_NOVEL" }),
	] {
		let response = app
			.execute_gql_with_token(
				REVIEW_KIND,
				Some(json!({ "id": id, "input": input })),
				&uploader.token,
			)
			.await;
		assert_no_errors(&response);
		let reviewed = &response["data"]["intakeReview"];
		assert_eq!(reviewed["decision"]["seriesKind"], "MANGA", "{reviewed:#}");
		assert_eq!(
			reviewed["destination"]["relativeDir"], "Berserk",
			"{reviewed:#}"
		);
	}

	let response = app
		.execute_gql_with_token(ACCEPT, Some(json!({ "ids": [id] })), &uploader.token)
		.await;
	assert_no_errors(&response);
	let filed = &response["data"]["intakeAccept"][0];
	assert_eq!(filed["state"], "FILED", "{filed:#}");
	assert!(fixture.manga_dir.join("Berserk/Berserk v01.cbz").is_file());
	assert!(fixture.manga_dir.join("Berserk/Berserk v02.cbz").is_file());
	for folder in ["_Manga", "_Novela ligera"] {
		assert!(!fixture.manga_dir.join(folder).exists(), "{folder}");
		assert!(!fixture.book_dir.join(folder).exists(), "{folder}");
	}
	// the library holds the series folder and nothing else
	let entries: Vec<String> = fs::read_dir(&fixture.manga_dir)
		.unwrap()
		.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
		.collect();
	assert_eq!(entries, vec!["Berserk".to_string()]);
}

/// a manga aimed at a book library is filed in that library's `_Manga` folder, with a note and the
/// offer of the manga library; moving it analyses it again for the manga library
#[tokio::test(flavor = "multi_thread")]
async fn test_a_manga_aimed_at_a_book_library_can_move_to_the_manga_library() {
	let fixture = typed_setup("Manga").await;
	let app = &fixture.app;
	let uploader = make_uploader(app).await;

	let item = upload_pack(app, &uploader.token, &fixture.book_library.id).await;
	assert_eq!(item["libraryType"], "BOOK", "{item:#}");
	assert_eq!(item["proposal"]["seriesKind"], "MANGA", "{item:#}");
	assert_eq!(
		item["destination"]["relativeDir"], "_Manga/Berserk",
		"{item:#}"
	);
	let warnings = item["proposal"]["warnings"].as_array().unwrap();
	assert!(
		warnings.iter().any(|w| w == LOOKS_LIKE_MANGA),
		"the note is there: {item:#}"
	);
	// a note, not a block
	assert!(item["problems"].as_array().unwrap().is_empty(), "{item:#}");
	assert_eq!(
		item["suggestedLibrary"],
		json!({ "id": fixture.manga_library.id, "name": "Manga" }),
		"{item:#}"
	);
	let id = item["id"].as_str().expect("item id").to_string();

	let response = app
		.execute_gql_with_token(
			CHANGE_LIBRARY,
			Some(json!({ "id": id, "libraryId": fixture.manga_library.id })),
			&uploader.token,
		)
		.await;
	assert_no_errors(&response);
	let moved = &response["data"]["intakeChangeLibrary"];
	assert_eq!(moved["libraryId"], fixture.manga_library.id, "{moved:#}");
	assert_eq!(moved["state"], "ANALYSING", "{moved:#}");

	let items = analysed_items_with(app, &uploader.token, TYPED_ITEMS).await;
	assert_eq!(items.as_array().unwrap().len(), 1, "{items:#}");
	let item = &items[0];
	assert_eq!(item["libraryId"], fixture.manga_library.id, "{item:#}");
	assert_eq!(item["libraryType"], "MANGA", "{item:#}");
	assert_eq!(item["destination"]["relativeDir"], "Berserk", "{item:#}");
	assert!(
		!item["proposal"]["warnings"]
			.as_array()
			.unwrap()
			.iter()
			.any(|w| w == LOOKS_LIKE_MANGA),
		"{item:#}"
	);
	assert!(item["suggestedLibrary"].is_null(), "{item:#}");
	assert!(!fixture.book_dir.join("_Manga").exists());
}

/// a library the uploader may not use is never named in a proposal or an offer, and an item can't
/// be moved into it
#[tokio::test(flavor = "multi_thread")]
async fn test_a_library_hidden_from_the_uploader_is_never_offered() {
	let fixture = typed_setup("Estante Secreta").await;
	let app = &fixture.app;
	let uploader = make_uploader(app).await;
	hide_library(app, &uploader, &fixture.manga_library).await;

	let item = upload_pack(app, &uploader.token, &fixture.book_library.id).await;
	assert_eq!(item["proposal"]["seriesKind"], "MANGA", "{item:#}");
	assert!(item["suggestedLibrary"].is_null(), "{item:#}");
	assert!(
		!item.to_string().contains("Estante Secreta"),
		"nothing names the hidden library: {item:#}"
	);
	let id = item["id"].as_str().expect("item id").to_string();

	let response = app
		.execute_gql_with_token(
			CHANGE_LIBRARY,
			Some(json!({ "id": id, "libraryId": fixture.manga_library.id })),
			&uploader.token,
		)
		.await;
	assert_eq!(
		response["errors"][0]["message"], "Library not found",
		"{response:#}"
	);
	// it is still in the library it was in
	let items = analysed_items_with(app, &uploader.token, TYPED_ITEMS).await;
	assert_eq!(items[0]["libraryId"], fixture.book_library.id, "{items:#}");

	// and nobody else's item can be moved
	let other = CreateTestUser {
		username: "other".to_string(),
		permissions: vec![UserPermission::AddBooks],
		..Default::default()
	}
	.insert(app)
	.await;
	let response = app
		.execute_gql_with_token(
			CHANGE_LIBRARY,
			Some(json!({ "id": id, "libraryId": fixture.book_library.id })),
			&other.token,
		)
		.await;
	assert!(response.get("errors").is_some(), "{response:#}");
}

/// an empty series folder is for manga and light novel libraries; a book library needs real ones
#[tokio::test(flavor = "multi_thread")]
async fn test_empty_series_folders_are_for_typed_libraries_only() {
	let fixture = typed_setup("Manga").await;
	let app = &fixture.app;

	let response = app
		.execute_gql(
			SETTINGS_AT_ROOT,
			Some(json!({ "libraryId": fixture.book_library.id, "dir": "" })),
		)
		.await;
	assert_eq!(
		response["errors"][0]["message"], "Series folders must be a single folder name",
		"{response:#}"
	);
	// a typed library takes an empty folder, and the setting reads back as saved
	let response = app
		.execute_gql(
			SETTINGS_AT_ROOT,
			Some(json!({ "libraryId": fixture.manga_library.id, "dir": "" })),
		)
		.await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["updateLibraryIntakeSettings"]["mangaDir"], "",
		"{response:#}"
	);
	// a typed library still refuses a folder that isn't one name
	let response = app
		.execute_gql(
			SETTINGS_AT_ROOT,
			Some(json!({ "libraryId": fixture.manga_library.id, "dir": "../outside" })),
		)
		.await;
	assert_eq!(
		response["errors"][0]["message"],
		"Series folders must be empty or a single folder name",
		"{response:#}"
	);
	// and takes a named one
	let response = app
		.execute_gql(
			SETTINGS_AT_ROOT,
			Some(json!({ "libraryId": fixture.manga_library.id, "dir": "Series" })),
		)
		.await;
	assert_no_errors(&response);
}

const REANALYSE: &str = r#"mutation Reanalyse($id: ID!) {
	intakeReanalyse(id: $id, online: false) { id state libraryId }
}"#;

/// puts an inbox item in a state, as the server does when it files, discards or starts an analysis
async fn set_item_state(app: &TestApp, id: &str, state: IntakeState) {
	intake_item::ActiveModel {
		id: Unchanged(id.to_string()),
		state: Set(state),
		..Default::default()
	}
	.update(app.conn())
	.await
	.unwrap();
}

/// an item moves to another library only while it waits for its uploader: not while it is being
/// analysed, and not once it is filed or discarded; an analysis still marked as running can be
/// asked again (it may never have answered), a filed or discarded item can't
#[tokio::test(flavor = "multi_thread")]
async fn test_an_item_moves_only_while_it_waits_for_its_uploader() {
	let fixture = typed_setup("Manga").await;
	let app = &fixture.app;
	let uploader = make_uploader(app).await;
	let item = upload_pack(app, &uploader.token, &fixture.book_library.id).await;
	let id = item["id"].as_str().expect("item id").to_string();
	let to_manga = Some(json!({ "id": id, "libraryId": fixture.manga_library.id }));
	let message = |response: &Value| response["errors"][0]["message"].clone();

	// being analysed: it can't be moved or reviewed; asking for the analysis again works
	set_item_state(app, &id, IntakeState::Analysing).await;
	let response = app
		.execute_gql_with_token(CHANGE_LIBRARY, to_manga.clone(), &uploader.token)
		.await;
	assert_eq!(
		message(&response),
		"An item that is ANALYSING can't be moved to another library",
		"{response:#}"
	);
	let response = app
		.execute_gql_with_token(
			REVIEW_KIND,
			Some(json!({ "id": id, "input": { "title": "New title" } })),
			&uploader.token,
		)
		.await;
	assert_eq!(
		message(&response),
		"An item that is ANALYSING can't be changed",
		"{response:#}"
	);
	let response = app
		.execute_gql_with_token(REANALYSE, Some(json!({ "id": id })), &uploader.token)
		.await;
	assert_no_errors(&response);
	let items = analysed_items_with(app, &uploader.token, TYPED_ITEMS).await;
	assert_eq!(items.as_array().unwrap().len(), 1, "{items:#}");
	assert_eq!(items[0]["libraryId"], fixture.book_library.id, "{items:#}");
	assert!(
		items[0]["state"] == "READY" || items[0]["state"] == "NEEDS_REVIEW",
		"it is analysed again, in the library it was in: {items:#}"
	);

	// waiting for its uploader (analysed again): it moves, and is analysed in the new library
	let response = app
		.execute_gql_with_token(CHANGE_LIBRARY, to_manga.clone(), &uploader.token)
		.await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["intakeChangeLibrary"]["libraryId"], fixture.manga_library.id,
		"{response:#}"
	);
	let items = analysed_items_with(app, &uploader.token, TYPED_ITEMS).await;
	assert_eq!(items[0]["libraryId"], fixture.manga_library.id, "{items:#}");

	// filed or discarded: it is no longer in the inbox, so neither works
	for closed in [IntakeState::Filed, IntakeState::Discarded] {
		set_item_state(app, &id, closed).await;
		let response = app
			.execute_gql_with_token(
				CHANGE_LIBRARY,
				Some(json!({ "id": id, "libraryId": fixture.book_library.id })),
				&uploader.token,
			)
			.await;
		assert_eq!(
			message(&response),
			"This item is no longer in the inbox",
			"{closed}: {response:#}"
		);
		let response = app
			.execute_gql_with_token(REANALYSE, Some(json!({ "id": id })), &uploader.token)
			.await;
		assert_eq!(
			message(&response),
			"This item is no longer in the inbox",
			"{closed}: {response:#}"
		);
	}
}
