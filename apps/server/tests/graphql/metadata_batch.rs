//! The metadata batch API (`editMediaMetadataBatch`, `mediaByPaths`): saves through the editor's
//! path, dry runs, partial failures, one rescan, and who may use it (API keys included)

use std::{
	fs::File,
	io::{Read, Write},
	path::{Path, PathBuf},
};

use crate::common::{account::CreateTestUser, api_key::create_api_key_for_user, TestApp};
use models::{
	entity::{media_metadata, media_metadata_edit, user},
	shared::{api_key::APIKeyPermissions, enums::UserPermission},
};
use sea_orm::{prelude::*, ActiveValue::Set};
use serde_json::{json, Value};
use tempfile::TempDir;
use tests::fake_data;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive};

const BATCH: &str = r#"mutation Batch($inputs: [MediaMetadataBatchInput!]!, $options: MediaMetadataBatchOptions) {
	editMediaMetadataBatch(inputs: $inputs, options: $options) {
		dryRun saved unchanged wouldSave failed notFound rescanBooks
		items {
			index requestedId requestedPath mediaId path status saveId errors
			changes { field oldValue newValue writtenToFile }
			file { status fileFields stumpOnlyFields locked message }
		}
	}
}"#;
const BY_PATHS: &str = r#"query ByPaths($paths: [String!]!) {
	mediaByPaths(paths: $paths) { path media { id metadata { title rating } } }
}"#;

const OPF: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Notes from the End of Everything</dc:title>
    <dc:creator>Robert Pantano</dc:creator>
    <dc:subject>Physics &amp; Astronomy</dc:subject>
    <dc:language>en</dc:language>
    <dc:identifier id="id">urn:uuid:1</dc:identifier>
  </metadata>
  <manifest>
    <item id="c1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="c1"/>
  </spine>
</package>"##;

fn write_epub(path: &Path) {
	std::fs::create_dir_all(path.parent().unwrap()).unwrap();
	let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
	zip.start_file(
		"mimetype",
		SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
	)
	.unwrap();
	zip.write_all(b"application/epub+zip").unwrap();
	zip.start_file("META-INF/container.xml", SimpleFileOptions::default())
		.unwrap();
	zip.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
	zip.start_file("OEBPS/content.opf", SimpleFileOptions::default())
		.unwrap();
	zip.write_all(OPF.as_bytes()).unwrap();
	zip.start_file("OEBPS/chapter.xhtml", SimpleFileOptions::default())
		.unwrap();
	zip.write_all(b"<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>c</title></head><body><p>Hello</p></body></html>").unwrap();
	zip.finish().unwrap();
}

fn opf_of(path: &Path) -> String {
	let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
	let mut opf = String::new();
	archive
		.by_name("OEBPS/content.opf")
		.unwrap()
		.read_to_string(&mut opf)
		.unwrap();
	opf
}

struct Fixture {
	app: TestApp,
	books: Vec<(String, PathBuf)>,
	_dir: TempDir,
}

/// A library folder with `n` readable EPUBs, each a book with metadata
async fn setup(n: usize) -> Fixture {
	let app = TestApp::new_with_default_user().await;
	let dir = tempfile::tempdir().unwrap();
	let root = dir.path().join("biblioteca");
	let library = fake_data::Library {
		path: Some(root.to_string_lossy().into_owned()),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let series = fake_data::Series {
		library_id: Some(library.id.clone()),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let mut books = vec![];
	for i in 0..n {
		let path = root.join(format!("Robert_Pantano/Book_{i}.epub"));
		write_epub(&path);
		let book = fake_data::Media {
			series_id: series.id.clone(),
			path: Some(path.to_string_lossy().into_owned()),
			..Default::default()
		}
		.insert(app.conn())
		.await;
		media_metadata::ActiveModel {
			media_id: Set(Some(book.id.clone())),
			title: Set(Some("Notes from the End of Everything".into())),
			writers: Set(Some("Robert Pantano".into())),
			genres: Set(Some("Physics & Astronomy".into())),
			..Default::default()
		}
		.insert(app.conn())
		.await
		.unwrap();
		books.push((book.id, path));
	}
	Fixture {
		app,
		books,
		_dir: dir,
	}
}

fn assert_no_errors(response: &Value) {
	assert!(
		response.get("errors").is_none(),
		"should not have had errors: {response:#}"
	);
}

/// Saved books, an unknown path and a bad value in one call: each item on its own, the files
/// written, read back and recorded, and one rescan for the rewritten ones
#[tokio::test]
async fn test_batch_saves_reports_each_item_and_queues_one_rescan() {
	let fixture = setup(3).await;
	let app = &fixture.app;
	let (id0, path0) = &fixture.books[0];
	let (_, path1) = &fixture.books[1];
	let response = app
		.execute_gql(
			BATCH,
			Some(json!({ "inputs": [
				{ "id": id0, "input": { "genres": ["Philosophy", "Philosophy"], "rating": 4.5 } },
				{ "path": path1.to_string_lossy(), "input": { "title": "Notes, revised" } },
				{ "path": "/nowhere/Book.epub", "input": { "title": "x" } },
				{ "id": fixture.books[2].0, "input": { "rating": 7 } },
				{ "input": { "title": "nameless" } },
			]})),
		)
		.await;
	assert_no_errors(&response);
	let result = &response["data"]["editMediaMetadataBatch"];
	assert_eq!(result["dryRun"], json!(false));
	assert_eq!(
		(
			&result["saved"],
			&result["failed"],
			&result["notFound"],
			&result["rescanBooks"]
		),
		(&json!(2), &json!(1), &json!(2), &json!(2))
	);
	let items = result["items"].as_array().unwrap();
	let statuses: Vec<&str> = items
		.iter()
		.map(|i| i["status"].as_str().unwrap())
		.collect();
	assert_eq!(
		statuses,
		vec!["SAVED", "SAVED", "NOT_FOUND", "FAILED", "NOT_FOUND"]
	);
	assert_eq!(items[0]["mediaId"], json!(id0));
	assert_eq!(items[0]["file"]["status"], json!("WRITTEN"));
	let changes = items[0]["changes"].as_array().unwrap();
	let rating = changes.iter().find(|c| c["field"] == "rating").unwrap();
	assert_eq!(rating["newValue"], json!("4.5"));
	assert_eq!(rating["writtenToFile"], json!(true));
	assert!(items[0]["file"]["locked"]
		.as_array()
		.unwrap()
		.contains(&json!("RATING")));
	assert!(items[0]["saveId"].is_string());
	assert_eq!(items[1]["requestedPath"], json!(path1.to_string_lossy()));
	assert!(items[2]["errors"][0]
		.as_str()
		.unwrap()
		.contains("No book at this path"));
	assert!(items[3]["errors"][0]
		.as_str()
		.unwrap()
		.contains("between 0 and 5"));
	assert!(items[4]["errors"][0]
		.as_str()
		.unwrap()
		.contains("by id or by path"));

	let opf = opf_of(path0);
	assert!(
		opf.contains(r#"<meta name="bcurator:rating" content="4.5"/>"#),
		"{opf}"
	);
	assert!(opf.contains("<dc:subject>Philosophy</dc:subject>"), "{opf}");
	assert!(opf_of(path1).contains("<dc:title>Notes, revised</dc:title>"));
	let history = media_metadata_edit::Entity::find()
		.filter(media_metadata_edit::Column::MediaId.eq(id0.clone()))
		.all(app.conn())
		.await
		.unwrap();
	assert_eq!(history.len(), 2);
	assert!(history.iter().all(|row| row.written_to_file));

	// the books by path, in the order asked
	let response = app
		.execute_gql(
			BY_PATHS,
			Some(json!({ "paths": [path0.to_string_lossy(), "/nowhere.epub"] })),
		)
		.await;
	assert_no_errors(&response);
	let found = response["data"]["mediaByPaths"].as_array().unwrap();
	assert_eq!(found[0]["media"]["id"], json!(id0));
	assert_eq!(found[0]["media"]["metadata"]["rating"], json!("4.5"));
	assert!(found[1]["media"].is_null());
	assert_eq!(found[1]["path"], json!("/nowhere.epub"));
}

/// A dry run reports what would change and changes nothing; writeToFile off keeps the file
#[tokio::test]
async fn test_batch_dry_run_and_stump_only() {
	let fixture = setup(1).await;
	let app = &fixture.app;
	let (id, path) = &fixture.books[0];
	let before = std::fs::read(path).unwrap();
	let response = app
		.execute_gql(
			BATCH,
			Some(json!({
				"inputs": [{ "id": id, "input": { "genres": ["Philosophy"], "notes": "checked" } }],
				"options": { "dryRun": true }
			})),
		)
		.await;
	assert_no_errors(&response);
	let result = &response["data"]["editMediaMetadataBatch"];
	assert_eq!(result["dryRun"], json!(true));
	assert_eq!(result["wouldSave"], json!(1));
	assert_eq!(result["rescanBooks"], json!(0));
	let item = &result["items"][0];
	assert_eq!(item["status"], json!("WOULD_SAVE"));
	assert!(item["saveId"].is_null());
	assert_eq!(item["file"]["fileFields"], json!(["genres"]));
	assert_eq!(item["file"]["stumpOnlyFields"], json!(["notes"]));
	assert_eq!(std::fs::read(path).unwrap(), before);
	assert!(media_metadata_edit::Entity::find()
		.all(app.conn())
		.await
		.unwrap()
		.is_empty());

	let response = app
		.execute_gql(
			BATCH,
			Some(json!({
				"inputs": [{ "id": id, "input": { "genres": ["Philosophy"] } }],
				"options": { "writeToFile": false }
			})),
		)
		.await;
	assert_no_errors(&response);
	let item = &response["data"]["editMediaMetadataBatch"]["items"][0];
	assert_eq!(item["status"], json!("SAVED"));
	assert_eq!(item["file"]["status"], json!("SKIPPED"));
	assert_eq!(std::fs::read(path).unwrap(), before);
	let stored = media_metadata::Entity::find()
		.filter(media_metadata::Column::MediaId.eq(id.clone()))
		.one(app.conn())
		.await
		.unwrap()
		.unwrap();
	assert_eq!(stored.genres.as_deref(), Some("Philosophy"));
}

/// The batch needs EDIT_METADATA, from a session or from an API key that carries it
#[tokio::test]
async fn test_batch_permissions_and_api_keys() {
	let fixture = setup(1).await;
	let app = &fixture.app;
	let (id, path) = &fixture.books[0];
	let inputs = json!({ "inputs": [{ "id": id, "input": { "title": "By key" } }] });

	let reader = CreateTestUser {
		username: "reader".to_string(),
		permissions: vec![UserPermission::AccessApiKeys],
		..Default::default()
	}
	.insert(app)
	.await;
	let response = app
		.execute_gql_with_token(BATCH, Some(inputs.clone()), &reader.token)
		.await;
	assert!(response.get("errors").is_some(), "{response:#}");

	let editor = CreateTestUser {
		username: "editor".to_string(),
		permissions: vec![UserPermission::EditMetadata, UserPermission::AccessApiKeys],
		..Default::default()
	}
	.insert(app)
	.await;
	let editor_model = user::Entity::find_by_id(editor.id.clone())
		.one(app.conn())
		.await
		.unwrap()
		.unwrap();
	// a key without the permission is refused, one with it works as a Bearer token
	let (koreader_key, _) = create_api_key_for_user(
		app,
		&editor_model,
		Some(APIKeyPermissions::Custom(vec![
			UserPermission::AccessKoreaderSync,
		])),
	)
	.await;
	let response = app
		.execute_gql_with_token(BATCH, Some(inputs.clone()), &koreader_key)
		.await;
	assert!(response.get("errors").is_some(), "{response:#}");
	let (edit_key, _) = create_api_key_for_user(
		app,
		&editor_model,
		Some(APIKeyPermissions::Custom(vec![
			UserPermission::EditMetadata,
		])),
	)
	.await;
	let response = app
		.execute_gql_with_token(BATCH, Some(inputs), &edit_key)
		.await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["editMediaMetadataBatch"]["items"][0]["status"],
		json!("SAVED")
	);
	assert!(opf_of(path).contains("<dc:title>By key</dc:title>"));
}
