use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use models::{
	entity::{held_file, library, library_exclusion, media, media_purge},
	shared::enums::UserPermission,
};
use sea_orm::{prelude::*, QueryOrder, QuerySelect, QueryTrait};
use std::collections::HashMap;
use stump_core::trash;

use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
	object::{
		deleted_book::{
			exists, usernames, DeletedBook, DeletedBooksSettings, HeldFile, PurgedBook,
		},
		media::Media,
	},
};

#[derive(Default)]
pub struct TrashQuery;

#[Object]
impl TrashQuery {
	/// The books in *Deleted books* (removed from their library, not yet deleted
	/// permanently) in the libraries the user can see, most recently removed first
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn deleted_books(
		&self,
		ctx: &Context<'_>,
		library_id: Option<ID>,
	) -> Result<Vec<DeletedBook>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let libraries: HashMap<String, library::Model> =
			library::Entity::find_for_user(user)
				.all(conn)
				.await?
				.into_iter()
				.map(|library| (library.id.clone(), library))
				.collect();
		let mut query = media::ModelWithMetadata::find()
			.filter(media::Column::DeletedAt.is_not_null())
			.filter(
				media::Column::LibraryId
					.is_in(libraries.keys().cloned().collect::<Vec<_>>()),
			);
		if let Some(library_id) = library_id {
			query = query.filter(media::Column::LibraryId.eq(library_id.to_string()));
		}
		let rows = query
			.order_by_desc(media::Column::DeletedAt)
			.order_by_asc(media::Column::Name)
			.into_model::<media::ModelWithMetadata>()
			.all(conn)
			.await?;

		let names = usernames(
			conn,
			rows.iter()
				.filter_map(|row| row.media.deleted_by.clone())
				.collect(),
		)
		.await?;
		let retention = trash::retention_days(conn).await?;
		let now = Utc::now();
		let mut books = Vec::with_capacity(rows.len());
		for row in rows {
			let Some(library) = libraries.get(&row.media.library_id) else {
				continue;
			};
			let deleted_at = row.media.deleted_at.map(|at| at.to_utc()).unwrap_or(now);
			let path = std::path::PathBuf::from(&row.media.path);
			books.push(DeletedBook {
				deleted_at,
				deleted_by: row
					.media
					.deleted_by
					.as_ref()
					.and_then(|id| names.get(id).cloned()),
				expires_at: trash::expires_at(deleted_at, retention),
				purge_at: trash::purge_at(deleted_at, retention, now),
				library_id: library.id.clone(),
				library_name: library.name.clone(),
				relative_path: trash::files::library_relative(
					std::path::Path::new(&library.path),
					&path,
				)
				.to_string_lossy()
				.into_owned(),
				file_exists: exists(&path).await,
				book: Media::from(row),
			});
		}
		Ok(books)
	}

	/// How long removed books are kept and when they are deleted permanently. Users who may
	/// remove books see the retention; library managers also see the trash folder and the
	/// last automatic purge
	#[graphql(
		guard = "PermissionGuard::new(&[UserPermission::RemoveBook, UserPermission::ManageLibrary])"
	)]
	async fn deleted_books_settings(
		&self,
		ctx: &Context<'_>,
	) -> Result<DeletedBooksSettings> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		DeletedBooksSettings::load(
			core,
			user.has_permission(UserPermission::ManageLibrary),
		)
		.await
	}

	/// The books deleted permanently, most recent first (a log: their files were deleted)
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn purged_books(
		&self,
		ctx: &Context<'_>,
		#[graphql(default = 20)] limit: u64,
	) -> Result<Vec<PurgedBook>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let rows = media_purge::Entity::find()
			.filter(
				sea_orm::Condition::any()
					.add(media_purge::Column::LibraryId.is_null())
					.add(media_purge::Column::LibraryId.not_in_subquery(
						library_exclusion::Entity::library_hidden_to_user_query(user),
					)),
			)
			.order_by_desc(media_purge::Column::PurgedAt)
			.order_by_desc(media_purge::Column::Id)
			.limit(limit.min(200))
			.all(conn)
			.await?;
		PurgedBook::from_rows(conn, rows).await
	}

	/// The files Stump holds in its trash folder for the retention (originals replaced by
	/// post-processing, versions replaced by a put-back) in the libraries the user can see,
	/// most recent first
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn held_files(
		&self,
		ctx: &Context<'_>,
		library_id: Option<ID>,
	) -> Result<Vec<HeldFile>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();
		let libraries: HashMap<String, library::Model> =
			library::Entity::find_for_user(user)
				.all(conn)
				.await?
				.into_iter()
				.map(|library| (library.id.clone(), library))
				.collect();
		let mut query = held_file::Entity::find().filter(
			held_file::Column::LibraryId
				.is_in(libraries.keys().cloned().collect::<Vec<_>>()),
		);
		if let Some(library_id) = library_id {
			query = query.filter(held_file::Column::LibraryId.eq(library_id.to_string()));
		}
		let rows = query
			.order_by_desc(held_file::Column::HeldAt)
			.order_by_desc(held_file::Column::Id)
			.all(conn)
			.await?;
		let book_ids: Vec<String> =
			rows.iter().filter_map(|r| r.media_id.clone()).collect();
		let books: HashMap<String, media::ModelWithMetadata> = if book_ids.is_empty() {
			HashMap::new()
		} else {
			media::ModelWithMetadata::find()
				.filter(media::Column::Id.is_in(book_ids))
				.into_model::<media::ModelWithMetadata>()
				.all(conn)
				.await?
				.into_iter()
				.map(|row| (row.media.id.clone(), row))
				.collect()
		};
		let names = usernames(
			conn,
			rows.iter().filter_map(|r| r.held_by.clone()).collect(),
		)
		.await?;
		let retention = trash::retention_days(conn).await?;
		let now = Utc::now();
		let mut out = Vec::with_capacity(rows.len());
		for row in rows {
			let library = row.library_id.as_ref().and_then(|id| libraries.get(id));
			let book = row.media_id.as_ref().and_then(|id| books.get(id)).cloned();
			let file = trash::held::held_file_path(&core.config, &row);
			out.push(HeldFile {
				id: row.id,
				reason: row.reason,
				steps: row.steps.clone(),
				title: row.title.clone(),
				can_put_back: book.is_some() && exists(&file).await,
				book: book.map(Media::from),
				library_id: row.library_id.clone(),
				library_name: library.map(|l| l.name.clone()),
				relative_path: match library {
					Some(library) => trash::files::library_relative(
						std::path::Path::new(&library.path),
						std::path::Path::new(&row.original_path),
					)
					.to_string_lossy()
					.into_owned(),
					None => row.original_path.clone(),
				},
				bytes: row.bytes,
				held_at: row.held_at,
				held_by: row.held_by.as_ref().and_then(|id| names.get(id).cloned()),
				expires_at: trash::expires_at(row.held_at, retention),
				purge_at: trash::purge_at(row.held_at, retention, now),
			});
		}
		Ok(out)
	}
}

/// The libraries `user` can see, as a subquery
pub(crate) fn visible_libraries(
	user: &models::entity::user::AuthUser,
) -> sea_orm::sea_query::SelectStatement {
	library::Entity::find_for_user(user)
		.select_only()
		.column(library::Column::Id)
		.into_query()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_visible_libraries_excludes_hidden_ones() {
		let user = crate::tests::common::get_default_user();
		let sql =
			visible_libraries(&user).to_string(sea_orm::sea_query::SqliteQueryBuilder);
		assert!(sql.contains("library_exclusions"), "{sql}");
	}
}
