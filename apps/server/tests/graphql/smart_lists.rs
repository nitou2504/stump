//! Smart lists that can be shared (package SL-2).
//!
//! A smart list is private (its creator sees it) or public (everyone with the permission to use
//! smart lists sees it, each one with the books they may read). Every test builds the world of
//! [`VisibilityFixture`]: **ana** has limits (her hidden set H is `adult`, `unrated`, `removed` and
//! `hidden`) and may use smart lists, **ben** has no permissions, **zoe** was deleted, and the owner
//! sees everything but `removed`. What ana gets from a list, its rules included, must never hold
//! anything that comes from H, and nobody but a list's creator changes it (the server owner may
//! moderate the public lists of others).

use models::{
	entity::{smart_list, smart_list_view},
	shared::enums::UserPermission,
};
use sea_orm::prelude::*;
use serde_json::{json, Value};

use crate::common::{
	account::{CreateTestUser, TestUser},
	visibility::VisibilityFixture,
	TestApp,
};

const NOT_FOUND: &str = "Smart list not found";
const EITHER: &str = "A smart list is either private or public";

async fn world() -> (TestApp, VisibilityFixture) {
	let app = TestApp::new_with_default_user().await;
	let world = VisibilityFixture::build(&app).await;
	(app, world)
}

/// Another user who may use smart lists, without ana's limits
async fn friend_with_lists(app: &TestApp, username: &str) -> TestUser {
	CreateTestUser {
		username: username.to_string(),
		permissions: vec![UserPermission::AccessSmartList],
		..Default::default()
	}
	.insert(app)
	.await
}

/// What a list of lists asks for, as the lists page and the sidebar do
const LIST: &str = "id name description visibility canEdit filters creator { id username } meta { matchedBooks matchedSeries matchedLibraries } books(limit: 20) { id }";

const CREATE: &str = r#"
	mutation Create($input: SaveSmartListInput!) {
		createSmartList(input: $input) { id name visibility canEdit filters creator { id username } }
	}
"#;

const UPDATE: &str = r#"
	mutation Update($id: ID!, $input: SaveSmartListInput!) {
		updateSmartList(id: $id, input: $input) { id name visibility canEdit }
	}
"#;

const DELETE: &str = r#"
	mutation Delete($id: ID!) { deleteSmartList(id: $id) { id } }
"#;

const SET_VISIBILITY: &str = r#"
	mutation Visibility($ids: [ID!]!, $visibility: EntityVisibility!) {
		setSmartListsVisibility(ids: $ids, visibility: $visibility) { id name visibility canEdit }
	}
"#;

const CREATE_VIEW: &str = r#"
	mutation CreateView($input: SaveSmartListView!) {
		createSmartListView(input: $input) { id name }
	}
"#;

const ITEMS: &str = r#"
	query Items($id: ID!, $filter: MediaFilterInput, $grouping: SmartListGrouping) {
		smartListItems(id: $id, filter: $filter, grouping: $grouping) {
			__typename
			... on SmartListUngrouped { books { id } }
			... on SmartListGrouped { items { entity { __typename ... on Series { id } ... on Library { id } } books { id } } }
		}
	}
"#;

/// The response as it is, errors and all
async fn run(app: &TestApp, token: &str, query: &str, variables: Value) -> Value {
	app.execute_gql_with_token(query, Some(variables), token)
		.await
}

/// The data of a response that must not have failed
async fn ok(app: &TestApp, token: &str, query: &str, variables: Value) -> Value {
	let response = run(app, token, query, variables).await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"].clone()
}

/// What a refused request said
fn error_message(response: &Value) -> String {
	response["errors"][0]["message"]
		.as_str()
		.unwrap_or_else(|| panic!("an error in {response}"))
		.to_string()
}

fn input(name: &str, visibility: &str) -> Value {
	json!({
		"filters": [],
		"name": name,
		"joiner": "AND",
		"defaultGrouping": "BY_BOOKS",
		"visibility": visibility,
	})
}

/// The rules of a list that names the hidden library: it holds the hidden library's books and the
/// general book. For anyone who may not read the hidden library, only the general book is left
fn rules_naming_the_hidden_library(world: &VisibilityFixture) -> Value {
	json!([{
		"groups": [
			{ "media": { "libraryId": { "eq": world.hidden_library.id } } },
			{ "media": { "name": { "eq": world.general.name } } },
		],
		"joiner": "OR",
	}])
}

/// Makes a list as `token` and returns its ID
async fn create(
	app: &TestApp,
	token: &str,
	name: &str,
	visibility: &str,
	filters: Option<Value>,
) -> String {
	let mut list = input(name, visibility);
	if let Some(filters) = filters {
		list["filters"] = filters;
	}
	let data = ok(app, token, CREATE, json!({ "input": list })).await;
	data["createSmartList"]["id"]
		.as_str()
		.expect("an id")
		.to_string()
}

/// The lists as the sidebar and the lists page ask for them: `smartLists(input: {})`, which is
/// the user's own and everyone's public ones
async fn lists(app: &TestApp, token: &str) -> Vec<Value> {
	let query = format!("query Lists {{ smartLists(input: {{}}) {{ {LIST} }} }}");
	let data = ok(app, token, &query, json!({})).await;
	data["smartLists"]
		.as_array()
		.expect("a list of lists")
		.clone()
}

fn names(lists: &[Value]) -> Vec<String> {
	lists
		.iter()
		.map(|list| list["name"].as_str().expect("a name").to_string())
		.collect()
}

fn by_name<'a>(lists: &'a [Value], name: &str) -> &'a Value {
	lists
		.iter()
		.find(|list| list["name"] == name)
		.unwrap_or_else(|| panic!("no list named {name} in {lists:?}"))
}

async fn stored(app: &TestApp, id: &str) -> Option<smart_list::Model> {
	smart_list::Entity::find()
		.filter(smart_list::Column::Id.eq(id))
		.one(app.conn())
		.await
		.expect("should query the smart lists")
}

/// The books of a page of `smartListItems`, ungrouped or grouped
fn books_of(data: &Value) -> Vec<String> {
	let page = &data["smartListItems"];
	let books: Vec<&Value> = match page["__typename"].as_str() {
		Some("SmartListUngrouped") => page["books"].as_array().unwrap().iter().collect(),
		Some("SmartListGrouped") => page["items"]
			.as_array()
			.unwrap()
			.iter()
			.flat_map(|item| item["books"].as_array().unwrap())
			.collect(),
		other => panic!("not a page of a smart list: {other:?} in {data}"),
	};
	let mut ids = books
		.into_iter()
		.map(|book| book["id"].as_str().expect("an id").to_string())
		.collect::<Vec<_>>();
	ids.sort();
	ids
}

// ---------------------------------------------------------------------------------------
// What a viewer sees of a list
// ---------------------------------------------------------------------------------------

/// The lists page and the sidebar: a user's own lists and everyone's public ones, by name (capitals
/// and small letters together), the private lists of others left out, and the rules of a list only
/// for who made it
#[tokio::test]
async fn a_user_sees_her_own_lists_and_the_public_ones_by_name() {
	let (app, world) = world().await;
	let owner = world.owner_token.clone();

	create(
		&app,
		&owner,
		"Zebra shelf",
		"PUBLIC",
		Some(rules_naming_the_hidden_library(&world)),
	)
	.await;
	create(&app, &owner, "Mango shelf", "PUBLIC", None).await;
	create(&app, &owner, "Owner's secret", "PRIVATE", None).await;
	create(&app, &world.ana.token, "apple (mine)", "PRIVATE", None).await;

	let seen = lists(&app, &world.ana.token).await;
	assert_eq!(
		names(&seen),
		// "apple" comes before "Mango" and "Zebra" whatever the case of its first letter
		vec!["apple (mine)", "Mango shelf", "Zebra shelf"],
		"her own list and the two public ones, as one sorted list: {seen:?}"
	);

	// nothing in what she got comes from H, not even a rule that names the hidden library
	world.assert_none_hidden(&json!(seen));

	let zebra = by_name(&seen, "Zebra shelf");
	assert_eq!(zebra["canEdit"], false);
	assert_eq!(
		zebra["filters"], "[]",
		"the rules are for who made the list"
	);
	assert_eq!(zebra["creator"]["username"], "initial-server-admin");
	// the books she may read of a list that names a library she may not: the general one
	assert_eq!(zebra["meta"]["matchedBooks"], 1);
	assert_eq!(zebra["meta"]["matchedSeries"], 1);
	assert_eq!(zebra["meta"]["matchedLibraries"], 1);
	assert_eq!(
		zebra["books"],
		json!([{ "id": world.general.id }]),
		"{zebra:?}"
	);

	let mine = by_name(&seen, "apple (mine)");
	assert_eq!(mine["canEdit"], true);
	assert_eq!(
		mine["filters"], "[]",
		"her own list has no rules, and says so"
	);
	assert_eq!(mine["creator"]["username"], "ana");

	// the owner sees his own lists and the public ones, and the rules of what he can change; a
	// friend's private list stays hers, even from him
	let seen = lists(&app, &owner).await;
	assert_eq!(
		names(&seen),
		vec!["Mango shelf", "Owner's secret", "Zebra shelf"],
		"{seen:?}"
	);
	let zebra = by_name(&seen, "Zebra shelf");
	assert_eq!(zebra["canEdit"], true);
	assert!(
		zebra["filters"]
			.as_str()
			.unwrap()
			.contains(&world.hidden_library.id),
		"the owner gets the rules of his list: {zebra:?}"
	);
}

/// The default of `smartLists` is the user's own lists only
#[tokio::test]
async fn without_an_input_only_her_own_lists_are_listed() {
	let (app, world) = world().await;
	create(
		&app,
		&world.owner_token,
		"Public of the owner",
		"PUBLIC",
		None,
	)
	.await;
	create(&app, &world.ana.token, "Hers", "PRIVATE", None).await;

	let data = ok(
		&app,
		&world.ana.token,
		"query Mine { smartLists { name } }",
		json!({}),
	)
	.await;
	assert_eq!(data["smartLists"], json!([{ "name": "Hers" }]));
}

/// A public list, opened by id: the books she may read, counted and listed as such, whatever the
/// grouping and the filter; a private list of someone else is as good as none
#[tokio::test]
async fn a_public_list_holds_only_what_she_may_read_and_a_private_one_nothing() {
	let (app, world) = world().await;
	let public = create(
		&app,
		&world.owner_token,
		"Shelf",
		"PUBLIC",
		Some(rules_naming_the_hidden_library(&world)),
	)
	.await;
	let private = create(
		&app,
		&world.owner_token,
		"Secret",
		"PRIVATE",
		Some(rules_naming_the_hidden_library(&world)),
	)
	.await;
	let ana = &world.ana.token;

	let data = ok(
		&app,
		ana,
		"query ById($id: ID!) { smartListById(id: $id) { id name canEdit filters creator { username } } }",
		json!({ "id": public }),
	)
	.await;
	assert_eq!(data["smartListById"]["canEdit"], false);
	assert_eq!(data["smartListById"]["filters"], "[]");
	world.assert_none_hidden(&data);

	// every grouping, with and without a filter: exactly the general book, each once
	for grouping in [
		Value::Null,
		json!("BY_BOOKS"),
		json!("BY_SERIES"),
		json!("BY_LIBRARY"),
	] {
		for filter in [Value::Null, json!({ "name": { "contains": "book" } })] {
			let data = ok(
				&app,
				ana,
				ITEMS,
				json!({ "id": public, "grouping": grouping, "filter": filter }),
			)
			.await;
			assert_eq!(books_of(&data), vec![world.general.id.clone()], "{data}");
			world.assert_none_hidden(&data);
		}
	}

	let data = ok(
		&app,
		ana,
		"query Meta($id: ID!) { smartListMeta(id: $id) { matchedBooks matchedSeries matchedLibraries } }",
		json!({ "id": public }),
	)
	.await;
	assert_eq!(
		data["smartListMeta"],
		json!({ "matchedBooks": 1, "matchedSeries": 1, "matchedLibraries": 1 })
	);

	// the owner's private list is not hers to see: it answers as a list that doesn't exist
	let data = ok(
		&app,
		ana,
		"query ById($id: ID!) { smartListById(id: $id) { id } }",
		json!({ "id": private }),
	)
	.await;
	assert_eq!(data["smartListById"], Value::Null);
	let missing = Uuid::new_v4().to_string();
	for id in [&private, &missing] {
		let response = run(&app, ana, ITEMS, json!({ "id": id })).await;
		assert_eq!(error_message(&response), NOT_FOUND, "{response}");
		let response = run(
			&app,
			ana,
			"query Meta($id: ID!) { smartListMeta(id: $id) { matchedBooks } }",
			json!({ "id": id }),
		)
		.await;
		assert_eq!(error_message(&response), NOT_FOUND, "{response}");
	}

	// the owner sees all four books he may read through the very same list
	let data = ok(&app, &world.owner_token, ITEMS, json!({ "id": public })).await;
	assert_eq!(
		books_of(&data),
		{
			let mut expected = vec![world.general.id.clone(), world.hidden.id.clone()];
			expected.sort();
			expected
		},
		"the list holds the hidden library's book for who may read it"
	);
}

/// Who made a list that stays in the database when they go is no one's to show: the list of a
/// deleted or a locked user is out of every other user's sight until they are back
#[tokio::test]
async fn the_lists_of_a_deleted_or_locked_user_are_not_shown() {
	let (app, world) = world().await;
	let kim = friend_with_lists(&app, "kim").await;
	let lou = friend_with_lists(&app, "lou").await;
	let kims = create(&app, &kim.token, "Kim's shelf", "PUBLIC", None).await;
	create(&app, &lou.token, "Lou's shelf", "PUBLIC", None).await;
	assert_eq!(
		names(&lists(&app, &world.ana.token).await),
		vec!["Kim's shelf", "Lou's shelf"]
	);

	let data = ok(
		&app,
		&world.owner_token,
		r#"mutation Delete($id: ID!) { deleteUser(id: $id) { id } }"#,
		json!({ "id": kim.id }),
	)
	.await;
	assert_eq!(data["deleteUser"]["id"], kim.id);
	ok(
		&app,
		&world.owner_token,
		r#"mutation Lock($id: ID!) { updateUserLockStatus(id: $id, lock: true) { id } }"#,
		json!({ "id": lou.id }),
	)
	.await;

	assert_eq!(
		names(&lists(&app, &world.ana.token).await),
		Vec::<String>::new()
	);
	let data = ok(
		&app,
		&world.ana.token,
		"query ById($id: ID!) { smartListById(id: $id) { id } }",
		json!({ "id": kims }),
	)
	.await;
	assert_eq!(data["smartListById"], Value::Null);
}

// ---------------------------------------------------------------------------------------
// Who changes a list
// ---------------------------------------------------------------------------------------

/// A public list stays its maker's: ana can't rewrite it, delete it, or give it a saved view, and
/// nothing about the list changes; the server owner can moderate a public list of hers
#[tokio::test]
async fn only_the_maker_changes_a_list_and_it_stays_hers() {
	let (app, world) = world().await;
	let public = create(&app, &world.owner_token, "Owner's shelf", "PUBLIC", None).await;
	let ana = &world.ana.token;

	let response = run(
		&app,
		ana,
		UPDATE,
		json!({ "id": public, "input": input("Mine now", "PUBLIC") }),
	)
	.await;
	assert_eq!(error_message(&response), NOT_FOUND, "{response}");
	let response = run(&app, ana, DELETE, json!({ "id": public })).await;
	assert_eq!(error_message(&response), NOT_FOUND, "{response}");
	let response = run(
		&app,
		ana,
		CREATE_VIEW,
		json!({ "input": {
			"name": "Her view",
			"listId": public,
			"bookColumns": [],
			"groupColumns": [],
			"bookSorting": [],
			"groupSorting": [],
		}}),
	)
	.await;
	assert_eq!(error_message(&response), NOT_FOUND, "{response}");

	let list = stored(&app, &public).await.expect("still there");
	assert_eq!(list.name, "Owner's shelf");
	assert_eq!(list.creator_id, app_owner_id(&app).await);
	let views = smart_list_view::Entity::find_by_list_id(&public)
		.all(app.conn())
		.await
		.expect("should query the views");
	assert!(views.is_empty(), "{views:?}");

	// hers, she changes, and it stays hers whoever edits it
	let hers = create(&app, ana, "Hers", "PUBLIC", None).await;
	let data = ok(
		&app,
		ana,
		UPDATE,
		json!({ "id": hers, "input": input("Hers, renamed", "PUBLIC") }),
	)
	.await;
	assert_eq!(data["updateSmartList"]["name"], "Hers, renamed");
	let data = ok(
		&app,
		&world.owner_token,
		UPDATE,
		json!({ "id": hers, "input": input("Moderated", "PRIVATE") }),
	)
	.await;
	assert_eq!(data["updateSmartList"]["visibility"], "PRIVATE");
	let list = stored(&app, &hers).await.expect("still there");
	assert_eq!(list.name, "Moderated");
	assert_eq!(list.creator_id, world.ana.id, "it is still hers");
}

async fn app_owner_id(app: &TestApp) -> String {
	let data = ok(
		app,
		&app.access_token.read().await.clone().expect("a token"),
		"query Me { me { id } }",
		json!({}),
	)
	.await;
	data["me"]["id"].as_str().expect("an id").to_string()
}

/// A list is private or public. `SHARED` needs per-person rules nothing writes, so it is refused
/// when a list is made or changed, also for the server owner
#[tokio::test]
async fn a_list_cannot_be_shared_with_some() {
	let (app, world) = world().await;
	for token in [&world.ana.token, &world.owner_token] {
		let response = run(
			&app,
			token,
			CREATE,
			json!({ "input": input("Some", "SHARED") }),
		)
		.await;
		assert_eq!(error_message(&response), EITHER, "{response}");

		let id = create(&app, token, "Plain", "PRIVATE", None).await;
		let response = run(
			&app,
			token,
			UPDATE,
			json!({ "id": id, "input": input("Plain", "SHARED") }),
		)
		.await;
		assert_eq!(error_message(&response), EITHER, "{response}");
		assert_eq!(stored(&app, &id).await.unwrap().name, "Plain");
	}
}

// ---------------------------------------------------------------------------------------
// Share all my lists
// ---------------------------------------------------------------------------------------

/// `setSmartListsVisibility` makes a user's own lists public or private in one go, and does nothing
/// but that
#[tokio::test]
async fn a_user_shares_and_unshares_her_lists_in_one_go() {
	let (app, world) = world().await;
	let ana = &world.ana.token;
	let ben = friend_with_lists(&app, "ben2").await;
	let first = create(&app, ana, "First", "PRIVATE", None).await;
	let second = create(&app, ana, "Second", "PUBLIC", None).await;
	assert_eq!(names(&lists(&app, &ben.token).await), vec!["Second"]);

	let data = ok(
		&app,
		ana,
		SET_VISIBILITY,
		json!({ "ids": [second, first, first], "visibility": "PUBLIC" }),
	)
	.await;
	let changed = data["setSmartListsVisibility"].as_array().unwrap();
	assert_eq!(
		changed
			.iter()
			.map(|list| list["id"].as_str().unwrap())
			.collect::<Vec<_>>(),
		vec![second.as_str(), first.as_str()],
		"each list once, in the order asked for: {changed:?}"
	);
	assert!(changed.iter().all(|list| list["visibility"] == "PUBLIC"));
	assert!(changed.iter().all(|list| list["canEdit"] == true));
	assert_eq!(
		names(&lists(&app, &ben.token).await),
		vec!["First", "Second"],
		"now he sees both"
	);

	let data = ok(
		&app,
		ana,
		SET_VISIBILITY,
		json!({ "ids": [first, second], "visibility": "PRIVATE" }),
	)
	.await;
	assert!(data["setSmartListsVisibility"]
		.as_array()
		.unwrap()
		.iter()
		.all(|list| list["visibility"] == "PRIVATE"));
	assert_eq!(names(&lists(&app, &ben.token).await), Vec::<String>::new());

	// nothing to change is nothing
	let data = ok(
		&app,
		ana,
		SET_VISIBILITY,
		json!({ "ids": [], "visibility": "PUBLIC" }),
	)
	.await;
	assert_eq!(data["setSmartListsVisibility"], json!([]));

	// SHARED is not offered
	let response = run(
		&app,
		ana,
		SET_VISIBILITY,
		json!({ "ids": [first], "visibility": "SHARED" }),
	)
	.await;
	assert_eq!(error_message(&response), EITHER, "{response}");
	assert_eq!(
		stored(&app, &first).await.unwrap().visibility,
		models::shared::enums::EntityVisibility::Private
	);
}

/// Only the maker of a list changes who sees it: an ID that is somebody else's (public or private)
/// or doesn't exist fails the whole request with the same answer, and no list changes, whoever asks,
/// the server owner included
#[tokio::test]
async fn nobody_makes_the_lists_of_somebody_else_public_or_private() {
	let (app, world) = world().await;
	let ana = &world.ana.token;
	let owner = &world.owner_token;
	let hers = create(&app, ana, "Hers", "PRIVATE", None).await;
	let owners_public = create(&app, owner, "Owner's public", "PUBLIC", None).await;
	let owners_private = create(&app, owner, "Owner's private", "PRIVATE", None).await;
	let missing = Uuid::new_v4().to_string();

	for other in [&owners_public, &owners_private, &missing] {
		let response = run(
			&app,
			ana,
			SET_VISIBILITY,
			json!({ "ids": [hers, other], "visibility": "PUBLIC" }),
		)
		.await;
		assert_eq!(error_message(&response), NOT_FOUND, "{response}");
		assert_eq!(
			stored(&app, &hers).await.unwrap().visibility,
			models::shared::enums::EntityVisibility::Private,
			"her own list changes with the others or not at all"
		);
	}

	// the owner moderates a public list by changing it, not by this
	let response = run(
		&app,
		owner,
		SET_VISIBILITY,
		json!({ "ids": [hers], "visibility": "PUBLIC" }),
	)
	.await;
	assert_eq!(error_message(&response), NOT_FOUND, "{response}");
	assert_eq!(
		stored(&app, &hers).await.unwrap().visibility,
		models::shared::enums::EntityVisibility::Private
	);

	// without the permission to use smart lists there is nothing to change
	let response = run(
		&app,
		&world.ben.token,
		SET_VISIBILITY,
		json!({ "ids": [owners_public], "visibility": "PRIVATE" }),
	)
	.await;
	assert!(response.get("errors").is_some(), "{response}");
	assert_eq!(
		stored(&app, &owners_public).await.unwrap().visibility,
		models::shared::enums::EntityVisibility::Public
	);
}

// ---------------------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------------------

/// Everyone but the server owner has at most 20 lists, with a name of at most 200 characters, a
/// description of at most 2,000 and rules of at most 128 KB, so that nobody fills the database
#[tokio::test]
async fn a_user_has_limits_the_owner_does_not() {
	let (app, world) = world().await;
	let ana = &world.ana.token;
	let owner = &world.owner_token;

	for number in 0..20 {
		create(&app, ana, &format!("List {number}"), "PRIVATE", None).await;
	}
	let response = run(
		&app,
		ana,
		CREATE,
		json!({ "input": input("One more", "PRIVATE") }),
	)
	.await;
	assert_eq!(
		error_message(&response),
		"You can have at most 20 smart lists",
		"{response}"
	);
	// a list that is gone makes room
	let data = ok(&app, ana, "query Mine { smartLists { id } }", json!({})).await;
	assert_eq!(data["smartLists"].as_array().unwrap().len(), 20);
	let first = data["smartLists"][0]["id"].as_str().unwrap().to_string();
	ok(&app, ana, DELETE, json!({ "id": first })).await;
	create(&app, ana, "One more", "PRIVATE", None).await;

	// the owner has no limit on the number
	for number in 0..22 {
		create(&app, owner, &format!("Owner's {number}"), "PRIVATE", None).await;
	}

	// a name and a description have a length
	let mut long_name = input(&"n".repeat(201), "PRIVATE");
	let response = run(
		&app,
		ana,
		UPDATE,
		json!({ "id": first, "input": long_name.clone() }),
	)
	.await;
	assert!(response.get("errors").is_some(), "{response}");
	let response = run(&app, ana, CREATE, json!({ "input": long_name.clone() })).await;
	assert_eq!(
		error_message(&response),
		"A smart list's name can have at most 200 characters"
	);
	long_name["name"] = json!("n".repeat(200));
	let mut long_description = long_name;
	long_description["description"] = json!("d".repeat(2_001));
	let response = run(
		&app,
		ana,
		CREATE,
		json!({ "input": long_description.clone() }),
	)
	.await;
	assert_eq!(
		error_message(&response),
		"A smart list's description can have at most 2000 characters"
	);
	// the owner may
	ok(
		&app,
		owner,
		CREATE,
		json!({ "input": {
			"filters": [],
			"name": "n".repeat(300),
			"description": "d".repeat(5_000),
			"joiner": "AND",
			"defaultGrouping": "BY_BOOKS",
			"visibility": "PRIVATE",
		}}),
	)
	.await;

	// rules have a size: a list of 4,000 paths is about 200 KB
	let paths = (0..4_000)
		.map(|number| {
			format!("/library/some/author/a-book-with-a-longish-name-{number:05}.epub")
		})
		.collect::<Vec<_>>();
	let rules = json!([{ "groups": [{ "media": { "path": { "anyOf": paths } } }], "joiner": "OR" }]);
	let mut heavy = input("Heavy", "PRIVATE");
	heavy["filters"] = rules;
	let response = run(&app, ana, CREATE, json!({ "input": heavy.clone() })).await;
	assert_eq!(
		error_message(&response),
		"A smart list's rules can take at most 128 KB",
		"{response}"
	);
	ok(&app, owner, CREATE, json!({ "input": heavy })).await;
}
