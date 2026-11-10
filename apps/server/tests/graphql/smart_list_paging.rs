//! A smart list holds at most 200 books at a time (the query complexity limit prices every list at
//! that size), so the web app reads a longer one page by page: `smartListItems(limit, offset)`.
//! The books come in a fixed order, so the pages follow each other without gaps or repeats, and
//! `smartListMeta` (which book-curator reads) still counts every match.

use crate::common::{series::setup_single_series_with_n_books, TestApp};
use sea_orm::ConnectionTrait;
use serde_json::{json, Value};
use std::collections::HashSet;
use tests::fake_data;

/// A list with no rules matches every book
const CREATE_LIST: &str = r#"
mutation CreateList($input: SaveSmartListInput!) {
	createSmartList(input: $input) { id }
}
"#;

const ITEMS: &str = r#"
query Items($id: ID!, $limit: Int, $offset: Int, $grouping: SmartListGrouping) {
	smartListItems(id: $id, limit: $limit, offset: $offset, grouping: $grouping) {
		__typename
		... on SmartListUngrouped { books { id } }
		... on SmartListGrouped { items { entity { __typename ... on Series { id } } books { id } } }
	}
}
"#;

async fn make_list(app: &TestApp, grouping: &str) -> String {
	let response = app
		.execute_gql(
			CREATE_LIST,
			Some(json!({ "input": {
				"filters": [],
				"name": "everything",
				"joiner": "AND",
				"defaultGrouping": grouping,
				"visibility": "PRIVATE",
			}})),
		)
		.await;
	assert!(response.get("errors").is_none(), "{response:#}");
	response["data"]["createSmartList"]["id"]
		.as_str()
		.expect("an id")
		.to_string()
}

/// The ids of the books of an ungrouped page
fn ungrouped_ids(response: &Value) -> Vec<String> {
	assert!(response.get("errors").is_none(), "{response:#}");
	response["data"]["smartListItems"]["books"]
		.as_array()
		.unwrap_or_else(|| panic!("not an ungrouped page: {response:#}"))
		.iter()
		.map(|book| book["id"].as_str().unwrap().to_string())
		.collect()
}

async fn page(
	app: &TestApp,
	list: &str,
	limit: Option<u64>,
	offset: Option<u64>,
) -> Value {
	app.execute_gql(
		ITEMS,
		Some(json!({ "id": list, "limit": limit, "offset": offset })),
	)
	.await
}

/// 250 books are 3 pages of 100: every book once, in the same order whatever the page size, and
/// nothing after the last one
#[tokio::test]
async fn a_long_list_is_read_page_by_page() {
	let app = TestApp::new_with_default_user().await;
	setup_single_series_with_n_books(&app, fake_data::Series::default(), 250).await;
	let list = make_list(&app, "BY_BOOKS").await;

	let first = ungrouped_ids(&page(&app, &list, Some(100), Some(0)).await);
	let second = ungrouped_ids(&page(&app, &list, Some(100), Some(100)).await);
	let third = ungrouped_ids(&page(&app, &list, Some(100), Some(200)).await);
	assert_eq!((first.len(), second.len(), third.len()), (100, 100, 50));

	let all: Vec<String> = [first, second, third].concat();
	let distinct: HashSet<&String> = all.iter().collect();
	assert_eq!(distinct.len(), 250, "a book was left out or came twice");

	// the same books in the same order from pages of another size
	let mut other_pages = Vec::new();
	for offset in (0..250).step_by(75) {
		other_pages.extend(ungrouped_ids(
			&page(&app, &list, Some(75), Some(offset)).await,
		));
	}
	assert_eq!(other_pages, all);

	// past the end there is nothing, and that is no error
	let past_the_end = page(&app, &list, Some(100), Some(250)).await;
	assert_eq!(ungrouped_ids(&past_the_end), Vec::<String>::new());
	let far_past_the_end = page(&app, &list, Some(100), Some(u32::MAX as u64)).await;
	assert_eq!(ungrouped_ids(&far_past_the_end), Vec::<String>::new());
}

/// Nothing a client sends gets more than 200 books out of one page, but the first 200 are there
/// without any argument (a list with no `limit` is what older clients ask for)
#[tokio::test]
async fn a_page_still_holds_at_most_200_books() {
	let app = TestApp::new_with_default_user().await;
	setup_single_series_with_n_books(&app, fake_data::Series::default(), 250).await;
	let list = make_list(&app, "BY_BOOKS").await;

	let without_limit = ungrouped_ids(&page(&app, &list, None, None).await);
	assert_eq!(without_limit.len(), 200);
	let huge_limit = ungrouped_ids(&page(&app, &list, Some(5000), None).await);
	assert_eq!(huge_limit, without_limit);

	// the 50 left are the next page
	let rest = ungrouped_ids(&page(&app, &list, None, Some(200)).await);
	assert_eq!(rest.len(), 50);
	assert!(rest.iter().all(|id| !without_limit.contains(id)));
}

/// A list grouped by series pages through its books: a series can be split over two pages, and
/// the web app puts it back together by the series' id
#[tokio::test]
async fn a_grouped_list_is_read_page_by_page_too() {
	let app = TestApp::new_with_default_user().await;
	let (series, _) =
		setup_single_series_with_n_books(&app, fake_data::Series::default(), 130).await;
	let (other_series, _) =
		setup_single_series_with_n_books(&app, fake_data::Series::default(), 60).await;
	let list = make_list(&app, "BY_SERIES").await;

	let mut books_of: std::collections::HashMap<String, HashSet<String>> =
		Default::default();
	let mut pages = 0;
	for offset in (0..190).step_by(100) {
		let response = page(&app, &list, Some(100), Some(offset)).await;
		assert!(response.get("errors").is_none(), "{response:#}");
		let items = &response["data"]["smartListItems"];
		assert_eq!(items["__typename"], "SmartListGrouped", "{response:#}");
		for item in items["items"].as_array().unwrap() {
			let series_id = item["entity"]["id"].as_str().unwrap().to_string();
			for book in item["books"].as_array().unwrap() {
				let fresh = books_of
					.entry(series_id.clone())
					.or_default()
					.insert(book["id"].as_str().unwrap().to_string());
				assert!(fresh, "a book came on two pages: {book}");
			}
		}
		pages += 1;
	}
	assert_eq!(pages, 2);
	assert_eq!(books_of[&series.id].len(), 130);
	assert_eq!(books_of[&other_series.id].len(), 60);
}

/// book-curator counts the matches of a list with `smartListMeta`: that is not a list of books,
/// and no page, so it counts them all
#[tokio::test]
async fn the_meta_of_a_list_still_counts_every_match() {
	let app = TestApp::new_with_default_user().await;
	setup_single_series_with_n_books(&app, fake_data::Series::default(), 250).await;
	let list = make_list(&app, "BY_BOOKS").await;

	let response = app
		.execute_gql(
			r#"query($id: ID!) { smartListMeta(id: $id) { matchedBooks matchedSeries } }"#,
			Some(json!({ "id": list })),
		)
		.await;
	assert!(response.get("errors").is_none(), "{response:#}");
	assert_eq!(response["data"]["smartListMeta"]["matchedBooks"], 250);
	assert_eq!(response["data"]["smartListMeta"]["matchedSeries"], 1);
}

// ---------------------------------------------------------------------------------------
// The cursor: the same pages, without a book skipped or repeated when the list changes
// ---------------------------------------------------------------------------------------

const CURSOR_ITEMS: &str = r#"
query Items($id: ID!, $limit: Int, $offset: Int, $after: ID, $grouping: SmartListGrouping) {
	smartListItems(id: $id, limit: $limit, offset: $offset, after: $after, grouping: $grouping) {
		__typename
		... on SmartListUngrouped { nextCursor books { id } }
		... on SmartListGrouped { nextCursor items { entity { __typename ... on Series { id } } books { id } } }
	}
}
"#;

async fn cursor_page(
	app: &TestApp,
	list: &str,
	limit: u64,
	after: Option<&str>,
) -> Value {
	app.execute_gql(
		CURSOR_ITEMS,
		Some(json!({ "id": list, "limit": limit, "after": after })),
	)
	.await
}

/// The next cursor of a page of `smartListItems` (null on the last page)
fn next_cursor(response: &Value) -> Option<String> {
	assert!(response.get("errors").is_none(), "{response:#}");
	response["data"]["smartListItems"]["nextCursor"]
		.as_str()
		.map(str::to_string)
}

/// 250 books are 3 pages of 100 by cursor: every book once, in the same order as by offset, the
/// cursor of each page leads to the next, and the last page has none
#[tokio::test]
async fn a_long_list_is_read_by_cursor() {
	let app = TestApp::new_with_default_user().await;
	setup_single_series_with_n_books(&app, fake_data::Series::default(), 250).await;
	let list = make_list(&app, "BY_BOOKS").await;

	let mut expected = Vec::new();
	for offset in [0, 100, 200] {
		expected.extend(ungrouped_ids(
			&page(&app, &list, Some(100), Some(offset)).await,
		));
	}

	let mut by_cursor = Vec::new();
	let mut cursor: Option<String> = None;
	let mut pages = 0;
	loop {
		let response = cursor_page(&app, &list, 100, cursor.as_deref()).await;
		by_cursor.extend(ungrouped_ids(&response));
		pages += 1;
		cursor = next_cursor(&response);
		if cursor.is_none() {
			break;
		}
		assert!(pages < 10, "the pages never end");
	}
	assert_eq!(pages, 3);
	assert_eq!(by_cursor, expected, "the same books in the same order");
	assert_eq!(by_cursor.len(), 250);

	// a list of exactly one page has no next page
	let first = cursor_page(&app, &list, 250, None).await;
	assert_eq!(
		ungrouped_ids(&first).len(),
		200,
		"a page holds at most 200 books"
	);
	assert!(next_cursor(&first).is_some());
	let last = cursor_page(&app, &list, 100, Some(&by_cursor[249])).await;
	assert_eq!(ungrouped_ids(&last), Vec::<String>::new());
	assert_eq!(next_cursor(&last), None);
}

/// A book added while a list is being read doesn't make the cursor skip or repeat one, which an
/// offset does: the pages by offset shift by the new book, so the last book of a page comes again
#[tokio::test]
async fn a_cursor_survives_a_book_added_between_two_pages() {
	let app = TestApp::new_with_default_user().await;
	let (series, _) =
		setup_single_series_with_n_books(&app, fake_data::Series::default(), 250).await;
	let list = make_list(&app, "BY_BOOKS").await;

	let first = cursor_page(&app, &list, 100, None).await;
	let first_ids = ungrouped_ids(&first);
	let cursor = next_cursor(&first).expect("a next page");
	let offset_first = ungrouped_ids(&page(&app, &list, Some(100), Some(0)).await);
	assert_eq!(first_ids, offset_first);

	// a book that sorts before everything is added, and the list is worked out again
	fake_data::Media {
		id: Some("0".to_string()),
		series_id: series.id.clone(),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	app.conn()
		.execute_unprepared("UPDATE smart_list_materializations SET computed_at = '2020-01-01T00:00:00.000Z'")
		.await
		.unwrap();

	// by offset, the second page starts one book early: the last book of the first page repeats
	let by_offset = ungrouped_ids(&page(&app, &list, Some(100), Some(100)).await);
	assert!(
		by_offset.contains(first_ids.last().unwrap()),
		"the pages by offset shifted: {by_offset:?}"
	);

	// by cursor, it continues right after the last book it gave
	let mut rest = Vec::new();
	let mut cursor = Some(cursor);
	while let Some(after) = cursor {
		let response = cursor_page(&app, &list, 100, Some(&after)).await;
		rest.extend(ungrouped_ids(&response));
		cursor = next_cursor(&response);
	}
	let mut all: Vec<String> = first_ids.iter().chain(rest.iter()).cloned().collect();
	assert_eq!(all.len(), 250, "no book repeated, none skipped");
	all.sort();
	all.dedup();
	assert_eq!(all.len(), 250);
	assert!(
		!rest.contains(&"0".to_string()),
		"the new book is before the cursor"
	);
}

/// A grouped page has the cursor too, and `offset` and `after` are not used together
#[tokio::test]
async fn a_grouped_page_has_a_cursor_and_offset_and_after_are_not_mixed() {
	let app = TestApp::new_with_default_user().await;
	setup_single_series_with_n_books(&app, fake_data::Series::default(), 130).await;
	let list = make_list(&app, "BY_SERIES").await;

	let response = cursor_page(&app, &list, 100, None).await;
	assert_eq!(
		response["data"]["smartListItems"]["__typename"],
		"SmartListGrouped"
	);
	let cursor = next_cursor(&response).expect("a next page");
	let rest = cursor_page(&app, &list, 100, Some(&cursor)).await;
	assert_eq!(
		rest["data"]["smartListItems"]["__typename"],
		"SmartListGrouped"
	);
	assert_eq!(next_cursor(&rest), None);
	let books: usize = rest["data"]["smartListItems"]["items"]
		.as_array()
		.unwrap()
		.iter()
		.map(|item| item["books"].as_array().unwrap().len())
		.sum();
	assert_eq!(books, 30);

	let both = app
		.execute_gql(
			CURSOR_ITEMS,
			Some(json!({ "id": list, "limit": 10, "offset": 10, "after": cursor })),
		)
		.await;
	assert!(
		both["errors"][0]["message"]
			.as_str()
			.unwrap_or_default()
			.contains("either `offset` or `after`"),
		"{both:#}"
	);
}
