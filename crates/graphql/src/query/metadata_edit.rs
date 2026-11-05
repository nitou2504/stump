use std::collections::HashMap;

use async_graphql::{Context, Object, Result, ID};
use metadata_integrations::RequestPriority;
use models::{entity::media, shared::enums::UserPermission};
use sea_orm::{prelude::*, Condition};
use stump_core::intake::{
	describe::{find_descriptions, summary_locked, summary_missing},
	edit::{edit_history, suggest_for_book},
};

use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
	object::metadata_edit::{
		MediaDescriptionStatus, MediaDescriptions, MediaGenreSuggestions,
		MetadataEditSave,
	},
};

/// The most books `findMediaDescriptions` looks up in one call
const DESCRIPTION_BATCH_MAX: usize = 25;

#[derive(Default)]
pub struct MetadataEditQuery;

#[Object]
impl MetadataEditQuery {
	/// Tags for a book already in the library: from its author's and series' other books,
	/// books with a similar description and (when `online`, else the library's intake
	/// setting) Open Library / Google Books subjects, with its current tags judged against
	/// the same evidence
	#[graphql(guard = "PermissionGuard::one(UserPermission::EditMetadata)")]
	async fn media_genre_suggestions(
		&self,
		ctx: &Context<'_>,
		id: ID,
		online: Option<bool>,
	) -> Result<MediaGenreSuggestions> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		let book = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::Id.eq(id.to_string()))
			.into_model::<media::ModelWithMetadata>()
			.one(core.conn.as_ref())
			.await?
			.ok_or("Media not found")?;
		let meta = book.metadata.clone().unwrap_or_default();
		Ok(Box::pin(suggest_for_book(core, &book.media, &meta, online))
			.await?
			.into())
	}

	/// Descriptions for a book from the online sources (Hardcover first, then Open Library
	/// and Google Books), for its summary in *Edit details*. Nothing is saved
	#[graphql(guard = "PermissionGuard::one(UserPermission::EditMetadata)")]
	async fn media_description_suggestions(
		&self,
		ctx: &Context<'_>,
		id: ID,
	) -> Result<MediaDescriptions> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		let book = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::Id.eq(id.to_string()))
			.filter(media::Column::DeletedAt.is_null())
			.into_model::<media::ModelWithMetadata>()
			.one(core.conn.as_ref())
			.await?
			.ok_or("Media not found")?;
		let meta = book.metadata.clone().unwrap_or_default();
		let found = Box::pin(find_descriptions(
			core.conn.as_ref(),
			&book.media,
			&meta,
			RequestPriority::Interactive,
		))
		.await;
		Ok(MediaDescriptions::found(
			&book.media,
			meta.summary.clone(),
			found,
		))
	}

	/// Descriptions for many books, for scripts: the books asked by `ids` and `paths` (as
	/// Stump stores them), in that order, one result each. With `onlyMissing` (the default)
	/// a book whose summary has 15 words or more is not searched (`HAS_SUMMARY`); a book whose
	/// summary is locked (set or cleared by hand) never is (`LOCKED`). Nothing is saved:
	/// review the `candidates`, then save with `editMediaMetadataBatch`. Hardcover is asked
	/// as background work: within its daily budget for background work, behind people's
	/// lookups (about one request every 4 seconds, 2 or 3 a book); when the budget is used
	/// up, books report `RETRY_LATER`. At most 25 books per call
	#[graphql(guard = "PermissionGuard::one(UserPermission::EditMetadata)")]
	async fn find_media_descriptions(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] ids: Vec<ID>,
		#[graphql(default)] paths: Vec<String>,
		#[graphql(default = true)] only_missing: bool,
	) -> Result<Vec<MediaDescriptions>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();
		if ids.len() + paths.len() > DESCRIPTION_BATCH_MAX {
			return Err(format!("At most {DESCRIPTION_BATCH_MAX} books per call").into());
		}
		let ids: Vec<String> = ids.into_iter().map(|id| id.to_string()).collect();
		let books = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::DeletedAt.is_null())
			.filter(
				Condition::any()
					.add(media::Column::Id.is_in(ids.clone()))
					.add(media::Column::Path.is_in(paths.clone())),
			)
			.into_model::<media::ModelWithMetadata>()
			.all(conn)
			.await?;
		let by_id: HashMap<&str, &media::ModelWithMetadata> =
			books.iter().map(|b| (b.media.id.as_str(), b)).collect();
		let by_path: HashMap<&str, &media::ModelWithMetadata> =
			books.iter().map(|b| (b.media.path.as_str(), b)).collect();
		let asked = ids
			.iter()
			.map(|id| (Some(id.clone()), None, by_id.get(id.as_str()).copied()))
			.chain(paths.iter().map(|path| {
				(
					None,
					Some(path.clone()),
					by_path.get(path.as_str()).copied(),
				)
			}))
			.collect::<Vec<_>>();

		let mut results = Vec::with_capacity(asked.len());
		for (id, path, book) in asked {
			let Some(book) = book else {
				results.push(MediaDescriptions::skipped(
					id,
					path,
					MediaDescriptionStatus::NotFound,
					None,
				));
				continue;
			};
			let meta = book.metadata.clone().unwrap_or_default();
			let skip = if summary_locked(&meta) {
				Some(MediaDescriptionStatus::Locked)
			} else if only_missing && !summary_missing(&meta) {
				Some(MediaDescriptionStatus::HasSummary)
			} else {
				None
			};
			if let Some(status) = skip {
				results.push(MediaDescriptions::skipped(
					Some(book.media.id.clone()),
					Some(book.media.path.clone()),
					status,
					meta.summary.clone(),
				));
				continue;
			}
			let found = Box::pin(find_descriptions(
				conn,
				&book.media,
				&meta,
				RequestPriority::Background,
			))
			.await;
			results.push(MediaDescriptions::found(
				&book.media,
				meta.summary.clone(),
				found,
			));
		}
		Ok(results)
	}

	/// The latest saves of a book's metadata edit history, newest first (20 by default)
	#[graphql(guard = "PermissionGuard::one(UserPermission::EditMetadata)")]
	async fn media_metadata_edits(
		&self,
		ctx: &Context<'_>,
		media_id: ID,
		limit: Option<i32>,
	) -> Result<Vec<MetadataEditSave>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		let book = media::Entity::find_for_user(user)
			.filter(media::Column::Id.eq(media_id.to_string()))
			.one(core.conn.as_ref())
			.await?
			.ok_or("Media not found")?;
		let limit = limit.unwrap_or(20).clamp(1, 200) as usize;
		Ok(edit_history(core, &book.id, limit)
			.await?
			.into_iter()
			.map(MetadataEditSave::from)
			.collect())
	}
}
