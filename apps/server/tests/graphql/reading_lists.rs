//! Reading lists that can be public (package A1).
//!
//! A list is private (its creator sees it) or public (everyone on the server sees it, each with the
//! books they may read). Every test builds the world of [`VisibilityFixture`]: **ana** has limits
//! (her hidden set H is `adult`, `unrated`, `removed` and `hidden`), **ben** has none, **zoe** was
//! deleted, and the owner sees everything but `removed`. What ana gets from a list must never hold
//! anything that comes from H, and nobody but a list's creator changes it (the server owner may only
//! take a public list of someone else back to private, or delete it).

use models::entity::{reading_list, reading_list_item, user};
use sea_orm::{prelude::*, ActiveModelTrait, Set};
use serde_json::{json, Value};

use crate::common::{
	visibility::{ids, sorted, VisibilityFixture},
	TestApp,
};

const OWNER: &str = "initial-server-admin";
const NOT_FOUND: &str = "Reading list not found";
const CANT_CHANGE: &str = "You can't change this list";
const CANT_CREATE: &str = "Couldn't create the reading list";
const BAD_ID: &str =
	"A reading list's ID can have at most 64 characters: letters, digits, _ and -";

async fn world() -> (TestApp, VisibilityFixture) {
	let app = TestApp::new_with_default_user().await;
	let world = VisibilityFixture::build(&app).await;
	(app, world)
}

/// What a list of lists asks for: a few of each list's books, as the web app does (every book of every
/// list at once would be refused as too complex)
const LIST: &str = "id name description visibility canEdit bookCount creator { id username } books(limit: 20) { id }";

const CREATE: &str = r#"
	mutation Create($input: ReadingListInput!) {
		createReadingList(input: $input) { id name description visibility canEdit bookCount creator { id username } books { id } }
	}
"#;

const UPDATE: &str = r#"
	mutation Update($id: ID!, $input: UpdateReadingListInput!) {
		updateReadingList(id: $id, input: $input) { id name description visibility canEdit bookCount books { id } }
	}
"#;

const ADD: &str = r#"
	mutation Add($id: ID!, $mediaIds: [ID!]!) {
		addBooksToReadingList(id: $id, mediaIds: $mediaIds) { id bookCount books { id } }
	}
"#;

const REMOVE: &str = r#"
	mutation Remove($id: ID!, $mediaIds: [ID!]!) {
		removeBooksFromReadingList(id: $id, mediaIds: $mediaIds) { id bookCount }
	}
"#;

const DELETE: &str = r#"
	mutation Delete($id: String!) { deleteReadingList(id: $id) { id } }
"#;

const BY_ID: &str = r#"
	query ById($id: ID!) { readingListById(id: $id) { id name visibility canEdit bookCount creator { username } books { id } } }
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

async fn lists(app: &TestApp, token: &str, scope: &str) -> Vec<Value> {
	let query = format!(
		"query Lists($scope: ReadingListScope!) {{ readingLists(scope: $scope, pagination: {{ none: {{ unpaginated: true }} }}) {{ nodes {{ {LIST} }} }} }}"
	);
	let data = ok(app, token, &query, json!({ "scope": scope })).await;
	data["readingLists"]["nodes"]
		.as_array()
		.expect("a list of lists")
		.clone()
}

fn list_ids(lists: &[Value]) -> Vec<String> {
	sorted(
		lists
			.iter()
			.map(|list| list["id"].as_str().expect("an id").to_string())
			.collect(),
	)
}

/// Makes a list as `token` and returns its ID
async fn create(
	app: &TestApp,
	token: &str,
	name: &str,
	visibility: &str,
	media: &[&String],
) -> String {
	let data = ok(
		app,
		token,
		CREATE,
		json!({ "input": { "name": name, "visibility": visibility, "mediaIds": media } }),
	)
	.await;
	data["createReadingList"]["id"]
		.as_str()
		.expect("the new list's id")
		.to_string()
}

/// A list's books, in its order, as one user sees them
async fn books_of(app: &TestApp, token: &str, list_id: &str) -> Vec<String> {
	let data = ok(app, token, BY_ID, json!({ "id": list_id })).await;
	data["readingListById"]["books"]
		.as_array()
		.expect("books")
		.iter()
		.map(|book| book["id"].as_str().expect("an id").to_string())
		.collect()
}

/// What the list's row holds, whoever looks
async fn stored(
	app: &TestApp,
	list_id: &str,
) -> Option<(reading_list::Model, Vec<String>)> {
	let model = reading_list::Entity::find_by_id(list_id.to_string())
		.one(app.conn())
		.await
		.expect("should look for the list")?;
	let mut items = reading_list_item::Entity::find()
		.filter(reading_list_item::Column::ReadingListId.eq(list_id))
		.all(app.conn())
		.await
		.expect("should read the items");
	items.sort_by_key(|item| item.display_order);
	Some((model, items.into_iter().map(|item| item.media_id).collect()))
}

#[tokio::test]
async fn a_public_list_shows_each_viewer_the_books_they_may_read() {
	let (app, world) = world().await;
	let everything = world
		.all_books()
		.iter()
		.map(|book| &book.id)
		.collect::<Vec<_>>();
	let list_id = create(&app, &world.owner_token, "Recs", "PUBLIC", &everything).await;

	// the owner's list holds the four books that exist: the removed one is out for everybody
	let visible_to_all_but_ana = vec![
		world.general.id.clone(),
		world.adult.id.clone(),
		world.unrated.id.clone(),
		world.hidden.id.clone(),
	];
	assert_eq!(
		sorted(books_of(&app, &world.owner_token, &list_id).await),
		sorted(visible_to_all_but_ana.clone())
	);
	assert_eq!(
		sorted(books_of(&app, &world.ben.token, &list_id).await),
		sorted(visible_to_all_but_ana.clone())
	);

	// ana gets the list and the one book of it she may read: nothing of H, not even a count
	let response = run(&app, &world.ana.token, BY_ID, json!({ "id": list_id })).await;
	assert!(response.get("errors").is_none(), "{response}");
	let list = &response["data"]["readingListById"];
	assert_eq!(list["bookCount"], 1);
	assert_eq!(ids(&list["books"]), vec![world.general.id.clone()]);
	assert_eq!(list["canEdit"], false);
	world.assert_none_hidden(&response);

	for scope in ["ALL", "OTHERS"] {
		let found = lists(&app, &world.ana.token, scope).await;
		assert_eq!(list_ids(&found), vec![list_id.clone()], "{scope}");
		assert_eq!(found[0]["bookCount"], 1);
		world.assert_none_hidden(&Value::Array(found));
	}
	// and the list is not ana's own
	assert!(lists(&app, &world.ana.token, "MINE").await.is_empty());
}

#[tokio::test]
async fn a_private_list_is_the_same_as_one_that_does_not_exist() {
	let (app, world) = world().await;
	let private = create(
		&app,
		&world.owner_token,
		"Secret",
		"PRIVATE",
		&[&world.general.id],
	)
	.await;

	for token in [&world.ana.token, &world.ben.token] {
		let unknown = run(&app, token, BY_ID, json!({ "id": "no-such-list" })).await;
		let hidden = run(&app, token, BY_ID, json!({ "id": private })).await;
		assert_eq!(error_message(&unknown), NOT_FOUND);
		// not a word of difference, so that it can't tell which private lists there are
		assert_eq!(unknown["errors"], hidden["errors"], "{hidden}");

		for scope in ["ALL", "OTHERS", "MINE"] {
			assert!(lists(&app, token, scope).await.is_empty(), "{scope}");
		}
	}

	// the owner has it as their own
	assert_eq!(
		list_ids(&lists(&app, &world.owner_token, "MINE").await),
		vec![private.clone()]
	);
	assert!(lists(&app, &world.owner_token, "OTHERS").await.is_empty());
}

#[tokio::test]
async fn the_lists_of_people_who_are_gone_are_not_there() {
	let (app, world) = world().await;
	let public_list = |creator: &str| reading_list::ActiveModel {
		id: Set(Uuid::new_v4().to_string()),
		name: Set("Gone".to_string()),
		description: Set(None),
		updated_at: Set(chrono::Utc::now().into()),
		visibility: Set("PUBLIC".to_string()),
		ordering: Set("MANUAL".to_string()),
		creating_user_id: Set(creator.to_string()),
		..Default::default()
	};

	// zoe was deleted: her list is still in the database and nobody sees it
	let zoes = public_list(&world.zoe.id).insert(app.conn()).await.unwrap();
	assert!(lists(&app, &world.ben.token, "ALL").await.is_empty());
	let response = run(&app, &world.ben.token, BY_ID, json!({ "id": zoes.id })).await;
	assert_eq!(error_message(&response), NOT_FOUND);

	// ben's list is seen until his account is locked, and again when it's unlocked
	let bens = create(
		&app,
		&world.ben.token,
		"Ben's",
		"PUBLIC",
		&[&world.general.id],
	)
	.await;
	assert_eq!(
		list_ids(&lists(&app, &world.ana.token, "OTHERS").await),
		vec![bens.clone()]
	);
	let set_locked = |locked: bool| {
		let app = &app;
		let ben_id = world.ben.id.clone();
		async move {
			let model = user::Entity::find_by_id(ben_id)
				.one(app.conn())
				.await
				.unwrap()
				.unwrap();
			user::ActiveModel {
				is_locked: Set(locked),
				..model.into()
			}
			.update(app.conn())
			.await
			.unwrap();
		}
	};
	set_locked(true).await;
	assert!(lists(&app, &world.ana.token, "OTHERS").await.is_empty());
	assert!(lists(&app, &world.owner_token, "OTHERS").await.is_empty());
	let response = run(&app, &world.ana.token, BY_ID, json!({ "id": bens })).await;
	assert_eq!(error_message(&response), NOT_FOUND);
	set_locked(false).await;
	assert_eq!(
		list_ids(&lists(&app, &world.ana.token, "OTHERS").await),
		vec![bens]
	);
}

#[tokio::test]
async fn books_ana_may_not_read_never_get_into_her_lists() {
	let (app, world) = world().await;
	let everything = world
		.all_books()
		.iter()
		.map(|book| &book.id)
		.collect::<Vec<_>>();

	// a list made with all five books holds the one she may read, and only that is stored
	let mine = create(&app, &world.ana.token, "Mine", "PRIVATE", &everything).await;
	assert_eq!(
		books_of(&app, &world.ana.token, &mine).await,
		vec![world.general.id.clone()]
	);
	let (_, items) = stored(&app, &mine).await.expect("the list");
	assert_eq!(items, vec![world.general.id.clone()]);

	// so does adding them
	let data = ok(
		&app,
		&world.ana.token,
		ADD,
		json!({ "id": mine, "mediaIds": everything }),
	)
	.await;
	assert_eq!(data["addBooksToReadingList"]["bookCount"], 1);
	let (_, items) = stored(&app, &mine).await.expect("the list");
	assert_eq!(items, vec![world.general.id.clone()]);

	// *Save a copy* of the owner's public list, with the ids the owner sees
	let owners = create(&app, &world.owner_token, "Recs", "PUBLIC", &everything).await;
	let owners_books = books_of(&app, &world.owner_token, &owners).await;
	let data = ok(
		&app,
		&world.ana.token,
		CREATE,
		json!({ "input": { "name": "Copy of Recs", "mediaIds": owners_books } }),
	)
	.await;
	assert_eq!(data["createReadingList"]["bookCount"], 1);
	assert_eq!(
		ids(&data["createReadingList"]["books"]),
		vec![world.general.id.clone()]
	);
	world.assert_none_hidden(&data);
	let copy = data["createReadingList"]["id"].as_str().unwrap();
	let (_, items) = stored(&app, copy).await.expect("the copy");
	assert_eq!(items, vec![world.general.id.clone()]);
}

#[tokio::test]
async fn nobody_but_the_creator_changes_a_list() {
	let (app, world) = world().await;
	let public = create(
		&app,
		&world.ana.token,
		"Mine",
		"PUBLIC",
		&[&world.general.id],
	)
	.await;
	let private = create(
		&app,
		&world.ana.token,
		"Secret",
		"PRIVATE",
		&[&world.general.id],
	)
	.await;
	let before_public = stored(&app, &public).await.unwrap();
	let before_private = stored(&app, &private).await.unwrap();
	let general = vec![world.general.id.clone()];

	let attempts = |id: &str| {
		vec![
			(
				UPDATE,
				json!({ "id": id, "input": { "name": "Hijacked", "description": "x", "visibility": "PRIVATE" } }),
			),
			(
				ADD,
				json!({ "id": id, "mediaIds": [world.general.id.clone()] }),
			),
			(
				REMOVE,
				json!({ "id": id, "mediaIds": [world.general.id.clone()] }),
			),
			(DELETE, json!({ "id": id })),
		]
	};

	// ben sees the public list and is told he can't change it; the private one isn't there for him
	for (query, variables) in attempts(&public) {
		let response = run(&app, &world.ben.token, query, variables).await;
		assert_eq!(error_message(&response), CANT_CHANGE, "{query}");
	}
	let unknown = run(
		&app,
		&world.ben.token,
		DELETE,
		json!({ "id": "no-such-list" }),
	)
	.await;
	for (query, variables) in attempts(&private) {
		let response = run(&app, &world.ben.token, query, variables).await;
		assert_eq!(error_message(&response), NOT_FOUND, "{query}");
	}
	assert_eq!(error_message(&unknown), NOT_FOUND);

	// the server owner may only take a public list back to private or delete it: not change it
	for (query, variables) in [
		(
			UPDATE,
			json!({ "id": public, "input": { "name": "Hijacked", "description": "x", "visibility": "PUBLIC" } }),
		),
		(
			UPDATE,
			json!({ "id": public, "input": { "name": "Hijacked" } }),
		),
		(
			ADD,
			json!({ "id": public, "mediaIds": [world.adult.id.clone()] }),
		),
		(
			REMOVE,
			json!({ "id": public, "mediaIds": [world.general.id.clone()] }),
		),
	] {
		let response = run(&app, &world.owner_token, query, variables).await;
		assert_eq!(error_message(&response), CANT_CHANGE, "{query}");
	}
	for (query, variables) in attempts(&private) {
		let response = run(&app, &world.owner_token, query, variables).await;
		assert_eq!(error_message(&response), NOT_FOUND, "{query}");
	}

	// nothing changed
	assert_eq!(stored(&app, &public).await.unwrap(), before_public);
	assert_eq!(stored(&app, &private).await.unwrap(), before_private);
	assert_eq!(before_public.1, general);
}

#[tokio::test]
async fn the_server_owner_can_unpublish_or_delete_a_public_list() {
	let (app, world) = world().await;
	let public = create(
		&app,
		&world.ana.token,
		"Mine",
		"PUBLIC",
		&[&world.general.id],
	)
	.await;
	let other = create(
		&app,
		&world.ana.token,
		"Other",
		"PUBLIC",
		&[&world.general.id],
	)
	.await;
	let before = stored(&app, &public).await.unwrap();

	// unpublished: private again, and as its creator left it
	let data = ok(
		&app,
		&world.owner_token,
		UPDATE,
		json!({ "id": public, "input": { "name": "Hijacked", "description": "x", "visibility": "PRIVATE" } }),
	)
	.await;
	assert_eq!(data["updateReadingList"]["visibility"], "PRIVATE");
	assert_eq!(data["updateReadingList"]["name"], "Mine");
	assert_eq!(data["updateReadingList"]["canEdit"], false);
	let (after, items) = stored(&app, &public).await.unwrap();
	assert_eq!(after.visibility, "PRIVATE");
	assert_eq!(after.name, before.0.name);
	assert_eq!(after.description, before.0.description);
	assert_eq!(after.updated_at, before.0.updated_at);
	assert_eq!(items, before.1);

	// ben no longer sees it; ana has it as her own, private
	let gone = run(&app, &world.ben.token, BY_ID, json!({ "id": public })).await;
	assert_eq!(error_message(&gone), NOT_FOUND);
	let data = ok(&app, &world.ana.token, BY_ID, json!({ "id": public })).await;
	assert_eq!(data["readingListById"]["visibility"], "PRIVATE");
	assert_eq!(data["readingListById"]["canEdit"], true);

	// deleting a public list of someone else takes its entries along
	ok(&app, &world.owner_token, DELETE, json!({ "id": other })).await;
	assert!(stored(&app, &other).await.is_none());
	// and a private one is not theirs to delete (it is not there for them)
	let refused = run(&app, &world.owner_token, DELETE, json!({ "id": public })).await;
	assert_eq!(error_message(&refused), NOT_FOUND);
	assert!(stored(&app, &public).await.is_some());
}

#[tokio::test]
async fn the_creator_changes_their_own_list() {
	let (app, world) = world().await;
	let id = create(
		&app,
		&world.ana.token,
		"Mine",
		"PRIVATE",
		&[&world.general.id],
	)
	.await;

	let data = ok(
		&app,
		&world.ana.token,
		UPDATE,
		json!({ "id": id, "input": { "name": "  Renamed ", "description": "About it", "visibility": "PUBLIC" } }),
	)
	.await;
	assert_eq!(data["updateReadingList"]["name"], "Renamed");
	assert_eq!(data["updateReadingList"]["description"], "About it");
	assert_eq!(data["updateReadingList"]["visibility"], "PUBLIC");
	// and back, without touching the rest when the visibility is left out
	let data = ok(
		&app,
		&world.ana.token,
		UPDATE,
		json!({ "id": id, "input": { "name": "Renamed" } }),
	)
	.await;
	assert_eq!(data["updateReadingList"]["visibility"], "PUBLIC");

	ok(
		&app,
		&world.ana.token,
		REMOVE,
		json!({ "id": id, "mediaIds": [world.general.id.clone()] }),
	)
	.await;
	assert!(stored(&app, &id).await.unwrap().1.is_empty());
	ok(&app, &world.ana.token, DELETE, json!({ "id": id })).await;
	assert!(stored(&app, &id).await.is_none());
}

#[tokio::test]
async fn shared_is_not_offered() {
	let (app, world) = world().await;

	let response = run(
		&app,
		&world.ana.token,
		CREATE,
		json!({ "input": { "name": "Shared", "visibility": "SHARED" } }),
	)
	.await;
	assert_eq!(
		error_message(&response),
		"A reading list is either private or public"
	);

	let id = create(&app, &world.ana.token, "Mine", "PRIVATE", &[]).await;
	let response = run(
		&app,
		&world.ana.token,
		UPDATE,
		json!({ "id": id, "input": { "name": "Mine", "visibility": "SHARED" } }),
	)
	.await;
	assert_eq!(
		error_message(&response),
		"A reading list is either private or public"
	);
	assert_eq!(stored(&app, &id).await.unwrap().0.visibility, "PRIVATE");
}

#[tokio::test]
async fn friends_have_limits_and_the_server_owner_has_none() {
	let (app, world) = world().await;

	// the 101st list
	for number in 0..100 {
		create(
			&app,
			&world.ana.token,
			&format!("List {number}"),
			"PRIVATE",
			&[],
		)
		.await;
	}
	let response = run(
		&app,
		&world.ana.token,
		CREATE,
		json!({ "input": { "name": "One too many" } }),
	)
	.await;
	assert_eq!(
		error_message(&response),
		"You can have at most 100 reading lists"
	);
	// lists are counted per person
	create(&app, &world.ben.token, "Ben's first", "PRIVATE", &[]).await;

	// a name of 201 characters, a description of 2,001, and more than 2,000 books at once
	let response = run(
		&app,
		&world.ben.token,
		CREATE,
		json!({ "input": { "name": "é".repeat(201) } }),
	)
	.await;
	assert_eq!(
		error_message(&response),
		"A reading list's name can have at most 200 characters"
	);
	let response = run(
		&app,
		&world.ben.token,
		CREATE,
		json!({ "input": { "name": "Fine", "description": "d".repeat(2_001) } }),
	)
	.await;
	assert_eq!(
		error_message(&response),
		"A reading list's description can have at most 2000 characters"
	);
	let crowd = vec![world.general.id.clone(); 2_001];
	let response = run(
		&app,
		&world.ben.token,
		CREATE,
		json!({ "input": { "name": "Crowd", "mediaIds": crowd } }),
	)
	.await;
	assert_eq!(
		error_message(&response),
		"A reading list can hold at most 2000 books"
	);
	let id = create(&app, &world.ben.token, "Short", "PRIVATE", &[]).await;
	let response = run(
		&app,
		&world.ben.token,
		ADD,
		json!({ "id": id, "mediaIds": vec![world.general.id.clone(); 2_001] }),
	)
	.await;
	assert_eq!(
		error_message(&response),
		"A reading list can hold at most 2000 books"
	);

	// the server owner isn't held to any of it
	let data = ok(
		&app,
		&world.owner_token,
		CREATE,
		json!({ "input": { "name": "é".repeat(201), "description": "d".repeat(2_001) } }),
	)
	.await;
	assert_eq!(data["createReadingList"]["name"], "é".repeat(201));
}

#[tokio::test]
async fn an_id_that_is_taken_gives_the_same_error_whether_or_not_the_list_can_be_seen() {
	let (app, world) = world().await;
	let named = |name: &str, id: &str| json!({ "input": { "id": id, "name": name, "visibility": "PUBLIC" } });

	ok(
		&app,
		&world.ben.token,
		CREATE,
		named("Ben's", "taken-by-a-public-list"),
	)
	.await;
	let private = create(&app, &world.owner_token, "Secret", "PRIVATE", &[]).await;

	let public = run(
		&app,
		&world.ana.token,
		CREATE,
		named("Ana's", "taken-by-a-public-list"),
	)
	.await;
	let hidden = run(&app, &world.ana.token, CREATE, named("Ana's", &private)).await;
	assert_eq!(error_message(&public), CANT_CREATE);
	assert_eq!(public["errors"], hidden["errors"]);

	// an ID that is free is hers to choose
	let data = ok(&app, &world.ana.token, CREATE, named("Ana's", "my-own-id")).await;
	assert_eq!(data["createReadingList"]["id"], "my-own-id");
	// and nothing was written for the refused ones
	assert_eq!(stored(&app, &private).await.unwrap().0.name, "Secret");
	assert_eq!(
		stored(&app, "taken-by-a-public-list").await.unwrap().0.name,
		"Ben's"
	);
}

#[tokio::test]
async fn an_id_that_would_break_the_lists_address_is_refused_and_nothing_is_stored() {
	let (app, world) = world().await;
	let named = |id: &str| json!({ "input": { "id": id, "name": "Mine", "visibility": "PUBLIC" } });
	let too_long = "a".repeat(65);
	let before = reading_list::Entity::find()
		.count(app.conn())
		.await
		.unwrap();

	// the same for someone with limits and for the server owner, who has none
	for token in [&world.ana.token, &world.owner_token] {
		for id in [
			"a/b",
			"x?y",
			"#",
			"has a space",
			"..",
			"é",
			too_long.as_str(),
		] {
			let response = run(&app, token, CREATE, named(id)).await;
			assert_eq!(error_message(&response), BAD_ID, "{id:?}");
			assert!(stored(&app, id).await.is_none(), "{id:?} was stored");
		}
	}
	assert_eq!(
		reading_list::Entity::find()
			.count(app.conn())
			.await
			.unwrap(),
		before,
		"a refused request wrote a list"
	);

	// the check comes before any lookup: an ID that is bad says so whether or not the lists around exist
	let private = create(&app, &world.owner_token, "Secret", "PRIVATE", &[]).await;
	let with_hidden_id_inside = run(
		&app,
		&world.ana.token,
		CREATE,
		named(&format!("{private}/x")),
	)
	.await;
	assert_eq!(error_message(&with_hidden_id_inside), BAD_ID);

	// the longest ID, a UUID and the usual names are hers to choose, and they open from their address
	let longest = "a".repeat(64);
	let uuid = Uuid::new_v4().to_string();
	for id in [longest.as_str(), uuid.as_str(), "Ana_s-list-1"] {
		let data = ok(&app, &world.ana.token, CREATE, named(id)).await;
		assert_eq!(data["createReadingList"]["id"], id);
		let found = ok(&app, &world.ben.token, BY_ID, json!({ "id": id })).await;
		assert_eq!(found["readingListById"]["id"], id);
	}
}

#[tokio::test]
async fn a_list_is_read_page_by_page() {
	let (app, world) = world().await;
	let mut books = vec![world.general.id.clone()];
	for number in 2..=7 {
		let book = crate::common::visibility::add_book(
			&app,
			&world.open_series,
			&format!("Extra book {number}"),
			crate::common::visibility::Meta {
				age: Some(0),
				number: Some(f64::from(number)),
				..Default::default()
			},
		)
		.await;
		books.push(book.id);
	}
	let list_id = create(
		&app,
		&world.owner_token,
		"Seven",
		"PUBLIC",
		&books.iter().collect::<Vec<_>>(),
	)
	.await;

	let page = |limit: u64, offset: u64| {
		let app = &app;
		let token = world.ana.token.clone();
		let list_id = list_id.clone();
		async move {
			let data = ok(
				app,
				&token,
				r#"query Page($id: ID!, $limit: Int, $offset: Int) {
					readingListById(id: $id) { bookCount books(limit: $limit, offset: $offset) { id } }
				}"#,
				json!({ "id": list_id, "limit": limit, "offset": offset }),
			)
			.await;
			assert_eq!(data["readingListById"]["bookCount"], 7);
			data["readingListById"]["books"]
				.as_array()
				.unwrap()
				.iter()
				.map(|book| book["id"].as_str().unwrap().to_string())
				.collect::<Vec<_>>()
		}
	};

	// every book once, in the list's order
	let mut all = Vec::new();
	for offset in [0, 3, 6, 9] {
		all.extend(page(3, offset).await);
	}
	assert_eq!(all, books);
	assert!(page(3, 7).await.is_empty());
	// no `limit` is the biggest page, and nobody gets more than that
	assert_eq!(page(1_000_000, 0).await, books);
}

#[tokio::test]
async fn the_scopes_split_my_lists_from_everyone_elses() {
	let (app, world) = world().await;
	let mine_private =
		create(&app, &world.ana.token, "Mine, private", "PRIVATE", &[]).await;
	let mine_public = create(&app, &world.ana.token, "Mine, public", "PUBLIC", &[]).await;
	let owners_public =
		create(&app, &world.owner_token, "Owner's public", "PUBLIC", &[]).await;
	let _owners_private =
		create(&app, &world.owner_token, "Owner's private", "PRIVATE", &[]).await;
	let bens_public = create(&app, &world.ben.token, "Ben's public", "PUBLIC", &[]).await;

	let ids_of = |mut ids: Vec<String>| {
		ids.sort();
		ids
	};
	assert_eq!(
		list_ids(&lists(&app, &world.ana.token, "MINE").await),
		ids_of(vec![mine_private.clone(), mine_public.clone()])
	);
	assert_eq!(
		list_ids(&lists(&app, &world.ana.token, "OTHERS").await),
		ids_of(vec![owners_public.clone(), bens_public.clone()])
	);
	assert_eq!(
		list_ids(&lists(&app, &world.ana.token, "ALL").await),
		ids_of(vec![mine_private, mine_public, owners_public, bens_public])
	);
}

#[tokio::test]
async fn a_list_says_who_made_it_and_whether_you_may_change_it() {
	let (app, world) = world().await;
	let owners = create(
		&app,
		&world.owner_token,
		"Owner's",
		"PUBLIC",
		&[&world.general.id],
	)
	.await;
	let anas = create(&app, &world.ana.token, "Ana's", "PRIVATE", &[]).await;

	let data = ok(&app, &world.ana.token, BY_ID, json!({ "id": owners })).await;
	assert_eq!(data["readingListById"]["canEdit"], false);
	assert_eq!(data["readingListById"]["creator"]["username"], OWNER);

	let data = ok(&app, &world.ana.token, BY_ID, json!({ "id": anas })).await;
	assert_eq!(data["readingListById"]["canEdit"], true);
	assert_eq!(data["readingListById"]["creator"]["username"], "ana");

	// the owner has no right to edit ana's public list either: only to take it back
	let public = create(&app, &world.ana.token, "Ana's public", "PUBLIC", &[]).await;
	let data = ok(&app, &world.owner_token, BY_ID, json!({ "id": public })).await;
	assert_eq!(data["readingListById"]["canEdit"], false);

	// a profile is a name and an avatar: nothing else can be asked for
	let response = run(
		&app,
		&world.ana.token,
		r#"query ($id: ID!) { readingListById(id: $id) { creator { id username avatarUrl isServerOwner } } }"#,
		json!({ "id": owners }),
	)
	.await;
	assert!(response.get("errors").is_some(), "{response}");
}
