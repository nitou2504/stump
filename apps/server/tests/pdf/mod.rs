//! The PDF reader's routes (`/api/v2/pdf/{id}/…`) and the PDF checks on annotations and
//! bookmarks. Contract: `/tmp/pdf-lane/CONTRACT.md` (copied in the branch's FORK.md notes).
//!
//! The rendering tests need PDFium (`PDFIUM_PATH`); without it they check only auth and errors.

use std::path::PathBuf;

use models::entity::media;
use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};
use serde_json::{json, Value};
use tests::fake_data;

use crate::common::{series::setup_single_series_with_n_books, TestApp};

fn fixture(name: &str) -> String {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("../../core/integration-tests/data")
		.join(name)
		.canonicalize()
		.expect("fixture")
		.to_string_lossy()
		.to_string()
}

/// A library with one book whose file is `file` and whose page count is `pages`
async fn setup_book(file: &str, pages: i32) -> (TestApp, String) {
	let app = TestApp::new_with_default_user().await;
	let db = app.conn();
	let library = fake_data::Library::default().insert(db).await;
	let (_, books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;
	let book = books.into_iter().next().expect("one book");
	let mut active: media::ActiveModel = book.into_active_model();
	active.path = ActiveValue::Set(fixture(file));
	active.extension =
		ActiveValue::Set(file.rsplit('.').next().unwrap_or_default().to_string());
	active.pages = ActiveValue::Set(pages);
	let book = active.update(db).await.expect("update media");
	(app, book.id)
}

/// PDFium is not installed where the tests run: the route answers 500 and says so
fn pdfium_missing(response: &axum_test::TestResponse) -> bool {
	let missing =
		response.status_code().is_server_error() && response.text().contains("PDFium");
	if missing {
		eprintln!("Skipping the rendering checks: PDFium is not configured");
	}
	missing
}

#[tokio::test]
async fn test_pdf_routes_require_auth_and_a_visible_pdf() {
	let (app, book_id) = setup_book("rust_book.pdf", 2).await;

	let response = app
		.server
		.get(&format!("/api/v2/pdf/{book_id}/layout"))
		.await;
	assert!(
		response.status_code().is_client_error(),
		"expected a client error without auth, got {}",
		response.status_code()
	);

	app.get("/api/v2/pdf/does-not-exist/layout")
		.await
		.assert_status_not_found();

	let (epub_app, epub_id) = setup_book("book.epub", 2).await;
	let response = epub_app.get(&format!("/api/v2/pdf/{epub_id}/layout")).await;
	response.assert_status_bad_request();
	assert!(response.text().contains("not a PDF"));
}

#[tokio::test]
async fn test_pdf_layout_pages_text_and_search() {
	let (app, book_id) = setup_book("rust_book.pdf", 2).await;

	let response = app.get(&format!("/api/v2/pdf/{book_id}/layout")).await;
	if pdfium_missing(&response) {
		return;
	}
	response.assert_status_ok();
	let etag = response
		.headers()
		.get("etag")
		.and_then(|value| value.to_str().ok())
		.expect("etag")
		.to_string();
	let layout: Value = response.json();
	let version = layout["v"].as_str().expect("v").to_string();
	assert_eq!(etag, format!("\"{version}\""));
	let page_count = layout["pageCount"].as_i64().expect("pageCount");
	assert!(page_count >= 1);
	assert_eq!(layout["pages"].as_array().unwrap().len() as i64, page_count);
	assert!(layout["pages"][0][0].as_f64().unwrap() > 0.0);
	assert!(layout["labels"].is_null());
	assert!(layout["outline"].is_array());
	assert_eq!(layout["tiers"], json!([720, 1000, 1400, 2000]));

	// a conditional request with the version is not modified
	let response = app
		.server
		.get(&format!("/api/v2/pdf/{book_id}/layout"))
		.add_header("Authorization", app.auth_header().await)
		.add_header("If-None-Match", etag.clone())
		.await;
	assert_eq!(response.status_code(), 304);

	// a picture snapped to a tier, immutable for the current version
	let response = app
		.get(&format!("/api/v2/pdf/{book_id}/page/1?w=500&v={version}"))
		.await;
	response.assert_status_ok();
	let headers = response.headers().clone();
	assert_eq!(headers["content-type"], "image/webp");
	assert_eq!(headers["x-pdf-tier"], "720");
	assert!(headers["cache-control"]
		.to_str()
		.unwrap()
		.contains("immutable"));
	let size = headers["x-pdf-page-size"].to_str().unwrap().to_string();
	assert!(size.starts_with("720x"), "size {size}");
	let bytes = response.as_bytes();
	assert_eq!(&bytes[0..4], b"RIFF");
	assert_eq!(&bytes[12..16], b"VP8 ", "lossy WebP");

	// the second request is served from the cache, same bytes, any stale version revalidates
	let again = app
		.get(&format!("/api/v2/pdf/{book_id}/page/1?w=700&v=old"))
		.await;
	again.assert_status_ok();
	assert_eq!(again.as_bytes(), bytes);
	assert!(again.headers()["cache-control"]
		.to_str()
		.unwrap()
		.contains("no-cache"));

	app.get(&format!("/api/v2/pdf/{book_id}/page/{}", page_count + 1))
		.await
		.assert_status_bad_request();
	app.get(&format!("/api/v2/pdf/{book_id}/page/1?w=0"))
		.await
		.assert_status_bad_request();

	// the text layer: runs as [x, y, w, h, text, firstChar, eol] fractions of the page
	let response = app.get(&format!("/api/v2/pdf/{book_id}/text/1")).await;
	response.assert_status_ok();
	let text: Value = response.json();
	assert!(text["w"].as_f64().unwrap() > 0.0);
	let runs = text["runs"].as_array().expect("runs");
	assert!(!runs.is_empty(), "the first page of the Rust book has text");
	for run in runs {
		let run = run.as_array().unwrap();
		assert_eq!(run.len(), 7);
		for value in &run[0..4] {
			let value = value.as_f64().unwrap();
			assert!((0.0..=1.0).contains(&value), "fraction {value}");
		}
		assert!(run[4].is_string());
		assert!(run[5].as_i64().unwrap() >= 0);
	}
	let words: String = runs
		.iter()
		.map(|run| run[4].as_str().unwrap().to_string())
		.collect::<Vec<_>>()
		.join(" ");
	let word = words
		.split_whitespace()
		.find(|word| word.chars().count() >= 4 && word.chars().all(char::is_alphabetic))
		.expect("a word on page 1")
		.to_lowercase();

	// search finds that word on page 1, with its char range and boxes
	let response = app
		.get(&format!("/api/v2/pdf/{book_id}/search?q={word}&limit=5"))
		.await;
	response.assert_status_ok();
	let search: Value = response.json();
	let results = search["results"].as_array().expect("results");
	let hit = results
		.iter()
		.find(|hit| hit["page"] == 1)
		.unwrap_or_else(|| panic!("no hit for {word} on page 1: {search:#}"));
	let [start, end] = [
		hit["chars"][0].as_i64().unwrap(),
		hit["chars"][1].as_i64().unwrap(),
	];
	assert!(end > start);
	assert!(!hit["rects"].as_array().unwrap().is_empty());
	assert!(hit["snippet"]
		.as_str()
		.unwrap()
		.to_lowercase()
		.contains(&word));

	app.get(&format!("/api/v2/pdf/{book_id}/search?q=a"))
		.await
		.assert_status_bad_request();
}

const CREATE_ANNOTATION: &str = r#"mutation CreateAnnotation($input: CreateAnnotationInput!) {
	createAnnotation(input: $input) { id style }
}"#;
const CREATE_BOOKMARK: &str = r#"mutation CreateBookmark($input: BookmarkInput!) {
	createBookmark(input: $input) { id page }
}"#;

fn pdf_locator(position: Value, rects: Value) -> Value {
	json!({
		"href": "page/2",
		"type": "application/pdf",
		"locations": {
			"position": position,
			"fragments": ["page=2"],
			"rects": rects,
		},
		"text": { "highlight": "a quote" }
	})
}

fn errors(response: &Value) -> bool {
	response.get("errors").is_some()
}

#[tokio::test]
async fn test_pdf_locators_are_checked_and_bookmarks_keep_their_page() {
	let (app, book_id) = setup_book("rust_book.pdf", 10).await;
	let rect = json!([{ "x": 0.1, "y": 0.2, "width": 0.5, "height": 0.02 }]);

	let create = |locator: Value| json!({ "input": { "mediaId": book_id, "locator": locator, "style": "INVERT" } });

	let ok = app
		.execute_gql(
			CREATE_ANNOTATION,
			Some(create(pdf_locator(json!(2), rect.clone()))),
		)
		.await;
	assert!(!errors(&ok), "{ok:#}");
	assert_eq!(ok["data"]["createAnnotation"]["style"], "INVERT");

	for (position, rects) in [
		(json!(null), rect.clone()),
		(json!(0), rect.clone()),
		(json!(11), rect.clone()),
		(
			json!(2),
			json!([{ "x": 0.8, "y": 0.2, "width": 0.5, "height": 0.02 }]),
		),
		(
			json!(2),
			json!([{ "x": -0.1, "y": 0.2, "width": 0.5, "height": 0.02 }]),
		),
	] {
		let response = app
			.execute_gql(
				CREATE_ANNOTATION,
				Some(create(pdf_locator(position.clone(), rects))),
			)
			.await;
		assert!(errors(&response), "position {position} should be rejected");
	}

	// an EPUB-style locator is not checked
	let epub = app
		.execute_gql(
			CREATE_ANNOTATION,
			Some(create(json!({
				"href": "/resource/chapter-1.xhtml",
				"locations": { "progression": 0.5 }
			}))),
		)
		.await;
	assert!(!errors(&epub), "{epub:#}");

	let bookmark = app
		.execute_gql(
			CREATE_BOOKMARK,
			Some(json!({ "input": { "mediaId": book_id, "locator": pdf_locator(json!(7), json!([])) } })),
		)
		.await;
	assert!(!errors(&bookmark), "{bookmark:#}");
	assert_eq!(bookmark["data"]["createBookmark"]["page"], 7);
}
