//! A permission is meant to allow what it says, and no more. These hold what the permission
//! guards are for: an action that takes several permissions at once needs all of them, a
//! user manager hands out only what they hold themselves, a list or an emoji can be changed by
//! who made it (and the server owner), the file explorer stays inside the libraries for those
//! who aren't allowed to point a library anywhere, and what a metadata search found for a book
//! is for those who can see the book.

use crate::common::{
	account::{CreateTestUser, TestUser},
	TestApp,
};
use models::{
	entity::{
		custom_emoji, library, library_exclusion, media, metadata_fetch_record, series,
		smart_list, smart_list_view, user,
	},
	shared::enums::{MetadataFetchStatus, UserPermission},
};
use sea_orm::{prelude::*, ActiveModelTrait, QueryFilter, Set};
use serde_json::{json, Value};
use tests::fake_data;

const FORBIDDEN: &str = "You do not have permission to perform this action";

fn is_forbidden(response: &Value) -> bool {
	response["errors"].to_string().contains(FORBIDDEN)
}

fn has_errors(response: &Value) -> bool {
	response.get("errors").is_some()
}

fn errors(response: &Value) -> String {
	response["errors"].to_string()
}

async fn user_with(
	app: &TestApp,
	username: &str,
	permissions: &[UserPermission],
) -> TestUser {
	CreateTestUser {
		username: username.to_string(),
		permissions: permissions.to_vec(),
		..Default::default()
	}
	.insert(app)
	.await
}

async fn run(app: &TestApp, user: &TestUser, query: &str, variables: Value) -> Value {
	app.execute_gql_with_token(query, Some(variables), &user.token)
		.await
}

async fn add_library(app: &TestApp, name: &str, path: Option<String>) -> library::Model {
	fake_data::Library {
		name: Some(name.to_string()),
		path,
		..Default::default()
	}
	.insert(app.conn())
	.await
}

async fn add_series(
	app: &TestApp,
	library: &library::Model,
	name: &str,
) -> series::Model {
	fake_data::Series {
		name: Some(name.to_string()),
		library_id: Some(library.id.clone()),
		..Default::default()
	}
	.insert(app.conn())
	.await
}

async fn add_book(
	app: &TestApp,
	series: &series::Model,
	name: &str,
	path: Option<String>,
) -> media::Model {
	fake_data::Media {
		series_id: series.id.clone(),
		name: Some(name.to_string()),
		extension: Some("epub".to_string()),
		pages: Some(10),
		path,
		..Default::default()
	}
	.insert(app.conn())
	.await
}

async fn hide_library(app: &TestApp, user: &TestUser, library: &library::Model) {
	library_exclusion::ActiveModel {
		user_id: Set(user.id.clone()),
		library_id: Set(library.id.clone()),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should hide the library");
}

// ---------------------------------------------------------------------------------------
// Several permissions at once; the narrow guards
// ---------------------------------------------------------------------------------------

const EXCLUDE_USERS: &str = r#"
mutation Exclude($id: ID!, $users: [String!]!) {
    updateLibraryExcludedUsers(id: $id, userIds: $users) { id }
}
"#;

async fn is_excluded(app: &TestApp, library: &library::Model, user: &TestUser) -> bool {
	library_exclusion::Entity::find()
		.filter(library_exclusion::Column::LibraryId.eq(library.id.clone()))
		.filter(library_exclusion::Column::UserId.eq(user.id.clone()))
		.one(app.conn())
		.await
		.expect("should query the exclusions")
		.is_some()
}

/// who sees a library is decided by managing the library AND the users: reading the users, or
/// managing the library, isn't enough
#[tokio::test]
async fn test_library_exclusions_take_managing_both_the_library_and_the_users() {
	let app = TestApp::new_with_default_user().await;
	let library = add_library(&app, "Shelf", None).await;
	let target = user_with(&app, "target", &[]).await;
	let reads_users = user_with(&app, "reads-users", &[UserPermission::ReadUsers]).await;
	let manages_users =
		user_with(&app, "manages-users", &[UserPermission::ManageUsers]).await;
	let manages_library =
		user_with(&app, "manages-library", &[UserPermission::ManageLibrary]).await;
	let manages_both = user_with(
		&app,
		"manages-both",
		&[UserPermission::ManageLibrary, UserPermission::ManageUsers],
	)
	.await;

	let variables = json!({ "id": library.id, "users": [target.id] });

	for denied in [&reads_users, &manages_users, &manages_library] {
		let response = run(&app, denied, EXCLUDE_USERS, variables.clone()).await;
		assert!(
			is_forbidden(&response),
			"{} must not change who sees a library: {response:#}",
			denied.username
		);
	}
	assert!(!is_excluded(&app, &library, &target).await);

	let response = run(&app, &manages_both, EXCLUDE_USERS, variables.clone()).await;
	assert!(!has_errors(&response), "{response:#}");
	assert!(is_excluded(&app, &library, &target).await);

	// and the server owner, who holds everything
	let response = app
		.execute_gql(
			EXCLUDE_USERS,
			Some(json!({ "id": library.id, "users": [] })),
		)
		.await;
	assert!(!has_errors(&response), "{response:#}");
	assert!(!is_excluded(&app, &library, &target).await);
}

const CLEAR_SCAN_HISTORY: &str = r#"
mutation Clear($id: ID!) { clearScanHistory(id: $id) }
"#;

/// wiping a library's scan records is the library manager's, not whoever may read jobs or logs
#[tokio::test]
async fn test_clearing_the_scan_history_takes_managing_the_library() {
	let app = TestApp::new_with_default_user().await;
	let library = add_library(&app, "Shelf", None).await;
	let reads_jobs = user_with(&app, "reads-jobs", &[UserPermission::ReadJobs]).await;
	let reads_logs =
		user_with(&app, "reads-logs", &[UserPermission::ReadPersistedLogs]).await;
	let manager = user_with(&app, "manager", &[UserPermission::ManageLibrary]).await;

	for denied in [&reads_jobs, &reads_logs] {
		let response = run(
			&app,
			denied,
			CLEAR_SCAN_HISTORY,
			json!({ "id": library.id }),
		)
		.await;
		assert!(is_forbidden(&response), "{response:#}");
	}

	let response = run(
		&app,
		&manager,
		CLEAR_SCAN_HISTORY,
		json!({ "id": library.id }),
	)
	.await;
	assert!(!has_errors(&response), "{response:#}");
}

const MEDIA_COVER: &str = r#"
mutation Cover($id: ID!, $image: String!) {
    uploadMediaThumbnailBase64(id: $id, image: $image) { id }
}
"#;

const SERIES_COVER: &str = r#"
mutation Cover($id: ID!, $image: String!) {
    uploadSeriesThumbnailBase64(id: $id, image: $image) { id }
}
"#;

/// being allowed to upload files isn't being allowed to change covers: that is editing thumbnails
#[tokio::test]
async fn test_changing_a_cover_takes_editing_thumbnails_not_just_uploading() {
	let app = TestApp::new_with_default_user().await;
	let library = add_library(&app, "Shelf", None).await;
	let series = add_series(&app, &library, "Alpha").await;
	let book = add_book(&app, &series, "Alpha 1", None).await;

	let uploader = user_with(&app, "uploader", &[UserPermission::UploadFile]).await;
	let editor = user_with(&app, "editor", &[UserPermission::EditThumbnails]).await;

	// not an image: the point is only whether the guard lets the request through
	let book_vars = json!({ "id": book.id, "image": "not an image" });
	let series_vars = json!({ "id": series.id, "image": "not an image" });

	let response = run(&app, &uploader, MEDIA_COVER, book_vars.clone()).await;
	assert!(is_forbidden(&response), "{response:#}");
	let response = run(&app, &uploader, SERIES_COVER, series_vars.clone()).await;
	assert!(is_forbidden(&response), "{response:#}");

	let response = run(&app, &editor, MEDIA_COVER, book_vars).await;
	assert!(has_errors(&response), "the image isn't one: {response:#}");
	assert!(!is_forbidden(&response), "{response:#}");
	let response = run(&app, &editor, SERIES_COVER, series_vars).await;
	assert!(has_errors(&response), "the image isn't one: {response:#}");
	assert!(!is_forbidden(&response), "{response:#}");
}

// ---------------------------------------------------------------------------------------
// Handing out permissions
// ---------------------------------------------------------------------------------------

const CREATE_USER: &str = r#"
mutation CreateUser($input: CreateUserInput!) {
    createUser(input: $input) { id permissions }
}
"#;

const UPDATE_USER: &str = r#"
mutation UpdateUser($id: ID!, $input: UpdateUserInput!) {
    updateUser(id: $id, input: $input) { id permissions }
}
"#;

async fn stored_user(app: &TestApp, username: &str) -> Option<user::Model> {
	user::Entity::find()
		.filter(user::Column::Username.eq(username))
		.one(app.conn())
		.await
		.expect("should query the users")
}

/// someone who manages users creates accounts with the permissions they hold, no others
#[tokio::test]
async fn test_a_user_manager_creates_accounts_with_the_permissions_they_hold_only() {
	let app = TestApp::new_with_default_user().await;
	let manager = user_with(
		&app,
		"manager",
		&[UserPermission::ManageUsers, UserPermission::DownloadFile],
	)
	.await;

	for (name, permissions) in [
		("wants-library", json!(["MANAGE_LIBRARY"])),
		("wants-server", json!(["DOWNLOAD_FILE", "MANAGE_SERVER"])),
		("wants-keys", json!(["MANAGE_USERS", "ACCESS_API_KEYS"])),
	] {
		let response = run(
			&app,
			&manager,
			CREATE_USER,
			json!({ "input": {
				"username": name,
				"password": "password",
				"permissions": permissions,
			}}),
		)
		.await;
		assert!(
			has_errors(&response),
			"{name} must be refused: {response:#}"
		);
		assert!(
			errors(&response).contains("only grant the permissions you hold"),
			"{response:#}"
		);
		assert!(
			stored_user(&app, name).await.is_none(),
			"{name} must not have been created"
		);
	}

	// what they hold themselves (ManageUsers brings ReadUsers along), or nothing
	for (name, permissions) in [
		("gets-download", json!(["DOWNLOAD_FILE"])),
		("gets-users", json!(["MANAGE_USERS"])),
		("gets-nothing", json!([])),
	] {
		let response = run(
			&app,
			&manager,
			CREATE_USER,
			json!({ "input": {
				"username": name,
				"password": "password",
				"permissions": permissions,
			}}),
		)
		.await;
		assert!(!has_errors(&response), "{name}: {response:#}");
		assert!(stored_user(&app, name).await.is_some());
	}

	// the server owner holds them all
	let owner_made = app
		.execute_gql(
			CREATE_USER,
			Some(json!({ "input": {
				"username": "owner-made",
				"password": "password",
				"permissions": ["MANAGE_SERVER", "MANAGE_LIBRARY", "MANAGE_USERS"],
			}})),
		)
		.await;
	assert!(!has_errors(&owner_made), "{owner_made:#}");
}

/// the same for an existing account, and a user manager can't take over an account that holds
/// what they don't (resetting its password would hand them those permissions)
#[tokio::test]
async fn test_a_user_manager_changes_only_what_they_hold() {
	let app = TestApp::new_with_default_user().await;
	let manager = user_with(
		&app,
		"manager",
		&[UserPermission::ManageUsers, UserPermission::DownloadFile],
	)
	.await;
	let friend = user_with(&app, "friend", &[UserPermission::DownloadFile]).await;
	let admin = user_with(&app, "admin", &[UserPermission::ManageServer]).await;

	// more than they hold
	let response = run(
		&app,
		&manager,
		UPDATE_USER,
		json!({ "id": friend.id, "input": {
			"username": "friend",
			"permissions": ["DOWNLOAD_FILE", "MANAGE_LIBRARY"],
		}}),
	)
	.await;
	assert!(has_errors(&response), "{response:#}");
	let stored = stored_user(&app, "friend").await.expect("the friend");
	assert!(
		!stored
			.permissions
			.unwrap_or_default()
			.contains("MANAGE_LIBRARY"),
		"the friend must not have got a permission the manager lacks"
	);

	// what they hold: fine, and taking permissions away too
	let response = run(
		&app,
		&manager,
		UPDATE_USER,
		json!({ "id": friend.id, "input": {
			"username": "friend",
			"permissions": [],
		}}),
	)
	.await;
	assert!(!has_errors(&response), "{response:#}");

	// an account that holds what they don't: not theirs to change, nor to reset the password of
	let before = stored_user(&app, "admin").await.expect("the admin");
	let response = run(
		&app,
		&manager,
		UPDATE_USER,
		json!({ "id": admin.id, "input": {
			"username": "admin",
			"password": "taken-over",
			"permissions": ["MANAGE_SERVER"],
		}}),
	)
	.await;
	assert!(is_forbidden(&response), "{response:#}");
	let after = stored_user(&app, "admin").await.expect("the admin");
	assert_eq!(
		before.hashed_password, after.hashed_password,
		"the password must be unchanged"
	);

	// the server owner can
	let response = app
		.execute_gql(
			UPDATE_USER,
			Some(json!({ "id": admin.id, "input": {
				"username": "admin",
				"permissions": ["MANAGE_SERVER"],
			}})),
		)
		.await;
	assert!(!has_errors(&response), "{response:#}");
}

// ---------------------------------------------------------------------------------------
// Smart lists
// ---------------------------------------------------------------------------------------

const CREATE_LIST: &str = r#"
mutation CreateList($input: SaveSmartListInput!) {
    createSmartList(input: $input) { id name }
}
"#;

const UPDATE_LIST: &str = r#"
mutation UpdateList($id: ID!, $input: SaveSmartListInput!) {
    updateSmartList(id: $id, input: $input) { id name }
}
"#;

const DELETE_LIST: &str = r#"
mutation DeleteList($id: ID!) { deleteSmartList(id: $id) { id } }
"#;

const CREATE_VIEW: &str = r#"
mutation CreateView($input: SaveSmartListView!) {
    createSmartListView(input: $input) { id name }
}
"#;

const UPDATE_VIEW: &str = r#"
mutation UpdateView($name: String!, $input: SaveSmartListView!) {
    updateSmartListView(originalName: $name, input: $input) { id name }
}
"#;

const DELETE_VIEW: &str = r#"
mutation DeleteView($id: ID!, $name: String!) {
    deleteSmartListView(id: $id, name: $name) { id }
}
"#;

fn list_input(name: &str, visibility: &str) -> Value {
	json!({
		"filters": [],
		"name": name,
		"joiner": "AND",
		"defaultGrouping": "BY_BOOKS",
		"visibility": visibility,
	})
}

fn view_input(list_id: &str, name: &str) -> Value {
	json!({
		"name": name,
		"listId": list_id,
		"bookColumns": [],
		"groupColumns": [],
		"bookSorting": [],
		"groupSorting": [],
	})
}

async fn stored_list(app: &TestApp, id: &str) -> Option<smart_list::Model> {
	smart_list::Entity::find()
		.filter(smart_list::Column::Id.eq(id))
		.one(app.conn())
		.await
		.expect("should query the smart lists")
}

async fn make_list(
	app: &TestApp,
	user: &TestUser,
	name: &str,
	visibility: &str,
) -> String {
	let response = run(
		app,
		user,
		CREATE_LIST,
		json!({ "input": list_input(name, visibility) }),
	)
	.await;
	assert!(!has_errors(&response), "{response:#}");
	response["data"]["createSmartList"]["id"]
		.as_str()
		.expect("an id")
		.to_string()
}

/// everyone sees a PUBLIC list, but only who made it (and the server owner) changes or deletes it,
/// and it stays its maker's whoever edits it
#[tokio::test]
async fn test_a_public_smart_list_is_changed_only_by_its_maker_or_the_owner() {
	let app = TestApp::new_with_default_user().await;
	let maker = user_with(&app, "maker", &[UserPermission::AccessSmartList]).await;
	let other = user_with(&app, "other", &[UserPermission::AccessSmartList]).await;

	let id = make_list(&app, &maker, "Shelf", "PUBLIC").await;

	// someone else can't take it over, rename it or delete it
	let response = run(
		&app,
		&other,
		UPDATE_LIST,
		json!({ "id": id, "input": list_input("Mine now", "PUBLIC") }),
	)
	.await;
	assert!(has_errors(&response), "{response:#}");
	let response = run(&app, &other, DELETE_LIST, json!({ "id": id })).await;
	assert!(has_errors(&response), "{response:#}");

	let stored = stored_list(&app, &id).await.expect("still there");
	assert_eq!(stored.name, "Shelf");
	assert_eq!(stored.creator_id, maker.id);

	// its maker can
	let response = run(
		&app,
		&maker,
		UPDATE_LIST,
		json!({ "id": id, "input": list_input("Renamed", "PUBLIC") }),
	)
	.await;
	assert!(!has_errors(&response), "{response:#}");
	let stored = stored_list(&app, &id).await.expect("still there");
	assert_eq!(stored.name, "Renamed");
	assert_eq!(stored.creator_id, maker.id);

	// so can the server owner, and the list stays the maker's
	let response = app
		.execute_gql(
			UPDATE_LIST,
			Some(json!({ "id": id, "input": list_input("Moderated", "PUBLIC") })),
		)
		.await;
	assert!(!has_errors(&response), "{response:#}");
	let stored = stored_list(&app, &id).await.expect("still there");
	assert_eq!(stored.name, "Moderated");
	assert_eq!(stored.creator_id, maker.id);

	let response = run(&app, &maker, DELETE_LIST, json!({ "id": id })).await;
	assert!(!has_errors(&response), "{response:#}");
	assert!(stored_list(&app, &id).await.is_none());
}

/// the server owner can't see another user's PRIVATE list, so can't change or delete it either
#[tokio::test]
async fn test_the_owner_cannot_change_a_private_smart_list_of_someone_else() {
	let app = TestApp::new_with_default_user().await;
	let maker = user_with(&app, "maker", &[UserPermission::AccessSmartList]).await;
	let id = make_list(&app, &maker, "Private shelf", "PRIVATE").await;

	let response = app
		.execute_gql(
			UPDATE_LIST,
			Some(json!({ "id": id, "input": list_input("Taken", "PUBLIC") })),
		)
		.await;
	assert!(
		errors(&response).contains("Smart list not found"),
		"{response:#}"
	);
	let response = app
		.execute_gql(DELETE_LIST, Some(json!({ "id": id })))
		.await;
	assert!(
		errors(&response).contains("Smart list not found"),
		"{response:#}"
	);

	let stored = stored_list(&app, &id).await.expect("still there");
	assert_eq!(stored.name, "Private shelf");
	assert_eq!(stored.creator_id, maker.id);
}

/// a list's saved views are part of the list: they're changed by who may change the list
#[tokio::test]
async fn test_the_views_of_a_smart_list_are_changed_only_by_who_may_change_the_list() {
	let app = TestApp::new_with_default_user().await;
	let maker = user_with(&app, "maker", &[UserPermission::AccessSmartList]).await;
	let other = user_with(&app, "other", &[UserPermission::AccessSmartList]).await;
	let id = make_list(&app, &maker, "Shelf", "PUBLIC").await;

	let response = run(
		&app,
		&maker,
		CREATE_VIEW,
		json!({ "input": view_input(&id, "Mine") }),
	)
	.await;
	assert!(!has_errors(&response), "{response:#}");

	// someone who can only see the list can't add, change or delete a view
	let response = run(
		&app,
		&other,
		CREATE_VIEW,
		json!({ "input": view_input(&id, "Theirs") }),
	)
	.await;
	assert!(has_errors(&response), "{response:#}");
	let response = run(
		&app,
		&other,
		UPDATE_VIEW,
		json!({ "name": "Mine", "input": view_input(&id, "Mine") }),
	)
	.await;
	assert!(has_errors(&response), "{response:#}");
	let response = run(
		&app,
		&other,
		DELETE_VIEW,
		json!({ "id": id, "name": "Mine" }),
	)
	.await;
	assert!(has_errors(&response), "{response:#}");

	let views = smart_list_view::Entity::find_by_list_id(&id)
		.all(app.conn())
		.await
		.expect("should query the views");
	assert_eq!(views.len(), 1, "only the maker's view: {views:?}");
	assert_eq!(views[0].name, "Mine");

	// its maker can
	let response = run(
		&app,
		&maker,
		DELETE_VIEW,
		json!({ "id": id, "name": "Mine" }),
	)
	.await;
	assert!(!has_errors(&response), "{response:#}");
}

// ---------------------------------------------------------------------------------------
// Custom emoji
// ---------------------------------------------------------------------------------------

const UPDATE_EMOJI: &str = r#"
mutation UpdateEmoji($id: ID!, $name: String!) {
    updateCustomEmoji(id: $id, input: { name: $name }) { id name }
}
"#;

const DELETE_EMOJI: &str = r#"
mutation DeleteEmoji($id: ID!) { deleteCustomEmoji(id: $id) }
"#;

async fn add_emoji(app: &TestApp, maker: &TestUser, name: &str) -> i32 {
	custom_emoji::ActiveModel {
		name: Set(name.to_string()),
		is_animated: Set(false),
		file_extension: Set("png".to_string()),
		created_by_id: Set(maker.id.clone()),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should insert the emoji")
	.id
}

async fn emoji_name(app: &TestApp, id: i32) -> Option<String> {
	custom_emoji::Entity::find_by_id(id)
		.one(app.conn())
		.await
		.expect("should query the emoji")
		.map(|emoji| emoji.name)
}

/// an emoji is renamed or deleted by who uploaded it, or by the server owner
#[tokio::test]
async fn test_a_custom_emoji_is_changed_only_by_its_uploader_or_the_owner() {
	let app = TestApp::new_with_default_user().await;
	let maker = user_with(&app, "maker", &[UserPermission::UploadFile]).await;
	let other = user_with(&app, "other", &[UserPermission::UploadFile]).await;
	let id = add_emoji(&app, &maker, "wave").await;

	let response = run(
		&app,
		&other,
		UPDATE_EMOJI,
		json!({ "id": id.to_string(), "name": "mine" }),
	)
	.await;
	assert!(is_forbidden(&response), "{response:#}");
	let response = run(&app, &other, DELETE_EMOJI, json!({ "id": id.to_string() })).await;
	assert!(is_forbidden(&response), "{response:#}");
	assert_eq!(emoji_name(&app, id).await.as_deref(), Some("wave"));

	let response = run(
		&app,
		&maker,
		UPDATE_EMOJI,
		json!({ "id": id.to_string(), "name": "hello" }),
	)
	.await;
	assert!(!has_errors(&response), "{response:#}");
	assert_eq!(emoji_name(&app, id).await.as_deref(), Some("hello"));

	let response = app
		.execute_gql(
			UPDATE_EMOJI,
			Some(json!({ "id": id.to_string(), "name": "moderated" })),
		)
		.await;
	assert!(!has_errors(&response), "{response:#}");
	assert_eq!(emoji_name(&app, id).await.as_deref(), Some("moderated"));

	let response = run(&app, &maker, DELETE_EMOJI, json!({ "id": id.to_string() })).await;
	assert!(!has_errors(&response), "{response:#}");
	assert_eq!(emoji_name(&app, id).await, None);
}

// ---------------------------------------------------------------------------------------
// The file explorer
// ---------------------------------------------------------------------------------------

const LIST_DIRECTORY: &str = r#"
query ListDirectory($path: String) {
    listDirectory(
        pagination: { offset: { page: 1, pageSize: 50 } }
        input: { path: $path }
    ) {
        nodes { parent files { name path isDirectory media { id } } }
    }
}
"#;

/// A temporary folder with a library's folder (a book, a sub folder, a symbolic link that leads
/// out of it) next to a folder that belongs to no library
struct Folders {
	_dir: tempfile::TempDir,
	library: std::path::PathBuf,
	outside: std::path::PathBuf,
}

fn make_folders() -> Folders {
	let dir = tempfile::tempdir().expect("a temporary folder");
	let root = std::fs::canonicalize(dir.path()).expect("its real path");
	let library = root.join("library");
	let outside = root.join("outside");

	std::fs::create_dir_all(library.join("sub")).expect("the library folder");
	std::fs::create_dir_all(&outside).expect("the outside folder");
	std::fs::write(library.join("a.epub"), b"book").expect("a book");
	std::fs::write(outside.join("secret.txt"), b"secret").expect("a file");
	#[cfg(unix)]
	std::os::unix::fs::symlink(&outside, library.join("escape"))
		.expect("a link out of the library");

	Folders {
		_dir: dir,
		library,
		outside,
	}
}

fn listed_names(response: &Value) -> Vec<String> {
	response["data"]["listDirectory"]["nodes"][0]["files"]
		.as_array()
		.expect("a list of files")
		.iter()
		.map(|file| file["name"].as_str().expect("a name").to_string())
		.collect()
}

async fn list_as(
	app: &TestApp,
	user: &TestUser,
	path: Option<&std::path::Path>,
) -> Value {
	run(
		app,
		user,
		LIST_DIRECTORY,
		json!({ "path": path.map(|p| p.to_string_lossy().to_string()) }),
	)
	.await
}

/// the file explorer permission lets a user look at their libraries' folders, not at the whole
/// server (`/`, the config folder with the database, `/etc`)
#[tokio::test]
async fn test_the_file_explorer_stays_inside_the_libraries_for_most_users() {
	let app = TestApp::new_with_default_user().await;
	let folders = make_folders();
	add_library(
		&app,
		"Shelf",
		Some(folders.library.to_string_lossy().to_string()),
	)
	.await;

	let explorer = user_with(&app, "explorer", &[UserPermission::FileExplorer]).await;

	// the library's folder, and inside it
	let response = list_as(&app, &explorer, Some(&folders.library)).await;
	assert!(!has_errors(&response), "{response:#}");
	let names = listed_names(&response);
	assert!(names.contains(&"a.epub".to_string()), "{names:?}");
	assert!(
		response["data"]["listDirectory"]["nodes"][0]["parent"].is_null(),
		"there's no way up out of a library: {response:#}"
	);

	let response = list_as(&app, &explorer, Some(&folders.library.join("sub"))).await;
	assert!(!has_errors(&response), "{response:#}");
	assert_eq!(
		response["data"]["listDirectory"]["nodes"][0]["parent"],
		json!(folders.library.to_string_lossy())
	);

	// anywhere else gets the answer of a folder that isn't there, whether it exists or not
	let missing = folders.outside.join("nothing-here");
	let climbing = folders.library.join("..").join("outside");
	let through_link = folders.library.join("escape");
	let mut outside_attempts = vec![
		list_as(&app, &explorer, Some(&folders.outside)).await,
		list_as(&app, &explorer, Some(&missing)).await,
		list_as(&app, &explorer, Some(&climbing)).await,
		list_as(&app, &explorer, Some(std::path::Path::new("/"))).await,
		list_as(&app, &explorer, Some(std::path::Path::new("/etc"))).await,
		list_as(&app, &explorer, None).await,
	];
	#[cfg(unix)]
	outside_attempts.push(list_as(&app, &explorer, Some(&through_link)).await);

	for response in &outside_attempts {
		assert!(has_errors(response), "must be refused: {response:#}");
		assert!(errors(response).contains("does not exist"), "{response:#}");
	}
	// a relative path isn't a way round it either
	let relative = run(
		&app,
		&explorer,
		LIST_DIRECTORY,
		json!({ "path": "library/sub" }),
	)
	.await;
	assert!(has_errors(&relative), "{relative:#}");
}

/// who may point a library at any folder picks it with the explorer, so they (and the server
/// owner) browse anywhere
#[tokio::test]
async fn test_those_who_pick_library_folders_and_the_owner_browse_anywhere() {
	let app = TestApp::new_with_default_user().await;
	let folders = make_folders();
	add_library(
		&app,
		"Shelf",
		Some(folders.library.to_string_lossy().to_string()),
	)
	.await;

	let picker = user_with(
		&app,
		"picker",
		&[UserPermission::FileExplorer, UserPermission::EditLibrary],
	)
	.await;

	let response = list_as(&app, &picker, Some(&folders.outside)).await;
	assert!(!has_errors(&response), "{response:#}");
	assert_eq!(listed_names(&response), vec!["secret.txt".to_string()]);

	let response = app
		.execute_gql(
			LIST_DIRECTORY,
			Some(json!({ "path": folders.outside.to_string_lossy() })),
		)
		.await;
	assert!(!has_errors(&response), "{response:#}");
}

/// a listing names the book behind a file only when the viewer may see that book
#[tokio::test]
async fn test_a_directory_listing_shows_only_the_books_the_viewer_may_see() {
	let app = TestApp::new_with_default_user().await;
	let folders = make_folders();
	let book_path = folders.library.join("a.epub");

	let library = add_library(
		&app,
		"Shelf",
		Some(folders.library.to_string_lossy().to_string()),
	)
	.await;
	let series = add_series(&app, &library, "Alpha").await;
	let book = add_book(
		&app,
		&series,
		"Alpha 1",
		Some(book_path.to_string_lossy().to_string()),
	)
	.await;

	// may browse anywhere, but the library is hidden from them
	let curator = user_with(
		&app,
		"curator",
		&[UserPermission::FileExplorer, UserPermission::EditLibrary],
	)
	.await;
	hide_library(&app, &curator, &library).await;
	// sees the library
	let reader = user_with(&app, "reader", &[UserPermission::FileExplorer]).await;

	let book_of = |response: &Value| {
		response["data"]["listDirectory"]["nodes"][0]["files"]
			.as_array()
			.expect("files")
			.iter()
			.find(|file| file["name"] == "a.epub")
			.expect("the book's file")["media"]
			.clone()
	};

	let response = list_as(&app, &curator, Some(&folders.library)).await;
	assert!(!has_errors(&response), "{response:#}");
	assert!(
		book_of(&response).is_null(),
		"a book in a hidden library must not come through: {response:#}"
	);

	let response = list_as(&app, &reader, Some(&folders.library)).await;
	assert!(!has_errors(&response), "{response:#}");
	assert_eq!(book_of(&response)["id"], json!(book.id));

	let response = app
		.execute_gql(
			LIST_DIRECTORY,
			Some(json!({ "path": folders.library.to_string_lossy() })),
		)
		.await;
	assert!(!has_errors(&response), "{response:#}");
	assert_eq!(book_of(&response)["id"], json!(book.id));
}

// ---------------------------------------------------------------------------------------
// Metadata fetch records
// ---------------------------------------------------------------------------------------

const FETCH_RECORD: &str = r#"
query Record($id: MetadataFetchRecordId!) {
    metadataFetchRecord(id: $id) { id mediaId seriesId }
}
"#;

const PENDING_MATCHES: &str = r#"
query Pending { pendingMetadataMatches { id mediaId seriesId } }
"#;

const FETCH_MEDIA: &str = r#"
mutation Fetch($id: ID!) { fetchMediaMetadata(id: $id) { id } }
"#;

const REJECT_MEDIA: &str = r#"
mutation Reject($id: ID!) { rejectMediaMatch(mediaId: $id, candidateIndex: 0) { id } }
"#;

const FETCH_SERIES: &str = r#"
mutation Fetch($id: ID!) { fetchSeriesMetadata(id: $id) { __typename } }
"#;

const REJECT_SERIES: &str = r#"
mutation Reject($id: ID!) { rejectSeriesMatch(seriesId: $id, candidateIndex: 0) { id } }
"#;

async fn add_record(app: &TestApp, media_id: Option<&str>, series_id: Option<&str>) {
	metadata_fetch_record::ActiveModel {
		status: Set(MetadataFetchStatus::AwaitingReview),
		media_id: Set(media_id.map(str::to_string)),
		series_id: Set(series_id.map(str::to_string)),
		match_candidates: Set(Some(json!([]))),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should insert the fetch record");
}

/// what a metadata search found for a book or a series is for those who can see it
#[tokio::test]
async fn test_a_fetch_record_is_for_those_who_can_see_its_book_or_series() {
	let app = TestApp::new_with_default_user().await;
	let open = add_library(&app, "Open", None).await;
	let hidden = add_library(&app, "Hidden", None).await;
	let open_series = add_series(&app, &open, "Open series").await;
	let hidden_series = add_series(&app, &hidden, "Hidden series").await;
	let open_book = add_book(&app, &open_series, "Open book", None).await;
	let hidden_book = add_book(&app, &hidden_series, "Hidden book", None).await;

	for book in [&open_book, &hidden_book] {
		add_record(&app, Some(&book.id), None).await;
	}
	for series in [&open_series, &hidden_series] {
		add_record(&app, None, Some(&series.id)).await;
	}

	let reviewer = user_with(
		&app,
		"reviewer",
		&[
			UserPermission::MetadataFetchRecordRead,
			UserPermission::MetadataFetchRecordManage,
		],
	)
	.await;
	hide_library(&app, &reviewer, &hidden).await;

	let record_of = |response: &Value| response["data"]["metadataFetchRecord"].clone();

	let response = run(
		&app,
		&reviewer,
		FETCH_RECORD,
		json!({ "id": { "media": open_book.id } }),
	)
	.await;
	assert_eq!(
		record_of(&response)["mediaId"],
		json!(open_book.id),
		"{response:#}"
	);
	let response = run(
		&app,
		&reviewer,
		FETCH_RECORD,
		json!({ "id": { "series": open_series.id } }),
	)
	.await;
	assert_eq!(
		record_of(&response)["seriesId"],
		json!(open_series.id),
		"{response:#}"
	);

	let response = run(
		&app,
		&reviewer,
		FETCH_RECORD,
		json!({ "id": { "media": hidden_book.id } }),
	)
	.await;
	assert!(!has_errors(&response), "{response:#}");
	assert!(record_of(&response).is_null(), "{response:#}");
	let response = run(
		&app,
		&reviewer,
		FETCH_RECORD,
		json!({ "id": { "series": hidden_series.id } }),
	)
	.await;
	assert!(record_of(&response).is_null(), "{response:#}");

	// and the list of those waiting for review holds only the visible ones
	let response = run(&app, &reviewer, PENDING_MATCHES, json!({})).await;
	assert!(!has_errors(&response), "{response:#}");
	let pending = response["data"]["pendingMetadataMatches"]
		.as_array()
		.expect("a list");
	let media_ids = pending
		.iter()
		.filter_map(|record| record["mediaId"].as_str())
		.collect::<Vec<_>>();
	let series_ids = pending
		.iter()
		.filter_map(|record| record["seriesId"].as_str())
		.collect::<Vec<_>>();
	assert_eq!(media_ids, vec![open_book.id.as_str()], "{response:#}");
	assert_eq!(series_ids, vec![open_series.id.as_str()], "{response:#}");

	// the owner sees them all
	let response = app.execute_gql(PENDING_MATCHES, None).await;
	assert_eq!(
		response["data"]["pendingMetadataMatches"]
			.as_array()
			.expect("a list")
			.len(),
		4,
		"{response:#}"
	);

	// and a hidden book or series can't be searched for or have its match decided: the same
	// answer as for one that doesn't exist
	for (query, id) in [
		(FETCH_MEDIA, &hidden_book.id),
		(REJECT_MEDIA, &hidden_book.id),
		(FETCH_SERIES, &hidden_series.id),
		(REJECT_SERIES, &hidden_series.id),
	] {
		let response = run(&app, &reviewer, query, json!({ "id": id })).await;
		assert!(
			errors(&response).contains("not found"),
			"{query}: {response:#}"
		);
	}
}
