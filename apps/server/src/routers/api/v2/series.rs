use axum::{
	extract::{Path, State},
	middleware,
	routing::get,
	Extension, Router,
};
use graphql::data::AuthContext;
use models::{
	entity::{library_config, media, series},
	shared::image_processor_options::SupportedImageFormat,
};
use sea_orm::{prelude::*, sea_query::Query};
use stump_core::{
	config::StumpConfig,
	fs_utils::ContentType,
	image::thumbnail::{get_saved_thumbnail, get_thumbnail, is_generated_thumbnail},
};

use crate::{
	config::state::AppState,
	errors::{APIError, APIResult},
	middleware::auth::auth_middleware,
	utils::http::ImageResponse,
};

use super::media::get_media_thumbnail;

pub(crate) fn mount(app_state: AppState) -> Router<AppState> {
	Router::new()
		.nest(
			"/series/{id}",
			Router::new().route("/thumbnail", get(get_series_thumbnail_handler)),
		)
		.layer(middleware::from_fn_with_state(app_state, auth_middleware))
}

/// Whether the saved cover at `path` may be shown to a user. A generated cover is the cover of the
/// first book of a series (or a library), whatever its age rating, so a user with an age restriction
/// ("`hides_books`") gets the cover of a book they can see instead. A cover set by hand is what it
/// was set to.
pub(crate) fn cover_may_be_shown(path: &std::path::Path, hides_books: bool) -> bool {
	!hides_books || !is_generated_thumbnail(path)
}

pub(crate) async fn get_series_thumbnail(
	series: &series::SeriesThumbSelect,
	first_book: Option<media::MediaThumbSelect>,
	image_format: Option<SupportedImageFormat>,
	config: &StumpConfig,
	hides_books: bool,
) -> APIResult<(ContentType, Vec<u8>)> {
	// Note: This doesn't hard-fail because if the saved thumbnail is missing or corrupt, we want
	// to just pull something else instead of erroring out entirely.
	if let Some(path) = &series.thumbnail_path {
		if cover_may_be_shown(std::path::Path::new(path), hides_books) {
			match get_saved_thumbnail(std::path::Path::new(path)).await {
				Ok(result) => return Ok(result),
				Err(_) => {
					tracing::warn!(path = ?path, "Failed to get saved thumbnail");
				},
			}
		}
	}

	// the generated cover (a file named by the series' ID) is the first book's, see above
	let generated_thumb = if hides_books {
		None
	} else {
		get_thumbnail(config.thumbnails_directory(), &series.id, image_format).await?
	};

	match (generated_thumb, first_book) {
		(Some(result), _) => Ok(result),
		(None, Some(book)) => get_media_thumbnail(&book, image_format, config).await,
		(None, None) => Err(APIError::NotFound(
			"Series does not have a thumbnail".to_string(),
		)),
	}
}

async fn get_series_thumbnail_handler(
	Path(id): Path<String>,
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
) -> APIResult<ImageResponse> {
	let user = req.user();
	let series = series::Entity::find_for_user(&user)
		.filter(series::Column::Id.eq(id.clone()))
		.into_model::<series::SeriesThumbSelect>()
		.one(ctx.conn.as_ref())
		.await?
		.ok_or(APIError::NotFound("Series not found".to_string()))?;

	let hides_books = user.age_restriction.is_some();

	// Note: This doesn't hard-fail because if the saved thumbnail is missing or corrupt, we want
	// to just pull something else instead of erroring out entirely.
	if let Some(path) = &series.thumbnail_path {
		if cover_may_be_shown(std::path::Path::new(path), hides_books) {
			match get_saved_thumbnail(std::path::Path::new(path)).await {
				Ok(result) => return Ok(result.into()),
				Err(_) => {
					tracing::warn!(path = ?path, "Failed to get saved thumbnail");
				},
			}
		}
	}

	let first_book = media::Entity::order_in_series(
		media::Entity::find_for_user(&user)
			.filter(media::Column::SeriesId.eq(series.id.clone())),
	)
	.into_model::<media::MediaThumbSelect>()
	.one(ctx.conn.as_ref())
	.await?;

	let library_config = library_config::Entity::find()
		.filter(
			library_config::Column::LibraryId.in_subquery(
				Query::select()
					.column(series::Column::LibraryId)
					.from(series::Entity)
					.and_where(series::Column::Id.eq(series.id.clone()))
					.to_owned(),
			),
		)
		.one(ctx.conn.as_ref())
		.await?;
	let image_format = library_config.and_then(|o| o.thumbnail_config.map(|c| c.format));

	let (content_type, bytes) = get_series_thumbnail(
		&series,
		first_book,
		image_format,
		ctx.config.as_ref(),
		hides_books,
	)
	.await?;

	Ok(ImageResponse::new(content_type, bytes))
}
