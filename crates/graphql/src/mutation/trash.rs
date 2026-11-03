use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use models::{
	entity::{held_file, media},
	shared::{enums::UserPermission, trash::TrashPurgeError},
};
use sea_orm::{prelude::*, sea_query::Expr};
use stump_core::trash;

use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
	object::{
		deleted_book::{
			trashed_ids, DeletedBooksPurgeResult, DeletedBooksSettings, HeldFileKept,
			HeldFilePutBack, HeldFilesDeleteResult,
		},
		media::Media,
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

	/// Deletes books in *Deleted books* permanently: the book's data and its file (and the
	/// originals Stump holds for it) are deleted, for good. A book whose file or data can't be
	/// deleted stays in Deleted books; one whose file is already gone only has its data deleted
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

	/// Puts a held file back in its book's place (the original of a book post-processing
	/// replaced): the book's current file is held in turn, so this can be undone the same way
	/// until the retention is over
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn put_back_held_file(
		&self,
		ctx: &Context<'_>,
		id: i32,
	) -> Result<HeldFilePutBack> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		visible_held_files(core, user, &[id]).await?;
		let put = trash::held::put_back(core, id, &user.id).await?;
		let book = media::ModelWithMetadata::find_by_id(put.book.id.clone())
			.into_model::<media::ModelWithMetadata>()
			.one(core.conn.as_ref())
			.await?
			.ok_or("Media not found")?;
		Ok(HeldFilePutBack {
			book: Media::from(book),
			replaced: put.replaced.map(|r| r.id),
		})
	}

	/// Deletes held files for good (their copies in Stump's trash folder and their rows)
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn delete_held_files(
		&self,
		ctx: &Context<'_>,
		ids: Vec<i32>,
	) -> Result<HeldFilesDeleteResult> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		let allowed = visible_held_files(core, user, &ids).await?;
		let purge =
			trash::held::purge_held(core.conn.as_ref(), &core.config, &allowed).await?;
		Ok(HeldFilesDeleteResult {
			deleted: purge.deleted,
			freed_bytes: purge.bytes,
			kept: purge
				.kept
				.into_iter()
				.map(|(id, title, error)| HeldFileKept { id, title, error })
				.collect(),
		})
	}
}

/// The ids among `ids` of held files in libraries the user can see (an error when none is)
async fn visible_held_files(
	core: &stump_core::Ctx,
	user: &models::entity::user::AuthUser,
	ids: &[i32],
) -> Result<Vec<i32>> {
	let allowed: Vec<i32> = held_file::Entity::find()
		.filter(held_file::Column::Id.is_in(ids.to_vec()))
		.filter(held_file::Column::LibraryId.in_subquery(visible_libraries(user)))
		.all(core.conn.as_ref())
		.await?
		.into_iter()
		.map(|row| row.id)
		.collect();
	if allowed.is_empty() && !ids.is_empty() {
		return Err("Held file not found".into());
	}
	Ok(allowed)
}
