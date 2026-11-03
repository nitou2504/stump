use async_graphql::{Context, Object, Result, ID};
use models::{entity::media, shared::enums::UserPermission};
use sea_orm::prelude::*;
use stump_core::intake::edit::{edit_history, suggest_for_book};

use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
	object::metadata_edit::{MediaGenreSuggestions, MetadataEditSave},
};

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
