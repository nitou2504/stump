use crate::common::{
	account::CreateTestUser, series::setup_single_series_with_n_books, TestApp,
};
use models::shared::enums::UserPermission;
use serde_json::{json, Value};
use tests::fake_data;

const REMOVE_BOOK: &str = r#"mutation RemoveBook($id: ID!) {
	deleteMedia(id: $id) { id }
}"#;
const RESTORE_BOOK: &str = r#"mutation RestoreBook($id: ID!) {
	restoreMedia(id: $id)
}"#;
const BOOK: &str = r#"query Book($id: ID!) {
	mediaById(id: $id) { id }
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

async fn is_visible(app: &TestApp, book_id: &str) -> bool {
	let response = app.execute_gql(BOOK, Some(json!({ "id": book_id }))).await;
	!response["data"]["mediaById"].is_null()
}

fn assert_forbidden(response: &Value) {
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

/// managing libraries is not enough: an admin grants `REMOVE_BOOK` to each user who may
/// remove books
#[tokio::test]
async fn test_removing_a_book_requires_the_remove_book_permission() {
	let app = TestApp::new_with_default_user().await;
	let book_id = setup_book(&app).await;
	let manager = CreateTestUser {
		username: "library-manager".to_string(),
		permissions: vec![UserPermission::ManageLibrary, UserPermission::DeleteLibrary],
		..Default::default()
	}
	.insert(&app)
	.await;

	let response = app
		.execute_gql_with_token(
			REMOVE_BOOK,
			Some(json!({ "id": book_id })),
			&manager.token,
		)
		.await;

	assert_forbidden(&response);
	assert!(is_visible(&app, &book_id).await);
}

/// a user granted `REMOVE_BOOK` hides the book for everyone; restoring it stays with
/// library managers
#[tokio::test]
async fn test_remove_book_permission_removes_and_managers_restore() {
	let app = TestApp::new_with_default_user().await;
	let book_id = setup_book(&app).await;
	let remover = CreateTestUser {
		username: "remover".to_string(),
		permissions: vec![UserPermission::RemoveBook],
		..Default::default()
	}
	.insert(&app)
	.await;

	let response = app
		.execute_gql_with_token(
			REMOVE_BOOK,
			Some(json!({ "id": book_id })),
			&remover.token,
		)
		.await;
	assert_no_errors(&response);
	assert!(!is_visible(&app, &book_id).await);

	let response = app
		.execute_gql_with_token(
			RESTORE_BOOK,
			Some(json!({ "id": book_id })),
			&remover.token,
		)
		.await;
	assert_forbidden(&response);
	assert!(!is_visible(&app, &book_id).await);

	// the server owner (default user) can manage libraries
	let response = app
		.execute_gql(RESTORE_BOOK, Some(json!({ "id": book_id })))
		.await;
	assert_no_errors(&response);
	assert!(is_visible(&app, &book_id).await);
}
