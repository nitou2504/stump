use crate::object::{
	author::Author, book_club_discussion_message::BookClubDiscussionMessage,
	directory_listing::DirectoryListing, job::Job, library::Library, log::Log,
	media::Media, missing_entity::MissingEntity, reading_list::ReadingList,
	reading_timeline::GlobalReadingTimelineNode, series::Series, user::User,
};
use async_graphql::{
	CustomValidator, InputObject, InputValueError, OneofObject, OutputType, Result,
	SimpleObject, Union,
};
use sea_orm::{prelude::*, FromQueryResult, QuerySelect, Select};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::Debug;

/// The most items a page or a list field returns at once. Bigger offset pages and `take`/`limit`
/// arguments are clamped to it (e.g. OPDS' `page_size`), bigger cursor limits are rejected, and
/// the query complexity limit prices every list at this size at most (see [crate::complexity]).
/// Nothing a client sends can make a single list hold more than this.
pub const MAX_LIST_SIZE: u64 = 200;

/// The size of a list field with an optional `take`/`limit` argument: the argument, or
/// [MAX_LIST_SIZE] when it is left out, and never more than that
pub fn list_limit(take: Option<u64>) -> u64 {
	take.unwrap_or(MAX_LIST_SIZE).min(MAX_LIST_SIZE)
}

/// A simple cursor-based pagination input object
#[derive(Debug, Clone, InputObject)]
pub struct CursorPagination {
	pub after: Option<String>,
	// At most MAX_LIST_SIZE: the validators reject more
	#[graphql(default_with = "default_limit()")]
	pub limit: u64,
}

impl Default for CursorPagination {
	fn default() -> Self {
		Self {
			after: None,
			limit: 20,
		}
	}
}

fn default_restful_page() -> u64 {
	1
}

fn default_page_size() -> Option<u64> {
	Some(20)
}

fn default_limit() -> u64 {
	20
}

fn default_zero_based() -> Option<bool> {
	Some(false)
}

/// A `page_size` of 0 is a client error, not an empty page (OPDS clients following the `next`
/// link of an empty page would loop forever)
fn deserialize_page_size<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
	D: Deserializer<'de>,
{
	let page_size = Option::<u64>::deserialize(deserializer)?;
	if page_size == Some(0) {
		return Err(serde::de::Error::custom("page_size must be greater than 0"));
	}
	Ok(page_size)
}

/// A simple offset-based pagination input object
#[derive(Debug, Clone, InputObject, Serialize, Deserialize)]
pub struct OffsetPagination {
	/// The page to start from. This is 1-based by default, but can be
	/// changed to 0-based by setting the `zero_based` field to true.
	#[serde(default = "default_restful_page")]
	pub page: u64,
	/// The number of items to return per page. This is 20 by default.
	// A bigger page is cut to MAX_LIST_SIZE items (the page info says so). Not part of the doc
	// comment: it would change the generated schema.graphql
	#[graphql(default_with = "default_page_size()")]
	#[serde(
		default = "default_page_size",
		deserialize_with = "deserialize_page_size"
	)]
	pub page_size: Option<u64>,
	/// Whether or not the page is zero-based. This is false by default.
	#[graphql(default_with = "default_zero_based()")]
	#[serde(default = "default_zero_based")]
	pub zero_based: Option<bool>,
}

impl Default for OffsetPagination {
	fn default() -> Self {
		Self {
			page: 1,
			page_size: default_page_size(),
			zero_based: default_zero_based(),
		}
	}
}

impl OffsetPagination {
	/// The largest value that can be bound as an SQL integer. Anything larger panics in the
	/// sqlx binder, which (with `panic = "abort"`) takes the whole server down.
	const MAX_SQL_INT: u64 = i64::MAX as u64;

	pub fn offset(&self) -> u64 {
		let page = if self.zero_based.unwrap_or(false) {
			self.page
		} else {
			// A 1-based page of 0 is invalid; treat it as the first page instead of
			// underflowing (e.g. `?page=0` used to crash the server)
			self.page.saturating_sub(1)
		};
		page.saturating_mul(self.limit()).min(Self::MAX_SQL_INT)
	}

	pub fn limit(&self) -> u64 {
		self.page_size
			.unwrap_or(20)
			.min(MAX_LIST_SIZE)
			.min(Self::MAX_SQL_INT)
	}

	pub fn next_page(&self) -> u64 {
		self.page + 1
	}

	pub fn previous_page(&self) -> Option<u64> {
		if self.zero_based.unwrap_or(false) {
			if self.page > 0 {
				Some(self.page - 1)
			} else {
				None
			}
		} else if self.page > 1 {
			Some(self.page - 1)
		} else {
			None
		}
	}
}

/// A simple pagination input object which does not paginate. An explicit struct is
/// required as a limitation of async_graphql's [OneofObject], which doesn't allow
/// for empty variants.
#[derive(Debug, Clone, InputObject)]
pub struct Unpaginated {
	pub(crate) unpaginated: bool,
}

/// A union of the supported pagination flavors which Stump supports. The resulting
/// response will be dependent on the pagination type used, e.g. a [CursorPaginatedResponse]
/// will be returned if the [CursorPagination] type is used.
///
/// You may use a conditional fragment in your GraphQL query for type-specific fields:
/// ```graphql
/// query MyQuery {
///     media(pagination: { offset: { page: 1, pageSize: 20 } }) {
///         ... on OffsetPaginationInfo {
///            totalPages
///            currentPage
///         }
///     }
/// }
/// ```
///
/// A special case is the `None` variant, which will return an offset-based pagination info
/// object based on the size of the result set. This will not paginate the results, so be
/// cautious when using this with large result sets.
///
/// **Note**: Be sure to call [Pagination::resolve] before using the pagination object
/// to ensure that the pagination object is in a valid state.
#[derive(Debug, Clone, OneofObject)]
pub enum Pagination {
	Cursor(CursorPagination),
	Offset(OffsetPagination),
	None(Unpaginated),
}

impl Pagination {
	/// Resolves the pagination object, ensuring that the pagination object is in a valid state. This
	/// primarily ensures that the `None` variant is converted to an offset-based pagination object
	/// when an invalid-but-non-error state of unpaginated=false is present.
	pub fn resolve(self) -> Pagination {
		match self {
			Pagination::None(input) if !input.unpaginated => {
				Pagination::Offset(OffsetPagination {
					page: 1,
					page_size: default_page_size(),
					zero_based: default_zero_based(),
				})
			},
			_ => self,
		}
	}
}

impl Pagination {
	/// The most items this pagination can return, which is what a query costs (see
	/// [crate::complexity]): the page size or cursor limit (never above [MAX_LIST_SIZE]), or
	/// `unpaginated_rows` for the tables where everything at once is allowed
	pub fn max_items(&self, unpaginated_rows: u64) -> u64 {
		match self {
			Pagination::Cursor(cursor) => cursor.limit.min(MAX_LIST_SIZE),
			Pagination::Offset(offset) => offset.limit(),
			Pagination::None(unpaginated) if unpaginated.unpaginated => unpaginated_rows,
			Pagination::None(_) => default_page_size().unwrap_or(20),
		}
	}
}

impl Default for Pagination {
	fn default() -> Self {
		Pagination::Offset(OffsetPagination {
			page: 1,
			page_size: default_page_size(),
			zero_based: default_zero_based(),
		})
	}
}

fn check_pagination(
	input: &Pagination,
	allow_unpaginated: bool,
) -> Result<(), InputValueError<Pagination>> {
	match input {
		Pagination::Offset(offset) => {
			if !offset.zero_based.unwrap_or(false) && offset.page == 0 {
				Err(InputValueError::custom("Page must be greater than 0"))
			} else if offset.page_size.is_some_and(|x| x == 0) {
				Err(InputValueError::custom("Page size must be greater than 0"))
			} else {
				Ok(())
			}
		},
		Pagination::Cursor(cursor) if cursor.limit > MAX_LIST_SIZE => Err(
			InputValueError::custom(format!("Limit must be at most {MAX_LIST_SIZE}")),
		),
		Pagination::None(unpaginated) if unpaginated.unpaginated && !allow_unpaginated => {
			Err(InputValueError::custom(
				"Unpaginated queries are not supported here, use offset or cursor pagination",
			))
		},
		_ => Ok(()),
	}
}

/// A custom validator for the Pagination enum that ensures that the input is valid
/// for the given pagination type. Unpaginated queries (everything at once) are refused: the
/// tables behind these queries can be huge, use [SmallTablePaginationValidator] for the ones
/// that can't.
#[derive(Default)]
pub struct PaginationValidator;

impl CustomValidator<Pagination> for PaginationValidator {
	fn check(&self, input: &Pagination) -> Result<(), InputValueError<Pagination>> {
		check_pagination(input, false)
	}
}

/// The same as [PaginationValidator], but unpaginated queries are allowed. For tables that are
/// small by nature: libraries, users and reading lists.
#[derive(Default)]
pub struct SmallTablePaginationValidator;

impl CustomValidator<Pagination> for SmallTablePaginationValidator {
	fn check(&self, input: &Pagination) -> Result<(), InputValueError<Pagination>> {
		check_pagination(input, true)
	}
}

impl CustomValidator<CursorPagination> for PaginationValidator {
	fn check(
		&self,
		input: &CursorPagination,
	) -> Result<(), InputValueError<CursorPagination>> {
		if input.limit > MAX_LIST_SIZE {
			Err(InputValueError::custom(format!(
				"Limit must be at most {MAX_LIST_SIZE}"
			)))
		} else {
			Ok(())
		}
	}
}

/// Information about the current cursor pagination state
#[derive(Debug, SimpleObject)]
pub struct CursorPaginationInfo {
	/// The cursor of the current page. This should only be None if there are no results,
	/// since there is no cursor present to pull from. This technically deviates from
	/// popular (read: Relay) specs, but it works better for Stump
	pub current_cursor: Option<String>,
	/// The cursor the next page should use, if it exists.
	pub next_cursor: Option<String>,
	/// The limit used when querying the database
	pub limit: u64,
}

/// Information about the current offset pagination state
#[derive(Debug, SimpleObject)]
pub struct OffsetPaginationInfo {
	/// The number of pages available. This is **not** affected by the zero-based flag,
	/// so a client requesting zero-based pagination will need to adjust their pagination
	/// logic accordingly.
	pub total_pages: u64,
	/// The total number of items available
	pub total_items: u64,
	/// The current page, zero-indexed.
	pub current_page: u64,
	/// The number of elements per page.
	pub page_size: u64,
	/// The offset of the current page. E.g. if current page is 1, and pageSize is 10, the offset is 20.
	pub page_offset: u64,
	/// Whether or not the page is zero-indexed.
	pub zero_based: bool,
}

impl OffsetPaginationInfo {
	pub fn new(input: OffsetPagination, count: u64) -> Self {
		let zero_based = input.zero_based.unwrap_or(false);
		let page_size = input.limit();
		// A 1-based page of 0 is invalid, normalize it to the first page
		let current_page = if zero_based {
			input.page
		} else {
			input.page.max(1)
		};

		let total_pages = (count as f64 / page_size.max(1) as f64).ceil() as u64;
		let page_offset = input.offset();

		Self {
			current_page,
			page_size,
			total_pages,
			page_offset,
			zero_based,
			total_items: count,
		}
	}

	pub fn unpaged(count: u64) -> Self {
		Self {
			current_page: 1,
			page_size: count,
			total_pages: 1,
			page_offset: 0,
			zero_based: false,
			total_items: count,
		}
	}
}

#[derive(Debug, SimpleObject)]
#[graphql(concrete(name = "CursorPaginatedMediaResponse", params(Media)))]
#[graphql(concrete(
	name = "CursorPaginatedBookClubDiscussionMessageResponse",
	params(BookClubDiscussionMessage)
))]
#[graphql(concrete(
	name = "CursorPaginatedGlobalReadingTimelineNodeResponse",
	params(GlobalReadingTimelineNode)
))]
pub struct CursorPaginatedResponse<T>
where
	T: OutputType,
{
	pub nodes: Vec<T>,
	pub cursor_info: CursorPaginationInfo,
}

#[derive(Debug, SimpleObject)]
#[graphql(concrete(
	name = "OffsetPaginatedDirectoryListingResponse",
	params(DirectoryListing)
))]
pub struct OffsetPaginatedResponse<T>
where
	T: OutputType,
{
	pub nodes: Vec<T>,
	pub offset_info: OffsetPaginationInfo,
}

#[derive(Debug, Union)]
pub enum PaginationInfo {
	Cursor(CursorPaginationInfo),
	Offset(OffsetPaginationInfo),
}

#[derive(Debug, SimpleObject)]
#[graphql(concrete(name = "PaginatedAuthorResponse", params(Author)))]
#[graphql(concrete(
	name = "PaginatedDirectoryListingResponse",
	params(DirectoryListing)
))]
#[graphql(concrete(name = "PaginatedJobResponse", params(Job)))]
#[graphql(concrete(name = "PaginatedLogResponse", params(Log)))]
#[graphql(concrete(name = "PaginatedMediaResponse", params(Media)))]
#[graphql(concrete(name = "PaginatedLibraryResponse", params(Library)))]
#[graphql(concrete(name = "PaginatedSeriesResponse", params(Series)))]
#[graphql(concrete(name = "PaginatedReadingListResponse", params(ReadingList)))]
#[graphql(concrete(name = "PaginatedUserResponse", params(User)))]
#[graphql(concrete(name = "PaginatedMissingEntityResponse", params(MissingEntity)))]
pub struct PaginatedResponse<T>
where
	T: OutputType,
{
	pub nodes: Vec<T>,
	pub page_info: PaginationInfo,
}

async fn get_paginated_offset_results<
	EntityType: EntityTrait<Model: Debug + FromQueryResult + Sized + Send + Sync>,
	GraphqlOutputType: From<EntityType::Model> + OutputType,
>(
	query: Select<EntityType>,
	conn: &DbConn,
	info: OffsetPagination,
) -> Result<PaginatedResponse<GraphqlOutputType>> {
	let count = query.clone().count(conn).await?;

	let models = query
		.offset(info.offset())
		.limit(info.limit())
		.into_model::<EntityType::Model>()
		.all(conn)
		.await?;

	Ok(PaginatedResponse {
		nodes: models.into_iter().map(GraphqlOutputType::from).collect(),
		page_info: OffsetPaginationInfo::new(info, count).into(),
	})
}

async fn get_paginated_cursor_results<
	EntityType: EntityTrait<Model: Debug + FromQueryResult + Sized + Send + Sync>,
	GraphqlOutputType: From<EntityType::Model> + OutputType,
>(
	query: Select<EntityType>,
	cursor_column: EntityType::Column,
	conn: &DbConn,
	info: CursorPagination,
	get_cursor: impl Fn(&EntityType::Model) -> String,
) -> Result<PaginatedResponse<GraphqlOutputType>> {
	let mut cursor = query.clone().cursor_by(cursor_column);
	if let Some(ref id) = info.after {
		let last_row = query
			.filter(cursor_column.eq(id.clone()))
			.into_model::<EntityType::Model>()
			.one(conn)
			.await?
			.ok_or("Cursor not found")?;
		cursor.after(last_row.get(cursor_column));
	}
	cursor.first(info.limit);

	let models = cursor.into_model::<EntityType::Model>().all(conn).await?;
	let current_cursor = info.after.or_else(|| models.first().map(&get_cursor));
	let next_cursor = match models.last().map(get_cursor) {
		Some(cursor) if models.len() == info.limit as usize => Some(cursor),
		_ => None,
	};

	Ok(PaginatedResponse {
		nodes: models.into_iter().map(GraphqlOutputType::from).collect(),
		page_info: CursorPaginationInfo {
			current_cursor,
			next_cursor,
			limit: info.limit,
		}
		.into(),
	})
}

async fn get_paginated_none_results<
	EntityType: EntityTrait<Model: Debug + FromQueryResult + Sized + Send + Sync>,
	GraphqlOutputType: From<EntityType::Model> + OutputType,
>(
	query: Select<EntityType>,
	conn: &DbConn,
) -> Result<PaginatedResponse<GraphqlOutputType>> {
	let models = query.into_model::<EntityType::Model>().all(conn).await?;
	let count = models.len().try_into()?;

	Ok(PaginatedResponse {
		nodes: models.into_iter().map(GraphqlOutputType::from).collect(),
		page_info: OffsetPaginationInfo::unpaged(count).into(),
	})
}

/// A helper function to get paginated results based on the pagination type, reducing
/// boilerplate in the resolver functions.
pub async fn get_paginated_results<
	EntityType: EntityTrait<Model: Debug + FromQueryResult + Sized + Send + Sync>,
	GraphqlOutputType: From<EntityType::Model> + OutputType,
>(
	query: Select<EntityType>,
	cursor_column: EntityType::Column,
	conn: &DbConn,
	pagination: Pagination,
	get_cursor: impl Fn(&EntityType::Model) -> String,
) -> Result<PaginatedResponse<GraphqlOutputType>> {
	match pagination {
		Pagination::Cursor(info) => {
			get_paginated_cursor_results(query, cursor_column, conn, info, get_cursor)
				.await
		},
		Pagination::Offset(info) => get_paginated_offset_results(query, conn, info).await,
		Pagination::None(_) => get_paginated_none_results(query, conn).await,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	mod offset_pagination {
		use super::*;

		#[test]
		fn offset_does_not_underflow_or_overflow() {
			// page=0 on a 1-based pagination used to underflow and crash the server
			let pagination = OffsetPagination {
				page: 0,
				page_size: Some(20),
				zero_based: Some(false),
			};
			assert_eq!(pagination.offset(), 0);

			let pagination = OffsetPagination {
				page: u64::MAX,
				page_size: Some(u64::MAX),
				zero_based: Some(true),
			};
			assert_eq!(pagination.offset(), i64::MAX as u64);
			// a page is never bigger than MAX_LIST_SIZE
			assert_eq!(pagination.limit(), MAX_LIST_SIZE);
		}

		#[test]
		fn page_size_is_cut_to_the_maximum() {
			let pagination = OffsetPagination {
				page: 3,
				page_size: Some(5_000),
				zero_based: Some(false),
			};
			assert_eq!(pagination.limit(), MAX_LIST_SIZE);
			assert_eq!(pagination.offset(), 2 * MAX_LIST_SIZE);

			let pagination = OffsetPagination {
				page: 1,
				page_size: Some(50),
				zero_based: Some(false),
			};
			assert_eq!(pagination.limit(), 50);
		}

		#[test]
		fn serde_rejects_a_page_size_of_zero() {
			let ok: OffsetPagination =
				serde_json::from_str(r#"{"page":2,"page_size":50}"#).unwrap();
			assert_eq!(ok.page_size, Some(50));
			let default: OffsetPagination = serde_json::from_str("{}").unwrap();
			assert_eq!(default.page_size, Some(20));
			assert!(serde_json::from_str::<OffsetPagination>(r#"{"page_size":0}"#)
				.is_err());
		}

		#[test]
		fn next_page_returns_current_page_plus_one_when_one_based() {
			let pagination = OffsetPagination {
				page: 1,
				page_size: Some(20),
				zero_based: Some(false),
			};
			assert_eq!(pagination.next_page(), 2);
		}

		#[test]
		fn next_page_returns_current_page_plus_one_when_zero_based() {
			let pagination = OffsetPagination {
				page: 0,
				page_size: Some(20),
				zero_based: Some(true),
			};
			assert_eq!(pagination.next_page(), 1);
		}

		#[test]
		fn next_page_works_for_page_5_one_based() {
			let pagination = OffsetPagination {
				page: 5,
				page_size: Some(20),
				zero_based: Some(false),
			};
			assert_eq!(pagination.next_page(), 6);
		}

		#[test]
		fn next_page_works_for_page_5_zero_based() {
			let pagination = OffsetPagination {
				page: 5,
				page_size: Some(20),
				zero_based: Some(true),
			};
			assert_eq!(pagination.next_page(), 6);
		}
	}

	mod validators {
		use super::*;

		fn offset(page: u64, page_size: u64) -> Pagination {
			Pagination::Offset(OffsetPagination {
				page,
				page_size: Some(page_size),
				zero_based: Some(false),
			})
		}

		#[test]
		fn rejects_unpaginated_unless_the_table_is_small() {
			let everything = Pagination::None(Unpaginated { unpaginated: true });
			assert!(PaginationValidator.check(&everything).is_err());
			assert!(SmallTablePaginationValidator.check(&everything).is_ok());
			// the "invalid but harmless" `unpaginated: false` resolves to the default page
			let nothing = Pagination::None(Unpaginated { unpaginated: false });
			assert!(PaginationValidator.check(&nothing).is_ok());
		}

		#[test]
		fn keeps_checking_offsets_and_bounds_cursors() {
			assert!(PaginationValidator.check(&offset(1, 20)).is_ok());
			assert!(PaginationValidator.check(&offset(0, 20)).is_err());
			assert!(PaginationValidator.check(&offset(1, 0)).is_err());
			// big offset pages are cut by `limit()`, not refused
			assert!(PaginationValidator.check(&offset(1, 10_000)).is_ok());

			let cursor = |limit| Pagination::Cursor(CursorPagination { after: None, limit });
			assert!(PaginationValidator.check(&cursor(MAX_LIST_SIZE)).is_ok());
			assert!(PaginationValidator.check(&cursor(MAX_LIST_SIZE + 1)).is_err());
			let bare = CursorPagination {
				after: None,
				limit: MAX_LIST_SIZE + 1,
			};
			assert!(PaginationValidator.check(&bare).is_err());
		}

		#[test]
		fn list_limit_defaults_to_and_never_exceeds_the_maximum() {
			assert_eq!(list_limit(None), MAX_LIST_SIZE);
			assert_eq!(list_limit(Some(5)), 5);
			assert_eq!(list_limit(Some(u64::MAX)), MAX_LIST_SIZE);
		}
	}
}
