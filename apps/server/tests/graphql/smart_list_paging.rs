//! A smart list holds at most 200 books at a time (the query complexity limit prices every list at
//! that size), so the web app reads a longer one page by page: `smartListItems(limit, offset)`.
//! The books come in a fixed order, so the pages follow each other without gaps or repeats, and
//! `smartListMeta` (which book-curator reads) still counts every match.

use crate::common::{series::setup_single_series_with_n_books, TestApp};
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
