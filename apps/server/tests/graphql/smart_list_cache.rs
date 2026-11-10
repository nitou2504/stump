//! Smart lists that are kept (package SL-3).
//!
//! The books of a smart list are worked out once and kept (`smart_list_members`), and every read
//! narrows them to what the reader may see. So a viewer with limits (**ana**: the *Hidden* library
//! is hidden from her, and she may read nothing above 16 or unrated) gets exactly what the list
//! holds for her, though the cache holds what it holds for everybody: nothing of the hidden set
//! comes out of it, and what a list gives is the same as when its rules are worked out on the spot.

use models::shared::enums::UserPermission;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::{json, Value};

use crate::common::{
	account::{CreateTestUser, TestUser},
	series::setup_single_series_with_n_books,
	visibility::{add_book, Meta, VisibilityFixture},
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
	query Items($id: ID!, $limit: Int, $offset: Int) {
		smartListItems(id: $id, limit: $limit, offset: $offset) {
			__typename
			... on SmartListUngrouped { books { id name path } }
		}
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
