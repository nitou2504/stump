use std::time::Duration;

use axum::{
	body::Body,
	extract::{Path, Query, State},
	http::{header, HeaderMap, HeaderValue, StatusCode},
	middleware,
	response::{IntoResponse, Response},
	routing::get,
	Extension, Router,
};
use graphql::data::AuthContext;
use models::entity::{media, user::AuthUser};
use sea_orm::prelude::*;
use serde::Deserialize;
use stump_core::pdf_reader::{
	document_version, search_document, ClientKey, PdfReaderError, PdfReaderService,
	Priority,
};

use crate::{
	config::state::AppState,
	errors::{APIError, APIResult},
	middleware::auth::auth_middleware,
};

/// The PDF reader's routes: layout, page pictures sized to the screen, text layers and search.
///
/// Auth model matches EPUB streaming: `auth_middleware` plus [`media::Entity::find_for_user`]
/// (library access, age restriction, removed books). `DownloadFile` is not required: the file
/// itself never leaves the server. Contract: `/tmp/pdf-lane/CONTRACT.md`.
pub(crate) fn mount(app_state: AppState) -> Router<AppState> {
	Router::new()
		.nest(
			"/pdf/{id}",
			Router::new()
				.route("/layout", get(get_pdf_layout))
				.route("/page/{page}", get(get_pdf_page))
				.route("/text/{page}", get(get_pdf_text))
				.route("/search", get(get_pdf_search)),
		)
		.layer(middleware::from_fn_with_state(app_state, auth_middleware))
}

const IMMUTABLE: &str = "private, max-age=31536000, immutable";
const REVALIDATE: &str = "private, no-cache";
/// How long one search call may scan before returning a cursor
const SEARCH_BUDGET: Duration = Duration::from_secs(2);
const SEARCH_DEFAULT_LIMIT: usize = 50;
const SEARCH_MAX_LIMIT: usize = 200;

async fn find_pdf_for_user(
	conn: &DatabaseConnection,
	user: &AuthUser,
	id: &str,
) -> APIResult<media::MediaIdentSelect> {
	let book = media::Entity::find_for_user(user)
		.filter(media::Column::Id.eq(id.to_string()))
		.into_model::<media::MediaIdentSelect>()
		.one(conn)
		.await?
		.ok_or_else(|| APIError::NotFound("Book not found".to_string()))?;
	let is_pdf = std::path::Path::new(&book.path)
		.extension()
		.and_then(|extension| extension.to_str())
		.is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
	if !is_pdf {
		return Err(APIError::BadRequest("This book is not a PDF".to_string()));
	}
	Ok(book)
}

fn reader_error(error: PdfReaderError) -> APIError {
	match error {
		PdfReaderError::PageNotFound => {
			APIError::BadRequest("The page is out of range".to_string())
		},
		PdfReaderError::Dropped => APIError::ServiceUnavailable(error.to_string()),
		PdfReaderError::Failed(message) => APIError::InternalServerError(message),
	}
}

fn cache_control(requested: Option<&str>, current: &str) -> &'static str {
	if requested == Some(current) {
		IMMUTABLE
	} else {
		REVALIDATE
	}
}

#[derive(Debug, Deserialize)]
struct LayoutQuery {
	/// The page the reader will show first: its picture starts rendering with the layout,
	/// so the page request that follows finds it in flight or cached
	page: Option<u32>,
	/// The width for that picture, as in `/page/{n}?w=`
	w: Option<u32>,
}

async fn get_pdf_layout(
	Path(id): Path<String>,
	Query(query): Query<LayoutQuery>,
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
	headers: HeaderMap,
) -> APIResult<Response> {
	let book = find_pdf_for_user(ctx.conn.as_ref(), &req.user, &id).await?;
	let path = std::path::Path::new(&book.path);
	let service = PdfReaderService::global(ctx.config.as_ref());

	let first_page = query
		.page
		.and_then(|page| i32::try_from(page).ok())
		.map(|page| {
			let client = ClientKey {
				user_id: req.user.id.clone(),
				media_id: book.id.clone(),
			};
			(page, query.w.unwrap_or(DEFAULT_WIDTH).max(1), client)
		});
	let layout = service
		.layout_then(path, first_page)
		.await
		.map_err(reader_error)?;

	let etag = format!("\"{}\"", layout.v);
	let not_modified = headers
		.get(header::IF_NONE_MATCH)
		.and_then(|value| value.to_str().ok())
		.is_some_and(|value| value.split(',').any(|tag| tag.trim() == etag));
	let builder = Response::builder()
		.header(header::CACHE_CONTROL, REVALIDATE)
		.header(header::ETAG, &etag);
	let response = if not_modified {
		builder.status(StatusCode::NOT_MODIFIED).body(Body::empty())
	} else {
		let body = serde_json::to_vec(layout.as_ref())
			.map_err(|error| APIError::InternalServerError(error.to_string()))?;
		builder
			.status(StatusCode::OK)
			.header(header::CONTENT_TYPE, "application/json")
			.body(Body::from(body))
	};
	response.map_err(|error| APIError::InternalServerError(error.to_string()))
}

#[derive(Debug, Deserialize)]
struct PageQuery {
	w: Option<u32>,
	v: Option<String>,
	p: Option<String>,
}

const DEFAULT_WIDTH: u32 = 1000;

async fn get_pdf_page(
	Path((id, page)): Path<(String, u32)>,
	Query(query): Query<PageQuery>,
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
) -> APIResult<Response> {
	let book = find_pdf_for_user(ctx.conn.as_ref(), &req.user, &id).await?;
	let path = std::path::Path::new(&book.path);
	let page = i32::try_from(page)
		.map_err(|_| APIError::BadRequest("The page is out of range".to_string()))?;
	let width = query.w.unwrap_or(DEFAULT_WIDTH);
	if width == 0 {
		return Err(APIError::BadRequest("w must be at least 1".to_string()));
	}
	let priority = match query.p.as_deref() {
		Some("prefetch") => Priority::Prefetch,
		_ => Priority::Foreground,
	};
	let client = ClientKey {
		user_id: req.user.id.clone(),
		media_id: book.id.clone(),
	};

	let service = PdfReaderService::global(ctx.config.as_ref());
	let picture = match service
		.picture(path, page, width, priority, Some(client))
		.await
	{
		Ok(picture) => picture,
		Err(PdfReaderError::Dropped) => {
			return Response::builder()
				.status(StatusCode::SERVICE_UNAVAILABLE)
				.header(header::RETRY_AFTER, "1")
				.header(header::CACHE_CONTROL, "no-store")
				.body(Body::empty())
				.map_err(|error| APIError::InternalServerError(error.to_string()));
		},
		Err(error) => return Err(reader_error(error)),
	};

	let version = document_version(path).map_err(reader_error)?;
	let bytes = std::sync::Arc::try_unwrap(picture.bytes)
		.unwrap_or_else(|shared| (*shared).clone());
	let mut response = Response::builder()
		.status(StatusCode::OK)
		.header(header::CONTENT_TYPE, "image/webp")
		.header(
			header::CACHE_CONTROL,
			cache_control(query.v.as_deref(), &version),
		)
		.header("X-Pdf-Tier", picture.tier.to_string())
		.body(Body::from(bytes))
		.map_err(|error| APIError::InternalServerError(error.to_string()))?;
	if picture.height > 0 {
		if let Ok(value) =
			HeaderValue::from_str(&format!("{}x{}", picture.width, picture.height))
		{
			response.headers_mut().insert("X-Pdf-Page-Size", value);
		}
	}
	Ok(response)
}

#[derive(Debug, Deserialize)]
struct TextQuery {
	v: Option<String>,
	p: Option<String>,
}

async fn get_pdf_text(
	Path((id, page)): Path<(String, u32)>,
	Query(query): Query<TextQuery>,
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
) -> APIResult<Response> {
	let book = find_pdf_for_user(ctx.conn.as_ref(), &req.user, &id).await?;
	let path = std::path::Path::new(&book.path);
	let page = i32::try_from(page)
		.map_err(|_| APIError::BadRequest("The page is out of range".to_string()))?;
	let priority = match query.p.as_deref() {
		Some("prefetch") => Priority::Prefetch,
		_ => Priority::Foreground,
	};
	let service = PdfReaderService::global(ctx.config.as_ref());
	let text = service
		.text(path, page, priority)
		.await
		.map_err(reader_error)?;
	let version = document_version(path).map_err(reader_error)?;
	let body = serde_json::to_vec(text.as_ref())
		.map_err(|error| APIError::InternalServerError(error.to_string()))?;
	Response::builder()
		.status(StatusCode::OK)
		.header(header::CONTENT_TYPE, "application/json")
		.header(
			header::CACHE_CONTROL,
			cache_control(query.v.as_deref(), &version),
		)
		.body(Body::from(body))
		.map_err(|error| APIError::InternalServerError(error.to_string()))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
	q: String,
	limit: Option<usize>,
	cursor: Option<String>,
}

async fn get_pdf_search(
	Path(id): Path<String>,
	Query(query): Query<SearchQuery>,
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
) -> APIResult<impl IntoResponse> {
	let book = find_pdf_for_user(ctx.conn.as_ref(), &req.user, &id).await?;
	let path = std::path::Path::new(&book.path);
	if stump_core::pdf_reader::fold_query(&query.q).chars().count() < 2 {
		return Err(APIError::BadRequest(
			"The search needs at least 2 characters".to_string(),
		));
	}
	let start_page = match query.cursor.as_deref() {
		Some(cursor) if !cursor.is_empty() => cursor
			.parse::<i32>()
			.map_err(|_| APIError::BadRequest("Invalid cursor".to_string()))?,
		_ => 1,
	};
	let limit = query
		.limit
		.unwrap_or(SEARCH_DEFAULT_LIMIT)
		.clamp(1, SEARCH_MAX_LIMIT);

	let service = PdfReaderService::global(ctx.config.as_ref());
	let layout = service.layout(path).await.map_err(reader_error)?;
	// Dropping the request future stops the scan: every page waits on the render thread
	let results = search_document(
		service,
		path,
		layout.page_count,
		&query.q,
		start_page,
		limit,
		SEARCH_BUDGET,
	)
	.await
	.map_err(reader_error)?;
	Ok(axum::Json(results))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pictures_are_immutable_only_for_the_current_version() {
		assert_eq!(cache_control(Some("abc"), "abc"), IMMUTABLE);
		assert_eq!(cache_control(Some("old"), "abc"), REVALIDATE);
		assert_eq!(cache_control(None, "abc"), REVALIDATE);
	}
}
