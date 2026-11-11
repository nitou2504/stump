use async_graphql::{Context, Object, Result, ID};
use models::entity::{bookmark, media, media_annotation};

use crate::{
	data::{AuthContext, CoreContext},
	object::{bookmark::Bookmark, epub::Epub, media_annotation::MediaAnnotation},
};

#[derive(Default)]
pub struct EpubQuery;

#[Object]
impl EpubQuery {
	/// Get a single epub by its media ID
	async fn epub_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Epub> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let model = media::Entity::find_media_ids_for_user(id.to_string(), user)
			.into_model::<media::MediaIdentSelect>()
			.one(conn)
			.await?
			.ok_or("Media not found")?;

		// Only an EPUB is opened here, as in the REST routes (and as the web reader chooses it, by
		// the file's extension). Any other book is a book that isn't here, answered exactly as an
		// unknown id is, and its file is never opened: a zip is a zip, so a PDF or a comic with a
		// zip stored in it (a friend's upload keeps its own bytes in those formats) would be parsed
		// as a book by a reader that was never meant to see it, and a table of contents nested
		// thousands of levels deep overflows the stack of the worker that reads it.
		if !model.is_epub() {
			return Err("Media not found".into());
		}

		Epub::try_from_async(model).await
	}

	/// Get all bookmarks for a single epub by its media ID
	async fn bookmarks_by_media_id(
		&self,
		ctx: &Context<'_>,
		id: ID,
	) -> Result<Vec<Bookmark>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		Ok(
			bookmark::Entity::find_for_user_and_media_id(user, id.as_ref())
				.into_model::<bookmark::Model>()
				.all(conn)
				.await?
				.into_iter()
				.map(Bookmark::from)
				.collect(),
		)
	}

	/// Get all annotations (highlights/notes) for a single book
	async fn annotations_by_media_id(
		&self,
		ctx: &Context<'_>,
		id: ID,
	) -> Result<Vec<MediaAnnotation>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		Ok(media_annotation::Model::find_for_user_and_media_id(
			&user.id,
			id.as_ref(),
			conn,
		)
		.await?
		.into_iter()
		.map(MediaAnnotation::from)
		.collect())
	}
}
