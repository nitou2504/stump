use async_graphql::{Context, Enum, Object, Result, ID};

use crate::pagination::get_paginated_results;
use models::entity::{reading_list, user::AuthUser};
use sea_orm::prelude::*;

use crate::{
	data::{AuthContext, CoreContext},
	object::reading_list::{ReadingList, NOT_FOUND},
	pagination::{PaginatedResponse, Pagination, SmallTablePaginationValidator},
};

/// Which reading lists to ask for
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Enum)]
pub enum ReadingListScope {
	/// The lists you made and everyone's public lists
	#[default]
	All,
	/// The lists you made
	Mine,
	/// The public lists other people made
	Others,
}

#[derive(Default)]
pub struct ReadingListQuery;

#[Object]
impl ReadingListQuery {
	/// Retrieves a paginated list of the reading lists the current user can see: by default
	/// their own and the public lists of everyone else (lists of people who are deleted or
	/// locked are left out). `scope` narrows it to their own lists or to other people's.
	///
	/// # Returns
	///
	/// A paginated list of reading lists.
	#[graphql(
		complexity = "crate::complexity::pagination_cost(&pagination, child_complexity)"
	)]
	async fn reading_lists(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] scope: ReadingListScope,
		#[graphql(default, validator(custom = "SmallTablePaginationValidator"))]
		pagination: Pagination,
	) -> Result<PaginatedResponse<ReadingList>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		get_paginated_reading_list(user, conn, scope, pagination).await
	}

	/// Retrieves a reading list by ID for the current user.
	///
	/// # Returns
	/// A reading list with the given ID. If no reading list with this ID exists, or it is
	/// private and isn't the current user's, the same error is returned.
	async fn reading_list_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<ReadingList> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let query = reading_list::Entity::find_for_user_and_id(user, id.as_ref())
			.into_model::<reading_list::Model>()
			.one(conn)
			.await?;

		Ok(ReadingList::from(query.ok_or(NOT_FOUND)?))
	}
}

async fn get_paginated_reading_list(
	user: &AuthUser,
	conn: &DbConn,
	scope: ReadingListScope,
	pagination: Pagination,
) -> Result<PaginatedResponse<ReadingList>> {
	let query = match scope {
		ReadingListScope::All => reading_list::Entity::find_for_user(user),
		ReadingListScope::Mine => reading_list::Entity::find_mine(user),
		ReadingListScope::Others => reading_list::Entity::find_by_others(user),
	};
	let get_cursor =
		|m: &<models::entity::reading_list::Entity as sea_orm::EntityTrait>::Model| {
			m.id.to_string()
		};
	get_paginated_results(
		query,
		reading_list::Column::Id,
		conn,
		pagination,
		get_cursor,
	)
	.await
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::pagination::{CursorPagination, OffsetPagination, PaginationInfo};
	use crate::tests::common::*;
	use sea_orm::MockDatabase;

	fn get_test_model() -> reading_list::Model {
		reading_list::Model {
			id: "1".to_string(),
			name: "hello".to_string(),
			description: Some("world".to_string()),
			updated_at: "2021-08-01T00:00:00Z".parse().unwrap(),
			visibility: "PUBLIC".to_string(),
			ordering: "MANUAL".to_string(),
			creating_user_id: "42".to_string(),
		}
	}

	#[tokio::test]
	async fn test_cursor_pagination() {
		let user = get_default_user();
		let mock_db = MockDatabase::new(sea_orm::DatabaseBackend::Sqlite)
			.append_query_results(vec![vec![get_test_model()], vec![get_test_model()]])
			.into_connection();

		let cursor_info = Pagination::Cursor(CursorPagination {
			after: Some("abc".to_string()),
			limit: 1,
		});
		let reading_lists = get_paginated_reading_list(
			&user,
			&mock_db,
			ReadingListScope::All,
			cursor_info,
		)
		.await
		.unwrap();

		assert_eq!(reading_lists.nodes.len(), 1);
		assert_eq!(reading_lists.nodes[0].model.id, "1");
		assert_eq!(reading_lists.nodes[0].model.name, "hello");

		match reading_lists.page_info {
			PaginationInfo::Cursor(info) => {
				assert_eq!(info.current_cursor, Some("abc".to_string()));
				assert_eq!(info.next_cursor, Some("1".to_string()));
			},
			_ => panic!("Expected Offset pagination info"),
		}
	}

	#[tokio::test]
	async fn test_cursor_pagination_cursor_not_found() {
		let user = get_default_user();
		let mock_db = MockDatabase::new(sea_orm::DatabaseBackend::Sqlite)
			.append_query_results::<reading_list::Model, _, _>(vec![vec![]])
			.into_connection();

		let cursor_info = Pagination::Cursor(CursorPagination {
			after: Some("abc".to_string()),
			limit: 1,
		});
		let reading_lists = get_paginated_reading_list(
			&user,
			&mock_db,
			ReadingListScope::All,
			cursor_info,
		)
		.await;

		assert!(reading_lists.is_err());
	}

	#[tokio::test]
	async fn test_cursor_pagination_no_more_results() {
		let user = get_default_user();
		let mock_db = MockDatabase::new(sea_orm::DatabaseBackend::Sqlite)
			.append_query_results(vec![vec![get_test_model()], vec![]])
			.into_connection();

		let cursor_info = Pagination::Cursor(CursorPagination {
			after: Some("abc".to_string()),
			limit: 1,
		});
		let reading_lists = get_paginated_reading_list(
			&user,
			&mock_db,
			ReadingListScope::All,
			cursor_info,
		)
		.await
		.unwrap();

		assert!(reading_lists.nodes.is_empty());

		match reading_lists.page_info {
			PaginationInfo::Cursor(info) => {
				assert_eq!(info.current_cursor, Some("abc".to_string()));
				assert_eq!(info.next_cursor, None);
			},
			_ => panic!("Expected Offset pagination info"),
		}
	}

	#[tokio::test]
	async fn test_offset_pagination() {
		let user = get_default_user();
		let mock_db = MockDatabase::new(sea_orm::DatabaseBackend::Sqlite)
			.append_query_results(vec![vec![maplit::btreemap! {
				"num_items" => Into::<Value>::into(1i64),
			}]])
			.append_query_results(vec![vec![get_test_model()]])
			.into_connection();

		let info = Pagination::Offset(OffsetPagination {
			page: 1,
			page_size: Some(1),
			zero_based: None,
		});
		let reading_lists =
			get_paginated_reading_list(&user, &mock_db, ReadingListScope::All, info)
				.await
				.unwrap();

		assert_eq!(reading_lists.nodes.len(), 1);
		assert_eq!(reading_lists.nodes[0].model, get_test_model());

		match reading_lists.page_info {
			PaginationInfo::Offset(info) => {
				assert_eq!(info.total_pages, 1);
				assert_eq!(info.current_page, 1);
				assert_eq!(info.page_size, 1);
				assert_eq!(info.page_offset, 0);
			},
			_ => panic!("Expected Offset pagination info"),
		}
	}

	#[tokio::test]
	async fn test_offset_pagination_no_results() {
		let user = get_default_user();
		let mock_db = MockDatabase::new(sea_orm::DatabaseBackend::Sqlite)
			.append_query_results(vec![vec![maplit::btreemap! {
				"num_items" => Into::<Value>::into(0i64),
			}]])
			.append_query_results::<reading_list::Model, _, _>(vec![vec![]])
			.into_connection();

		let info = Pagination::Offset(OffsetPagination {
			page: 1,
			page_size: Some(1),
			zero_based: None,
		});
		let reading_lists =
			get_paginated_reading_list(&user, &mock_db, ReadingListScope::All, info)
				.await
				.unwrap();

		assert!(reading_lists.nodes.is_empty());

		match reading_lists.page_info {
			PaginationInfo::Offset(info) => {
				assert_eq!(info.total_pages, 0);
				assert_eq!(info.current_page, 1);
				assert_eq!(info.page_size, 1);
				assert_eq!(info.page_offset, 0);
			},
			_ => panic!("Expected Offset pagination info"),
		}
	}
}
