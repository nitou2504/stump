use crate::common::{
	account::CreateTestUser, series::setup_single_series_with_n_books, TestApp,
};
use graphql::input::user::AgeRestrictionInput;
use serde_json::{json, Value};
use tests::fake_data;

const CREATE_ANNOTATION: &str = r#"mutation CreateAnnotation($input: CreateAnnotationInput!) {
	createAnnotation(input: $input) { id style annotationText userId }
}"#;
const UPDATE_ANNOTATION: &str = r#"mutation UpdateAnnotation($input: UpdateAnnotationInput!) {
	updateAnnotation(input: $input) { id style annotationText }
}"#;
const DELETE_ANNOTATION: &str = r#"mutation DeleteAnnotation($id: String!) {
	deleteAnnotation(id: $id) { id }
}"#;
const ANNOTATIONS: &str = r#"query Annotations($id: ID!) {
	annotationsByMediaId(id: $id) {
		id
		style
		annotationText
		locator { href locations { position rects { x y width height } } text { highlight } }
	}
}"#;
const CREATE_BOOKMARK: &str = r#"mutation CreateBookmark($input: BookmarkInput!) {
	createBookmark(input: $input) { id }
}"#;
const DELETE_BOOKMARK: &str = r#"mutation DeleteBookmark($id: String!) {
	deleteBookmark(id: $id) { id }
}"#;
const BOOKMARKS: &str = r#"query Bookmarks($id: ID!) {
	bookmarksByMediaId(id: $id) { id }
}"#;

async fn setup_book(app: &TestApp) -> String {
	let library = fake_data::Library::default().insert(app.conn()).await;
	let (_, books) = setup_single_series_with_n_books(
		app,
		fake_data::Series {
			library_id: Some(library.id),
			..Default::default()
		},
		1,
	)
	.await;
	books[0].id.clone()
}

fn locator() -> Value {
	json!({
		"href": "/resource/chapter-1.xhtml",
		"chapterTitle": "Chapter One",
		"locations": { "progression": 0.5, "position": 3 },
		"text": { "highlight": "a quote", "before": "before ", "after": " after" }
	})
}

fn assert_rejected(response: &Value) {
	assert!(
		response.get("errors").is_some(),
		"should have been rejected: {response:#}"
	);
}

fn assert_no_errors(response: &Value) {
	assert!(
		response.get("errors").is_none(),
		"should not have had errors: {response:#}"
	);
}

/// the annotations the server-owner (default user) sees for the book
async fn owner_annotations(app: &TestApp, book_id: &str) -> Vec<Value> {
	let response = app
		.execute_gql(ANNOTATIONS, Some(json!({ "id": book_id })))
		.await;
	assert_no_errors(&response);
	response["data"]["annotationsByMediaId"]
		.as_array()
		.expect("annotations should be a list")
		.clone()
}

/// a reader's annotations and bookmarks are theirs alone: another reader of the same book
/// can't list, change or delete them by ID
#[tokio::test]
async fn test_annotations_and_bookmarks_are_private_to_their_user() {
	let app = TestApp::new_with_default_user().await;
	let book_id = setup_book(&app).await;
	let other = CreateTestUser {
		username: "other-reader".to_string(),
		..Default::default()
	}
	.insert(&app)
	.await;

	let response = app
		.execute_gql(
			CREATE_ANNOTATION,
			Some(json!({ "input": {
				"mediaId": book_id,
				"locator": locator(),
				"annotationText": "my note",
				"style": "UNDERLINE",
			}})),
		)
		.await;
	assert_no_errors(&response);
	let annotation_id = response["data"]["createAnnotation"]["id"]
		.as_str()
		.expect("should return the annotation id")
		.to_string();
	assert_eq!(response["data"]["createAnnotation"]["style"], "UNDERLINE");

	let response = app
		.execute_gql(
			CREATE_BOOKMARK,
			Some(json!({ "input": { "mediaId": book_id, "locator": locator() } })),
		)
		.await;
	assert_no_errors(&response);
	let bookmark_id = response["data"]["createBookmark"]["id"]
		.as_str()
		.expect("should return the bookmark id")
		.to_string();

	// the other reader sees none of them
	let response = app
		.execute_gql_with_token(ANNOTATIONS, Some(json!({ "id": book_id })), &other.token)
		.await;
	assert_no_errors(&response);
	assert_eq!(response["data"]["annotationsByMediaId"], json!([]));
	let response = app
		.execute_gql_with_token(BOOKMARKS, Some(json!({ "id": book_id })), &other.token)
		.await;
	assert_no_errors(&response);
	assert_eq!(response["data"]["bookmarksByMediaId"], json!([]));

	// ... and can't change or delete them by ID
	let response = app
		.execute_gql_with_token(
			UPDATE_ANNOTATION,
			Some(json!({ "input": {
				"id": annotation_id,
				"annotationText": "not yours",
				"style": "INVERT",
			}})),
			&other.token,
		)
		.await;
	assert_rejected(&response);
	let response = app
		.execute_gql_with_token(
			DELETE_ANNOTATION,
			Some(json!({ "id": annotation_id })),
			&other.token,
		)
		.await;
	assert_rejected(&response);
	let response = app
		.execute_gql_with_token(
			DELETE_BOOKMARK,
			Some(json!({ "id": bookmark_id })),
			&other.token,
		)
		.await;
	assert_rejected(&response);

	let annotations = owner_annotations(&app, &book_id).await;
	assert_eq!(annotations.len(), 1);
	assert_eq!(annotations[0]["style"], "UNDERLINE");
	assert_eq!(annotations[0]["annotationText"], "my note");
	let response = app
		.execute_gql(BOOKMARKS, Some(json!({ "id": book_id })))
		.await;
	assert_eq!(
		response["data"]["bookmarksByMediaId"]
			.as_array()
			.map(Vec::len),
		Some(1)
	);

	// their own annotation stays theirs too
	let response = app
		.execute_gql_with_token(
			CREATE_ANNOTATION,
			Some(json!({ "input": { "mediaId": book_id, "locator": locator() } })),
			&other.token,
		)
		.await;
	assert_no_errors(&response);
	assert_eq!(response["data"]["createAnnotation"]["style"], "HIGHLIGHT");
	assert_eq!(response["data"]["createAnnotation"]["userId"], other.id);
	let response = app
		.execute_gql_with_token(ANNOTATIONS, Some(json!({ "id": book_id })), &other.token)
		.await;
	assert_eq!(
		response["data"]["annotationsByMediaId"]
			.as_array()
			.map(Vec::len),
		Some(1)
	);
	assert_eq!(owner_annotations(&app, &book_id).await.len(), 1);
}

/// an update only changes what it names: the style alone keeps the note, a null note
/// removes it and keeps the style
#[tokio::test]
async fn test_update_annotation_changes_only_the_given_fields() {
	let app = TestApp::new_with_default_user().await;
	let book_id = setup_book(&app).await;

	let response = app
		.execute_gql(
			CREATE_ANNOTATION,
			Some(json!({ "input": {
				"mediaId": book_id,
				"locator": locator(),
				"annotationText": "keep me",
			}})),
		)
		.await;
	assert_no_errors(&response);
	let id = response["data"]["createAnnotation"]["id"].clone();
	assert_eq!(response["data"]["createAnnotation"]["style"], "HIGHLIGHT");

	let response = app
		.execute_gql(
			UPDATE_ANNOTATION,
			Some(json!({ "input": { "id": id, "style": "STRIKETHROUGH" } })),
		)
		.await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["updateAnnotation"]["style"],
		"STRIKETHROUGH"
	);
	assert_eq!(
		response["data"]["updateAnnotation"]["annotationText"],
		"keep me"
	);

	let response = app
		.execute_gql(
			UPDATE_ANNOTATION,
			Some(json!({ "input": { "id": id, "annotationText": null } })),
		)
		.await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["updateAnnotation"]["style"],
		"STRIKETHROUGH"
	);
	assert!(response["data"]["updateAnnotation"]["annotationText"].is_null());
}

/// a fixed-layout (PDF) annotation keeps its page and selection boxes in the locator
#[tokio::test]
async fn test_annotation_locator_keeps_page_rects() {
	let app = TestApp::new_with_default_user().await;
	let book_id = setup_book(&app).await;

	let response = app
		.execute_gql(
			CREATE_ANNOTATION,
			Some(json!({ "input": {
				"mediaId": book_id,
				"style": "INVERT",
				"locator": {
					"href": "/api/v2/media/book.pdf",
					"type": "application/pdf",
					"locations": {
						"position": 12,
						"fragments": ["page=12"],
						"rects": [
							{ "x": 0.1, "y": 0.2, "width": 0.5, "height": 0.02 },
							{ "x": 0.1, "y": 0.23, "width": 0.3, "height": 0.02 }
						]
					},
					"text": { "highlight": "two lines" }
				},
			}})),
		)
		.await;
	assert_no_errors(&response);

	let annotations = owner_annotations(&app, &book_id).await;
	assert_eq!(annotations.len(), 1);
	assert_eq!(annotations[0]["style"], "INVERT");
	let locations = &annotations[0]["locator"]["locations"];
	assert_eq!(locations["position"], 12);
	let rects = locations["rects"].as_array().expect("rects should be kept");
	assert_eq!(rects.len(), 2);
	// decimals come back as strings
	let y = rects[1]["y"]
		.as_f64()
		.or_else(|| rects[1]["y"].as_str().and_then(|s| s.parse().ok()))
		.expect("y should be a number");
	assert!((y - 0.23).abs() < 1e-9, "unexpected y: {y}");
}

/// annotations and bookmarks can only be made on a book the user can see
#[tokio::test]
async fn test_cannot_annotate_or_bookmark_a_hidden_book() {
	let app = TestApp::new_with_default_user().await;
	let book_id = setup_book(&app).await;
	// the book has no age rating, so a restricted reader can't see it
	let restricted = CreateTestUser {
		username: "restricted-reader".to_string(),
		age_restriction: Some(AgeRestrictionInput {
			age: 10,
			restrict_on_unset: true,
		}),
		..Default::default()
	}
	.insert(&app)
	.await;

	let response = app
		.execute_gql_with_token(
			CREATE_ANNOTATION,
			Some(json!({ "input": { "mediaId": book_id, "locator": locator() } })),
			&restricted.token,
		)
		.await;
	assert_rejected(&response);
	let response = app
		.execute_gql_with_token(
			CREATE_BOOKMARK,
			Some(json!({ "input": { "mediaId": book_id, "locator": locator() } })),
			&restricted.token,
		)
		.await;
	assert_rejected(&response);

	let response = app
		.execute_gql(
			CREATE_ANNOTATION,
			Some(json!({ "input": { "mediaId": "no-such-book", "locator": locator() } })),
		)
		.await;
	assert_rejected(&response);
}
