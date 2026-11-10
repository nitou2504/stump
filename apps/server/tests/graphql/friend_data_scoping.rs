//! Where the server keeps its files is not for readers (package C5).
//!
//! The folder of a library, the path of a book or a series on disk, where its thumbnail is cached
//! and the hashes of a book's file are the server's business. Whoever runs the libraries (the
//! owner, a manager, whoever edits a library or browses the server's files) gets them. Everybody
//! else gets the path **inside** the library (what `relativeLibraryPath` has always been), an
//! empty text for a library's folder and nothing for the rest, on every type that has them.
//!
//! The world is the fixture of P0, with its files put at places like the real ones (`/biblioteca/…`,
//! a hash and a cached thumbnail each), so that a test can tell a path inside the library from one
//! of the server.

use models::{
	entity::{bookmark, library, media, series},
	shared::enums::{ReadingStatus, UserPermission},
};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Value};
use tests::fake_data;

use crate::common::{visibility::VisibilityFixture, TestApp};

/// The folders of the two libraries, where a real server keeps them
const OPEN_FOLDER: &str = "/biblioteca/open";
const HIDDEN_FOLDER: &str = "/biblioteca/hidden";

/// A book's cached thumbnail and its hashes, which only the server's side may see
fn thumbnail_of(id: &str) -> String {
	format!("/config/thumbnails/{id}.webp")
}
fn hash_of(name: &str) -> String {
	format!("hash-of-{name}")
}
fn koreader_hash_of(name: &str) -> String {
	format!("koreader-hash-of-{name}")
}

/// The books of the fixture at places like the real ones, and the fixture updated to match
async fn put_files_where_a_server_keeps_them(
	app: &TestApp,
	world: &mut VisibilityFixture,
) {
	async fn library_at(
		app: &TestApp,
		model: &library::Model,
		folder: &str,
	) -> library::Model {
		let mut active: library::ActiveModel = model.clone().into();
		active.path = Set(folder.to_string());
		active.thumbnail_path =
			Set(Some(thumbnail_of(&format!("library-{}", model.name))));
		active
			.update(app.conn())
			.await
			.expect("should move the library")
	}
	async fn series_at(
		app: &TestApp,
		model: &series::Model,
		folder: &str,
	) -> series::Model {
		let mut active: series::ActiveModel = model.clone().into();
		active.path = Set(Some(format!("{folder}/{}", model.name)));
		active.thumbnail_path =
			Set(Some(thumbnail_of(&format!("series-{}", model.name))));
		active
			.update(app.conn())
			.await
			.expect("should move the series")
	}
	async fn book_at(
		app: &TestApp,
		model: &media::Model,
		folder: &str,
		shelf: &str,
	) -> media::Model {
		let mut active: media::ActiveModel = model.clone().into();
		active.path = Set(format!("{folder}/{shelf}/{}.epub", model.name));
		active.hash = Set(Some(hash_of(&model.name)));
		active.koreader_hash = Set(Some(koreader_hash_of(&model.name)));
		active.thumbnail_path = Set(Some(thumbnail_of(&model.id)));
		active
			.update(app.conn())
			.await
			.expect("should move the book")
	}

	world.open = library_at(app, &world.open, OPEN_FOLDER).await;
	world.hidden_library = library_at(app, &world.hidden_library, HIDDEN_FOLDER).await;
	world.open_series = series_at(app, &world.open_series, OPEN_FOLDER).await;
	world.loose_series = series_at(app, &world.loose_series, OPEN_FOLDER).await;
	world.hidden_series = series_at(app, &world.hidden_series, HIDDEN_FOLDER).await;
	world.general = book_at(app, &world.general, OPEN_FOLDER, "Open shelf").await;
	world.adult = book_at(app, &world.adult, OPEN_FOLDER, "Open shelf").await;
	world.removed = book_at(app, &world.removed, OPEN_FOLDER, "Open shelf").await;
	world.unrated = book_at(app, &world.unrated, OPEN_FOLDER, "Loose shelf").await;
	world.hidden = book_at(app, &world.hidden, HIDDEN_FOLDER, "Hidden shelf").await;
}

async fn world() -> (TestApp, VisibilityFixture) {
	let app = TestApp::new_with_default_user().await;
	let mut world = VisibilityFixture::build(&app).await;
	put_files_where_a_server_keeps_them(&app, &mut world).await;
	(app, world)
}

/// Runs a query as `token`, and fails if it fails
async fn ask(app: &TestApp, token: &str, query: &str) -> Value {
	let response = app.execute_gql_with_token(query, None, token).await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"].clone()
}

/// Everything a book says about where its files are, and the library and series that hold it
const PLACES: &str = "id path hash koreaderHash thumbnailPath relativeLibraryPath
	library { path thumbnailPath } series { path thumbnailPath }";

/// What `token` is told about the book `id` (the one ana may read: `general`)
async fn places_of(app: &TestApp, token: &str, id: &str) -> Value {
	let data = ask(
		app,
		token,
		&format!("{{ media {{ nodes {{ {PLACES} }} }} }}"),
	)
	.await;
	data["media"]["nodes"]
		.as_array()
		.expect("a list of books")
		.iter()
		.find(|node| node["id"] == id)
		.unwrap_or_else(|| panic!("the book {id} in {data}"))
		.clone()
}

/// What the server's side of `general` is, for those who may see it
fn general_as_the_server_has_it(world: &VisibilityFixture) -> Value {
	json!({
		"id": world.general.id,
		"path": format!("{OPEN_FOLDER}/Open shelf/General book.epub"),
		"hash": hash_of("General book"),
		"koreaderHash": koreader_hash_of("General book"),
		"thumbnailPath": thumbnail_of(&world.general.id),
		"relativeLibraryPath": "/Open shelf/General book.epub",
		"library": {
			"path": OPEN_FOLDER,
			"thumbnailPath": thumbnail_of("library-Open"),
		},
		"series": {
			"path": format!("{OPEN_FOLDER}/Open shelf"),
			"thumbnailPath": thumbnail_of("series-Open shelf"),
		},
	})
}

/// What `general` is for everybody else: the path inside the library, and nothing of the server
fn general_as_a_reader_has_it(world: &VisibilityFixture) -> Value {
	json!({
		"id": world.general.id,
		"path": "/Open shelf/General book.epub",
		"hash": null,
		"koreaderHash": null,
		"thumbnailPath": null,
		"relativeLibraryPath": "/Open shelf/General book.epub",
		"library": { "path": "", "thumbnailPath": null },
		"series": { "path": "/Open shelf", "thumbnailPath": null },
	})
}

/// Nothing in `json` names a folder of the server, a hash or a cached thumbnail
fn assert_nothing_of_the_server(json: &Value) {
	let text = json.to_string();
	for (what, marker) in [
		("a folder of the server", "/biblioteca"),
		("the cache of thumbnails", "/config"),
		("a hash", "hash-of"),
	] {
		assert!(!text.contains(marker), "{what} ({marker}) in {json}");
	}
}

/// The owner, who runs the libraries, is told where everything is
#[tokio::test]
async fn the_owner_is_told_where_the_files_are() {
	let (app, world) = world().await;

	let book = places_of(&app, &world.owner_token, &world.general.id).await;

	assert_eq!(book, general_as_the_server_has_it(&world));
}

/// A reader (ana has no permission to run anything) is told the path inside the library and no
/// hash, and a response that has the server's folders anywhere in it is a leak
#[tokio::test]
async fn a_reader_is_told_the_path_inside_the_library_and_no_hash() {
	let (app, world) = world().await;

	for token in [&world.ana.token, &world.ben.token] {
		let book = places_of(&app, token, &world.general.id).await;
		assert_eq!(book, general_as_a_reader_has_it(&world));
		assert_nothing_of_the_server(&book);
	}
}

/// Every place a book, a series or a library is listed: the lists too, and what they hold
#[tokio::test]
async fn the_lists_say_the_same_as_the_book() {
	let (app, world) = world().await;
	let lists = "{
		media { nodes { id path hash koreaderHash thumbnailPath relativeLibraryPath } }
		series { nodes { id path thumbnailPath } }
		libraries { nodes { id path thumbnailPath } }
		randomMedia(count: 5) { id path hash }
		seriesById(id: \"%SERIES%\") { path thumbnailPath media { path hash } }
		mediaById(id: \"%BOOK%\") { path hash koreaderHash thumbnailPath }
	}"
	.replace("%SERIES%", &world.open_series.id)
	.replace("%BOOK%", &world.general.id);

	let data = ask(&app, &world.ana.token, &lists).await;

	assert_nothing_of_the_server(&data);
	world.assert_none_hidden(&data);
	// and what is there is what the owner's would be, without the server's side
	assert_eq!(
		data["libraries"]["nodes"],
		json!([{ "id": world.open.id, "path": "", "thumbnailPath": null }]),
	);
	assert_eq!(
		data["mediaById"],
		json!({
			"path": "/Open shelf/General book.epub",
			"hash": null,
			"koreaderHash": null,
			"thumbnailPath": null,
		}),
	);
	assert_eq!(data["seriesById"]["path"], "/Open shelf");
	assert_eq!(data["seriesById"]["thumbnailPath"], Value::Null);

	// the owner's say where everything is
	let data = ask(&app, &world.owner_token, &lists).await;
	assert_eq!(
		data["seriesById"]["path"],
		format!("{OPEN_FOLDER}/Open shelf")
	);
	assert!(data["libraries"]["nodes"]
		.as_array()
		.expect("libraries")
		.iter()
		.all(|library| library["path"]
			.as_str()
			.is_some_and(|path| path.starts_with("/biblioteca/"))));
}

/// Who may see the server's side: the owner, and those who manage or edit libraries (the library
/// settings show the folder and save it back) or browse the server's files (they list the very
/// same folders). `CreateLibrary` and `DeleteLibrary` bring `EditLibrary` and `ManageLibrary`
#[tokio::test]
async fn those_who_run_libraries_are_told_where_the_files_are() {
	let (app, world) = world().await;

	for (index, permission) in [
		UserPermission::ManageLibrary,
		UserPermission::EditLibrary,
		UserPermission::FileExplorer,
		UserPermission::CreateLibrary,
		UserPermission::DeleteLibrary,
	]
	.into_iter()
	.enumerate()
	{
		let who = world
			.limited_like_ana(&app, &format!("runner-{index}"), vec![permission])
			.await;
		let book = places_of(&app, &who.token, &world.general.id).await;
		assert_eq!(
			book,
			general_as_the_server_has_it(&world),
			"{permission:?} is told where the files are"
		);
	}
}

/// What does not open it: everything else a person may be given, e.g. what a friend gets to read
/// and download, to add books and lists, and what a tool that edits metadata (book-curator's own
/// key) has
#[tokio::test]
async fn other_permissions_are_told_the_path_inside_the_library() {
	let (app, world) = world().await;

	for (index, permissions) in [
		vec![UserPermission::DownloadFile],
		vec![UserPermission::EditMetadata, UserPermission::AccessApiKeys],
		vec![UserPermission::UploadFile, UserPermission::AddBooks],
		vec![
			UserPermission::AccessSmartList,
			UserPermission::ChangePassword,
		],
	]
	.into_iter()
	.enumerate()
	{
		let who = world
			.limited_like_ana(&app, &format!("other-{index}"), permissions.clone())
			.await;
		let book = places_of(&app, &who.token, &world.general.id).await;
		assert_eq!(
			book,
			general_as_a_reader_has_it(&world),
			"{permissions:?} is told the path inside the library"
		);
	}
}

/// A book whose path is not under its library's folder (the folder was changed after the last
/// scan) is given as its file name: the folders of the server are not what fills the gap
#[tokio::test]
async fn a_path_outside_its_library_is_given_as_the_file_name() {
	let (app, world) = world().await;
	let mut active: media::ActiveModel = world.general.clone().into();
	active.path = Set("/mnt/elsewhere/Open shelf/General book.epub".to_string());
	active
		.update(app.conn())
		.await
		.expect("should move the book");

	let book = places_of(&app, &world.ana.token, &world.general.id).await;

	assert_eq!(book["path"], "General book.epub");
	assert_eq!(book["relativeLibraryPath"], "General book.epub");
	assert_nothing_of_the_server(&book);
	assert!(!book.to_string().contains("/mnt/elsewhere"), "{book}");
}

/// A reader still gets the bookmarks and notes of their own reading session (the check that a
/// session's events are its reader's alone must not take them from the reader)
#[tokio::test]
async fn a_reader_still_gets_the_events_of_their_own_session() {
	let (app, world) = world().await;
	let session = fake_data::ReadingSession {
		media_id: world.general.id.clone(),
		user_id: world.ana.id.clone(),
		end_percentage: 0.25,
		status: ReadingStatus::Reading,
		..Default::default()
	}
	.insert(app.conn())
	.await;
	bookmark::ActiveModel {
		id: Set("bookmark-of-ana".to_string()),
		media_id: Set(world.general.id.clone()),
		user_id: Set(world.ana.id.clone()),
		created_at: Set(chrono::Utc::now()),
		session_id: Set(Some(session.id)),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should insert the bookmark");

	let data = ask(
		&app,
		&world.ana.token,
		&format!(
			"{{ readingSessionById(id: {}) {{ events {{ __typename }} }} }}",
			session.id
		),
	)
	.await;

	assert_eq!(
		data["readingSessionById"]["events"],
		json!([{ "__typename": "Bookmark" }]),
		"{data}"
	);
}
