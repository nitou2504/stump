use async_graphql::{Context, Object, Result, ID};
use models::{entity::media, shared::enums::UserPermission};
use sea_orm::prelude::*;
use stump_core::intake::edit::suggest_for_book;

use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
	object::metadata_edit::MediaGenreSuggestions,
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
		Ok(suggest_for_book(core, &book.media, &meta, online).await?.into())
	}
}
