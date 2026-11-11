use std::path::PathBuf;

use axum::{
	extract::{Path, Query, State},
	http::{header, HeaderValue},
	middleware,
	response::{IntoResponse, Response},
	routing::get,
	Extension, Json, Router,
};
use graphql::data::AuthContext;
use models::{
	entity::{media, user::AuthUser},
	shared::readium::{RWPMPositions, RWPManifest},
};
use sea_orm::prelude::*;
use serde::Deserialize;
use stump_core::{
	media::processor::epub::get_resource_by_path,
	readium::{
		search::{search_epub, EpubSearchOptions, EPUB_SEARCH_DEFAULT_LIMIT},
		ReadiumManifestGenerator,
	},
};
use tokio::task::spawn_blocking;
use tokio_util::sync::CancellationToken;

use crate::{
	config::state::AppState,
	errors::{APIError, APIResult},
	middleware::{auth::auth_middleware, host::HostDetails, HostExtractor},
	utils::http::BufferResponse,
};

/// EPUB package streaming routes.
///
/// Auth model matches comic page streaming: `auth_middleware` plus
/// [`media::Entity::find_for_user`]. These routes do not require
/// `DownloadFile` — that permission applies only to full-archive download
/// via `GET /api/v2/media/{id}/file`.
///
/// Absolute `href`s in RWPM/positions use the request [`HostExtractor`]. Behind
/// a TLS terminator, enable `trust_proxy_headers` and forward
/// `X-Forwarded-Proto` / `Forwarded`.
pub(crate) fn mount(app_state: AppState) -> Router<AppState> {
	Router::new()
		.nest(
			"/epub/{id}",
			Router::new()
				.route("/manifest.json", get(get_epub_manifest))
				.route("/positions.json", get(get_epub_positions))
				.route("/search", get(get_epub_search))
				.route("/resource/{*path}", get(get_epub_resource)),
		)
		.layer(middleware::from_fn_with_state(app_state, auth_middleware))
}

async fn find_ebook_for_user(
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
	// Only an EPUB is read here, as the web reader chooses it (by the file's extension). These routes
	// open any file with an EPUB reader, and a zip is a zip: a comic or a PDF with the files of an
	// EPUB in it (a friend's upload keeps its own bytes in those formats) would be drawn by the
	// reader, scripts and all, on the app's own origin. Anything else is a book that isn't here.
	let is_epub = std::path::Path::new(&book.path)
		.extension()
		.is_some_and(|extension| extension.eq_ignore_ascii_case("epub"));
	if !is_epub {
		return Err(APIError::NotFound("Book not found".to_string()));
	}
	Ok(book)
}

/// Resolve the request-scoped base URL used in RWPM absolute resource links.
fn epub_service_base_url(host_details: &HostDetails, id: &str) -> String {
	host_details.url_for_path(&format!("api/v2/epub/{id}"))
}

struct WebPubManifestResponse(RWPManifest);

impl IntoResponse for WebPubManifestResponse {
	fn into_response(self) -> Response {
		(
			[(header::CONTENT_TYPE, "application/webpub+json")],
			Json(self.0),
		)
			.into_response()
	}
}

struct WebPubPositionsResponse(RWPMPositions);

impl IntoResponse for WebPubPositionsResponse {
	fn into_response(self) -> Response {
		(
			[(
				header::CONTENT_TYPE,
				"application/vnd.readium.position-list+json",
			)],
			Json(self.0),
		)
			.into_response()
	}
}

/// Get the Readium Web Publication Manifest for an epub file
///
/// See: https://readium.org/webpub-manifest/
async fn get_epub_manifest(
	Path(id): Path<String>,
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
	HostExtractor(host_details): HostExtractor,
) -> APIResult<WebPubManifestResponse> {
	let AuthContext { user, .. } = req;
	let ebook = find_ebook_for_user(ctx.conn.as_ref(), &user, &id).await?;

	let base_url = epub_service_base_url(&host_details, &id);
	let generator = ReadiumManifestGenerator::new(&ebook.path, base_url);
	// opening the book and reading its package is blocking work: off the async workers, which
	// the rest of the server's requests share
	let manifest = spawn_blocking(move || generator.generate_manifest())
		.await
		.map_err(|e| APIError::InternalServerError(e.to_string()))??;

	Ok(WebPubManifestResponse(manifest))
}

/// Get the positions list for an epub file
///
/// See: https://readium.org/architecture/models/locators/positions/
async fn get_epub_positions(
	Path(id): Path<String>,
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
	HostExtractor(host_details): HostExtractor,
) -> APIResult<WebPubPositionsResponse> {
	let AuthContext { user, .. } = req;
	let ebook = find_ebook_for_user(ctx.conn.as_ref(), &user, &id).await?;

	let base_url = epub_service_base_url(&host_details, &id);
	let generator = ReadiumManifestGenerator::new(&ebook.path, base_url);
	// TODO: consider making generator be async, not the place in this feature branch
	// to eval really
	let positions = spawn_blocking(move || generator.generate_positions())
		.await
		.map_err(|e| APIError::InternalServerError(e.to_string()))??;

	Ok(WebPubPositionsResponse(positions))
}

#[derive(Debug, Deserialize)]
struct EpubSearchQueryParams {
	q: String,
	#[serde(default = "default_search_limit")]
	limit: usize,
	cursor: Option<String>,
}

fn default_search_limit() -> usize {
	EPUB_SEARCH_DEFAULT_LIMIT
}

/// Bounded whole-book search over EPUB spine XHTML.
///
/// Returns plain-text excerpts and Readium locators. Does not require
/// `DownloadFile` — the scan never ships the archive to the client.
async fn get_epub_search(
	Path(id): Path<String>,
	Query(params): Query<EpubSearchQueryParams>,
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
	HostExtractor(host_details): HostExtractor,
) -> APIResult<impl IntoResponse> {
	let AuthContext { user, .. } = req;
	let ebook = find_ebook_for_user(ctx.conn.as_ref(), &user, &id).await?;
	let base_url = epub_service_base_url(&host_details, &id);

	let cursor = match params.cursor.as_deref() {
		Some(raw) if !raw.is_empty() => Some(EpubSearchOptions::decode_cursor(raw)?),
		_ => None,
	};

	let options = EpubSearchOptions::new(params.q)
		.with_limit(params.limit)
		.with_cursor(cursor);
	options.validate()?;

	let cancel = CancellationToken::new();
	let cancel_for_task = cancel.clone();
	let path = ebook.path.clone();

	// Dropping the request future cancels the token so the blocking scan can stop.
	let _guard = cancel.drop_guard();

	let response =
		spawn_blocking(move || search_epub(&path, &base_url, options, &cancel_for_task))
			.await
			.map_err(|e| APIError::InternalServerError(e.to_string()))??;

	Ok(Json(response))
}

/// Get a resource from an epub file by package-relative path.
///
/// Paths may be nested (e.g. `OEBPS/Images/cover.jpg`). Axum percent-decodes
/// path segments before extraction.
async fn get_epub_resource(
	Path((id, path)): Path<(String, String)>,
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
) -> APIResult<Response> {
	let resource = get_epub_resource_bytes(Path((id, path)), State(ctx), Extension(req))
		.await?
		.into_response();
	Ok(without_scripts(resource))
}

/// What opening a resource of a book at its address (not through the reader, which fetches the
/// text and draws it in a frame of its own) may do: nothing that runs. The page is shown in a
/// sandbox with no scripts, no frames and no forms, and only the book's own images, fonts and
/// styles load. The reader is not affected: a browser applies the policy of a document to the
/// document only, and the reader's pages are made from the text this returns.
const RESOURCE_POLICY: &str = "sandbox; default-src 'none'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; media-src 'self'";

fn without_scripts(mut response: Response) -> Response {
	let headers = response.headers_mut();
	headers.insert(
		header::CONTENT_SECURITY_POLICY,
		HeaderValue::from_static(RESOURCE_POLICY),
	);
	headers.insert(
		header::X_CONTENT_TYPE_OPTIONS,
		HeaderValue::from_static("nosniff"),
	);
	response
}

async fn get_epub_resource_bytes(
	Path((id, path)): Path<(String, String)>,
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
) -> APIResult<BufferResponse> {
	let AuthContext { user, .. } = req;
	let ebook = find_ebook_for_user(ctx.conn.as_ref(), &user, &id).await?;

	// Drop leading slash / fragments if a client accidentally includes them.
	let path = path.trim_start_matches('/');
	let path = path.split_once('#').map(|(p, _)| p).unwrap_or(path);
	let path_buf = PathBuf::from(path);

	let (root, resource) = match (path_buf.parent(), path_buf.file_name()) {
		(Some(parent), Some(file_name)) => (
			parent.to_string_lossy().to_string(),
			PathBuf::from(file_name),
		),
		_ => (String::new(), path_buf.clone()),
	};
	// every resource opens the book again and reads its package: blocking work, off the async
	// workers that the rest of the server's requests share
	let book = PathBuf::from(&ebook.path);
	let resource = spawn_blocking(move || get_resource_by_path(&book, &root, resource))
		.await
		.map_err(|e| APIError::InternalServerError(e.to_string()))??;

	Ok(resource.into())
}
