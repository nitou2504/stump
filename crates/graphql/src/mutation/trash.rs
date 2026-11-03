use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use models::{
	entity::{library_exclusion, media, media_purge},
	shared::{enums::UserPermission, trash::TrashPurgeError},
};
use sea_orm::{prelude::*, sea_query::Expr};
use stump_core::trash;

use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
	object::deleted_book::{
		trashed_ids, DeletedBooksPurgeResult, DeletedBooksSettings, PurgedBook,
	},
	query::trash::visible_libraries,
};

#[derive(Default)]
pub struct TrashMutation;

#[Object]
impl TrashMutation {
	/// Brings books in *Deleted books* back to their library exactly as they were (reading
	/// progress, annotations, bookmarks, tags and smart lists never left). Returns how many came
	/// back
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn restore_deleted_books(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
	) -> Result<u64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		if ids.is_empty() {
			return Ok(0);
		}
		let restored = media::Entity::update_many()
			.col_expr(
				media::Column::DeletedAt,
				Expr::value(Option::<DateTimeWithTimeZone>::None),
			)
			.col_expr(
				media::Column::DeletedBy,
				Expr::value(Option::<String>::None),
			)
			.col_expr(
				media::Column::UpdatedAt,
				Expr::value(Some(DateTimeWithTimeZone::from(Utc::now()))),
			)
			.filter(media::Column::Id.is_in(ids.iter().map(|id| id.to_string())))
			.filter(media::Column::DeletedAt.is_not_null())
			.filter(media::Column::LibraryId.in_subquery(visible_libraries(user)))
			.exec(conn)
			.await?
			.rows_affected;
		Ok(restored)
	}

	/// Deletes books in *Deleted books* permanently: each file is moved to the quarantine
	/// folder (`deleted/<date>/`, with a `restore_log.csv` row), then the book's data is
	/// deleted. A book whose file can't be moved stays in Deleted books; one whose file is
	/// already gone only has its data deleted
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn purge_deleted_books(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
	) -> Result<DeletedBooksPurgeResult> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();
		let requested: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
		let allowed =
			trashed_ids(conn, Some(requested.clone()), visible_libraries(user), None)
				.await?;
		let report =
			trash::purge_books(conn, &core.config, &allowed, Some(&user.id)).await?;
		let mut result = DeletedBooksPurgeResult::from(report);
		result.kept.extend(
			requested
				.into_iter()
				.filter(|id| !allowed.contains(id))
				.map(|id| TrashPurgeError {
					title: id.clone(),
					media_id: id,
					error: "It isn't in Deleted books".to_string(),
				}),
		);
		Ok(result)
	}

	/// Deletes every book in *Deleted books* permanently (in one library, if given), like
	/// `purgeDeletedBooks`
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn empty_deleted_books(
		&self,
		ctx: &Context<'_>,
		library_id: Option<ID>,
	) -> Result<DeletedBooksPurgeResult> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();
		let ids = trashed_ids(
			conn,
			None,
			visible_libraries(user),
			library_id.map(|id| id.to_string()),
		)
		.await?;
		let report = trash::purge_books(conn, &core.config, &ids, Some(&user.id)).await?;
		Ok(report.into())
	}

	/// How many days removed books stay in *Deleted books* before the daily purge deletes
	/// them permanently (0: until deleted by hand)
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageServer)")]
	async fn update_deleted_books_retention(
		&self,
		ctx: &Context<'_>,
		days: i32,
	) -> Result<DeletedBooksSettings> {
		let core = ctx.data::<CoreContext>()?;
		trash::set_retention_days(core.conn.as_ref(), days).await?;
		DeletedBooksSettings::load(core, true).await
	}

	/// Moves the file of a book deleted permanently back from the quarantine folder to where
	/// it was, and scans its folder: it comes back as a new book (its reading progress and
	/// annotations were deleted with it)
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn restore_purged_book(
		&self,
		ctx: &Context<'_>,
		id: i32,
	) -> Result<PurgedBook> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();
		let row = media_purge::Entity::find_by_id(id)
			.one(conn)
			.await?
			.ok_or("Deleted book not found")?;
		if let Some(library_id) = &row.library_id {
			let hidden = library_exclusion::Entity::find()
				.filter(library_exclusion::Column::UserId.eq(&user.id))
				.filter(library_exclusion::Column::LibraryId.eq(library_id))
				.count(conn)
				.await? > 0;
			if hidden {
				return Err("Deleted book not found".into());
			}
		}
		let restored = trash::restore_purged(core, id, &user.id).await?;
		Ok(PurgedBook::from_rows(conn, vec![restored])
			.await?
			.pop()
			.ok_or("Deleted book not found")?)
	}
}
