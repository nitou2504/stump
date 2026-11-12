//! The search box: the best title first (`media(relevanceTo:)`) and the authors group
//! (`searchAuthors`).
//!
//! Ranking only changes the order of a search, never what it finds, and the authors are read from
//! the books a user may see, so a user with limits (ana, of [`VisibilityFixture`]) gets no author
//! and no count that comes from a book hidden from her.

use models::entity::{media_metadata, series};
use sea_orm::{prelude::*, sea_query::Expr, QueryFilter};
use serde_json::{json, Value};

use crate::common::{
	visibility::{add_book, ids, sorted, Meta, VisibilityFixture},
	TestApp,
};

async fn world() -> (TestApp, VisibilityFixture) {
	let app = TestApp::new_with_default_user().await;
	let world = VisibilityFixture::build(&app).await;
	(app, world)
}

/// Runs a query as `token`, and fails if it fails
async fn ask(app: &TestApp, token: &str, query: &str, variables: Value) -> Value {
	let response = app
		.execute_gql_with_token(query, Some(variables), token)
		.await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"].clone()
}

/// A visible book of the open series, with the writers given (a comma-separated list)
async fn book(
	app: &TestApp,
	series: &series::Model,
	name: &str,
	writers: Option<&str>,
) -> models::entity::media::Model {
	add_book(
		app,
		series,
		name,
		Meta {
			age: Some(0),
			writer: writers,
			..Default::default()
		},
	)
	.await
}

/// The title a book is shown with when it has one of its own (its file name stays what it was)
async fn give_title(app: &TestApp, book: &models::entity::media::Model, title: &str) {
	let done = media_metadata::Entity::update_many()
		.col_expr(media_metadata::Column::Title, Expr::value(title))
		.filter(media_metadata::Column::MediaId.eq(book.id.clone()))
		.exec(app.conn())
		.await
		.expect("should give the title");
	assert_eq!(done.rows_affected, 1, "the book has its metadata row");
}

const SEARCH: &str = r#"query Search($search: String!, $relevanceTo: String, $orderBy: [MediaOrderBy!]!, $page: Int!, $size: Int!) {
	media(
		filter: { search: $search }
		relevanceTo: $relevanceTo
		orderBy: $orderBy
		pagination: { offset: { page: $page, pageSize: $size } }
	) {
		nodes { id resolvedName }
		pageInfo { __typename ... on OffsetPaginationInfo { totalItems } }
	}
}"#;

/// The order a list has when none is asked for
fn by_name() -> Value {
	json!([{ "media": { "field": "NAME", "direction": "ASC" } }])
}

/// The titles found, in the order they come
async fn found(
	app: &TestApp,
	token: &str,
	search: &str,
	relevance_to: Option<&str>,
	order_by: Option<Value>,
	page: i32,
	size: i32,
) -> Vec<String> {
	let data = ask(
		app,
		token,
		SEARCH,
		json!({
			"search": search,
			"relevanceTo": relevance_to,
			"orderBy": order_by.unwrap_or_else(by_name),
			"page": page,
			"size": size,
		}),
	)
	.await;
	data["media"]["nodes"]
		.as_array()
		.expect("a list")
		.iter()
		.map(|node| node["resolvedName"].as_str().expect("a title").to_string())
		.collect()
}

/// The exact title is first, then the titles that start with the search, then those with a word
/// that does, then the books found another way; each group in the order asked for (the name by
/// default). The title is the one a book is shown with, and ranking holds across pages.
#[tokio::test]
async fn the_best_title_is_first() {
	let (app, world) = world().await;
	let shelf = &world.open_series;
	for (name, writers) in [
		("Dune", None),
		("Dune Messiah", None),
		("Dune: Deluxe Edition", None),
		("Dunedin Guide", None),
		("Children of Dune", None),
		("Chapterhouse: Dune", None),
		("The Dunes of Mars", None),
		// found by its author only
		("Arrakis", Some("Frank Dune")),
	] {
		book(&app, shelf, name, writers).await;
	}
	// a scan with a file name nobody would search for, and the title it is shown with
	let scan = book(&app, shelf, "zzz_scan_0042", None).await;
	give_title(&app, &scan, "Dune Prophecy").await;

	let token = &world.owner_token;
	let ranked = [
		"Dune",
		// they start with it, by name
		"Dune Messiah",
		"Dune: Deluxe Edition",
		"Dunedin Guide",
		"Dune Prophecy",
		// a word of them starts with it
		"Chapterhouse: Dune",
		"Children of Dune",
		"The Dunes of Mars",
		// found by the author
		"Arrakis",
	];

	let by_name = found(&app, token, "dune", None, None, 1, 50).await;
	assert_eq!(
		by_name[0], "Arrakis",
		"without ranking the order is the name's: {by_name:?}"
	);
	assert_eq!(sorted(by_name), sorted(ranked.map(String::from).to_vec()));

	assert_eq!(
		found(&app, token, "dune", Some("dune"), None, 1, 50).await,
		ranked
	);
	// the case of the text doesn't matter
	assert_eq!(
		found(&app, token, "DUNE", Some("DuNe"), None, 1, 50).await,
		ranked
	);

	// the pages hold the same order, and only the owner's `orderBy` decides inside a group
	let mut paged = Vec::new();
	for page in 1..=5 {
		paged.extend(found(&app, token, "dune", Some("dune"), None, page, 2).await);
	}
	assert_eq!(paged, ranked);

	let descending = found(
		&app,
		token,
		"dune",
		Some("dune"),
		Some(json!([{ "media": { "field": "NAME", "direction": "DESC" } }])),
		1,
		50,
	)
	.await;
	assert_eq!(
		descending,
		[
			"Dune",
			"Dune Prophecy",
			"Dunedin Guide",
			"Dune: Deluxe Edition",
			"Dune Messiah",
			"The Dunes of Mars",
			"Children of Dune",
			"Chapterhouse: Dune",
			"Arrakis",
		]
	);
}

/// Accents and case don't count, and the text ranks only the books the search found: the
/// results of a search are the same set with or without it
#[tokio::test]
async fn ranking_ignores_accents_and_never_changes_what_is_found() {
	let (app, world) = world().await;
	let shelf = &world.open_series;
	for name in ["Atlas de Física", "Física Cuántica", "Física", "Química"] {
		book(&app, shelf, name, None).await;
	}
	let token = &world.owner_token;

	assert_eq!(
		found(&app, token, "fisica", Some("fisica"), None, 1, 50).await,
		["Física", "Física Cuántica", "Atlas de Física"]
	);
	assert_eq!(
		found(&app, token, "FÍSICA", Some("fisica"), None, 1, 50).await,
		["Física", "Física Cuántica", "Atlas de Física"]
	);
	// a text of another search ranks, it doesn't filter
	let with = found(&app, token, "fisica", Some("quimica"), None, 1, 50).await;
	let without = found(&app, token, "fisica", None, None, 1, 50).await;
	assert_eq!(sorted(with), sorted(without));
	// a text with no words ranks nothing
	assert_eq!(
		found(&app, token, "fisica", Some("  "), None, 1, 50).await,
		found(&app, token, "fisica", None, None, 1, 50).await
	);
}

const PAGE: &str = r#"query Page($search: String!, $relevanceTo: String, $page: Int!, $size: Int!) {
	media(
		filter: { search: $search }
		relevanceTo: $relevanceTo
		pagination: { offset: { page: $page, pageSize: $size } }
	) {
		nodes { id }
		pageInfo { __typename ... on OffsetPaginationInfo { totalItems totalPages currentPage } }
	}
}"#;

/// One page of a search: the IDs in the order they come, the total, the pages and the page
async fn page_of(
	app: &TestApp,
	token: &str,
	search: &str,
	relevance_to: Option<&str>,
	page: i32,
	size: i32,
) -> (Vec<String>, i64, i64, i64) {
	let data = ask(
		app,
		token,
		PAGE,
		json!({ "search": search, "relevanceTo": relevance_to, "page": page, "size": size }),
	)
	.await;
	let info = &data["media"]["pageInfo"];
	(
		data["media"]["nodes"]
			.as_array()
			.expect("a list")
			.iter()
			.map(|node| node["id"].as_str().expect("an id").to_string())
			.collect(),
		info["totalItems"].as_i64().expect("a total"),
		info["totalPages"].as_i64().expect("pages"),
		info["currentPage"].as_i64().expect("a page"),
	)
}

/// A ranked search counts and pages like the plain one: the same total and pages, the same books on
/// the pages (a page past the end is empty), and the total of a user with limits leaves out what is
/// hidden from her
#[tokio::test]
async fn a_ranked_search_counts_and_pages_like_a_plain_one() {
	let (app, world) = world().await;
	for n in 1..=7 {
		book(&app, &world.open_series, &format!("Atlas {n}"), None).await;
	}
	// two more that only people without limits may read
	for n in 1..=2 {
		book(&app, &world.hidden_series, &format!("Atlas hidden {n}"), None).await;
	}

	for (token, total) in [(&world.owner_token, 9), (&world.ana.token, 7)] {
		let mut ranked_books = Vec::new();
		let mut plain_books = Vec::new();
		for page in 1..=4 {
			let ranked = page_of(&app, token, "atlas", Some("atlas"), page, 3).await;
			let plain = page_of(&app, token, "atlas", None, page, 3).await;
			// the total, the pages and the page are the plain search's
			assert_eq!((ranked.1, ranked.2, ranked.3), (plain.1, plain.2, plain.3), "page {page}");
			assert_eq!(ranked.1, total, "page {page}");
			assert_eq!(ranked.0.len(), plain.0.len(), "page {page}");
			ranked_books.extend(ranked.0);
			plain_books.extend(plain.0);
		}
		// no book twice, none missing, none that is hidden: the pages hold the same books
		assert_eq!(ranked_books.len(), total as usize);
		assert_eq!(sorted(ranked_books), sorted(plain_books));
	}
	// past the end there is nothing to show, and the total is still the total
	let (books, total, _, _) =
		page_of(&app, &world.ana.token, "atlas", Some("atlas"), 9, 3).await;
	assert!(books.is_empty());
	assert_eq!(total, 7);
}

/// A rank is not a cursor: the cursor pages say so instead of paging in a wrong order
#[tokio::test]
async fn ranked_results_do_not_page_with_a_cursor() {
	let (app, world) = world().await;
	let response = app
		.execute_gql_with_token(
			r#"query { media(filter: { search: "book" }, relevanceTo: "book", pagination: { cursor: { limit: 2 } }) { nodes { id } } }"#,
			None,
			&world.owner_token,
		)
		.await;
	assert!(
		response["errors"].to_string().contains("cursor"),
		"{response}"
	);
	// without the text the cursor pages as ever
	let data = ask(
		&app,
		&world.owner_token,
		r#"query { media(filter: { search: "book" }, pagination: { cursor: { limit: 2 } }) { nodes { id } } }"#,
		json!({}),
	)
	.await;
	assert_eq!(data["media"]["nodes"].as_array().unwrap().len(), 2);
}

/// Ranking is not a way round the limits: ana gets the same visible books ranked, and nothing
/// that comes from her hidden set, with or without the text
#[tokio::test]
async fn a_friend_gets_the_visible_books_ranked_and_nothing_else() {
	let (app, world) = world().await;
	for word in ["book", "Adult", "Hidden", "Unrated", "Removed", "shelf"] {
		let ranked = ask(
			&app,
			&world.ana.token,
			SEARCH,
			json!({ "search": word, "relevanceTo": word, "orderBy": by_name(), "page": 1, "size": 50 }),
		)
		.await;
		world.assert_none_hidden(&ranked);
		let plain = ask(
			&app,
			&world.ana.token,
			SEARCH,
			json!({ "search": word, "relevanceTo": null, "orderBy": by_name(), "page": 1, "size": 50 }),
		)
		.await;
		assert_eq!(
			ids(&ranked["media"]["nodes"]),
			ids(&plain["media"]["nodes"]),
			"{word}"
		);
		assert_eq!(
			ranked["media"]["pageInfo"]["totalItems"],
			plain["media"]["pageInfo"]["totalItems"],
			"{word}"
		);
	}
	let ranked = ask(
		&app,
		&world.ana.token,
		SEARCH,
		json!({ "search": "book", "relevanceTo": "book", "orderBy": by_name(), "page": 1, "size": 50 }),
	)
	.await;
	assert_eq!(
		ids(&ranked["media"]["nodes"]),
		sorted(vec![world.general.id.clone()])
	);
}

const AUTHORS: &str = r#"query Authors($search: String!, $limit: Int!) {
	searchAuthors(search: $search, limit: $limit) { value count }
}"#;

/// The same without a limit, which is 4
const AUTHORS_DEFAULT_LIMIT: &str = r#"query Authors($search: String!) {
	searchAuthors(search: $search) { value count }
}"#;

/// What `searchAuthors` answers: (name, books), in the order they come
async fn authors(
	app: &TestApp,
	token: &str,
	search: &str,
	limit: Option<i32>,
) -> Vec<(String, u64)> {
	let data = match limit {
		Some(limit) => {
			ask(app, token, AUTHORS, json!({ "search": search, "limit": limit })).await
		},
		None => {
			ask(app, token, AUTHORS_DEFAULT_LIMIT, json!({ "search": search })).await
		},
	};
	data["searchAuthors"]
		.as_array()
		.expect("a list")
		.iter()
		.map(|author| {
			(
				author["value"].as_str().expect("a name").to_string(),
				author["count"].as_u64().expect("a count"),
			)
		})
		.collect()
}

fn named(authors: &[(&str, u64)]) -> Vec<(String, u64)> {
	authors
		.iter()
		.map(|(name, count)| (String::from(*name), *count))
		.collect()
}

/// The authors of the world: **Shared Author** wrote a book ana may read, one rated 18 and one in
/// the library hidden from her; **Open Author** wrote the one she may read; **Adult**, **Unrated**
/// and **Hidden Author** wrote only books she may not; **Removed Author**'s book was taken out of
/// the library, so nobody sees it.
async fn add_authors(app: &TestApp, world: &VisibilityFixture) {
	add_book(
		app,
		&world.open_series,
		"Open one",
		Meta {
			age: Some(0),
			writer: Some("Shared Author, Open Author"),
			..Default::default()
		},
	)
	.await;
	add_book(
		app,
		&world.open_series,
		"Adult one",
		Meta {
			age: Some(18),
			writer: Some("Shared Author, Adult Author"),
			..Default::default()
		},
	)
	.await;
	add_book(
		app,
		&world.loose_series,
		"Unrated one",
		Meta {
			age: None,
			writer: Some("Unrated Author"),
			..Default::default()
		},
	)
	.await;
	add_book(
		app,
		&world.hidden_series,
		"Hidden one",
		Meta {
			age: Some(0),
			writer: Some("Shared Author, Hidden Author"),
			..Default::default()
		},
	)
	.await;
	let removed = book(app, &world.open_series, "Removed one", Some("Removed Author")).await;
	let response = app
		.execute_gql(
			r#"mutation RemoveBook($id: ID!) { deleteMedia(id: $id) { id } }"#,
			Some(json!({ "id": removed.id })),
		)
		.await;
	assert!(response.get("errors").is_none(), "{response}");
}

/// A user with limits gets only the authors of books she may read, counted by those books; the
/// same search by someone with no limits shows what the limits cut (so an empty answer is the rule
/// at work and not a wrong request)
#[tokio::test]
async fn a_friend_gets_only_the_authors_of_books_she_may_read() {
	let (app, world) = world().await;
	add_authors(&app, &world).await;

	// everybody without limits: every author of a book that is in the library, counted by its books
	for token in [&world.owner_token, &world.ben.token] {
		assert_eq!(
			authors(&app, token, "author", Some(20)).await,
			named(&[
				("Shared Author", 3),
				("Adult Author", 1),
				("Hidden Author", 1),
				("Open Author", 1),
				("Unrated Author", 1),
			])
		);
	}

	// ana: Shared Author's other two books are not in her count, and the authors who wrote only
	// hidden books are not there at all
	assert_eq!(
		authors(&app, &world.ana.token, "author", Some(20)).await,
		named(&[("Open Author", 1), ("Shared Author", 1)])
	);
	// searching for a hidden author by name, whole or in part, finds nothing
	for name in [
		"Adult Author",
		"adult",
		"Unrated",
		"hidden author",
		"Hidden",
		"Removed",
	] {
		let found = authors(&app, &world.ana.token, name, Some(20)).await;
		assert!(found.is_empty(), "{name}: {found:?}");
	}
	// the owner finds them, but never an author of a book taken out of the library
	assert_eq!(
		authors(&app, &world.owner_token, "adult", None).await,
		named(&[("Adult Author", 1)])
	);
	assert!(authors(&app, &world.owner_token, "removed", None)
		.await
		.is_empty());

	// whatever she asks for, nothing of the hidden set is in the answer
	let data = ask(
		&app,
		&world.ana.token,
		AUTHORS,
		json!({ "search": "author", "limit": 20 }),
	)
	.await;
	world.assert_none_hidden(&data);
	for hidden in ["Adult Author", "Hidden Author", "Unrated Author", "Removed Author"] {
		assert!(!data.to_string().contains(hidden), "{hidden}: {data}");
	}
}

/// The text is searched like the books': any order of the words, each the start of a word, case
/// and accents ignored; the best first; the limit holds; no words, no authors
#[tokio::test]
async fn authors_are_found_by_the_start_of_their_words_best_first() {
	let (app, world) = world().await;
	for (name, writers) in [
		("One", "Gabriel García Márquez"),
		("Two", "Isaac Asimov"),
		("Three", "Isaac Asimov"),
		("Four", "Janet Asimov"),
		("Five", "Asimov"),
	] {
		book(&app, &world.open_series, name, Some(writers)).await;
	}
	let token = &world.owner_token;

	assert_eq!(
		authors(&app, token, "GARCIA marq", None).await,
		named(&[("Gabriel García Márquez", 1)])
	);
	assert_eq!(
		authors(&app, token, "asimov", None).await,
		// the name that is the search, then the one with more books, then the other
		named(&[("Asimov", 1), ("Isaac Asimov", 2), ("Janet Asimov", 1)])
	);
	assert_eq!(
		authors(&app, token, "asimov", Some(2)).await,
		named(&[("Asimov", 1), ("Isaac Asimov", 2)])
	);
	// at least one, whatever is asked
	assert_eq!(authors(&app, token, "asimov", Some(0)).await.len(), 1);
	// the middle of a word is not the start of one, and no words match no author
	assert!(authors(&app, token, "simov", None).await.is_empty());
	for nothing in ["", "   ", "&&&"] {
		assert!(authors(&app, token, nothing, None).await.is_empty(), "{nothing:?}");
	}
}
