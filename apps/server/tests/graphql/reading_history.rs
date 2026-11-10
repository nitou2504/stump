//! The *My reading* page (package A2): what a person has finished and is reading now.
//!
//! A person's reading is private until they say otherwise (`updateMyReadingPrivacy`). Then
//! everyone on the server can read it, each seeing it through their own limits. Every test builds
//! the world of [`VisibilityFixture`]: **ana** has limits (her hidden set H is `adult`, `unrated`,
//! `removed` and `hidden`), **ben** has none, **zoe** was deleted, and the owner sees everything
//! but `removed`. What ana gets from anyone's history must never hold anything that comes from H:
//! an id, a title, a cover, a count, a year count or a search hit.

use chrono::NaiveDate;
use models::{
	entity::{media, reading_session, user, user_sharing_settings},
	shared::{
		api_key::APIKeyPermissions,
		enums::{ReadingStatus, UserPermission},
	},
};
use sea_orm::{prelude::*, ActiveModelTrait, Set};
use serde_json::{json, Value};

use crate::common::{
	api_key::create_api_key_for_user,
	visibility::{add_book, Meta, VisibilityFixture},
	TestApp,
};

const OWNER: &str = "initial-server-admin";
const NOT_FOUND: &str = "Reader not found";

async fn world() -> (TestApp, VisibilityFixture) {
	let app = TestApp::new_with_default_user().await;
	let world = VisibilityFixture::build(&app).await;
	(app, world)
}

async fn owner_id(app: &TestApp) -> String {
	user::Entity::find()
		.filter(user::Column::Username.eq(OWNER))
		.one(app.conn())
		.await
		.expect("should look for the owner")
		.expect("the app has its owner")
		.id
}

fn day(year: i32, month: u32, day: u32) -> NaiveDate {
	NaiveDate::from_ymd_opt(year, month, day).expect("a valid date")
}

/// A session of `user_id` on a book: the nth read of it, on a day, in a status
async fn session(
	app: &TestApp,
	user_id: &str,
	book: &media::Model,
	readthrough: i32,
	on: NaiveDate,
	status: ReadingStatus,
) -> reading_session::Model {
	reading_session::ActiveModel {
		session_date: Set(on),
		status: Set(status),
		readthrough_number: Set(readthrough),
		media_id: Set(book.id.clone()),
		user_id: Set(user_id.to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should insert the session")
}

/// The first read of a book, finished on a day
async fn finish(app: &TestApp, user_id: &str, book: &media::Model, on: NaiveDate) {
	session(app, user_id, book, 1, on, ReadingStatus::Finished).await;
}

/// Everything the page asks for, in one query
const HISTORY: &str = r#"
	query History($userId: ID, $search: String, $year: Int, $order: ReadingHistoryOrder = NEWEST_FIRST, $pagination: OffsetPagination = { page: 1, pageSize: 24 }) {
		readingHistory(userId: $userId) {
			reader { id username }
			isSelf
			years(search: $search) { year count }
			reading { media { id resolvedName } percentage lastReadOn readthroughNumber }
			finished(search: $search, year: $year, order: $order, pagination: $pagination) {
				nodes { media { id resolvedName } readthroughNumber startedOn finishedOn }
				pageInfo { totalItems totalPages currentPage pageSize }
			}
		}
	}
"#;

const SHARING: &str = r#"
	query Sharing { myReadingPrivacy { shareReadingHistory } readersSharingHistory { id username } }
"#;

const SET_PRIVACY: &str = r#"
	mutation Privacy($share: Boolean!) {
		updateMyReadingPrivacy(shareReadingHistory: $share) { shareReadingHistory }
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

fn error_message(response: &Value) -> String {
	response["errors"][0]["message"]
		.as_str()
		.unwrap_or_else(|| panic!("an error in {response}"))
		.to_string()
}

/// The history of `reader` (the asker's own when `None`), with more variables
async fn history(
	app: &TestApp,
	token: &str,
	reader: Option<&str>,
	mut variables: Value,
) -> Value {
	variables["userId"] = reader.map_or(Value::Null, |id| json!(id));
	run(app, token, HISTORY, variables).await
}

async fn history_ok(
	app: &TestApp,
	token: &str,
	reader: Option<&str>,
	variables: Value,
) -> Value {
	let response = history(app, token, reader, variables).await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"]["readingHistory"].clone()
}

async fn share(app: &TestApp, token: &str, share: bool) {
	let data = ok(app, token, SET_PRIVACY, json!({ "share": share })).await;
	assert_eq!(data["updateMyReadingPrivacy"]["shareReadingHistory"], share);
}

/// The ids of the books of a list of `{ media { id } }` nodes, in order
fn book_ids(nodes: &Value) -> Vec<String> {
	nodes
		.as_array()
		.expect("a list")
		.iter()
		.map(|node| node["media"]["id"].as_str().expect("an id").to_string())
		.collect()
}

fn names(nodes: &Value) -> Vec<String> {
	nodes
		.as_array()
		.expect("a list")
		.iter()
		.map(|node| {
			node["media"]["resolvedName"]
				.as_str()
				.expect("a name")
				.to_string()
		})
		.collect()
}

fn years(history: &Value) -> Vec<(i64, i64)> {
	history["years"]
		.as_array()
		.expect("years")
		.iter()
		.map(|year| {
			(
				year["year"].as_i64().expect("a year"),
				year["count"].as_i64().expect("a count"),
			)
		})
		.collect()
}

/// The owner has read all five books of the world: four exist for the owner (the removed one is
/// gone for everybody) and one is being read
async fn the_owner_reads_everything(app: &TestApp, world: &VisibilityFixture) {
	let owner = owner_id(app).await;
	finish(app, &owner, &world.removed, day(2025, 11, 1)).await;
	finish(app, &owner, &world.hidden, day(2025, 12, 30)).await;
	finish(app, &owner, &world.general, day(2026, 3, 2)).await;
	finish(app, &owner, &world.adult, day(2026, 3, 3)).await;
	finish(app, &owner, &world.unrated, day(2026, 3, 4)).await;
}

// -- who may read a history ----------------------------------------------------------------

#[tokio::test]
async fn nobody_reads_a_history_that_is_not_shared() {
	let (app, world) = world().await;
	the_owner_reads_everything(&app, &world).await;
	let owner = owner_id(&app).await;
	finish(&app, &world.ben.id, &world.general, day(2026, 1, 1)).await;

	let unknown =
		history(&app, &world.ana.token, Some("no-such-reader"), json!({})).await;
	assert_eq!(error_message(&unknown), NOT_FOUND);

	// the owner never shared: ana, ben and zoe's absence all read as an unknown reader, to the word
	for (token, reader) in [
		(&world.ana.token, owner.as_str()),
		(&world.ben.token, owner.as_str()),
		(&world.owner_token, world.ben.id.as_str()),
		(&world.ana.token, world.ben.id.as_str()),
		// zoe was deleted
		(&world.ana.token, world.zoe.id.as_str()),
		// whatever else is asked
		(&world.ana.token, "  "),
		(&world.ana.token, "' OR 1=1 --"),
	] {
		let response = history(&app, token, Some(reader), json!({})).await;
		assert_eq!(error_message(&response), NOT_FOUND, "{reader}");
		assert_eq!(
			unknown["errors"], response["errors"],
			"nothing tells {reader:?} from an unknown reader"
		);
	}

	// nobody is on the list of people who share, and the server owner has no way around it
	for token in [&world.ana.token, &world.ben.token, &world.owner_token] {
		let data = ok(&app, token, SHARING, json!({})).await;
		assert_eq!(data["readersSharingHistory"], json!([]));
		assert_eq!(data["myReadingPrivacy"]["shareReadingHistory"], false);
	}
}

#[tokio::test]
async fn everybody_reads_their_own_history_without_sharing_it() {
	let (app, world) = world().await;
	let owner = owner_id(&app).await;
	finish(&app, &owner, &world.general, day(2026, 3, 2)).await;
	finish(&app, &world.ana.id, &world.general, day(2026, 3, 5)).await;

	let mine = history_ok(&app, &world.owner_token, None, json!({})).await;
	assert_eq!(mine["isSelf"], true);
	assert_eq!(mine["reader"]["id"], owner.as_str());
	assert_eq!(book_ids(&mine["finished"]["nodes"]), vec![world.general.id.clone()]);

	// asking for yourself by name is the same as not naming anybody
	let named = history_ok(&app, &world.ana.token, Some(&world.ana.id), json!({})).await;
	assert_eq!(named["isSelf"], true);
	assert_eq!(named["reader"]["username"], "ana");
	assert_eq!(named["finished"]["nodes"][0]["finishedOn"], "2026-03-05");
}

#[tokio::test]
async fn after_opting_in_ana_sees_only_what_she_may_read_of_it() {
	let (app, world) = world().await;
	the_owner_reads_everything(&app, &world).await;
	let owner = owner_id(&app).await;
	// two books being read: one of H, one not (a second read of a book already finished)
	session(&app, &owner, &world.adult, 2, day(2026, 3, 8), ReadingStatus::Reading)
		.await;
	reading_session::ActiveModel {
		end_percentage: Set(Some("0.425".parse().expect("a decimal"))),
		..session(&app, &owner, &world.general, 2, day(2026, 3, 9), ReadingStatus::Reading)
			.await
			.into()
	}
	.update(app.conn())
	.await
	.expect("should save the progress");

	share(&app, &world.owner_token, true).await;

	// the owner sees what they read (not the removed book)
	let own = history_ok(&app, &world.owner_token, None, json!({})).await;
	assert_eq!(
		book_ids(&own["finished"]["nodes"]),
		vec![
			world.unrated.id.clone(),
			world.adult.id.clone(),
			world.general.id.clone(),
			world.hidden.id.clone()
		]
	);
	assert_eq!(years(&own), vec![(2026, 3), (2025, 1)]);
	assert_eq!(own["reading"].as_array().unwrap().len(), 2);

	// ana is told who it is and sees the one book she may read: not one of the four others in an
	// id, a title, a count, a year, a date or a search
	let response = history(&app, &world.ana.token, Some(&owner), json!({})).await;
	assert!(response.get("errors").is_none(), "{response}");
	let seen = &response["data"]["readingHistory"];
	assert_eq!(seen["isSelf"], false);
	assert_eq!(seen["reader"]["id"], owner.as_str());
	assert_eq!(seen["reader"]["username"], OWNER);
	assert_eq!(ids_of_finished(seen), vec![world.general.id.clone()]);
	assert_eq!(seen["finished"]["pageInfo"]["totalItems"], 1);
	// the year count is one although the owner finished five books (and four of those exist)
	assert_eq!(years(seen), vec![(2026, 1)]);
	assert_eq!(book_ids(&seen["reading"]), vec![world.general.id.clone()]);
	assert_eq!(seen["reading"][0]["percentage"], 43);
	assert_eq!(seen["reading"][0]["lastReadOn"], "2026-03-09");
	assert_eq!(seen["reading"][0]["readthroughNumber"], 2);
	world.assert_none_hidden(&response);

	// ben has no limits: he sees what the owner sees
	let bens = history_ok(&app, &world.ben.token, Some(&owner), json!({})).await;
	assert_eq!(bens["finished"]["pageInfo"]["totalItems"], 4);
	assert_eq!(years(&bens), vec![(2026, 3), (2025, 1)]);

	// the owner is on the list of people who share, for the others and not for themselves
	for token in [&world.ana.token, &world.ben.token] {
		let data = ok(&app, token, SHARING, json!({})).await;
		assert_eq!(
			data["readersSharingHistory"],
			json!([{ "id": owner, "username": OWNER }])
		);
	}
	let data = ok(&app, &world.owner_token, SHARING, json!({})).await;
	assert_eq!(data["readersSharingHistory"], json!([]));
	assert_eq!(data["myReadingPrivacy"]["shareReadingHistory"], true);
}

fn ids_of_finished(history: &Value) -> Vec<String> {
	book_ids(&history["finished"]["nodes"])
}

#[tokio::test]
async fn a_search_by_ana_finds_nothing_that_comes_from_h() {
	let (app, world) = world().await;
	the_owner_reads_everything(&app, &world).await;
	let owner = owner_id(&app).await;
	share(&app, &world.owner_token, true).await;

	// every title of H, whole or in part, finds nothing: no hit, no count, no year
	for search in ["Adult", "adult book", "Unrated", "Removed", "Hidden", "hidd"] {
		let found = history_ok(
			&app,
			&world.ana.token,
			Some(&owner),
			json!({ "search": search }),
		)
		.await;
		assert_eq!(found["finished"]["nodes"], json!([]), "{search}");
		assert_eq!(found["finished"]["pageInfo"]["totalItems"], 0, "{search}");
		assert_eq!(years(&found), vec![], "{search}");

		// the owner finds them, so it is the viewer's limits that make the difference
		let theirs = history_ok(
			&app,
			&world.ben.token,
			Some(&owner),
			json!({ "search": search }),
		)
		.await;
		let hits = theirs["finished"]["pageInfo"]["totalItems"].as_i64().unwrap();
		// "Removed" is gone for everybody
		assert_eq!(hits, i64::from(search != "Removed"), "{search}");
	}

	// the book she may read is found, and its year is counted
	let found = history_ok(
		&app,
		&world.ana.token,
		Some(&owner),
		json!({ "search": "general" }),
	)
	.await;
	assert_eq!(ids_of_finished(&found), vec![world.general.id.clone()]);
	assert_eq!(years(&found), vec![(2026, 1)]);
	// and filtering by a year only the hidden books were finished in finds nothing
	let found = history_ok(
		&app,
		&world.ana.token,
		Some(&owner),
		json!({ "year": 2025 }),
	)
	.await;
	assert_eq!(found["finished"]["nodes"], json!([]));
}

#[tokio::test]
async fn a_users_own_history_is_narrowed_by_their_limits_too() {
	let (app, world) = world().await;
	// ana read these before her limits (or before the book was removed)
	for (book, on) in [
		(&world.general, day(2026, 1, 10)),
		(&world.adult, day(2026, 1, 11)),
		(&world.unrated, day(2026, 1, 12)),
		(&world.removed, day(2026, 1, 13)),
		(&world.hidden, day(2026, 1, 14)),
	] {
		finish(&app, &world.ana.id, book, on).await;
	}
	session(
		&app,
		&world.ana.id,
		&world.hidden,
		2,
		day(2026, 2, 1),
		ReadingStatus::Reading,
	)
	.await;

	let response = history(&app, &world.ana.token, None, json!({})).await;
	assert!(response.get("errors").is_none(), "{response}");
	let mine = &response["data"]["readingHistory"];

	assert_eq!(ids_of_finished(mine), vec![world.general.id.clone()]);
	assert_eq!(years(mine), vec![(2026, 1)]);
	assert_eq!(mine["reading"], json!([]));
	world.assert_none_hidden(&response);
}

// -- stopping --------------------------------------------------------------------------------

#[tokio::test]
async fn stopping_sharing_ends_it_at_once() {
	let (app, world) = world().await;
	the_owner_reads_everything(&app, &world).await;
	let owner = owner_id(&app).await;

	share(&app, &world.owner_token, true).await;
	assert_eq!(
		ids_of_finished(&history_ok(&app, &world.ana.token, Some(&owner), json!({})).await),
		vec![world.general.id.clone()]
	);

	share(&app, &world.owner_token, false).await;
	let response = history(&app, &world.ana.token, Some(&owner), json!({})).await;
	assert_eq!(error_message(&response), NOT_FOUND);
	let data = ok(&app, &world.ana.token, SHARING, json!({})).await;
	assert_eq!(data["readersSharingHistory"], json!([]));

	// turning it on again brings it back, and the owner still reads their own all along
	share(&app, &world.owner_token, true).await;
	history_ok(&app, &world.ana.token, Some(&owner), json!({})).await;
	share(&app, &world.owner_token, false).await;
	history_ok(&app, &world.owner_token, None, json!({})).await;
}

#[tokio::test]
async fn a_reader_who_is_deleted_or_locked_is_gone_at_once() {
	let (app, world) = world().await;
	finish(&app, &world.ben.id, &world.general, day(2026, 1, 1)).await;
	share(&app, &world.ben.token, true).await;
	// zoe was deleted a long time ago and had shared before
	user_sharing_settings::ActiveModel {
		user_id: Set(world.zoe.id.clone()),
		share_reading_history: Set(true),
		updated_at: Set(chrono::Utc::now()),
	}
	.insert(app.conn())
	.await
	.expect("should insert zoe's setting");
	finish(&app, &world.zoe.id, &world.general, day(2026, 1, 2)).await;

	let data = ok(&app, &world.ana.token, SHARING, json!({})).await;
	assert_eq!(
		data["readersSharingHistory"],
		json!([{ "id": world.ben.id, "username": "ben" }]),
		"zoe is deleted"
	);
	let zoes = history(&app, &world.ana.token, Some(&world.zoe.id), json!({})).await;
	assert_eq!(error_message(&zoes), NOT_FOUND);

	let set = |locked: Option<bool>, deleted: bool| {
		let app = &app;
		let ben = world.ben.id.clone();
		async move {
			let model = user::Entity::find_by_id(ben)
				.one(app.conn())
				.await
				.unwrap()
				.unwrap();
			let mut active: user::ActiveModel = model.into();
			if let Some(locked) = locked {
				active.is_locked = Set(locked);
			}
			active.deleted_at = Set(deleted.then(|| chrono::Utc::now().into()));
			active.update(app.conn()).await.unwrap();
		}
	};

	history_ok(&app, &world.ana.token, Some(&world.ben.id), json!({})).await;
	set(Some(true), false).await;
	let response = history(&app, &world.ana.token, Some(&world.ben.id), json!({})).await;
	assert_eq!(error_message(&response), NOT_FOUND);
	let data = ok(&app, &world.ana.token, SHARING, json!({})).await;
	assert_eq!(data["readersSharingHistory"], json!([]));

	// unlocked, he is back; deleted, he is gone again, the owner included
	set(Some(false), false).await;
	history_ok(&app, &world.ana.token, Some(&world.ben.id), json!({})).await;
	set(None, true).await;
	for token in [&world.ana.token, &world.owner_token] {
		let response = history(&app, token, Some(&world.ben.id), json!({})).await;
		assert_eq!(error_message(&response), NOT_FOUND);
	}
}

// -- the switch --------------------------------------------------------------------------------

#[tokio::test]
async fn the_privacy_switch_changes_only_the_callers_own_setting() {
	let (app, world) = world().await;
	let owner = owner_id(&app).await;

	share(&app, &world.ana.token, true).await;
	// twice is the same as once: one row, not a failure
	share(&app, &world.ana.token, true).await;

	let stored = user_sharing_settings::Entity::find()
		.all(app.conn())
		.await
		.expect("should read the settings");
	assert_eq!(stored.len(), 1);
	assert_eq!(stored[0].user_id, world.ana.id);
	assert!(stored[0].share_reading_history);

	// nobody else's changed: not ben's, not the owner's
	for (token, label) in [(&world.ben.token, "ben"), (&world.owner_token, "owner")] {
		let data = ok(&app, token, SHARING, json!({})).await;
		assert_eq!(data["myReadingPrivacy"]["shareReadingHistory"], false, "{label}");
		// ana is on their list, they are not on ana's
		assert_eq!(
			data["readersSharingHistory"],
			json!([{ "id": world.ana.id, "username": "ana" }])
		);
	}
	let data = ok(&app, &world.ana.token, SHARING, json!({})).await;
	assert_eq!(data["myReadingPrivacy"]["shareReadingHistory"], true);
	assert_eq!(data["readersSharingHistory"], json!([]));

	// the mutation has no way to name anyone else
	let response = run(
		&app,
		&world.ben.token,
		r#"mutation { updateMyReadingPrivacy(shareReadingHistory: true, userId: "x") { shareReadingHistory } }"#,
		json!({}),
	)
	.await;
	assert!(response.get("errors").is_some(), "{response}");
	assert_eq!(is_sharing(&app, &owner).await, false);

	// turning it off is the person's, and only theirs
	share(&app, &world.ana.token, false).await;
	assert!(!is_sharing(&app, &world.ana.id).await);
}

async fn is_sharing(app: &TestApp, user_id: &str) -> bool {
	user_sharing_settings::Entity::find_by_id(user_id.to_string())
		.one(app.conn())
		.await
		.expect("should read the setting")
		.is_some_and(|settings| settings.share_reading_history)
}

#[tokio::test]
async fn an_api_key_cannot_change_who_sees_a_reading() {
	let (app, world) = world().await;
	// someone who may have API keys, with ana's limits
	let keyed = world
		.limited_like_ana(&app, "keyed", vec![UserPermission::AccessApiKeys])
		.await;
	let model = user::Entity::find_by_id(keyed.id.clone())
		.one(app.conn())
		.await
		.unwrap()
		.unwrap();
	let (key, _) = create_api_key_for_user(&app, &model, None::<APIKeyPermissions>).await;

	// the key is good for what it is for: reading
	let data = ok(&app, &key, SHARING, json!({})).await;
	assert_eq!(data["myReadingPrivacy"]["shareReadingHistory"], false);

	// but whether the person's reading is seen is theirs to decide
	let response = run(&app, &key, SET_PRIVACY, json!({ "share": true })).await;
	assert!(error_message(&response).contains("API key"), "{response}");
	assert!(!is_sharing(&app, &keyed.id).await);

	// they can, signed in
	share(&app, &keyed.token, true).await;
	assert!(is_sharing(&app, &keyed.id).await);
}

// -- ordering, paging, search, years --------------------------------------------------------

/// Six finished reads, in the owner's history. "Zeta 1" was read twice
async fn six_reads(app: &TestApp, world: &VisibilityFixture) -> Vec<media::Model> {
	let owner = owner_id(app).await;
	let mut books = Vec::new();
	for n in 1..=5 {
		books.push(
			add_book(app, &world.open_series, &format!("Zeta {n}"), Meta::default())
				.await,
		);
	}
	session(app, &owner, &books[0], 1, day(2023, 6, 1), ReadingStatus::Finished).await;
	session(app, &owner, &books[0], 2, day(2024, 5, 1), ReadingStatus::Finished).await;
	finish(app, &owner, &books[1], day(2024, 8, 9)).await;
	// two books finished on the same day: the one written last is the later
	finish(app, &owner, &books[2], day(2025, 2, 14)).await;
	finish(app, &owner, &books[3], day(2025, 2, 14)).await;
	finish(app, &owner, &books[4], day(2026, 1, 20)).await;
	books
}

#[tokio::test]
async fn finished_books_come_latest_first_a_page_at_a_time() {
	let (app, world) = world().await;
	let z = six_reads(&app, &world).await;
	let id = |n: usize| z[n].id.clone();

	let first = history_ok(
		&app,
		&world.owner_token,
		None,
		json!({ "pagination": { "page": 1, "pageSize": 2 } }),
	)
	.await;
	assert_eq!(ids_of_finished(&first), vec![id(4), id(3)]);
	assert_eq!(first["finished"]["pageInfo"]["totalItems"], 6);
	assert_eq!(first["finished"]["pageInfo"]["totalPages"], 3);
	assert_eq!(first["finished"]["pageInfo"]["currentPage"], 1);
	assert_eq!(first["finished"]["pageInfo"]["pageSize"], 2);
	// the dates, and the read it was
	assert_eq!(first["finished"]["nodes"][0]["finishedOn"], "2026-01-20");
	assert_eq!(first["finished"]["nodes"][0]["startedOn"], "2026-01-20");

	let second = history_ok(
		&app,
		&world.owner_token,
		None,
		json!({ "pagination": { "page": 2, "pageSize": 2 } }),
	)
	.await;
	assert_eq!(ids_of_finished(&second), vec![id(2), id(1)]);

	// the last page has the two reads of "Zeta 1", the second one first
	let third = history_ok(
		&app,
		&world.owner_token,
		None,
		json!({ "pagination": { "page": 3, "pageSize": 2 } }),
	)
	.await;
	assert_eq!(ids_of_finished(&third), vec![id(0), id(0)]);
	let reads = third["finished"]["nodes"].as_array().unwrap();
	assert_eq!(reads[0]["readthroughNumber"], 2);
	assert_eq!(reads[0]["finishedOn"], "2024-05-01");
	assert_eq!(reads[1]["readthroughNumber"], 1);
	assert_eq!(reads[1]["startedOn"], "2023-06-01");

	// past the end is an empty page, not a failure
	let past = history_ok(
		&app,
		&world.owner_token,
		None,
		json!({ "pagination": { "page": 4, "pageSize": 2 } }),
	)
	.await;
	assert_eq!(past["finished"]["nodes"], json!([]));
	assert_eq!(past["finished"]["pageInfo"]["totalItems"], 6);

	// oldest first is the same list the other way round
	let oldest = history_ok(
		&app,
		&world.owner_token,
		None,
		json!({ "order": "OLDEST_FIRST", "pagination": { "page": 1, "pageSize": 3 } }),
	)
	.await;
	assert_eq!(ids_of_finished(&oldest), vec![id(0), id(0), id(1)]);
	assert_eq!(oldest["finished"]["nodes"][0]["readthroughNumber"], 1);
}

#[tokio::test]
async fn a_page_is_never_more_than_200_and_never_empty_by_mistake() {
	let (app, world) = world().await;
	six_reads(&app, &world).await;

	let big = history_ok(
		&app,
		&world.owner_token,
		None,
		json!({ "pagination": { "page": 1, "pageSize": 1000 } }),
	)
	.await;
	assert_eq!(big["finished"]["pageInfo"]["pageSize"], 200);
	assert_eq!(big["finished"]["nodes"].as_array().unwrap().len(), 6);

	for pagination in [
		json!({ "page": 0, "pageSize": 10 }),
		json!({ "page": 1, "pageSize": 0 }),
	] {
		let response = history(
			&app,
			&world.owner_token,
			None,
			json!({ "pagination": pagination }),
		)
		.await;
		assert!(response.get("errors").is_some(), "{response}");
	}
}

#[tokio::test]
async fn finished_books_are_searched_and_filtered_by_year() {
	let (app, world) = world().await;
	let z = six_reads(&app, &world).await;
	let id = |n: usize| z[n].id.clone();

	// the years, the latest first, count the reads of each
	let all = history_ok(&app, &world.owner_token, None, json!({})).await;
	assert_eq!(years(&all), vec![(2026, 1), (2025, 2), (2024, 2), (2023, 1)]);

	let in_2025 = history_ok(&app, &world.owner_token, None, json!({ "year": 2025 })).await;
	assert_eq!(ids_of_finished(&in_2025), vec![id(3), id(2)]);
	assert_eq!(in_2025["finished"]["pageInfo"]["totalItems"], 2);
	// the year chips don't change with the year picked
	assert_eq!(years(&in_2025), years(&all));

	// words must all match, in any order
	let found = history_ok(&app, &world.owner_token, None, json!({ "search": "zeta 2" })).await;
	assert_eq!(names(&found["finished"]["nodes"]), vec!["Zeta 2"]);
	// the years of a search count only what it finds
	assert_eq!(years(&found), vec![(2024, 1)]);
	let found = history_ok(&app, &world.owner_token, None, json!({ "search": "1 ZETA" })).await;
	assert_eq!(found["finished"]["pageInfo"]["totalItems"], 2);
	assert_eq!(years(&found), vec![(2024, 1), (2023, 1)]);

	// a search and a year together
	let both = history_ok(
		&app,
		&world.owner_token,
		None,
		json!({ "search": "zeta", "year": 2024 }),
	)
	.await;
	assert_eq!(both["finished"]["pageInfo"]["totalItems"], 2);
	let none = history_ok(
		&app,
		&world.owner_token,
		None,
		json!({ "search": "zeta 5", "year": 2024 }),
	)
	.await;
	assert_eq!(none["finished"]["nodes"], json!([]));

	// nothing to search for is everything, and a very long search is cut, not a failure
	let blank = history_ok(&app, &world.owner_token, None, json!({ "search": "   " })).await;
	assert_eq!(blank["finished"]["pageInfo"]["totalItems"], 6);
	let long = history_ok(
		&app,
		&world.owner_token,
		None,
		json!({ "search": "zeta ".repeat(5_000) }),
	)
	.await;
	assert_eq!(long["finished"]["pageInfo"]["totalItems"], 6);
}

#[tokio::test]
async fn what_is_being_read_is_the_newest_session_still_reading() {
	let (app, world) = world().await;
	let owner = owner_id(&app).await;
	let z = six_reads(&app, &world).await;
	let extra = add_book(&app, &world.open_series, "Omega", Meta::default()).await;
	let page_book =
		add_book(&app, &world.open_series, "Pages", Meta::default()).await;

	// a second read of "Zeta 1" in progress, a book given up, one read by page, one by percentage
	session(&app, &owner, &z[0], 3, day(2026, 2, 1), ReadingStatus::Reading).await;
	session(&app, &owner, &extra, 1, day(2026, 2, 2), ReadingStatus::Abandoned).await;
	reading_session::ActiveModel {
		end_page: Set(Some(5)),
		..session(&app, &owner, &page_book, 1, day(2026, 2, 3), ReadingStatus::Reading)
			.await
			.into()
	}
	.update(app.conn())
	.await
	.unwrap();
	reading_session::ActiveModel {
		end_percentage: Set(Some("0.5".parse().unwrap())),
		..session(&app, &owner, &z[2], 2, day(2026, 2, 4), ReadingStatus::Reading)
			.await
			.into()
	}
	.update(app.conn())
	.await
	.unwrap();

	let mine = history_ok(&app, &world.owner_token, None, json!({})).await;
	// the one read last first; the given-up book is not here
	assert_eq!(
		names(&mine["reading"]),
		vec!["Zeta 3", "Pages", "Zeta 1"]
	);
	assert_eq!(mine["reading"][0]["percentage"], 50);
	// 5 of the 10 pages the fixture's books have
	assert_eq!(mine["reading"][1]["percentage"], 50);
	assert_eq!(mine["reading"][2]["percentage"], Value::Null);
	assert_eq!(mine["reading"][0]["readthroughNumber"], 2);
	// the finished reads of the books being read are listed, too
	assert_eq!(mine["finished"]["pageInfo"]["totalItems"], 6);
}

// -- a big history ----------------------------------------------------------------------------

/// A person with thousands of sessions gets their page quickly. The server is a release build,
/// so the bound here (a debug build, on a busy machine) is generous: what is checked is that the
/// time doesn't grow with something it shouldn't, and the figure is printed
#[tokio::test]
async fn a_big_history_answers_quickly() {
	let (app, world) = world().await;
	let owner = owner_id(&app).await;
	let mut books = Vec::new();
	for n in 0..300 {
		books.push(
			add_book(&app, &world.open_series, &format!("Big {n}"), Meta::default())
				.await,
		);
	}
	// 6,000 sessions: ten reads of each of 300 books (3,000 reads), each finished the day after
	// it started
	let mut rows = Vec::new();
	for (n, book) in books.iter().enumerate() {
		for read in 1..=10_i32 {
			let start = day(2020, 1, 1)
				+ chrono::Duration::days(n as i64 * 3 + i64::from(read));
			for (offset, status) in
				[(0, ReadingStatus::Reading), (1, ReadingStatus::Finished)]
			{
				rows.push(reading_session::ActiveModel {
					session_date: Set(start + chrono::Duration::days(offset)),
					status: Set(status),
					readthrough_number: Set(read),
					media_id: Set(book.id.clone()),
					user_id: Set(owner.clone()),
					created_at: Set(chrono::Utc::now().into()),
					updated_at: Set(Some(chrono::Utc::now().into())),
					..Default::default()
				});
			}
		}
	}
	for chunk in rows.chunks(500) {
		reading_session::Entity::insert_many(chunk.to_vec())
			.exec(app.conn())
			.await
			.expect("should insert the sessions");
	}
	share(&app, &world.owner_token, true).await;

	let started = std::time::Instant::now();
	let mine = history_ok(
		&app,
		&world.owner_token,
		None,
		json!({ "pagination": { "page": 5, "pageSize": 48 } }),
	)
	.await;
	let own = started.elapsed();
	assert_eq!(mine["finished"]["pageInfo"]["totalItems"], 3_000);
	assert_eq!(mine["finished"]["nodes"].as_array().unwrap().len(), 48);

	let started = std::time::Instant::now();
	let seen = history_ok(
		&app,
		&world.ana.token,
		Some(&owner),
		json!({ "pagination": { "page": 1, "pageSize": 48 }, "search": "big 1" }),
	)
	.await;
	let others = started.elapsed();
	assert!(seen["finished"]["pageInfo"]["totalItems"].as_i64().unwrap() > 0);

	println!("own page of 6,000 sessions: {own:?}; a friend's, searched: {others:?}");
	assert!(own.as_millis() < 3_000, "the own page took {own:?}");
	assert!(others.as_millis() < 3_000, "the friend's page took {others:?}");
}
