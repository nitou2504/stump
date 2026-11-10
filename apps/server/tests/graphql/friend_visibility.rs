//! The baseline of what a user with limits gets from the surfaces a book appears on today.
//!
//! Every test builds the world of [`VisibilityFixture`] and asks one surface what **ana** gets. Her
//! hidden set, H (`adult`, `unrated`, `removed` and `hidden`), must not show in any ID, title,
//! path, count, page or search hit, and the same request by the owner (or by ben) shows the
//! surface works, so that an empty answer is the rule at work and not a wrong request.
//!
//! `data_scoping.rs` covers counts, alphabets, stats, tags, series covers, club books and
//! progress writes; this file covers the lists, search, picks, progress shelves, the REST routes
//! of a book and the OPDS feeds. A surface that fails here is filed under package C5 (see the plan,
//! P0 and C5) and its test is ignored with the reason, until C5 fixes it and removes the `ignore`.
//!
//! What it found (run them with `-- --ignored` to see them fail):
//!
//! - `on_deck_counts_only_what_she_may_read`: `onDeck`'s page is right (the next book that is
//!   hidden from her is left out), but its total counts it.
//! - `opds_2_progression_gives_nothing_away_about_a_hidden_book`: the OPDS 2 progression route has
//!   no access check, so a book hidden from her answers with the position she had in it before
//!   the limits, and one that doesn't exist with none: it tells the book is there.

use axum::http::StatusCode;
use models::{
	entity::user,
	shared::enums::{ReadingStatus, UserPermission},
};
use sea_orm::prelude::*;
use serde_json::{json, Value};
use tests::fake_data;

use crate::common::{
	visibility::{ids, sorted, VisibilityFixture},
	TestApp,
};

const BOOK: &str = "id name path seriesId libraryId thumbnail { url } series { id name } library { id name }";
const PAGE_INFO: &str =
	"pageInfo { __typename ... on OffsetPaginationInfo { totalItems totalPages currentPage } }";

async fn world() -> (TestApp, VisibilityFixture) {
	let app = TestApp::new_with_default_user().await;
	let world = VisibilityFixture::build(&app).await;
	(app, world)
}

/// The IDs of a list of books
fn books_of(list: &Value) -> Vec<String> {
	ids(&list["nodes"])
}

fn only(ids: &[&String]) -> Vec<String> {
	sorted(ids.iter().map(|id| id.to_string()).collect())
}

fn total_items(list: &Value) -> i64 {
	list["pageInfo"]["totalItems"]
		.as_i64()
		.unwrap_or_else(|| panic!("a total in {list}"))
}

/// Runs a query as `token`, and fails if it fails
async fn ask(app: &TestApp, token: &str, query: &str, variables: Value) -> Value {
	let response = app
		.execute_gql_with_token(query, Some(variables), token)
		.await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"].clone()
}

async fn books_page(
	app: &TestApp,
	token: &str,
	page: u32,
	size: u32,
	filter: Value,
) -> Value {
	let query = format!(
		"query Books($page: Int!, $size: Int!, $filter: MediaFilterInput!) {{
			media(filter: $filter, pagination: {{ offset: {{ page: $page, pageSize: $size }} }}) {{
				nodes {{ {BOOK} }}
				{PAGE_INFO}
			}}
		}}"
	);
	let data = ask(
		app,
		token,
		&query,
		json!({ "page": page, "size": size, "filter": filter }),
	)
	.await;
	data["media"].clone()
}

/// Every page of the books, `size` at a time, up to a page past the last
async fn all_pages(app: &TestApp, token: &str, size: u32, filter: Value) -> Vec<Value> {
	let mut pages = Vec::new();
	for page in 1..=6 {
		pages.push(books_page(app, token, page, size, filter.clone()).await);
	}
	pages
}

/// Who sees what: the owner and ben see all books but the removed one, ana sees the general one,
/// and zoe, who was deleted, sees nothing at all
#[tokio::test]
async fn the_world_is_as_the_fixture_says() {
	let (app, world) = world().await;

	let everyone_but_ana = only(&[
		&world.general.id,
		&world.adult.id,
		&world.unrated.id,
		&world.hidden.id,
	]);
	for token in [&world.owner_token, &world.ben.token] {
		let all = all_books(&app, token).await;
		assert_eq!(all, everyone_but_ana);
	}
	assert_eq!(
		all_books(&app, &world.ana.token).await,
		only(&[&world.general.id])
	);

	// the removed book is in the database and out of every list, even the owner's
	assert!(world.removed.deleted_at.is_some());
	// ana's limits are in her account, not in what she asked for
	let ana = user::Entity::find_by_id(world.ana.id.clone())
		.one(app.conn())
		.await
		.unwrap()
		.unwrap();
	assert!(ana.deleted_at.is_none());

	// zoe was deleted: her account is in the database, and she can't use it
	let zoe = user::Entity::find_by_id(world.zoe.id.clone())
		.one(app.conn())
		.await
		.unwrap()
		.unwrap();
	assert!(zoe.deleted_at.is_some());
	app.server
		.post("/api/graphql")
		.add_header("Authorization", format!("Bearer {}", world.zoe.token))
		.json(&json!({ "query": "{ me { id } }" }))
		.await
		.assert_status(StatusCode::UNAUTHORIZED);
}

async fn all_books(app: &TestApp, token: &str) -> Vec<String> {
	books_of(&books_page(app, token, 1, 50, json!({})).await)
}

/// `media`, page by page: nothing of H on any page, and the totals count only what she may read
#[tokio::test]
async fn the_book_list_holds_nothing_hidden_on_any_page() {
	let (app, world) = world().await;

	let mut seen = Vec::new();
	for page in all_pages(&app, &world.ana.token, 1, json!({})).await {
		world.assert_none_hidden(&page);
		seen.extend(books_of(&page));
		assert_eq!(total_items(&page), 1, "{page}");
		assert_eq!(page["pageInfo"]["totalPages"], 1, "{page}");
	}
	assert_eq!(seen, only(&[&world.general.id]));

	// the same list for the owner has them all, over two pages of two
	let mut seen = Vec::new();
	for page in all_pages(&app, &world.owner_token, 2, json!({})).await {
		seen.extend(books_of(&page));
		assert_eq!(total_items(&page), 4, "{page}");
	}
	seen.sort();
	assert_eq!(
		seen,
		only(&[
			&world.general.id,
			&world.adult.id,
			&world.unrated.id,
			&world.hidden.id
		])
	);
}

/// `mediaById` answers a hidden book like a book that doesn't exist
#[tokio::test]
async fn a_hidden_book_by_id_is_a_book_that_does_not_exist() {
	let (app, world) = world().await;
	let query = format!("query Book($id: ID!) {{ mediaById(id: $id) {{ {BOOK} }} }}");

	let book = |token: &str, id: &str| {
		let query = query.clone();
		let token = token.to_string();
		let id = id.to_string();
		let app = &app;
		async move { ask(app, &token, &query, json!({ "id": id })).await["mediaById"].clone() }
	};

	let general = book(&world.ana.token, &world.general.id).await;
	assert_eq!(general["id"], world.general.id.as_str());
	for hidden in world.hidden_from_ana() {
		let found = book(&world.ana.token, &hidden.id).await;
		assert!(found.is_null(), "{} is not hidden: {found}", hidden.name);
	}
	assert!(book(&world.ana.token, "no-such-book").await.is_null());

	// everyone else sees them (and nobody sees the removed one)
	for token in [&world.owner_token, &world.ben.token] {
		for visible in [&world.adult, &world.unrated, &world.hidden] {
			assert_eq!(
				book(token, &visible.id).await["id"],
				visible.id.as_str(),
				"{}",
				visible.name
			);
		}
		assert!(book(token, &world.removed.id).await.is_null());
	}
}

/// Series, libraries and the look-up of a book by its path: what is hidden is not there
#[tokio::test]
async fn hidden_series_libraries_and_paths_are_not_found() {
	let (app, world) = world().await;
	let query = format!(
		"query Shelves($hiddenSeries: ID!, $looseSeries: ID!, $openSeries: ID!, $hiddenLibrary: ID!, $openLibrary: ID!) {{
			series {{ nodes {{ id name }} }}
			libraries {{ nodes {{ id name }} }}
			hiddenSeries: seriesById(id: $hiddenSeries) {{ id }}
			looseSeries: seriesById(id: $looseSeries) {{ id }}
			openSeries: seriesById(id: $openSeries) {{ id name mediaCount }}
			hiddenLibrary: libraryById(id: $hiddenLibrary) {{ id }}
			openLibrary: libraryById(id: $openLibrary) {{ id name }}
		}}"
	);
	let variables = json!({
		"hiddenSeries": world.hidden_series.id,
		"looseSeries": world.loose_series.id,
		"openSeries": world.open_series.id,
		"hiddenLibrary": world.hidden_library.id,
		"openLibrary": world.open.id,
	});

	let shelves = ask(&app, &world.ana.token, &query, variables.clone()).await;

	world.assert_none_hidden(&shelves);
	assert_eq!(
		ids(&shelves["series"]["nodes"]),
		only(&[&world.open_series.id])
	);
	assert_eq!(ids(&shelves["libraries"]["nodes"]), only(&[&world.open.id]));
	assert!(shelves["hiddenSeries"].is_null(), "{shelves}");
	assert!(shelves["looseSeries"].is_null(), "{shelves}");
	assert!(shelves["hiddenLibrary"].is_null(), "{shelves}");
	assert_eq!(shelves["openSeries"]["id"], world.open_series.id.as_str());
	// of the three books of Open shelf she may read one, and it is the one that is counted
	assert_eq!(shelves["openSeries"]["mediaCount"], 1, "{shelves}");
	assert_eq!(shelves["openLibrary"]["id"], world.open.id.as_str());

	// the owner has them all
	let shelves = ask(&app, &world.owner_token, &query, variables).await;
	assert_eq!(shelves["series"]["nodes"].as_array().unwrap().len(), 3);
	assert_eq!(shelves["libraries"]["nodes"].as_array().unwrap().len(), 2);
	assert!(!shelves["hiddenSeries"].is_null() && !shelves["hiddenLibrary"].is_null());

	// a book looked up by its path (what a script that knows the library does)
	let by_path =
		"query ByPath($path: String!) { mediaByPath(path: $path) { id name path } }";
	let found = ask(
		&app,
		&world.ana.token,
		by_path,
		json!({ "path": world.general.path }),
	)
	.await;
	assert_eq!(
		found["mediaByPath"]["id"],
		world.general.id.as_str(),
		"{found}"
	);
	for hidden in world.hidden_from_ana() {
		let found = ask(
			&app,
			&world.ana.token,
			by_path,
			json!({ "path": hidden.path }),
		)
		.await;
		world.assert_none_hidden(&found);
		assert!(found["mediaByPath"].is_null(), "{}: {found}", hidden.name);
	}
}

/// Searching for a word of a hidden book's title, its series or its library finds nothing
#[tokio::test]
async fn search_finds_nothing_hidden() {
	let (app, world) = world().await;

	// "book" is in the title of every book
	let found =
		books_page(&app, &world.ana.token, 1, 50, json!({ "search": "book" })).await;
	world.assert_none_hidden(&found);
	assert_eq!(books_of(&found), only(&[&world.general.id]));
	assert_eq!(total_items(&found), 1);
	assert_eq!(
		books_of(
			&books_page(&app, &world.owner_token, 1, 50, json!({ "search": "book" }))
				.await
		)
		.len(),
		4
	);

	// the words that tell a hidden book from the others
	for (word, expected_for_owner) in [
		("Adult", vec![&world.adult.id]),
		("Unrated", vec![&world.unrated.id]),
		("Removed", vec![]),
		("Hidden", vec![&world.hidden.id]),
	] {
		let found =
			books_page(&app, &world.ana.token, 1, 50, json!({ "search": word })).await;
		world.assert_none_hidden(&found);
		assert!(books_of(&found).is_empty(), "{word}: {found}");
		assert_eq!(total_items(&found), 0, "{word}: {found}");

		let found =
			books_page(&app, &world.owner_token, 1, 50, json!({ "search": word })).await;
		assert_eq!(books_of(&found), only(&expected_for_owner), "{word}");
	}

	// series and libraries are searched the same way
	let query = r#"query Search($word: String!) {
		series(filter: { search: $word }) { nodes { id name } }
		libraries(search: $word) { nodes { id name } }
	}"#;
	for token in [&world.ana.token, &world.owner_token] {
		let found = ask(&app, token, query, json!({ "word": "Hidden" })).await;
		if token == &world.ana.token {
			world.assert_none_hidden(&found);
			assert!(found["series"]["nodes"].as_array().unwrap().is_empty());
			assert!(found["libraries"]["nodes"].as_array().unwrap().is_empty());
		} else {
			assert_eq!(found["series"]["nodes"].as_array().unwrap().len(), 1);
			assert_eq!(found["libraries"]["nodes"].as_array().unwrap().len(), 1);
		}
	}
	let found = ask(&app, &world.ana.token, query, json!({ "word": "shelf" })).await;
	world.assert_none_hidden(&found);
	assert_eq!(
		ids(&found["series"]["nodes"]),
		only(&[&world.open_series.id])
	);
}

/// The random picks (Home's *Picks of the week*, *Surprise me*) never pick a hidden book
#[tokio::test]
async fn random_picks_never_pick_a_hidden_book() {
	let (app, world) = world().await;
	let query = format!(
		"query Pick($seed: String) {{ randomMedia(count: 50, seed: $seed) {{ {BOOK} }} }}"
	);

	for seed in ["2026-W46", "2026-W46#1", "surprise", "x"] {
		let picked = ask(&app, &world.ana.token, &query, json!({ "seed": seed })).await;
		world.assert_none_hidden(&picked);
		assert_eq!(
			ids(&picked["randomMedia"]),
			only(&[&world.general.id]),
			"{seed}"
		);

		let picked = ask(&app, &world.owner_token, &query, json!({ "seed": seed })).await;
		assert_eq!(picked["randomMedia"].as_array().unwrap().len(), 4, "{seed}");
	}
	// without a seed, too
	let picked = ask(&app, &world.ana.token, &query, json!({ "seed": null })).await;
	world.assert_none_hidden(&picked);
}

/// *Recently added* lists what she may read
#[tokio::test]
async fn recently_added_books_are_the_ones_she_may_read() {
	let (app, world) = world().await;
	let query = format!(
		"query Recent {{ recentlyAddedMedia(pagination: {{ offset: {{ page: 1, pageSize: 20 }} }}) {{ nodes {{ {BOOK} }} {PAGE_INFO} }} }}"
	);

	let recent = ask(&app, &world.ana.token, &query, json!({})).await;
	world.assert_none_hidden(&recent);
	assert_eq!(
		books_of(&recent["recentlyAddedMedia"]),
		only(&[&world.general.id])
	);
	assert_eq!(total_items(&recent["recentlyAddedMedia"]), 1);

	let recent = ask(&app, &world.owner_token, &query, json!({})).await;
	assert_eq!(
		recent["recentlyAddedMedia"]["nodes"]
			.as_array()
			.unwrap()
			.len(),
		4
	);
}

/// She read the hidden books before she was restricted: they are still in her history, and in none
/// of what is shown to her
#[tokio::test]
async fn keep_reading_skips_what_she_read_before_the_limits() {
	let (app, world) = world().await;
	for book in world.all_books() {
		fake_data::ReadingSession {
			media_id: book.id.clone(),
			user_id: world.ana.id.clone(),
			end_percentage: 0.5,
			status: ReadingStatus::Reading,
			..Default::default()
		}
		.insert(app.conn())
		.await;
	}
	let query = format!(
		"query KeepReading {{ keepReading(pagination: {{ offset: {{ page: 1, pageSize: 20 }} }}) {{ nodes {{ {BOOK} }} {PAGE_INFO} }} }}"
	);

	let reading = ask(&app, &world.ana.token, &query, json!({})).await;
	world.assert_none_hidden(&reading);
	assert_eq!(
		books_of(&reading["keepReading"]),
		only(&[&world.general.id])
	);
	assert_eq!(total_items(&reading["keepReading"]), 1, "{reading}");
}

const ON_DECK: &str = "query OnDeck { onDeck(pagination: { offset: { page: 1, pageSize: 20 } }) { nodes { id name path seriesId libraryId thumbnail { url } series { id name } library { id name } } pageInfo { __typename ... on OffsetPaginationInfo { totalItems totalPages currentPage } } } }";

/// *On deck* offers the next book of a series that was being read. Ana finished `general`, and
/// the next books of her series are hidden from her, so she is offered none of them
#[tokio::test]
async fn on_deck_offers_no_book_she_may_not_read() {
	let (app, world) = world().await;
	fake_data::ReadingSession::completed(&world.general.id, &world.ana.id)
		.insert(app.conn())
		.await;

	let on_deck = ask(&app, &world.ana.token, ON_DECK, json!({})).await;

	world.assert_none_hidden(&on_deck);
	assert!(books_of(&on_deck["onDeck"]).is_empty(), "{on_deck}");
}

/// The same shelf for someone who may read the next book: it is offered (so that the list above
/// is empty for a reason)
#[tokio::test]
async fn on_deck_offers_the_next_book_to_those_who_may_read_it() {
	let (app, world) = world().await;
	fake_data::ReadingSession::completed(&world.general.id, &world.ben.id)
		.insert(app.conn())
		.await;

	let on_deck = ask(&app, &world.ben.token, ON_DECK, json!({})).await;

	assert_eq!(
		books_of(&on_deck["onDeck"]),
		only(&[&world.adult.id]),
		"{on_deck}"
	);
	assert_eq!(total_items(&on_deck["onDeck"]), 1, "{on_deck}");
}

/// What the on-deck page says it has is what she may read: the total counted the series whose next
/// book is hidden from her, with an empty page
#[tokio::test]
#[ignore = "C5: onDeck.pageInfo.totalItems counts a book she may not read (its query counts before the user's limits are applied)"]
async fn on_deck_counts_only_what_she_may_read() {
	let (app, world) = world().await;
	fake_data::ReadingSession::completed(&world.general.id, &world.ana.id)
		.insert(app.conn())
		.await;

	let on_deck = ask(&app, &world.ana.token, ON_DECK, json!({})).await;

	assert_eq!(total_items(&on_deck["onDeck"]), 0, "{on_deck}");
}

/// What a request got: its status and its body
async fn fetch(app: &TestApp, token: &str, path: &str) -> (u16, Vec<u8>) {
	let response = app.get_with_token(path, token).await;
	(
		response.status_code().as_u16(),
		response.as_bytes().to_vec(),
	)
}

/// A body without what changes from one request to the next: the times a feed was made at
fn without_times(body: &str) -> String {
	fn strip(value: &mut Value) {
		match value {
			Value::Object(fields) => {
				fields.remove("modified");
				fields.values_mut().for_each(strip);
			},
			Value::Array(items) => items.iter_mut().for_each(strip),
			_ => {},
		}
	}

	if let Ok(mut json) = serde_json::from_str::<Value>(body) {
		strip(&mut json);
		return json.to_string();
	}
	// an Atom feed: <updated>…</updated>
	let mut text = String::new();
	let mut rest = body;
	while let Some(start) = rest.find("<updated>") {
		let end = rest[start..]
			.find("</updated>")
			.map_or(rest.len(), |end| start + end);
		text.push_str(&rest[..start]);
		rest = &rest[end..];
	}
	text.push_str(rest);
	text
}

/// What the request for `path` got, with the ID it was made for taken out of the body and the times
/// out of the feed: a hidden ID and one that doesn't exist have to be answered alike, and an answer
/// that repeats the ID it was given tells nothing
async fn answer(app: &TestApp, token: &str, path: &str, id: &str) -> (u16, String) {
	let (status, body) = fetch(app, token, path).await;
	(
		status,
		without_times(&String::from_utf8_lossy(&body)).replace(id, "{id}"),
	)
}

/// `template` with `{id}` replaced
fn at(template: &str, id: &str) -> String {
	template.replace("{id}", id)
}

/// A hidden ID gets the answer of an ID that doesn't exist, on every route that takes one
async fn assert_like_unknown(app: &TestApp, token: &str, template: &str, hidden: &str) {
	let unknown_id = "no-such-id";
	let unknown = answer(app, token, &at(template, unknown_id), unknown_id).await;
	let given = answer(app, token, &at(template, hidden), hidden).await;
	assert_eq!(
		given, unknown,
		"{template} for a hidden ID is not answered like one that doesn't exist"
	);
}

/// A book's thumbnail, its pages and its file: served to those who may read the book, and not found
/// for those who may not (as for an ID that doesn't exist)
#[tokio::test]
async fn a_hidden_book_is_not_served() {
	let (app, mut world) = world().await;
	let _files = world.give_books_files(&app).await;
	// ana may not download; her twin may
	let downloader = world
		.limited_like_ana(&app, "ana-dl", vec![UserPermission::DownloadFile])
		.await;

	let thumbnail = "/api/v2/media/{id}/thumbnail";
	let page = "/api/v2/media/{id}/page/1";
	let file = "/api/v2/media/{id}/file";

	// the book she may read is served
	for route in [thumbnail, page] {
		let (status, body) =
			fetch(&app, &world.ana.token, &at(route, &world.general.id)).await;
		assert_eq!(status, 200, "{route}: {}", String::from_utf8_lossy(&body));
	}
	let (status, _) = fetch(&app, &downloader.token, &at(file, &world.general.id)).await;
	assert_eq!(status, 200, "{file}");
	// downloading is a permission of its own: she has none, whatever the book
	let (status, _) = fetch(&app, &world.ana.token, &at(file, &world.general.id)).await;
	assert_eq!(status, 403);

	// the ones she may not are not found, like a book that isn't there
	for hidden in world.hidden_from_ana() {
		for (route, token) in [
			(thumbnail, &world.ana.token),
			(page, &world.ana.token),
			(file, &downloader.token),
		] {
			assert_like_unknown(&app, token, route, &hidden.id).await;
			let (status, _) = fetch(&app, token, &at(route, &hidden.id)).await;
			assert_eq!(status, 404, "{route} of {}", hidden.name);
		}
		// without the permission to download, the answer is the same for every book
		assert_like_unknown(&app, &world.ana.token, file, &hidden.id).await;
	}

	// the same routes serve them to the owner, whose reading is not limited; the removed book is
	// not served at all (its cover is for library managers, who restore it)
	for visible in [&world.adult, &world.unrated, &world.hidden] {
		for route in [thumbnail, page, file] {
			let (status, _) =
				fetch(&app, &world.owner_token, &at(route, &visible.id)).await;
			assert_eq!(status, 200, "{route} of {}", visible.name);
		}
	}
	for route in [page, file] {
		let (status, _) =
			fetch(&app, &world.owner_token, &at(route, &world.removed.id)).await;
		assert_eq!(status, 404, "{route} of the removed book");
	}
}

/// A feed's text, which fails the test when the request failed
async fn feed(app: &TestApp, token: &str, path: &str) -> String {
	let (status, body) = fetch(app, token, path).await;
	let body = String::from_utf8_lossy(&body).to_string();
	assert_eq!(status, 200, "{path}: {body}");
	body
}

/// Everything in the OPDS 1.2 catalog that lists books, series or libraries
#[tokio::test]
async fn opds_1_2_feeds_hold_nothing_hidden() {
	let (app, mut world) = world().await;
	let _files = world.give_books_files(&app).await;
	let root = "/opds/v1.2";
	let open = &world.open.id;
	let open_series = &world.open_series.id;

	let mut feeds = vec![
		format!("{root}/keep-reading"),
		format!("{root}/smart-lists"),
		format!("{root}/reading-lists"),
		format!("{root}/authors"),
		format!("{root}/genres"),
		format!("{root}/libraries"),
		format!("{root}/libraries/{open}"),
		format!("{root}/libraries/{open}/books"),
		format!("{root}/series"),
		format!("{root}/series/latest"),
		format!("{root}/series/{open_series}"),
		format!("{root}/books"),
		format!("{root}/books/latest"),
	];
	for word in ["book", "Adult", "Unrated", "Removed", "Hidden", "shelf"] {
		feeds.push(format!("{root}/search/feed?search={word}"));
	}

	for path in &feeds {
		let text = feed(&app, &world.ana.token, path).await;
		world.assert_text_none_hidden(&text);
	}
	// the book she may read is in the feeds that list books, so an empty feed is not a broken one
	let books = feed(&app, &world.ana.token, &format!("{root}/books")).await;
	assert!(books.contains(&world.general.id), "{books}");
	let owner_books = feed(&app, &world.owner_token, &format!("{root}/books")).await;
	for visible in [&world.adult, &world.unrated, &world.hidden] {
		assert!(owner_books.contains(&visible.id), "{owner_books}");
	}

	// a hidden library, series or book is not found, like one that doesn't exist
	let hidden_library = &world.hidden_library.id;
	let hidden_series = &world.hidden_series.id;
	for template in [
		format!("{root}/libraries/{{id}}"),
		format!("{root}/libraries/{{id}}/books"),
	] {
		assert_like_unknown(&app, &world.ana.token, &template, hidden_library).await;
	}
	assert_like_unknown(
		&app,
		&world.ana.token,
		&format!("{root}/series/{{id}}"),
		hidden_series,
	)
	.await;
	let downloader = world
		.limited_like_ana(&app, "ana-dl", vec![UserPermission::DownloadFile])
		.await;
	for hidden in world.hidden_from_ana() {
		for (route, token) in [
			(format!("{root}/books/{{id}}/thumbnail"), &world.ana.token),
			(format!("{root}/books/{{id}}/pages/1"), &world.ana.token),
			(
				format!("{root}/books/{{id}}/file/book.cbz"),
				&downloader.token,
			),
		] {
			assert_like_unknown(&app, token, &route, &hidden.id).await;
			let (status, _) = fetch(&app, token, &at(&route, &hidden.id)).await;
			assert_eq!(status, 404, "{route} of {}", hidden.name);
		}
	}
	// and the owner gets what ana is refused
	for route in ["thumbnail", "pages/1"] {
		let (status, _) = fetch(
			&app,
			&world.owner_token,
			&format!("{root}/books/{}/{route}", world.adult.id),
		)
		.await;
		assert_eq!(status, 200, "{route}");
	}
}

/// Everything in the OPDS 2 catalog that lists books, series or libraries
#[tokio::test]
async fn opds_2_feeds_hold_nothing_hidden() {
	let (app, mut world) = world().await;
	let _files = world.give_books_files(&app).await;
	let root = "/opds/v2.0";
	let open = &world.open.id;
	let open_series = &world.open_series.id;

	let mut feeds = vec![
		format!("{root}/catalog"),
		format!("{root}/libraries"),
		format!("{root}/libraries/{open}"),
		format!("{root}/libraries/{open}/books"),
		format!("{root}/libraries/{open}/books/latest"),
		format!("{root}/libraries/{open}/series"),
		format!("{root}/series"),
		format!("{root}/series/{open_series}"),
		format!("{root}/books/browse"),
		format!("{root}/books/latest"),
		format!("{root}/books/keep-reading"),
	];
	for word in ["book", "Adult", "Unrated", "Removed", "Hidden", "shelf"] {
		feeds.push(format!("{root}/search?query={word}"));
	}

	for path in &feeds {
		let text = feed(&app, &world.ana.token, path).await;
		world.assert_text_none_hidden(&text);
	}
	let books = feed(&app, &world.ana.token, &format!("{root}/books/browse")).await;
	assert!(books.contains(&world.general.id), "{books}");
	let owner_books =
		feed(&app, &world.owner_token, &format!("{root}/books/browse")).await;
	for visible in [&world.adult, &world.unrated, &world.hidden] {
		assert!(owner_books.contains(&visible.id), "{owner_books}");
	}

	// a hidden library, series or book is not found, like one that doesn't exist
	let hidden_library = &world.hidden_library.id;
	let hidden_series = &world.hidden_series.id;
	for template in [
		format!("{root}/libraries/{{id}}"),
		format!("{root}/libraries/{{id}}/books"),
		format!("{root}/libraries/{{id}}/series"),
	] {
		assert_like_unknown(&app, &world.ana.token, &template, hidden_library).await;
	}
	assert_like_unknown(
		&app,
		&world.ana.token,
		&format!("{root}/series/{{id}}"),
		hidden_series,
	)
	.await;
	let downloader = world
		.limited_like_ana(&app, "ana-dl", vec![UserPermission::DownloadFile])
		.await;
	for hidden in world.hidden_from_ana() {
		for (route, token) in [
			(format!("{root}/books/{{id}}"), &world.ana.token),
			(format!("{root}/books/{{id}}/thumbnail"), &world.ana.token),
			(format!("{root}/books/{{id}}/pages/1"), &world.ana.token),
			(format!("{root}/books/{{id}}/file"), &downloader.token),
		] {
			assert_like_unknown(&app, token, &route, &hidden.id).await;
			let (status, _) = fetch(&app, token, &at(&route, &hidden.id)).await;
			assert_eq!(status, 404, "{route} of {}", hidden.name);
		}
	}
	for route in ["", "/thumbnail", "/pages/1"] {
		let (status, _) = fetch(
			&app,
			&world.owner_token,
			&format!("{root}/books/{}{route}", world.adult.id),
		)
		.await;
		assert_eq!(status, 200, "{route}");
	}
}

/// The position of a book (the OPDS 2 progression, which KOReader-like readers ask for) is the
/// user's own, found by the book's ID. A hidden book is answered like one that doesn't exist even
/// when she has a position in it from before the limits: the position says the book exists and
/// where it ends
#[tokio::test]
#[ignore = "C5: the OPDS 2 progression of a hidden book returns the user's old position (no access check on the route), which tells the book exists"]
async fn opds_2_progression_gives_nothing_away_about_a_hidden_book() {
	let (app, world) = world().await;
	for book in world.hidden_from_ana() {
		fake_data::ReadingSession {
			media_id: book.id.clone(),
			user_id: world.ana.id.clone(),
			end_percentage: 0.5,
			status: ReadingStatus::Reading,
			..Default::default()
		}
		.insert(app.conn())
		.await;
	}

	for hidden in world.hidden_from_ana() {
		assert_like_unknown(
			&app,
			&world.ana.token,
			"/opds/v2.0/books/{id}/progression",
			&hidden.id,
		)
		.await;
	}
}
