use crate::{
	data::{AuthContext, CoreContext},
	input::media::{
		BookmarkInput, CreateAnnotationInput, UpdateAnnotationInput, PDF_LOCATOR_TYPE,
	},
	object::{bookmark::Bookmark, media_annotation::MediaAnnotation},
};
use async_graphql::{Context, Object, Result};
use models::{
	entity::{bookmark, media, media_annotation, reading_session, user::AuthUser},
	shared::readium::ReadiumLocator,
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

/// The most boxes one PDF annotation may have (a selection over a few pages of lines)
const MAX_PDF_RECTS: usize = 500;

/// PDF locators address a page (`locations.position`, 1-based) and boxes on it as fractions of
/// the page; reject the ones the reader could not draw. Other locators are not checked.
async fn validate_pdf_locator(
	conn: &DatabaseConnection,
	media_id: &str,
	locator: &ReadiumLocator,
) -> Result<()> {
	if locator.r#type != PDF_LOCATOR_TYPE {
		return Ok(());
	}
	let page = locator
		.page()
		.ok_or("Invalid locator: a PDF locator needs locations.position (the page)")?;
	let pages = media::Entity::find_by_id(media_id.to_string())
		.select_only()
		.column(media::Column::Pages)
		.into_tuple::<i32>()
		.one(conn)
		.await?
		.unwrap_or(0);
	if page < 1 || (pages > 0 && page > pages) {
		return Err(format!("Invalid locator: page {page} is out of range").into());
	}
	let rects = locator
		.locations
		.as_ref()
		.and_then(|locations| locations.rects.as_ref());
	if let Some(rects) = rects {
		if rects.len() > MAX_PDF_RECTS {
			return Err("Invalid locator: too many rects".into());
		}
		let one = Decimal::ONE;
		let slack = Decimal::new(1001, 3);
		let valid = rects.iter().all(|rect| {
			[rect.x, rect.y, rect.width, rect.height]
				.iter()
				.all(|value| *value >= Decimal::ZERO && *value <= one)
				&& rect.x + rect.width <= slack
				&& rect.y + rect.height <= slack
		});
		if !valid {
			return Err(
				"Invalid locator: rects must be fractions (0-1) of the page".into()
			);
		}
	}
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
		validate_pdf_locator(conn, &input.media_id, &input.locator).await?;

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
		validate_pdf_locator(conn, &input.media_id, &input.locator).await?;

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
