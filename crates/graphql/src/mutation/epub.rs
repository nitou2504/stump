use crate::{
	data::{AuthContext, CoreContext},
	input::media::{BookmarkInput, CreateAnnotationInput, UpdateAnnotationInput},
	object::{bookmark::Bookmark, media_annotation::MediaAnnotation},
};
use async_graphql::{Context, Object, Result};
use models::entity::{
	bookmark, media, media_annotation, reading_session, user::AuthUser,
};
use sea_orm::{prelude::*, QuerySelect, Set};

/// Bookmarks and annotations are private to their user, and can only be made on a book the
/// user can see
async fn ensure_media_access(
	conn: &DatabaseConnection,
	user: &AuthUser,
	media_id: &str,
) -> Result<()> {
	media::Entity::find_media_ids_for_user(media_id.to_string(), user)
		.into_model::<media::MediaIdentSelect>()
		.one(conn)
		.await?
		.ok_or("Media not found")?;
	Ok(())
}

#[derive(Default)]
pub struct EpubMutation;

// TODO: Would it make sense to fold these into the media mutation? Do people want
// bookmarks/annotations/etc for non-epub content?

#[Object]
impl EpubMutation {
	/// Create a bookmark for a user
	async fn create_bookmark(
		&self,
		ctx: &Context<'_>,
		input: BookmarkInput,
	) -> Result<Bookmark> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		ensure_media_access(conn, user, &input.media_id).await?;

		// TODO: technically this _should_ always find a valid session, when landing in a
		// reader it should fire and handle the session extension vs creation, but i am
		// also wary of assuming this will always be the case. so add a test or review
		// readium reader code to ensure above
		// maybe it is worth adding `find_latest_non_lapsed_session_for_user_and_media`
		// to be safe
		let session_id = reading_session::Entity::find_latest_for_user_and_media(
			user,
			&input.media_id,
		)
		.select_only()
		.column(reading_session::Column::Id)
		.into_tuple::<i32>()
		.one(conn)
		.await?;

		let mut active_model = input.into_active_model(user);
		active_model.session_id = Set(session_id);
		let bookmark = active_model.insert(conn).await?;

		Ok(Bookmark { model: bookmark })
	}

	/// Delete a bookmark by ID, only if the user created it
	async fn delete_bookmark(&self, ctx: &Context<'_>, id: String) -> Result<Bookmark> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let bookmark = bookmark::Entity::find_for_user(user)
			.filter(bookmark::Column::Id.eq(id))
			.one(conn)
			.await?
			.ok_or("Bookmark not found")?;

		let _ = bookmark.clone().delete(conn).await?;
		Ok(Bookmark { model: bookmark })
	}

	/// Create an annotation (a highlight, underline, strikethrough or invert, with an
	/// optional note)
	async fn create_annotation(
		&self,
		ctx: &Context<'_>,
		input: CreateAnnotationInput,
	) -> Result<MediaAnnotation> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		ensure_media_access(conn, user, &input.media_id).await?;

		// TODO: technically this should always find a valid session, when landing in a
		// reader it should fire and handle the session extension vs creation, but i am
		// also wary of assuming this will always be the case. so add a test or review
		// readium reader code to ensure above
		let session_id = reading_session::Entity::find_latest_for_user_and_media(
			user,
			&input.media_id,
		)
		.select_only()
		.column(reading_session::Column::Id)
		.into_tuple::<i32>()
		.one(conn)
		.await?;

		let mut annotation = input.into_active_model(user);
		annotation.session_id = Set(session_id);
		let created_annotation = annotation.insert(conn).await?;

		Ok(MediaAnnotation::from(created_annotation))
	}

	/// Change an annotation's note and/or style, only if the user created it
	async fn update_annotation(
		&self,
		ctx: &Context<'_>,
		input: UpdateAnnotationInput,
	) -> Result<MediaAnnotation> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let annotation = media_annotation::Entity::find()
			.filter(media_annotation::Column::Id.eq(&input.id))
			.filter(media_annotation::Column::UserId.eq(&user.id))
			.one(conn)
			.await?
			.ok_or("Annotation not found")?;

		let mut active_model: media_annotation::ActiveModel = annotation.into();
		input.apply(&mut active_model);

		let updated = active_model.update(conn).await?;
		Ok(MediaAnnotation::from(updated))
	}

	/// Delete an annotation by ID, only if the user created it
	async fn delete_annotation(
		&self,
		ctx: &Context<'_>,
		id: String,
	) -> Result<MediaAnnotation> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let annotation = media_annotation::Entity::find()
			.filter(media_annotation::Column::Id.eq(&id))
			.filter(media_annotation::Column::UserId.eq(&user.id))
			.one(conn)
			.await?
			.ok_or("Annotation not found")?;

		let _ = annotation.clone().delete(conn).await?;
		Ok(MediaAnnotation::from(annotation))
	}
}
