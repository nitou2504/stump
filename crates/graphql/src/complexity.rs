//! What a GraphQL query costs, and how much a single query may cost.
//!
//! async-graphql prices every field at 1 plus its children, which says nothing about the size of
//! a list: `seriesById { media { series { media { id } } } }` costs 6 and, on a series with 750
//! books, builds over half a million books (one such query took the whole server down). A list
//! field therefore costs *its size times what one item costs*, wherever a client can nest it:
//!
//! - lists with a page size, `take` or `limit` are priced at that size, and no list ever holds
//!   more than [MAX_LIST_SIZE] items (see [list_limit] and `OffsetPagination::limit`)
//! - the other lists that lead from one book, series or library to more of them (an author's
//!   books, a library's authors) are priced at an assumed size, [RELATED_LIST] or [SMALL_LIST]
//! - unpaginated queries are only allowed for libraries, users and reading lists, which are priced
//!   at [UNPAGINATED_LIST] items
//!
//! A query costing more than [MAX_QUERY_COMPLEXITY] is refused before anything runs. The limit
//! leaves room for the biggest query the web app sends (a grid of 200 books, around 13,000), but
//! a second list inside a full-size list (200 books, each with the 200 books of its series) is
//! far over it, so a query can't ask for more than the single-digit thousands of items.
//!
//! The helpers are used by `#[graphql(complexity = "...")]` on the fields: the expressions can
//! name the field's arguments and `child_complexity`, the cost of what is selected inside.

use crate::pagination::{list_limit, CursorPagination, Pagination};

/// The most a query may cost; async-graphql refuses a more complex one with "Query is too
/// complex". See the module docs for how this was chosen.
pub const MAX_QUERY_COMPLEXITY: usize = 20_000;

/// The assumed size of a list of related books, series, authors or users (an author's books, a
/// library's authors) that has no size argument. These can hold more in a big library, but they
/// are only ever as big as what the library holds, and the multiplication stops at the depth of
/// the query.
pub const RELATED_LIST: usize = 50;

/// The assumed size of a list that is small by nature: the authors of one book, the readthroughs
/// of one book, the sessions of one readthrough
pub const SMALL_LIST: usize = 10;

/// The assumed size of an unpaginated query, which is only allowed for tables that are small by
/// nature: libraries, users and reading lists
pub const UNPAGINATED_LIST: u64 = 100;

/// The cost of a list field with an optional `take` or `limit` argument
pub fn list_cost(size: Option<u64>, child_complexity: usize) -> usize {
	(list_limit(size) as usize).saturating_mul(child_complexity)
}

/// The cost of a field with a `pagination` argument: the size of the page (what is selected
/// inside is selected for every item of it)
pub fn pagination_cost(pagination: &Pagination, child_complexity: usize) -> usize {
	(pagination.max_items(UNPAGINATED_LIST) as usize).saturating_mul(child_complexity)
}

/// The cost of a field with a cursor-based `pagination` argument
pub fn cursor_pagination_cost(
	pagination: &CursorPagination,
	child_complexity: usize,
) -> usize {
	(list_limit(Some(pagination.limit)) as usize).saturating_mul(child_complexity)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		pagination::{OffsetPagination, Unpaginated, MAX_LIST_SIZE},
		schema::build_schema_bare,
	};
	use async_graphql::Request;

	#[test]
	fn lists_cost_their_size_at_most_the_maximum() {
		assert_eq!(list_cost(Some(5), 10), 50);
		assert_eq!(list_cost(None, 3), MAX_LIST_SIZE as usize * 3);
		assert_eq!(list_cost(Some(u64::MAX), 3), MAX_LIST_SIZE as usize * 3);
		assert_eq!(list_cost(Some(1), usize::MAX), usize::MAX);
	}

	#[test]
	fn pages_cost_their_size() {
		let page = |page_size| {
			Pagination::Offset(OffsetPagination {
				page: 1,
				page_size: Some(page_size),
				zero_based: Some(false),
			})
		};
		assert_eq!(pagination_cost(&page(20), 4), 80);
		assert_eq!(pagination_cost(&page(10_000), 4), MAX_LIST_SIZE as usize * 4);

		let cursor = CursorPagination {
			after: None,
			limit: 7,
		};
		assert_eq!(cursor_pagination_cost(&cursor, 2), 14);
		assert_eq!(pagination_cost(&Pagination::Cursor(cursor), 2), 14);

		let everything = Pagination::None(Unpaginated { unpaginated: true });
		assert_eq!(
			pagination_cost(&everything, 3),
			UNPAGINATED_LIST as usize * 3
		);
		let nothing = Pagination::None(Unpaginated { unpaginated: false });
		assert_eq!(pagination_cost(&nothing, 3), 60);
	}

	/// Runs the query against the real schema, without any data (nothing is resolved without an
	/// authenticated user). Only the validation matters: it is where the complexity is checked.
	async fn rejected_as_too_complex(query: &str) -> bool {
		let response = build_schema_bare().execute(Request::new(query)).await;
		response
			.errors
			.iter()
			.any(|error| error.message.contains("too complex"))
	}

	#[tokio::test]
	async fn refuses_nested_lists_that_blew_up_the_server() {
		// the one that killed the server: the books of a series, their series, its books
		assert!(
			rejected_as_too_complex(
				r#"{ seriesById(id: "x") { media { id series { media { id } } } } }"#
			)
			.await
		);
		// libraries to series to books to their libraries to series
		assert!(
			rejected_as_too_complex(
				r#"{ libraryById(id: "x") { series { media { id library { series { id } } } } } }"#
			)
			.await
		);
		// a page of books, each with the books of its series, each with its series...
		assert!(
			rejected_as_too_complex(
				r#"{ media(pagination: { offset: { page: 1, pageSize: 200 } }) { nodes { id series { media { id } } } } }"#
			)
			.await
		);
		// an author's books lead to a library with all its authors
		assert!(
			rejected_as_too_complex(
				r#"{ libraryById(id: "x") { authors { books { library { authors { books { id } } } } } } }"#
			)
			.await
		);
		// small lists nested deeply are as bad: 10 readthroughs x 10 sessions, again and again
		assert!(
			rejected_as_too_complex(
				r#"{ mediaById(id: "x") { readingTimeline { readthroughs { sessions { session { media { readingTimeline { readthroughs { sessions { session { media { readingTimeline { readthroughs { sessions { session { id } } } } } } } } } } } } } } } }"#
			)
			.await
		);
		// asking for the same big list again and again adds up: 60 x 200 books
		let aliases = (0..60)
			.map(|i| format!("a{i}: seriesById(id: \"x\") {{ media {{ id resolvedName }} }}"))
			.collect::<Vec<_>>()
			.join(" ");
		assert!(rejected_as_too_complex(&format!("{{ {aliases} }}")).await);
	}

	#[tokio::test]
	async fn allows_what_the_web_app_asks_for() {
		// a grid of 200 books (the biggest page), with the usual details of a book
		let book = "id name extension size pages resolvedName path library { id name } \
			series { id resolvedName } metadata { title summary genres writers year number } \
			readProgress { page percentageCompleted updatedAt } thumbnail { url } tags { id name }";
		assert!(
			!rejected_as_too_complex(&format!(
				r#"{{ media(pagination: {{ offset: {{ page: 1, pageSize: 200 }} }}) {{ nodes {{ {book} }} pageInfo {{ __typename }} }} }}"#
			))
			.await
		);
		// the libraries page: all libraries with a preview of five series each
		assert!(
			!rejected_as_too_complex(
				r#"{ libraries(pagination: { none: { unpaginated: true } }) { nodes { id name series(take: 5) { id resolvedName thumbnail { url } } } } librariesStats { seriesCount bookCount } }"#
			)
			.await
		);
		// the reader asks for the next book
		assert!(
			!rejected_as_too_complex(
				r#"{ mediaById(id: "x") { id nextInSeries(pagination: { cursor: { limit: 1 } }) { nodes { id name } } } }"#
			)
			.await
		);
		// a series with some of its books
		assert!(
			!rejected_as_too_complex(
				r#"{ seriesById(id: "x") { id resolvedName media(take: 50) { id name pages } } }"#
			)
			.await
		);
		// a small list inside a small list is fine
		assert!(
			!rejected_as_too_complex(
				r#"{ libraryById(id: "x") { id authors { name books { id } } } }"#
			)
			.await
		);
	}
}
