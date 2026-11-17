//! `/api/v2/content`: the text index of the books (EPUB and PDF), for programs that read the
//! shelf (see `stump_core::content`). Generic: nothing here knows who asks.
//!
//! Auth is the server's usual (`auth_middleware`: session or API key). Who may see which book is
//! decided in ONE place for every endpoint, [`stump_core::content::catalog::visible_books`]: the
//! books `media::Entity::find_for_user` returns for the user (library access, age restriction,
//! removed books) that also pass the scope filters every endpoint takes: `tags_any`, `tags_none`
//! and `exclude`. A book outside that is answered as a book that does not exist (404), by id too:
//! `read` and `grep` can't reach what `search` hides.

use axum::{
	extract::{Path, Query, State},
	middleware,
	routing::{get, post},
	Extension, Json, Router,
};
use graphql::data::AuthContext;
use models::shared::enums::UserPermission;
use serde::{Deserialize, Serialize};
use stump_core::content::{
	catalog::{self, CatalogRow, Scope},
	job::ContentIndexConfig,
	query::{self, MAX_PAGE_SIZE, MAX_READ_CHARS},
	store, ContentError,
};
use stump_core::job::StumpJob;
use uuid::Uuid;

use crate::{
	config::state::AppState,
	errors::{APIError, APIResult},
	middleware::auth::auth_middleware,
};

pub(crate) fn mount(app_state: AppState) -> Router<AppState> {
	Router::new()
		.nest(
			"/content",
			Router::new()
				.route("/status", get(get_status))
				.route("/search", get(get_search))
				.route("/media/{id}/sections", get(get_sections))
				.route("/media/{id}/sections/{n}", get(get_section))
				.route("/media/{id}/grep", get(get_grep))
				.route("/lookup", get(get_lookup))
				.route("/tags", get(get_tags))
				.route("/reindex", post(post_reindex)),
		)
		.layer(middleware::from_fn_with_state(app_state, auth_middleware))
}

const MAX_PAGE: u32 = 100_000;
const MAX_FILTER_CHARS: usize = 200;
const MAX_PATH_CHARS: usize = 4096;

fn content_error(error: ContentError) -> APIError {
	match error {
		ContentError::Invalid(message) => APIError::BadRequest(message),
		ContentError::Db(error) => APIError::DbError(error),
		ContentError::Extract(message) => APIError::InternalServerError(message),
	}
}

fn scope_of(
	tags_any: &Option<String>,
	tags_none: &Option<String>,
	exclude: &Option<String>,
) -> APIResult<Scope> {
	Scope::parse(tags_any.as_deref(), tags_none.as_deref(), exclude.as_deref())
		.map_err(APIError::BadRequest)
}

fn page_of(page: Option<u32>) -> APIResult<u32> {
	match page.unwrap_or(1) {
		0 => Err(APIError::BadRequest("page starts at 1".to_string())),
		page => Ok(page.min(MAX_PAGE)),
	}
}

fn page_size_of(size: Option<u32>, default: u32) -> APIResult<u32> {
	match size.unwrap_or(default) {
		0 => Err(APIError::BadRequest("page_size starts at 1".to_string())),
		size => Ok(size.min(MAX_PAGE_SIZE)),
	}
}

fn short_text(name: &str, value: &Option<String>) -> APIResult<Option<String>> {
	match value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
		Some(v) if v.chars().count() > MAX_FILTER_CHARS => Err(APIError::BadRequest(
			format!("{name} is longer than {MAX_FILTER_CHARS} characters"),
		)),
		Some(v) => Ok(Some(v.to_string())),
		None => Ok(None),
	}
}

/// The one book the user may see with this id and scope, or 404
async fn book_for(
	ctx: &AppState,
	req: &AuthContext,
	scope: &Scope,
	id: &str,
) -> APIResult<CatalogRow> {
	catalog::visible_books(ctx.conn.as_ref(), &req.user, scope, Some(id), None)
		.await
		.map_err(content_error)?
		.into_iter()
		.next()
		.ok_or_else(|| APIError::NotFound("Book not found".to_string()))
}

// ---------------------------------------------------------------- status

#[derive(Debug, Deserialize)]
struct ScopeQuery {
	tags_any: Option<String>,
	tags_none: Option<String>,
	exclude: Option<String>,
}

#[derive(Serialize)]
struct StatusResponse {
	books: u64,
	indexed: u64,
	pending: u64,
	failed: u64,
	last_run: Option<store::LastRun>,
}

async fn get_status(
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
	Query(q): Query<ScopeQuery>,
) -> APIResult<Json<StatusResponse>> {
	let scope = scope_of(&q.tags_any, &q.tags_none, &q.exclude)?;
	let conn = ctx.conn.as_ref();
	let books = catalog::visible_books(conn, &req.user, &scope, None, None)
		.await
		.map_err(content_error)?;
	let counts = query::status_counts(conn, &books)
		.await
		.map_err(content_error)?;
	Ok(Json(StatusResponse {
		books: counts.books,
		indexed: counts.indexed,
		pending: counts.pending,
		failed: counts.failed,
		last_run: store::last_run(conn).await.map_err(content_error)?,
	}))
}

// ---------------------------------------------------------------- search

#[derive(Debug, Deserialize)]
struct SearchQuery {
	q: Option<String>,
	mode: Option<String>,
	#[serde(rename = "match")]
	match_: Option<String>,
	media_id: Option<String>,
	library_id: Option<String>,
	language: Option<String>,
	author: Option<String>,
	tags_any: Option<String>,
	tags_none: Option<String>,
	exclude: Option<String>,
	page: Option<u32>,
	page_size: Option<u32>,
}

#[derive(Serialize)]
struct SearchItem {
	media_id: String,
	title: String,
	authors: Vec<String>,
	language: Option<String>,
	library: String,
	tags: Vec<String>,
	score: f64,
	section: Option<i64>,
	section_title: Option<String>,
	pos: Option<usize>,
	snippet: Option<String>,
	page_label: Option<String>,
}

#[derive(Serialize)]
struct SearchResponse {
	total: u64,
	page: u32,
	page_size: u32,
	items: Vec<SearchItem>,
}

fn item(book: &CatalogRow, score: f64) -> SearchItem {
	SearchItem {
		media_id: book.media_id.clone(),
		title: book.title.clone(),
		authors: book.authors.clone(),
		language: book.language.clone(),
		library: book.library.clone(),
		tags: book.tags.clone(),
		score,
		section: None,
		section_title: None,
		pos: None,
		snippet: None,
		page_label: None,
	}
}

fn language_matches(book: &Option<String>, wanted: &str) -> bool {
	let wanted = stump_core::content::fold_str(wanted);
	book.as_deref().is_some_and(|language| {
		let language = stump_core::content::fold_str(language);
		language == wanted
			|| language
				.strip_prefix(&wanted)
				.is_some_and(|rest| rest.starts_with('-') || rest.starts_with('_'))
	})
}

async fn get_search(
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
	Query(q): Query<SearchQuery>,
) -> APIResult<Json<SearchResponse>> {
	let scope = scope_of(&q.tags_any, &q.tags_none, &q.exclude)?;
	let page = page_of(q.page)?;
	let page_size = page_size_of(q.page_size, 10)?;
	let catalog_mode = match q.mode.as_deref().unwrap_or("text") {
		"text" => false,
		"catalog" => true,
		_ => return Err(APIError::BadRequest("mode is text or catalog".to_string())),
	};
	let match_all = match q.match_.as_deref().unwrap_or("any") {
		"any" => false,
		"all" => true,
		_ => return Err(APIError::BadRequest("match is any or all".to_string())),
	};
	let language = short_text("language", &q.language)?;
	let author = short_text("author", &q.author)?;
	let library_id = short_text("library_id", &q.library_id)?;
	let media_id = short_text("media_id", &q.media_id)?;
	let terms = query::parse_terms(q.q.as_deref().unwrap_or(""), match_all)
		.map_err(content_error)?
		.ok_or_else(|| APIError::BadRequest("q needs at least one word".to_string()))?;

	let conn = ctx.conn.as_ref();
	let mut books = catalog::visible_books(conn, &req.user, &scope, media_id.as_deref(), None)
		.await
		.map_err(content_error)?;
	if media_id.is_some() && books.is_empty() {
		return Err(APIError::NotFound("Book not found".to_string()));
	}
	books.retain(|book| {
		book.kind.is_some()
			&& library_id.as_deref().is_none_or(|id| book.library_id == id)
			&& language
				.as_deref()
				.is_none_or(|wanted| language_matches(&book.language, wanted))
			&& author.as_deref().is_none_or(|wanted| {
				let wanted = stump_core::content::fold_str(wanted);
				book.authors
					.iter()
					.any(|a| stump_core::content::fold_str(a).contains(&wanted))
			})
	});

	if catalog_mode {
		let hits = query::search_catalog(conn, &books, &terms)
			.await
			.map_err(content_error)?;
		let total = hits.len() as u64;
		let skip = (page as usize - 1) * page_size as usize;
		let items = hits
			.iter()
			.skip(skip)
			.take(page_size as usize)
			.map(|hit| item(&books[hit.index], hit.score))
			.collect();
		return Ok(Json(SearchResponse {
			total,
			page,
			page_size,
			items,
		}));
	}

	let ids: Vec<String> = books.iter().map(|b| b.media_id.clone()).collect();
	let (total, passages) =
		query::search_text(conn, &ids, &terms, media_id.is_none(), page, page_size)
			.await
			.map_err(content_error)?;
	let items = passages
		.into_iter()
		.filter_map(|passage| {
			let book = books.iter().find(|b| b.media_id == passage.media_id)?;
			let mut it = item(book, passage.score);
			it.section = Some(passage.section);
			it.section_title = Some(passage.section_title);
			it.pos = Some(passage.pos);
			it.snippet = Some(passage.snippet);
			it.page_label = passage.page_label;
			Some(it)
		})
		.collect();
	Ok(Json(SearchResponse {
		total,
		page,
		page_size,
		items,
	}))
}

// ---------------------------------------------------------------- sections

#[derive(Debug, Deserialize)]
struct SectionsQuery {
	tags_any: Option<String>,
	tags_none: Option<String>,
	exclude: Option<String>,
	page: Option<u32>,
	page_size: Option<u32>,
}

#[derive(Serialize)]
struct SectionsResponse {
	media_id: String,
	title: String,
	authors: Vec<String>,
	language: Option<String>,
	tags: Vec<String>,
	total: u64,
	sections: Vec<query::SectionInfo>,
}

async fn get_sections(
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
	Path(id): Path<String>,
	Query(q): Query<SectionsQuery>,
) -> APIResult<Json<SectionsResponse>> {
	let scope = scope_of(&q.tags_any, &q.tags_none, &q.exclude)?;
	let page = page_of(q.page)?;
	let page_size = page_size_of(q.page_size, 50)?;
	let book = book_for(&ctx, &req, &scope, &id).await?;
	let (total, sections) = query::list_sections(ctx.conn.as_ref(), &id, page, page_size)
		.await
		.map_err(content_error)?;
	Ok(Json(SectionsResponse {
		media_id: book.media_id,
		title: book.title,
		authors: book.authors,
		language: book.language,
		tags: book.tags,
		total,
		sections,
	}))
}

#[derive(Debug, Deserialize)]
struct ReadQuery {
	tags_any: Option<String>,
	tags_none: Option<String>,
	exclude: Option<String>,
	pos: Option<usize>,
	chars: Option<usize>,
}

#[derive(Serialize)]
struct ReadResponse {
	media_id: String,
	#[serde(flatten)]
	chunk: query::ReadChunk,
}

async fn get_section(
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
	Path((id, n)): Path<(String, i64)>,
	Query(q): Query<ReadQuery>,
) -> APIResult<Json<ReadResponse>> {
	let scope = scope_of(&q.tags_any, &q.tags_none, &q.exclude)?;
	let chars = match q.chars.unwrap_or(6000) {
		0 => return Err(APIError::BadRequest("chars starts at 1".to_string())),
		chars => chars.min(MAX_READ_CHARS),
	};
	let book = book_for(&ctx, &req, &scope, &id).await?;
	let chunk = query::read_section(
		ctx.conn.as_ref(),
		&book.media_id,
		n,
		q.pos.unwrap_or(0),
		chars,
	)
	.await
	.map_err(content_error)?
	.ok_or_else(|| APIError::NotFound("Section not found".to_string()))?;
	Ok(Json(ReadResponse {
		media_id: book.media_id,
		chunk,
	}))
}

// ---------------------------------------------------------------- grep

#[derive(Debug, Deserialize)]
struct GrepQuery {
	pattern: Option<String>,
	regex: Option<bool>,
	case: Option<bool>,
	section: Option<i64>,
	tags_any: Option<String>,
	tags_none: Option<String>,
	exclude: Option<String>,
	page: Option<u32>,
	page_size: Option<u32>,
}

#[derive(Serialize)]
struct GrepResponse {
	total: u64,
	page: u32,
	hits: Vec<query::GrepHit>,
}

async fn get_grep(
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
	Path(id): Path<String>,
	Query(q): Query<GrepQuery>,
) -> APIResult<Json<GrepResponse>> {
	let scope = scope_of(&q.tags_any, &q.tags_none, &q.exclude)?;
	let page = page_of(q.page)?;
	let page_size = page_size_of(q.page_size, 20)?;
	let case = q.case.unwrap_or(false);
	let re = query::compile_pattern(
		q.pattern.as_deref().unwrap_or(""),
		q.regex.unwrap_or(false),
		case,
	)
	.map_err(content_error)?;
	let book = book_for(&ctx, &req, &scope, &id).await?;
	let (total, hits) = query::grep_book(
		ctx.conn.as_ref(),
		&book.media_id,
		&re,
		case,
		q.section,
		page,
		page_size,
	)
	.await
	.map_err(content_error)?;
	Ok(Json(GrepResponse { total, page, hits }))
}

// ---------------------------------------------------------------- lookup, tags

#[derive(Debug, Deserialize)]
struct LookupQuery {
	path: Option<String>,
}

#[derive(Serialize)]
struct LookupResponse {
	media_id: String,
}

async fn get_lookup(
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
	Query(q): Query<LookupQuery>,
) -> APIResult<Json<LookupResponse>> {
	let path = q.path.as_deref().unwrap_or("");
	if path.is_empty() || path.len() > MAX_PATH_CHARS {
		return Err(APIError::BadRequest(
			"path is an absolute path of a book".to_string(),
		));
	}
	let book = catalog::visible_books(
		ctx.conn.as_ref(),
		&req.user,
		&Scope::default(),
		None,
		Some(path),
	)
	.await
	.map_err(content_error)?
	.into_iter()
	.next()
	.ok_or_else(|| APIError::NotFound("Book not found".to_string()))?;
	Ok(Json(LookupResponse {
		media_id: book.media_id,
	}))
}

#[derive(Serialize)]
struct TagCount {
	name: String,
	count: u64,
}

#[derive(Serialize)]
struct TagsResponse {
	tags: Vec<TagCount>,
}

async fn get_tags(
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
	Query(q): Query<ScopeQuery>,
) -> APIResult<Json<TagsResponse>> {
	let scope = scope_of(&q.tags_any, &q.tags_none, &q.exclude)?;
	let mut books = catalog::visible_books(ctx.conn.as_ref(), &req.user, &scope, None, None)
		.await
		.map_err(content_error)?;
	books.retain(|book| book.kind.is_some());
	let tags = catalog::tag_counts(&books)
		.into_iter()
		.map(|(name, count)| TagCount { name, count })
		.collect();
	Ok(Json(TagsResponse { tags }))
}

// ---------------------------------------------------------------- reindex

#[derive(Serialize)]
struct ReindexResponse {
	job_id: String,
}

async fn post_reindex(
	State(ctx): State<AppState>,
	Extension(req): Extension<AuthContext>,
) -> APIResult<Json<ReindexResponse>> {
	let user = &req.user;
	if user.is_locked || !user.has_permission(UserPermission::ManageLibrary) {
		return Err(APIError::Forbidden(
			"Only the server owner or a library manager can reindex".to_string(),
		));
	}
	let job_id = Uuid::new_v4().to_string();
	ctx.enqueue(StumpJob::content_index(ContentIndexConfig {
		retry_failed: true,
		job_id: Some(job_id.clone()),
	}))
	.await
	.map_err(|error| APIError::InternalServerError(error.to_string()))?;
	Ok(Json(ReindexResponse { job_id }))
}
