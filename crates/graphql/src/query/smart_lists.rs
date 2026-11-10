use super::smart_lists_builder::{
	build_smart_list_items, filtered_media_ids, read_smart_list_page, smart_list_books,
	smart_list_meta,
};
use crate::{
	data::{AuthContext, CoreContext},
	filter::media::MediaFilterInput,
	guard::PermissionGuard,
	input::smart_lists::SmartListsInput,
	object::{
		media::Media,
		smart_list_item::SmartListItems,
		smart_lists::{SmartList, SmartListMeta},
	},
	pagination::{list_limit, list_offset},
};
use async_graphql::{Context, Object, Result, ID};
use models::{
	entity::{
		media,
		smart_list::{self},
	},
	shared::enums::UserPermission,
};
use sea_orm::{prelude::*, QueryOrder, TransactionTrait};

#[derive(Default, Clone, Copy)]
pub struct SmartListsQuery;

#[Object]
impl SmartListsQuery {
	#[graphql(guard = "PermissionGuard::one(UserPermission::AccessSmartList)")]
	async fn smart_lists(
		self,
		ctx: &Context<'_>,
		#[graphql(default)] input: SmartListsInput,
	) -> Result<Vec<SmartList>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let query_all = input.all.unwrap_or(false);
		if query_all && !user.is_server_owner {
			return Err(
				"Cannot query all smart lists unless you are a server owner".into()
			);
		}

		let mine = input.mine.unwrap_or(false);
		if query_all && mine {
			return Err("Cannot query all and mine at the same time".into());
		}

		// by name, the same order everywhere the lists are listed (the sidebar, the lists page)
		let smart_lists = smart_list::Entity::find_for_user(
			user,
			input.all.unwrap_or(false),
			input.mine.unwrap_or(false),
			input.search,
		)
		.order_by_asc(Expr::cust(r#"LOWER("smart_lists"."name")"#))
		.order_by_asc(smart_list::Column::Id)
		.all(conn)
		.await?;

		Ok(smart_lists.into_iter().map(SmartList::from).collect())
	}

	#[graphql(guard = "PermissionGuard::one(UserPermission::AccessSmartList)")]
	async fn smart_list_by_id(
		self,
		ctx: &Context<'_>,
		id: ID,
	) -> Result<Option<SmartList>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let smart_list = smart_list::Entity::find_by_id(user, id).one(conn).await?;

		Ok(smart_list.map(SmartList::from))
	}

	#[graphql(guard = "PermissionGuard::one(UserPermission::AccessSmartList)")]
	async fn smart_list_meta(
		self,
		ctx: &Context<'_>,
		id: ID,
	) -> Result<Option<SmartListMeta>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let smart_list = smart_list::Entity::find_by_id(user, id)
			.one(conn)
			.await?
			.ok_or("Smart list not found".to_string())?;

		// counted by the database in one query, over the list's cached members
		let books = smart_list_books(conn, user, &smart_list).await?;
		Ok(Some(smart_list_meta(conn, books).await?))
	}

	#[graphql(
		complexity = "crate::complexity::list_cost(limit, child_complexity)",
		guard = "PermissionGuard::one(UserPermission::AccessSmartList)"
	)]
	async fn smart_list_items(
		self,
		ctx: &Context<'_>,
		id: ID,
		#[graphql(
			default,
			desc = "At most 200 books, also without a limit: page with `offset`"
		)]
		limit: Option<u64>,
		#[graphql(
			default,
			desc = "The number of books to skip. The books come in a fixed order, so the pages of a long list follow each other without gaps or repeats"
		)]
		offset: Option<u64>,
		#[graphql(
			desc = "Start after this book (the `nextCursor` of the page before) instead of skipping books with `offset`: a book added or removed between two pages doesn't make the pages skip or repeat one. Not together with `offset`"
		)]
		after: Option<ID>,
		#[graphql(
			desc = "An additional filter narrowing down the list's books, e.g. by genre"
		)]
		filter: Option<MediaFilterInput>,
		#[graphql(
			desc = "Overrides the list's default grouping, e.g. to view a series-grouped list as plain books"
		)]
		grouping: Option<smart_list::SmartListGrouping>,
	) -> Result<SmartListItems> {
		if offset.is_some_and(|offset| offset > 0) && after.is_some() {
			return Err("Use either `offset` or `after`, not both".into());
		}
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let smart_list = smart_list::Entity::find_by_id(user, id)
			.one(conn)
			.await?
			.ok_or("Smart list not found".to_string())?;

		// the list's cached members, narrowed to what the user may read. Looked at before the
		// transaction starts: a cache that is out of date is worked out again in a write of its
		// own, which a transaction that is already reading wouldn't see
		let books_query = smart_list_books(conn, user, &smart_list).await?;
		let books_query = match filter {
			Some(filter) => books_query
				.filter(media::Column::Id.in_subquery(filtered_media_ids(user, filter))),
			None => books_query,
		};

		// At most MAX_LIST_SIZE books, also without a `limit`. A list longer than that is read
		// page by page (the web app does), so the order must be the same for every page
		let txn = conn.begin().await?;
		let (models, next_cursor) = read_smart_list_page(
			&txn,
			books_query,
			list_limit(limit),
			list_offset(offset),
			after.as_ref().map(|after| after.as_str()),
		)
		.await?;

		let books: Vec<Media> = models.into_iter().map(Media::from).collect();
		let items = build_smart_list_items(
			user,
			grouping.unwrap_or(smart_list.default_grouping),
			books,
			next_cursor,
			&txn,
		)
		.await?;

		txn.commit().await?;

		Ok(items)
	}
}
