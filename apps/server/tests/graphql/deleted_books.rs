//! Deleted books: who removed a book and when, the retention, restoring, deleting
//! permanently, and who may do what

use crate::common::{
	account::CreateTestUser, series::setup_single_series_with_n_books, TestApp,
};
use models::{
	entity::{media, media_purge, reading_session},
	shared::enums::UserPermission,
};
use sea_orm::{prelude::*, ActiveValue::Set, IntoActiveModel};
use serde_json::{json, Value};
use tests::fake_data;

const REMOVE_BOOK: &str = r#"mutation RemoveBook($id: ID!) {
	deleteMedia(id: $id) { id }
}"#;
const DELETED_BOOKS: &str = r#"query DeletedBooks($libraryId: ID) {
	deletedBooks(libraryId: $libraryId) {
		book { id resolvedName }
		deletedAt deletedBy expiresAt purgeAt libraryName relativePath fileExists
	}
}"#;
const SETTINGS: &str = r#"query Settings {
	deletedBooksSettings {
		retentionDays trashDir heldFiles heldBytes nextPurgeAt lastPurgeAt
	}
}"#;
const RESTORE: &str = r#"mutation Restore($ids: [ID!]!) {
	restoreDeletedBooks(ids: $ids)
}"#;
const PURGE: &str = r#"mutation Purge($ids: [ID!]!) {
	purgeDeletedBooks(ids: $ids) { purged missingFiles freedBytes kept { mediaId error } }
}"#;
const EMPTY: &str = r#"mutation Empty($libraryId: ID) {
	emptyDeletedBooks(libraryId: $libraryId) { purged missingFiles kept { mediaId error } }
}"#;
const RETENTION: &str = r#"mutation Retention($days: Int!) {
	updateDeletedBooksRetention(days: $days) { retentionDays nextPurgeAt }
}"#;
const PURGED_BOOKS: &str = r#"query Purged {
	purgedBooks { mediaId title originalPath bytes automatic purgedBy }
}"#;
const HELD_FILES: &str = r#"query Held {
	heldFiles { id reason title relativePath bytes canPutBack expiresAt }
}"#;
const BOOK: &str = r#"query Book($id: ID!) {
	mediaById(id: $id) { id }
}"#;

async fn setup_books(app: &TestApp, n: i32) -> (String, Vec<media::Model>) {
	let library = fake_data::Library::default().insert(app.conn()).await;
	let (_, books) = setup_single_series_with_n_books(
		app,
		fake_data::Series {
			library_id: Some(library.id.clone()),
			..Default::default()
		},
		n,
	)
	.await;
	(library.id, books)
}

async fn remover(app: &TestApp) -> crate::common::account::TestUser {
	CreateTestUser {
		username: "remover".to_string(),
		permissions: vec![UserPermission::RemoveBook],
		..Default::default()
	}
	.insert(app)
	.await
}

fn assert_no_errors(response: &Value) {
	assert!(
		response.get("errors").is_none(),
		"should not have had errors: {response:#}"
	);
}

fn assert_forbidden(response: &Value) {
	assert!(
		response.get("errors").is_some(),
		"should have been rejected: {response:#}"
	);
}

async fn deleted_books(app: &TestApp) -> Vec<Value> {
	let response = app.execute_gql(DELETED_BOOKS, None).await;
	assert_no_errors(&response);
	response["data"]["deletedBooks"]
		.as_array()
		.expect("deletedBooks should be a list")
		.clone()
}

/// a removed book is in Deleted books with who removed it, when, and when it goes for good
#[tokio::test]
async fn test_deleted_books_list_who_removed_them_and_when_they_expire() {
	let app = TestApp::new_with_default_user().await;
	let (_, books) = setup_books(&app, 2).await;
	let remover = remover(&app).await;

	let response = app
		.execute_gql_with_token(
			REMOVE_BOOK,
			Some(json!({ "id": books[0].id })),
			&remover.token,
		)
		.await;
	assert_no_errors(&response);

	let listed = deleted_books(&app).await;
	assert_eq!(listed.len(), 1);
	let entry = &listed[0];
	assert_eq!(entry["book"]["id"], json!(books[0].id));
	assert_eq!(entry["deletedBy"], json!("remover"));
	assert_eq!(entry["fileExists"], json!(false));
	let deleted_at: chrono::DateTime<chrono::Utc> =
		entry["deletedAt"].as_str().unwrap().parse().unwrap();
	let expires_at: chrono::DateTime<chrono::Utc> =
		entry["expiresAt"].as_str().unwrap().parse().unwrap();
	assert_eq!(expires_at - deleted_at, chrono::Duration::days(14));
	assert!(entry["purgeAt"].is_string());

	// the other book is untouched and still visible
	let response = app
		.execute_gql(BOOK, Some(json!({ "id": books[1].id })))
		.await;
	assert_eq!(response["data"]["mediaById"]["id"], json!(books[1].id));
}

/// removing books is REMOVE_BOOK; the trash itself is for library managers; the retention is a
/// server setting
#[tokio::test]
async fn test_deleted_books_permissions() {
	let app = TestApp::new_with_default_user().await;
	let (_, books) = setup_books(&app, 1).await;
	let remover = remover(&app).await;
	let manager = CreateTestUser {
		username: "manager".to_string(),
		permissions: vec![UserPermission::ManageLibrary],
		..Default::default()
	}
	.insert(&app)
	.await;
	let reader = CreateTestUser {
		username: "reader".to_string(),
		..Default::default()
	}
	.insert(&app)
	.await;
	let id = json!({ "id": books[0].id });
	let ids = json!({ "ids": [books[0].id] });

	// a reader can't remove, a manager without REMOVE_BOOK can't either
	for token in [&reader.token, &manager.token] {
		let response = app
			.execute_gql_with_token(REMOVE_BOOK, Some(id.clone()), token)
			.await;
		assert_forbidden(&response);
	}
	assert_no_errors(
		&app.execute_gql_with_token(REMOVE_BOOK, Some(id.clone()), &remover.token)
			.await,
	);

	// the remover sees the retention (for the confirmation), not the trash folder
	let response = app
		.execute_gql_with_token(SETTINGS, None, &remover.token)
		.await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["deletedBooksSettings"]["retentionDays"],
		json!(14)
	);
	assert!(response["data"]["deletedBooksSettings"]["trashDir"].is_null());
	assert_forbidden(
		&app.execute_gql_with_token(SETTINGS, None, &reader.token)
			.await,
	);

	// the trash, restoring and purging are for library managers
	for (query, variables) in [
		(DELETED_BOOKS, None),
		(RESTORE, Some(ids.clone())),
		(PURGE, Some(ids.clone())),
		(EMPTY, None),
		(PURGED_BOOKS, None),
		(HELD_FILES, None),
	] {
		let response = app
			.execute_gql_with_token(query, variables, &remover.token)
			.await;
		assert_forbidden(&response);
	}
	let response = app
		.execute_gql_with_token(DELETED_BOOKS, None, &manager.token)
		.await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["deletedBooks"].as_array().unwrap().len(),
		1
	);

	// the retention needs Manage server
	let response = app
		.execute_gql_with_token(RETENTION, Some(json!({ "days": 3 })), &manager.token)
		.await;
	assert_forbidden(&response);

	let response = app
		.execute_gql_with_token(RESTORE, Some(ids), &manager.token)
		.await;
	assert_no_errors(&response);
	assert_eq!(response["data"]["restoreDeletedBooks"], json!(1));
}

/// restoring brings the book back exactly as it was: its reading progress never left
#[tokio::test]
async fn test_restore_brings_books_back_with_their_progress() {
	let app = TestApp::new_with_default_user().await;
	let (_, books) = setup_books(&app, 2).await;
	let reader = fake_data::User::new("progress-reader")
		.insert(app.conn())
		.await;
	fake_data::ReadingSession {
		media_id: books[0].id.clone(),
		user_id: reader.id.clone(),
		end_percentage: 0.42,
		..Default::default()
	}
	.insert(app.conn())
	.await;

	for book in &books {
		assert_no_errors(
			&app.execute_gql(REMOVE_BOOK, Some(json!({ "id": book.id })))
				.await,
		);
	}
	assert_eq!(deleted_books(&app).await.len(), 2);

	let response = app
		.execute_gql(RESTORE, Some(json!({ "ids": [books[0].id, books[1].id] })))
		.await;
	assert_no_errors(&response);
	assert_eq!(response["data"]["restoreDeletedBooks"], json!(2));
	assert!(deleted_books(&app).await.is_empty());

	let restored = media::Entity::find_by_id(books[0].id.clone())
		.one(app.conn())
		.await
		.unwrap()
		.unwrap();
	assert_eq!((restored.deleted_at, restored.deleted_by), (None, None));
	let sessions = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(&books[0].id))
		.all(app.conn())
		.await
		.unwrap();
	assert_eq!(sessions.len(), 1);
	let response = app
		.execute_gql(BOOK, Some(json!({ "id": books[0].id })))
		.await;
	assert_eq!(response["data"]["mediaById"]["id"], json!(books[0].id));
}

/// deleting permanently deletes the book's file and its data; a book whose file is already gone
/// only loses its data; both are logged
#[tokio::test]
async fn test_purge_deletes_files_for_real() {
	let app = TestApp::new_with_default_user().await;
	let (_, books) = setup_books(&app, 2).await;
	let dir = tempfile::tempdir().unwrap();
	let file = dir.path().join("On disk.epub");
	std::fs::write(&file, b"a book").unwrap();
	let mut on_disk = books[0].clone().into_active_model();
	on_disk.path = Set(file.to_string_lossy().into_owned());
	on_disk.update(app.conn()).await.unwrap();
	for book in &books {
		assert_no_errors(
			&app.execute_gql(REMOVE_BOOK, Some(json!({ "id": book.id })))
				.await,
		);
	}

	let response = app.execute_gql(SETTINGS, None).await;
	assert_no_errors(&response);
	assert!(response["data"]["deletedBooksSettings"]["trashDir"]
		.as_str()
		.unwrap()
		.ends_with("/trash"));
	assert_eq!(
		response["data"]["deletedBooksSettings"]["heldFiles"],
		json!(0)
	);

	let response = app
		.execute_gql(PURGE, Some(json!({ "ids": [books[0].id, books[1].id] })))
		.await;
	assert_no_errors(&response);
	let result = &response["data"]["purgeDeletedBooks"];
	assert_eq!(result["purged"], json!(1));
	assert_eq!(result["missingFiles"], json!(1));
	assert_eq!(result["freedBytes"], json!(6));
	assert!(result["kept"].as_array().unwrap().is_empty());

	// the file is gone for good, nothing hidden left behind, and Deleted books is empty
	assert!(!file.exists());
	assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
	assert!(deleted_books(&app).await.is_empty());
	for book in &books {
		assert!(media::Entity::find_by_id(book.id.clone())
			.one(app.conn())
			.await
			.unwrap()
			.is_none());
	}
	let response = app.execute_gql(PURGED_BOOKS, None).await;
	assert_no_errors(&response);
	let purged = response["data"]["purgedBooks"].as_array().unwrap();
	assert_eq!(purged.len(), 2);
	let with_file = purged
		.iter()
		.find(|p| p["mediaId"] == json!(books[0].id))
		.unwrap();
	assert_eq!(with_file["bytes"], json!(6));
	assert_eq!(with_file["automatic"], json!(false));
	assert_eq!(with_file["purgedBy"], json!("initial-server-admin"));
	let without = purged
		.iter()
		.find(|p| p["mediaId"] == json!(books[1].id))
		.unwrap();
	assert!(without["bytes"].is_null());
	assert_eq!(
		media_purge::Entity::find()
			.filter(media_purge::Column::QuarantinePath.is_not_null())
			.count(app.conn())
			.await
			.unwrap(),
		0
	);
	let response = app.execute_gql(HELD_FILES, None).await;
	assert_no_errors(&response);
	assert!(response["data"]["heldFiles"].as_array().unwrap().is_empty());
}

/// emptying Deleted books takes only what is in it, in one library when asked
#[tokio::test]
async fn test_empty_deleted_books() {
	let app = TestApp::new_with_default_user().await;
	let (library_a, books_a) = setup_books(&app, 2).await;
	let (_, books_b) = setup_books(&app, 1).await;
	for book in books_a.iter().chain(&books_b) {
		assert_no_errors(
			&app.execute_gql(REMOVE_BOOK, Some(json!({ "id": book.id })))
				.await,
		);
	}
	let live = setup_books(&app, 1).await.1;

	let response = app
		.execute_gql(EMPTY, Some(json!({ "libraryId": library_a })))
		.await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["emptyDeletedBooks"]["missingFiles"],
		json!(2)
	);
	let listed = deleted_books(&app).await;
	assert_eq!(listed.len(), 1, "the other library's book stays");

	let response = app.execute_gql(EMPTY, None).await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["emptyDeletedBooks"]["missingFiles"],
		json!(1)
	);
	assert!(deleted_books(&app).await.is_empty());
	assert_eq!(
		media_purge::Entity::find().count(app.conn()).await.unwrap(),
		3
	);

	// books that were never removed can't be purged
	let response = app
		.execute_gql(PURGE, Some(json!({ "ids": [live[0].id] })))
		.await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["purgeDeletedBooks"]["kept"][0]["error"],
		json!("It isn't in Deleted books")
	);
	assert!(media::Entity::find_by_id(live[0].id.clone())
		.one(app.conn())
		.await
		.unwrap()
		.is_some());
}

/// the retention: 14 days by default, 0 keeps books until deleted by hand
#[tokio::test]
async fn test_retention_setting() {
	let app = TestApp::new_with_default_user().await;
	let (_, books) = setup_books(&app, 1).await;
	assert_no_errors(
		&app.execute_gql(REMOVE_BOOK, Some(json!({ "id": books[0].id })))
			.await,
	);

	let response = app.execute_gql(RETENTION, Some(json!({ "days": 0 }))).await;
	assert_no_errors(&response);
	assert_eq!(
		response["data"]["updateDeletedBooksRetention"],
		json!({ "retentionDays": 0, "nextPurgeAt": null })
	);
	let listed = deleted_books(&app).await;
	assert!(listed[0]["expiresAt"].is_null());
	assert!(listed[0]["purgeAt"].is_null());

	let response = app.execute_gql(RETENTION, Some(json!({ "days": 1 }))).await;
	assert_no_errors(&response);
	assert!(response["data"]["updateDeletedBooksRetention"]["nextPurgeAt"].is_string());

	for days in [-1, 100_000] {
		let response = app
			.execute_gql(RETENTION, Some(json!({ "days": days })))
			.await;
		assert_forbidden(&response);
	}
	let response = app.execute_gql(SETTINGS, None).await;
	assert_eq!(
		response["data"]["deletedBooksSettings"]["retentionDays"],
		json!(1)
	);
}
