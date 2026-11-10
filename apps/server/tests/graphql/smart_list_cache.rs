//! Smart lists that are kept (package SL-3).
//!
//! The books of a smart list are worked out once and kept (`smart_list_members`), and every read
//! narrows them to what the reader may see. So a viewer with limits (**ana**: the *Hidden* library
//! is hidden from her, and she may read nothing above 16 or unrated) gets exactly what the list
//! holds for her, though the cache holds what it holds for everybody: nothing of the hidden set
//! comes out of it, and what a list gives is the same as when its rules are worked out on the spot.

use models::{
	entity::{age_restriction, media, media_metadata, series},
	shared::enums::UserPermission,
};
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseBackend, Set, Statement};
use serde_json::{json, Value};

use crate::common::{
	account::{CreateTestUser, TestUser},
	series::setup_single_series_with_n_books,
	visibility::{add_book, hide_library, Meta, VisibilityFixture},
	TestApp,
};
use tests::fake_data;

async fn world() -> (TestApp, VisibilityFixture) {
	let app = TestApp::new_with_default_user().await;
	let world = VisibilityFixture::build(&app).await;
	(app, world)
}

const CREATE: &str = r#"
	mutation Create($input: SaveSmartListInput!) {
		createSmartList(input: $input) { id }
	}
"#;

const ITEMS: &str = r#"
	query Items($id: ID!, $limit: Int, $offset: Int, $after: ID) {
		smartListItems(id: $id, limit: $limit, offset: $offset, after: $after) {
			__typename
			... on SmartListUngrouped { books { id name path } nextCursor }
		}
	}
"#;

/// The facets of the filter drawer, from the list's books
const OVERVIEW: &str = r#"
	query Overview($id: ID!) {
		mediaMetadataOverview(smartListId: $id) { genres writers publishers }
	}
"#;

const META: &str = r#"
	query Meta($id: ID!) {
		smartListMeta(id: $id) { matchedBooks matchedSeries matchedLibraries }
		smartListById(id: $id) {
			meta { matchedBooks matchedSeries matchedLibraries }
			books(limit: 50) { id name path }
			items(limit: 50) {
				__typename
				... on SmartListUngrouped { books { id name path } }
			}
		}
	}
"#;

const REFRESH: &str = r#"
	mutation Refresh($ids: [ID!]) { refreshSmartLists(ids: $ids) }
"#;

async fn run(app: &TestApp, token: &str, query: &str, variables: Value) -> Value {
	app.execute_gql_with_token(query, Some(variables), token)
		.await
}

async fn ok(app: &TestApp, token: &str, query: &str, variables: Value) -> Value {
	let response = run(app, token, query, variables).await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"].clone()
}

fn input(name: &str, visibility: &str, filters: Value) -> Value {
	json!({
		"filters": filters,
		"name": name,
		"joiner": "AND",
		"defaultGrouping": "BY_BOOKS",
		"visibility": visibility,
	})
}

async fn create(
	app: &TestApp,
	token: &str,
	name: &str,
	visibility: &str,
	filters: Value,
) -> String {
	let data = ok(
		app,
		token,
		CREATE,
		json!({ "input": input(name, visibility, filters) }),
	)
	.await;
	data["createSmartList"]["id"]
		.as_str()
		.expect("an id")
		.to_string()
}

/// The ids of the books of a page of `smartListItems`
fn page_ids(books: &Value) -> Vec<String> {
	let mut ids = books
		.as_array()
		.expect("a list of books")
		.iter()
		.map(|book| book["id"].as_str().expect("an id").to_string())
		.collect::<Vec<_>>();
	ids.sort();
	ids
}

/// The books a reader gets from `smartListItems`
async fn items(app: &TestApp, token: &str, list: &str) -> Vec<String> {
	let data = ok(app, token, ITEMS, json!({ "id": list })).await;
	page_ids(&data["smartListItems"]["books"])
}

/// The IDs of the books kept as the members of a list
async fn members(app: &TestApp, list: &str) -> Vec<String> {
	let rows = app
		.conn()
		.query_all_raw(Statement::from_sql_and_values(
			DatabaseBackend::Sqlite,
			"SELECT media_id FROM smart_list_members WHERE smart_list_id = ?",
			[list.into()],
		))
		.await
		.expect("should read the members");
	let mut ids: Vec<String> = rows
		.iter()
		.map(|row| row.try_get::<String>("", "media_id").expect("an id"))
		.collect();
	ids.sort();
	ids
}

/// Makes the members of a list old enough for a reader to look at the catalog's revision
async fn age(app: &TestApp, list: &str) {
	app.conn()
		.execute_unprepared(&format!(
			"UPDATE smart_list_materializations SET computed_at = '2020-01-01T00:00:00.000Z' WHERE smart_list_id = '{list}'"
		))
		.await
		.expect("should age the members");
}

/// One page of `smartListItems` in the order the server gives it (by ID), and its cursor
async fn page(
	app: &TestApp,
	token: &str,
	list: &str,
	limit: u64,
	after: Option<&str>,
) -> (Vec<String>, Option<String>) {
	let data = ok(
		app,
		token,
		ITEMS,
		json!({ "id": list, "limit": limit, "after": after }),
	)
	.await;
	let result = &data["smartListItems"];
	assert_eq!(result["__typename"], "SmartListUngrouped", "{result}");
	let books = result["books"]
		.as_array()
		.expect("a list of books")
		.iter()
		.map(|book| book["id"].as_str().expect("an id").to_string())
		.collect();
	(books, result["nextCursor"].as_str().map(str::to_string))
}

/// What the cache says about a list: the revision its members were worked out at, and when
async fn state(app: &TestApp, list: &str) -> (i64, String) {
	let row = app
		.conn()
		.query_one_raw(Statement::from_sql_and_values(
			DatabaseBackend::Sqlite,
			"SELECT revision, computed_at FROM smart_list_materializations WHERE smart_list_id = ?",
			[list.into()],
		))
		.await
		.expect("should read the state of the list")
		.expect("the list was worked out");
	(
		row.try_get::<i64>("", "revision").expect("a revision"),
		row.try_get::<String>("", "computed_at").expect("a time"),
	)
}

/// The revision of the catalog now
async fn head(app: &TestApp) -> i64 {
	app.conn()
		.query_one_raw(Statement::from_string(
			DatabaseBackend::Sqlite,
			"SELECT revision FROM catalog_revision WHERE id = 1".to_string(),
		))
		.await
		.expect("should read the revision")
		.expect("the catalog has a revision")
		.try_get::<i64>("", "revision")
		.expect("a revision")
}

/// The sorted IDs of some books
fn ids_of(books: &[&media::Model]) -> Vec<String> {
	let mut ids: Vec<String> = books.iter().map(|book| book.id.clone()).collect();
	ids.sort();
	ids
}

/// A book with an ID of its own (`add_book` gives a random one), rated `age`: for the tests that
/// need to know where a book sorts, since a smart list's books come in the order of their IDs
async fn add_book_with_id(
	app: &TestApp,
	series: &series::Model,
	id: &str,
	name: &str,
	age: i32,
) -> media::Model {
	let book = fake_data::Media {
		series_id: series.id.clone(),
		id: Some(id.to_string()),
		name: Some(name.to_string()),
		pages: Some(10),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	media_metadata::ActiveModel {
		media_id: Set(Some(book.id.clone())),
		age_rating: Set(Some(age)),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should rate the book");
	book
}

/// Panics if `text` holds the ID, the title or the path of one of `books`
fn assert_lacks(what: &str, text: &str, books: &[&media::Model]) {
	for book in books {
		for (kind, marker) in [
			("the id", &book.id),
			("the title", &book.name),
			("the path", &book.path),
		] {
			assert!(
				!text.contains(marker.as_str()),
				"{what} holds {kind} of {} ({marker:?}):\n{text}",
				book.name
			);
		}
	}
}

/// Everything a reader can ask of a list, read now
struct Surfaces {
	/// `smartListItems`
	items: Vec<String>,
	/// `smartListMeta`
	meta: Value,
	/// `smartListById`: its `meta`, `books` and `items`
	by_id: Value,
	/// The list's shelf in the OPDS catalog
	shelf: String,
	/// The facets of the filter drawer
	overview: Value,
}

async fn surfaces(app: &TestApp, token: &str, list: &str) -> Surfaces {
	let data = ok(app, token, META, json!({ "id": list })).await;
	let shelf = app
		.get_with_token(&format!("/opds/v1.2/smart-lists/{list}"), token)
		.await;
	assert_eq!(shelf.status_code(), 200);
	Surfaces {
		items: items(app, token, list).await,
		meta: data["smartListMeta"].clone(),
		by_id: data["smartListById"].clone(),
		shelf: shelf.text(),
		overview: ok(app, token, OVERVIEW, json!({ "id": list })).await
			["mediaMetadataOverview"]
			.clone(),
	}
}

/// Every surface of a list gives exactly `shown` (and the counts of `counts`), nothing of `gone`,
/// and the facets of the books that are shown: for each name in `present` its writer, publisher
/// and genre ("Open" is "Open Writer", "Open Press" and "Open Genre"), and none of those of
/// `absent`
fn assert_exactly(
	got: &Surfaces,
	shown: &[&media::Model],
	gone: &[&media::Model],
	counts: Value,
	present: &[&str],
	absent: &[&str],
) {
	let expected = ids_of(shown);
	assert_eq!(got.items, expected, "smartListItems");
	assert_eq!(got.meta, counts, "smartListMeta");
	assert_eq!(got.by_id["meta"], counts, "SmartList.meta");
	assert_eq!(page_ids(&got.by_id["books"]), expected, "SmartList.books");
	assert_eq!(
		page_ids(&got.by_id["items"]["books"]),
		expected,
		"SmartList.items"
	);

	for (what, text) in [
		("smartListItems", got.items.join(" ")),
		("smartListById", got.by_id.to_string()),
		("the OPDS shelf", got.shelf.clone()),
	] {
		assert_lacks(what, &text, gone);
	}
	for book in shown {
		assert!(
			got.shelf.contains(&book.id),
			"the OPDS shelf lacks {}:\n{}",
			book.name,
			got.shelf
		);
	}

	for (facet, suffix) in [
		("writers", "Writer"),
		("publishers", "Press"),
		("genres", "Genre"),
	] {
		let values = got.overview[facet].as_array().expect("a list of values");
		for who in present {
			let value = json!(format!("{who} {suffix}"));
			assert!(values.contains(&value), "{facet} lacks {value}: {values:?}");
		}
		for who in absent {
			let value = json!(format!("{who} {suffix}"));
			assert!(
				!values.contains(&value),
				"{facet} holds {value}: {values:?}"
			);
		}
	}
}

/// Every book of the world has "book" in its name
fn every_book() -> Value {
	json!([{ "groups": [{ "media": { "name": { "contains": "book" } } }], "joiner": "AND" }])
}

/// Names the hidden library: the hidden library's books and the general book. For anyone who may
/// not read the hidden library, only the general book is left
fn rules_naming_the_hidden_library(world: &VisibilityFixture) -> Value {
	json!([{
		"groups": [
			{ "media": { "libraryId": { "eq": world.hidden_library.id } } },
			{ "media": { "name": { "eq": world.general.name } } },
		],
		"joiner": "OR",
	}])
}

async fn friend_with_lists(app: &TestApp, username: &str) -> TestUser {
	CreateTestUser {
		username: username.to_string(),
		permissions: vec![UserPermission::AccessSmartList],
		..Default::default()
	}
	.insert(app)
	.await
}

// ---------------------------------------------------------------------------------------
// The cache holds the catalog's view, a reader gets their own
// ---------------------------------------------------------------------------------------

/// The members hold every book the rules match, the hidden ones too; what ana gets from any way
/// of reading the list (the items, the books, the counts, an OPDS shelf) is only what she may see
#[tokio::test]
async fn a_cached_list_gives_ana_only_what_she_may_read() {
	let (app, world) = world().await;
	let list = create(
		&app,
		&world.owner_token,
		"Shared shelf",
		"PUBLIC",
		rules_naming_the_hidden_library(&world),
	)
	.await;

	// the owner reads it: the list is worked out and kept, with the hidden book in it
	let owner_books = items(&app, &world.owner_token, &list).await;
	let mut expected = vec![world.general.id.clone(), world.hidden.id.clone()];
	expected.sort();
	assert_eq!(owner_books, expected);
	assert_eq!(
		members(&app, &list).await,
		expected,
		"the members are the whole catalog's"
	);

	// ana reads it: from the same members, and only her share
	let ana_books = items(&app, &world.ana.token, &list).await;
	assert_eq!(ana_books, vec![world.general.id.clone()]);

	// a page of one: her book is the whole list for her, so her page has no cursor, and the
	// owner's has one. The books she may not see are no reason for a page to say there is more
	let (page_of_ana, cursor) = page(&app, &world.ana.token, &list, 1, None).await;
	assert_eq!(page_of_ana, vec![world.general.id.clone()]);
	assert_eq!(cursor, None);
	let (page_of_owner, cursor) = page(&app, &world.owner_token, &list, 1, None).await;
	assert_eq!(page_of_owner.len(), 1);
	assert_eq!(cursor, page_of_owner.first().cloned());

	let data = ok(&app, &world.ana.token, META, json!({ "id": list })).await;
	assert_eq!(
		data["smartListMeta"],
		json!({ "matchedBooks": 1, "matchedSeries": 1, "matchedLibraries": 1 })
	);
	assert_eq!(data["smartListById"]["meta"], data["smartListMeta"]);
	assert_eq!(
		page_ids(&data["smartListById"]["books"]),
		vec![world.general.id.clone()]
	);
	assert_eq!(
		page_ids(&data["smartListById"]["items"]["books"]),
		vec![world.general.id.clone()]
	);
	world.assert_none_hidden(&data);

	// the owner's counts are the whole list's
	let data = ok(&app, &world.owner_token, META, json!({ "id": list })).await;
	assert_eq!(
		data["smartListMeta"],
		json!({ "matchedBooks": 2, "matchedSeries": 2, "matchedLibraries": 2 })
	);

	// OPDS: the shelf's feed, from the same members
	let shelf = app
		.get_with_token(&format!("/opds/v1.2/smart-lists/{list}"), &world.ana.token)
		.await;
	assert_eq!(shelf.status_code(), 200);
	let feed = shelf.text();
	assert!(feed.contains(&world.general.id), "{feed}");
	world.assert_text_none_hidden(&feed);
	let shelf = app
		.get_with_token(
			&format!("/opds/v1.2/smart-lists/{list}"),
			&world.owner_token,
		)
		.await;
	assert!(shelf.text().contains(&world.hidden.id));

	// the facets of the filter drawer too (they embed the list's books in every field)
	let overview = ok(
		&app,
		&world.ana.token,
		r#"query O($id: ID!) { mediaMetadataOverview(smartListId: $id) { genres writers publishers } }"#,
		json!({ "id": list }),
	)
	.await;
	world.assert_none_hidden(&overview);
}

/// What a list gives is the same whether it is kept or worked out on the spot: the same rules
/// next to a rule that keeps a list live (a reading status every unread book passes) give the
/// same books, to the owner and to ana
#[tokio::test]
async fn a_kept_list_gives_what_the_same_list_worked_out_live_gives() {
	let (app, world) = world().await;
	let kept = create(&app, &world.owner_token, "Kept", "PUBLIC", every_book()).await;
	let live_rules = json!([
		{ "groups": [{ "media": { "name": { "contains": "book" } } }], "joiner": "AND" },
		{ "groups": [{ "media": { "readingStatus": { "isNot": "READING" } } }], "joiner": "AND" },
	]);
	let live = create(&app, &world.owner_token, "Live", "PUBLIC", live_rules).await;

	for (who, token) in [("the owner", &world.owner_token), ("ana", &world.ana.token)] {
		let from_cache = items(&app, token, &kept).await;
		let worked_out = items(&app, token, &live).await;
		assert!(!from_cache.is_empty(), "{who} gets nothing");
		assert_eq!(from_cache, worked_out, "{who}");

		let counts = |data: &Value| data["smartListMeta"].clone();
		let a = ok(&app, token, META, json!({ "id": kept })).await;
		let b = ok(&app, token, META, json!({ "id": live })).await;
		assert_eq!(counts(&a), counts(&b), "{who}");
	}

	// the one that keeps a list live has no members, and the other has
	assert!(!members(&app, &kept).await.is_empty());
	assert!(
		members(&app, &live).await.is_empty(),
		"a live list is never kept"
	);
}

// ---------------------------------------------------------------------------------------
// A change shows
// ---------------------------------------------------------------------------------------

/// A new book is in the list once the members are older than ten seconds, and a book that is
/// removed (not deleted) leaves it, and comes back, without the list being worked out again
#[tokio::test]
async fn a_change_shows_in_a_list_when_the_members_are_old_enough() {
	let (app, world) = world().await;
	let list = create(
		&app,
		&world.owner_token,
		"Everything",
		"PUBLIC",
		every_book(),
	)
	.await;
	// general, adult, unrated, hidden (removed is out for everybody)
	assert_eq!(items(&app, &world.owner_token, &list).await.len(), 4);
	assert_eq!(items(&app, &world.ana.token, &list).await.len(), 1);

	// a book is added (by a scan): the members are young, the list is as it was
	let late = add_book(
		&app,
		&world.open_series,
		"Late book",
		Meta {
			age: Some(0),
			..Default::default()
		},
	)
	.await;
	assert_eq!(items(&app, &world.owner_token, &list).await.len(), 4);

	// once they are old enough a reader looks at the catalog's revision, and the list is worked
	// out again
	age(&app, &list).await;
	let owner_books = items(&app, &world.owner_token, &list).await;
	assert_eq!(owner_books.len(), 5);
	assert!(owner_books.contains(&late.id));
	assert!(items(&app, &world.ana.token, &list)
		.await
		.contains(&late.id));

	// removing a book from the library is not a change of the list: it leaves it for everybody,
	// and is back when it is restored, with no new work
	let before = members(&app, &list).await;
	let removed = app
		.execute_gql(
			r#"mutation Remove($id: ID!) { deleteMedia(id: $id) { id } }"#,
			Some(json!({ "id": late.id })),
		)
		.await;
	assert!(removed.get("errors").is_none(), "{removed}");
	age(&app, &list).await;
	assert!(!items(&app, &world.owner_token, &list)
		.await
		.contains(&late.id));
	assert_eq!(
		members(&app, &list).await,
		before,
		"the members are the same"
	);
}

/// A saved change of the rules is the next thing a reader sees, however young the members were
#[tokio::test]
async fn new_rules_show_at_once() {
	let (app, world) = world().await;
	let list = create(
		&app,
		&world.owner_token,
		"Shelf",
		"PRIVATE",
		rules_naming_the_hidden_library(&world),
	)
	.await;
	assert_eq!(items(&app, &world.owner_token, &list).await.len(), 2);

	let data = ok(
		&app,
		&world.owner_token,
		r#"mutation Update($id: ID!, $input: SaveSmartListInput!) {
			updateSmartList(id: $id, input: $input) { id }
		}"#,
		json!({ "id": list, "input": input("Shelf", "PRIVATE", json!([
			{ "groups": [{ "media": { "name": { "eq": world.adult.name } } }], "joiner": "AND" }
		])) }),
	)
	.await;
	assert_eq!(data["updateSmartList"]["id"], list);

	assert_eq!(
		items(&app, &world.owner_token, &list).await,
		vec![world.adult.id.clone()]
	);
	assert_eq!(members(&app, &list).await, vec![world.adult.id.clone()]);
}

// ---------------------------------------------------------------------------------------
// Counting, and what can't be kept
// ---------------------------------------------------------------------------------------

/// What a reader may see is decided when they read, never when the members were worked out: a
/// new library exclusion, and then an age restriction, take books out of every surface of a kept
/// list at once, and the list is not worked out again for it. The members are the ones the first
/// read kept, the catalog's revision is the one they were worked out at, and the members are too
/// old to be good for a reader for any other reason than that revision.
#[tokio::test]
async fn a_change_of_access_shows_at_once_without_working_the_list_out_again() {
	let (app, world) = world().await;
	// books with something in each facet of the filter drawer: one in the hidden library, one
	// rated 18, and one anybody may read
	let open_facet = add_book(
		&app,
		&world.open_series,
		"Open facet book",
		Meta {
			age: Some(0),
			writer: Some("Open Writer"),
			publisher: Some("Open Press"),
			genre: Some("Open Genre"),
			..Default::default()
		},
	)
	.await;
	let hidden_facet = add_book(
		&app,
		&world.hidden_series,
		"Hidden facet book",
		Meta {
			age: Some(0),
			writer: Some("Hidden Writer"),
			publisher: Some("Hidden Press"),
			genre: Some("Hidden Genre"),
			..Default::default()
		},
	)
	.await;
	let adult_facet = add_book(
		&app,
		&world.open_series,
		"Adult facet book",
		Meta {
			age: Some(18),
			writer: Some("Adult Writer"),
			publisher: Some("Adult Press"),
			genre: Some("Adult Genre"),
			..Default::default()
		},
	)
	.await;
	let list = create(
		&app,
		&world.owner_token,
		"Shared shelf",
		"PUBLIC",
		every_book(),
	)
	.await;
	let reader = friend_with_lists(&app, "reader").await;

	// no limits yet: the reader gets every book but the removed one, which is in the members
	let everything = [
		&world.general,
		&world.adult,
		&world.unrated,
		&world.hidden,
		&open_facet,
		&hidden_facet,
		&adult_facet,
	];
	let got = surfaces(&app, &reader.token, &list).await;
	assert_exactly(
		&got,
		&everything,
		&[&world.removed],
		json!({ "matchedBooks": 7, "matchedSeries": 3, "matchedLibraries": 2 }),
		&["Open", "Hidden", "Adult"],
		&[],
	);
	let kept_members = members(&app, &list).await;
	assert_eq!(kept_members.len(), 8);

	// the members are old from here on: only the revision of the catalog keeps them good, so a
	// refresh would show in the time they were worked out at
	age(&app, &list).await;
	let kept = state(&app, &list).await;
	assert_eq!(
		kept.0,
		head(&app).await,
		"the members are of the catalog as it is"
	);
	assert_eq!(items(&app, &reader.token, &list).await, ids_of(&everything));
	assert_eq!(state(&app, &list).await, kept, "a read doesn't work it out");

	// a library is hidden from the reader
	hide_library(&app, &reader, &world.hidden_library).await;
	let got = surfaces(&app, &reader.token, &list).await;
	assert_exactly(
		&got,
		&[
			&world.general,
			&world.adult,
			&world.unrated,
			&open_facet,
			&adult_facet,
		],
		&[&world.hidden, &hidden_facet, &world.removed],
		json!({ "matchedBooks": 5, "matchedSeries": 2, "matchedLibraries": 1 }),
		&["Open", "Adult"],
		&["Hidden"],
	);
	assert_eq!(state(&app, &list).await, kept, "hiding a library");
	assert_eq!(head(&app).await, kept.0, "hiding a library");
	assert_eq!(members(&app, &list).await, kept_members);

	// and then nothing above 16, nor unrated: ana's limits
	age_restriction::ActiveModel {
		user_id: Set(reader.id.clone()),
		age: Set(16),
		restrict_on_unset: Set(true),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should restrict the reader");
	let got = surfaces(&app, &reader.token, &list).await;
	let mut gone = world.hidden_from_ana();
	gone.extend([&hidden_facet, &adult_facet]);
	assert_exactly(
		&got,
		&[&world.general, &open_facet],
		&gone,
		json!({ "matchedBooks": 2, "matchedSeries": 1, "matchedLibraries": 1 }),
		&["Open"],
		&["Hidden", "Adult"],
	);
	world.assert_none_hidden(&got.by_id);
	world.assert_text_none_hidden(&got.shelf);
	assert_eq!(state(&app, &list).await, kept, "an age restriction");
	assert_eq!(head(&app).await, kept.0, "an age restriction");
	assert_eq!(members(&app, &list).await, kept_members);

	// the same check does see a refresh when the catalog changes: with members this old, the next
	// reader works the list out again, and the book is in it
	let late = add_book(
		&app,
		&world.open_series,
		"Late book",
		Meta {
			age: Some(0),
			..Default::default()
		},
	)
	.await;
	assert!(head(&app).await > kept.0, "a new book changes the catalog");
	assert!(items(&app, &reader.token, &list).await.contains(&late.id));
	let worked_out = state(&app, &list).await;
	assert_eq!(worked_out.0, head(&app).await);
	assert_ne!(worked_out.1, kept.1, "the list was worked out again");
	assert_eq!(members(&app, &list).await.len(), kept_members.len() + 1);
}

// ---------------------------------------------------------------------------------------
// Counting, and what can't be kept
// ---------------------------------------------------------------------------------------

/// The counts of `smartListMeta` (which book-curator reads) are made by the database over the
/// members: every book, series and library, not capped at a page
#[tokio::test]
async fn the_counts_cover_every_book_of_a_long_list() {
	let app = TestApp::new_with_default_user().await;
	setup_single_series_with_n_books(&app, fake_data::Series::default(), 450).await;
	let token = app.access_token.read().await.clone().expect("a token");
	let list = create(&app, &token, "All of them", "PRIVATE", json!([])).await;

	let data = ok(&app, &token, META, json!({ "id": list })).await;
	assert_eq!(
		data["smartListMeta"],
		json!({ "matchedBooks": 450, "matchedSeries": 1, "matchedLibraries": 1 })
	);
	assert_eq!(members(&app, &list).await.len(), 450);

	// the pages of a long list follow each other from the members
	let mut seen = Vec::new();
	for offset in [0, 200, 400] {
		let page = ok(
			&app,
			&token,
			ITEMS,
			json!({ "id": list, "limit": 200, "offset": offset }),
		)
		.await;
		seen.extend(page_ids(&page["smartListItems"]["books"]));
	}
	seen.sort();
	seen.dedup();
	assert_eq!(seen.len(), 450);
}

/// A page of a list is cut from what the reader may see, so a book they may not see never makes a
/// page say there is more. A smart list's books come in the order of their IDs: two books that
/// sort after every other one (one in the hidden library, one rated 18) come after ana's only
/// book, so a page that asked the whole catalog whether there is more would give her a cursor.
#[tokio::test]
async fn books_a_reader_may_not_see_never_make_a_page_say_there_is_more() {
	let (app, world) = world().await;
	let last_hidden = add_book_with_id(
		&app,
		&world.hidden_series,
		"ffffffff-ffff-4fff-8fff-fffffffffff1",
		"Last hidden book",
		0,
	)
	.await;
	let last_adult = add_book_with_id(
		&app,
		&world.open_series,
		"ffffffff-ffff-4fff-8fff-fffffffffff2",
		"Last adult book",
		18,
	)
	.await;
	let list = create(
		&app,
		&world.owner_token,
		"Everything",
		"PUBLIC",
		every_book(),
	)
	.await;

	// ana's one book is the whole list for her, whatever the size of the page: no cursor, though
	// two books she may not see come after it
	for limit in [1, 2, 200] {
		let (books, cursor) = page(&app, &world.ana.token, &list, limit, None).await;
		assert_eq!(books, vec![world.general.id.clone()], "a page of {limit}");
		assert_eq!(cursor, None, "a page of {limit}");
	}
	// and after her book there is nothing for her: the books that sort after it are not hers
	let (books, cursor) = page(
		&app,
		&world.ana.token,
		&list,
		10,
		Some(world.general.id.as_str()),
	)
	.await;
	assert!(books.is_empty(), "{books:?}");
	assert_eq!(cursor, None);

	// the owner sees six books, in the order of their IDs: a page of five says there is more (the
	// cursor is the last book of the page), a page of six, which holds the rest, does not
	let owner_sees = ids_of(&[
		&world.general,
		&world.adult,
		&world.unrated,
		&world.hidden,
		&last_hidden,
		&last_adult,
	]);
	let (books, cursor) = page(&app, &world.owner_token, &list, 5, None).await;
	assert_eq!(books, owner_sees[..5]);
	assert_eq!(cursor.as_deref(), Some(owner_sees[4].as_str()));
	let (books, cursor) = page(&app, &world.owner_token, &list, 6, None).await;
	assert_eq!(books, owner_sees);
	assert_eq!(
		cursor, None,
		"a page that holds the rest of the list ends it"
	);

	// a walk by cursor, two books at a time: each book once, in order, ending with no cursor
	let mut seen = Vec::new();
	let mut after: Option<String> = None;
	let mut ended = false;
	for _ in 0..10 {
		let (books, cursor) =
			page(&app, &world.owner_token, &list, 2, after.as_deref()).await;
		seen.extend(books);
		after = cursor;
		if after.is_none() {
			ended = true;
			break;
		}
	}
	assert!(ended, "the pages never came to an end");
	assert_eq!(seen, owner_sees);
}

/// A list whose rules depend on who reads it is not kept, and gives the reader their own answer
#[tokio::test]
async fn a_list_that_depends_on_the_reader_is_not_kept() {
	let (app, world) = world().await;
	let list = create(
		&app,
		&world.owner_token,
		"Not started",
		"PUBLIC",
		json!([{ "groups": [{ "media": { "readingStatus": { "is": "NOT_STARTED" } } }], "joiner": "AND" }]),
	)
	.await;
	let reader = friend_with_lists(&app, "reader").await;

	assert!(!items(&app, &world.owner_token, &list).await.is_empty());
	assert!(members(&app, &list).await.is_empty());

	// the reader finishes a book: it is out of *their* list, and still in the owner's
	let finished = app
		.execute_gql_with_token(
			r#"mutation Finish($id: ID!) { finishMediaProgress(id: $id) }"#,
			Some(json!({ "id": world.general.id })),
			&reader.token,
		)
		.await;
	assert!(finished.get("errors").is_none(), "{finished}");
	let for_reader = items(&app, &reader.token, &list).await;
	let for_owner = items(&app, &world.owner_token, &list).await;
	assert!(!for_reader.contains(&world.general.id), "{for_reader:?}");
	assert!(for_owner.contains(&world.general.id), "{for_owner:?}");
}

// ---------------------------------------------------------------------------------------
// The owner's refresh
// ---------------------------------------------------------------------------------------

/// Only the server owner makes the lists be worked out again, all of them or the ones given; it
/// puts back members that something outside the server's writes took away
#[tokio::test]
async fn only_the_owner_makes_the_lists_be_worked_out_again() {
	let (app, world) = world().await;
	let list = create(
		&app,
		&world.owner_token,
		"Everything",
		"PUBLIC",
		every_book(),
	)
	.await;
	let other = create(
		&app,
		&world.owner_token,
		"Hidden's",
		"PUBLIC",
		rules_naming_the_hidden_library(&world),
	)
	.await;
	items(&app, &world.owner_token, &list).await;
	items(&app, &world.owner_token, &other).await;
	// the five books of the world: the removed one is a member too, nobody gets it
	assert_eq!(members(&app, &list).await.len(), 5);

	// ana is refused, owner or not of her own lists
	let refused = run(&app, &world.ana.token, REFRESH, json!({ "ids": null })).await;
	assert!(refused.get("errors").is_some(), "{refused}");
	assert_eq!(refused["data"], Value::Null);

	// something the triggers can't see empties the members
	app.conn()
		.execute_unprepared("DELETE FROM smart_list_members")
		.await
		.unwrap();
	assert!(members(&app, &list).await.is_empty());

	// one list
	let data = ok(&app, &world.owner_token, REFRESH, json!({ "ids": [list] })).await;
	assert_eq!(data["refreshSmartLists"], 1);
	assert_eq!(members(&app, &list).await.len(), 5);
	assert!(members(&app, &other).await.is_empty());

	// all of them
	let data = ok(&app, &world.owner_token, REFRESH, json!({ "ids": null })).await;
	assert_eq!(data["refreshSmartLists"], 2);
	assert_eq!(members(&app, &other).await.len(), 2);
}
