//! The GraphQL query that opens a book with the EPUB reader opens an `.epub` and nothing else, as
//! the REST routes do (`epub/streaming.rs`). A zip is a zip: a PDF or a comic with a zip stored in
//! it (a friend's upload keeps its own bytes in those formats) was parsed as a book by a reader
//! that was never meant to see it, and a table of contents nested thousands of levels deep in that
//! zip overflowed the stack of the worker that reads it and ended the server.

use std::path::{Path, PathBuf};

use models::entity::media;
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, IntoActiveModel};
use serde_json::{json, Value};
use tests::fake_data;

use crate::common::{series::setup_single_series_with_n_books, TestApp};

const EPUB_BY_ID: &str =
	"query($id: ID!) { epubById(id: $id) { mediaId spine { idref } } }";

fn fixture() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("../../core/integration-tests/data/book.epub")
		.canonicalize()
		.expect("book.epub fixture")
}

async fn setup_book() -> (TestApp, String) {
	let app = TestApp::new_with_default_user().await;
	let db = app.conn();

	let library = fake_data::Library {
		id: Some("gate_lib".to_string()),
		name: Some("Gate Library".to_string()),
		..Default::default()
	}
	.insert(db)
	.await;
	let (_, books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("gate_series".to_string()),
			name: Some("Gate Series".to_string()),
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		1,
	)
	.await;
	let book = books.into_iter().next().expect("one book");
	(app, book.id)
}

async fn point_book_at(app: &TestApp, id: &str, path: &Path) {
	let db = app.conn();
	let book = media::Entity::find_by_id(id.to_string())
		.one(db)
		.await
		.expect("the book")
		.expect("a book");
	let mut active = book.into_active_model();
	active.path = ActiveValue::Set(path.to_string_lossy().to_string());
	active.update(db).await.expect("update media path");
}

async fn epub_by_id(app: &TestApp, id: &str) -> Value {
	app.execute_gql(EPUB_BY_ID, Some(json!({ "id": id }))).await
}

/// the message of the first error of an answer, if it is one
fn error_of(answer: &Value) -> Option<String> {
	answer
		.get("errors")
		.and_then(|errors| errors.get(0))
		.and_then(|error| error.get("message"))
		.and_then(Value::as_str)
		.map(str::to_string)
}

#[tokio::test]
async fn test_epub_by_id_opens_only_an_epub() {
	let (app, id) = setup_book().await;
	let dir = tempfile::tempdir().expect("a temporary folder");

	// the control: as an EPUB, the book is read
	let copy = dir.path().join("book.epub");
	std::fs::copy(fixture(), &copy).expect("a copy of the fixture");
	point_book_at(&app, &id, &copy).await;
	let answer = epub_by_id(&app, &id).await;
	assert_eq!(error_of(&answer), None, "{answer:#}");
	assert_eq!(answer["data"]["epubById"]["mediaId"], json!(id));

	// the answer for a book that isn't there, which every refusal below must be
	let unknown = epub_by_id(&app, "no-such-book").await;
	assert_eq!(error_of(&unknown).as_deref(), Some("Media not found"));

	for name in [
		"book.cbz",
		"book.cbr",
		"book.pdf",
		"book.zip",
		"book.epub.cbz",
		"book.epub.pdf",
		"book",
	] {
		let copy = dir.path().join(name);
		std::fs::copy(fixture(), &copy).expect("a copy of the fixture");
		point_book_at(&app, &id, &copy).await;
		let answer = epub_by_id(&app, &id).await;
		assert_eq!(
			error_of(&answer).as_deref(),
			Some("Media not found"),
			"{name}: {answer:#}"
		);
		assert!(answer["data"].is_null(), "{name}: {answer:#}");
	}

	// an upper-case extension is still an EPUB
	let copy = dir.path().join("BOOK.EPUB");
	std::fs::copy(fixture(), &copy).expect("a copy of the fixture");
	point_book_at(&app, &id, &copy).await;
	let answer = epub_by_id(&app, &id).await;
	assert_eq!(error_of(&answer), None, "{answer:#}");
}

/// The case that matters: a PDF with the EPUB stored in it. Whatever finds the end record of a
/// zip from the back of a file finds it, so the reader opens the PDF as a book if it is let to
#[tokio::test]
async fn test_epub_by_id_does_not_open_a_pdf_with_a_book_in_it() {
	let (app, id) = setup_book().await;
	let dir = tempfile::tempdir().expect("a temporary folder");

	let mut bytes = b"%PDF-1.4\n1 0 obj\n<</Type/Catalog>>\nendobj\n".to_vec();
	bytes.extend_from_slice(&std::fs::read(fixture()).expect("the fixture"));

	// the control: the same bytes as an `.epub` are a book the reader opens, which is what makes
	// the file a way in
	let as_epub = dir.path().join("polyglot.epub");
	std::fs::write(&as_epub, &bytes).expect("write");
	point_book_at(&app, &id, &as_epub).await;
	let answer = epub_by_id(&app, &id).await;
	assert_eq!(error_of(&answer), None, "{answer:#}");
	assert_eq!(answer["data"]["epubById"]["mediaId"], json!(id));

	// filed as what it is, a PDF, it is not opened as one: the answer is the one for no book
	for name in ["polyglot.pdf", "polyglot.cbz", "polyglot.cbr"] {
		let path = dir.path().join(name);
		std::fs::write(&path, &bytes).expect("write");
		point_book_at(&app, &id, &path).await;
		let answer = epub_by_id(&app, &id).await;
		assert_eq!(
			error_of(&answer).as_deref(),
			Some("Media not found"),
			"{name}: {answer:#}"
		);
		assert!(answer["data"].is_null(), "{name}: {answer:#}");
	}
}

/// "Not opened" is what it says: a book that isn't an EPUB is refused before its file is looked
/// at, so a file that isn't there (which the reader would answer with a parse error) gets the
/// same answer as one that is
#[tokio::test]
async fn test_epub_by_id_refuses_before_it_opens_the_file() {
	let (app, id) = setup_book().await;
	let missing = Path::new("/no/such/folder");

	point_book_at(&app, &id, &missing.join("book.pdf")).await;
	let answer = epub_by_id(&app, &id).await;
	assert_eq!(
		error_of(&answer).as_deref(),
		Some("Media not found"),
		"{answer:#}"
	);

	// the control: as an `.epub`, the same missing file is looked for, and is a parse error
	point_book_at(&app, &id, &missing.join("book.epub")).await;
	let answer = epub_by_id(&app, &id).await;
	assert_eq!(
		error_of(&answer).as_deref(),
		Some("Error parsing epub file"),
		"{answer:#}"
	);
}
