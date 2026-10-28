//! Browsing additions for the OPDS 1.2 catalog:
//!
//! - A single book feed generator used by every book listing (all books, a library, a
//!   series, a smart list, an author, a genre). Each generated feed carries:
//!   - an OpenSearch-style `rel="search"` template link scoped to the feed, so clients
//!     like KOReader search *inside* the shelf/library/series being viewed
//!   - OPDS facets for sort order and language
//! - Smart lists ("Shelves") as navigation + book feeds
//! - Authors and genres indexes (letter → value → books)

use std::collections::BTreeMap;

use chrono::Utc;
use graphql::{
	data::AuthContext,
	filter::list_has_element,
	input::smart_lists::SmartListFilterGroupInput,
	pagination::OffsetPagination,
	query::{
		metadata_facets::{metadata_value_counts, sort_key, MetadataFacetField},
		smart_lists_builder::build_books_query,
	},
};
use models::{
	entity::{library, media, media_metadata, series, smart_list, user::AuthUser},
	shared::enums::UserPermission,
};
use sea_orm::{
	prelude::*,
	sea_query::{Expr as QExpr, SelectStatement},
	Condition, ExprTrait, Order, QueryOrder, QuerySelect, QueryTrait, Select,
};
use serde::Deserialize;
use stump_core::opds::{
	v1_2::{
		entry::{IntoOPDSEntry, OPDSEntryBuilder, OpdsEntry},
		feed::{
			OPDSFeedBuilder, OPDSFeedBuilderPageParams, OPDSFeedBuilderParams, OpdsFeed,
		},
		link::{OpdsGenericLink, OpdsLink, OpdsLinkRel, OpdsLinkType},
	},
	v2_0::entity::OPDSPublicationEntity,
};

use crate::{
	config::state::AppState,
	errors::{APIError, APIResult},
	utils::http::Xml,
};

/// Book feed query parameters, on top of pagination
#[derive(Debug, Default, Clone, Deserialize)]
pub(crate) struct BookFeedParams {
	#[serde(default)]
	pub search: Option<String>,
	/// One of `title`, `added`, `series`, `author`
	#[serde(default)]
	pub sort: Option<String>,
	/// A two-letter language code, matched as a prefix (`en` matches `en-US`, `eng`)
	#[serde(default)]
	pub lang: Option<String>,
	/// An exact genre (whole list element)
	#[serde(default)]
	pub genre: Option<String>,
	/// An exact writer (whole list element)
	#[serde(default)]
	pub author: Option<String>,
}

impl BookFeedParams {
	fn clean(value: &Option<String>) -> Option<String> {
		value
			.as_ref()
			.map(|v| v.trim().to_string())
			.filter(|v| !v.is_empty())
	}

	fn search(&self) -> Option<String> {
		Self::clean(&self.search)
	}

	fn genre(&self) -> Option<String> {
		Self::clean(&self.genre)
	}

	fn author(&self) -> Option<String> {
		Self::clean(&self.author)
	}

	fn lang(&self) -> Option<String> {
		Self::clean(&self.lang)
			.map(|l| l.to_lowercase())
			.filter(|l| l.len() == 2 && l.chars().all(|c| c.is_ascii_alphabetic()))
	}

	/// The sort used when none is requested: relevance while searching, otherwise the
	/// scope's natural order
	fn default_sort(&self, scope: &BookScope) -> BookSort {
		if self.search().is_some() {
			BookSort::Relevance
		} else {
			scope.default_sort()
		}
	}

	fn sort(&self, scope: &BookScope) -> BookSort {
		match self.sort.as_deref() {
			Some("added") => BookSort::Added,
			Some("series") => BookSort::Series,
			Some("author") => BookSort::Author,
			Some("title") => BookSort::Title,
			Some("relevance") if self.search().is_some() => BookSort::Relevance,
			_ => self.default_sort(scope),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BookSort {
	/// Title, author and series matches first, then summary-only matches
	Relevance,
	Title,
	Added,
	Series,
	Author,
}

impl BookSort {
	const ALL: [BookSort; 5] = [
		BookSort::Relevance,
		BookSort::Title,
		BookSort::Author,
		BookSort::Series,
		BookSort::Added,
	];

	fn key(&self) -> &'static str {
		match self {
			BookSort::Relevance => "relevance",
			BookSort::Title => "title",
			BookSort::Added => "added",
			BookSort::Series => "series",
			BookSort::Author => "author",
		}
	}

	fn label(&self) -> &'static str {
		match self {
			BookSort::Relevance => "Best match",
			BookSort::Title => "Title",
			BookSort::Added => "Recently added",
			BookSort::Series => "Series order",
			BookSort::Author => "Author",
		}
	}
}

/// What a book feed is scoped to
pub(crate) enum BookScope {
	All,
	Library {
		id: String,
		name: String,
	},
	Series {
		id: String,
		title: String,
	},
	SmartList {
		id: String,
		name: String,
		media_ids: SelectStatement,
	},
}

impl BookScope {
	fn default_sort(&self) -> BookSort {
		match self {
			BookScope::Series { .. } => BookSort::Series,
			_ => BookSort::Title,
		}
	}

	/// The path (relative to the catalog root) of the feed, without query params
	fn path(&self) -> String {
		match self {
			BookScope::All => "books".to_string(),
			BookScope::Library { id, .. } => format!("libraries/{id}/books"),
			BookScope::Series { id, .. } => format!("series/{id}"),
			BookScope::SmartList { id, .. } => format!("smart-lists/{id}"),
		}
	}

	fn feed_id(&self) -> String {
		match self {
			BookScope::All => "allBooks".to_string(),
			BookScope::Library { id, .. } => format!("library:{id}:books"),
			BookScope::Series { id, .. } => id.clone(),
			BookScope::SmartList { id, .. } => format!("smartList:{id}"),
		}
	}

	fn title(&self) -> String {
		match self {
			BookScope::All => "All books".to_string(),
			BookScope::Library { name, .. } => format!("{name}: all books"),
			BookScope::Series { title, .. } => title.clone(),
			BookScope::SmartList { name, .. } => name.clone(),
		}
	}

	fn apply(&self, query: Select<media::Entity>) -> Select<media::Entity> {
		match self {
			BookScope::All => query,
			BookScope::Library { id, .. } => {
				query.filter(media::Column::LibraryId.eq(id.clone()))
			},
			BookScope::Series { id, .. } => {
				query.filter(media::Column::SeriesId.eq(id.clone()))
			},
			BookScope::SmartList { media_ids, .. } => {
				query.filter(media::Column::Id.in_subquery(media_ids.clone()))
			},
		}
	}
}

/// The condition used for free-text search within a feed: file name, title, series,
/// summary and writers
pub(crate) fn book_search_condition(search: &str) -> Condition {
	Condition::any()
		.add(media::Column::Name.contains(search))
		.add(media_metadata::Column::Title.contains(search))
		.add(media_metadata::Column::Series.contains(search))
		.add(media_metadata::Column::Summary.contains(search))
		.add(media_metadata::Column::Writers.contains(search))
}

/// Applies the scope and every param filter. The language filter can be skipped, which
/// is used to compute the language facet for the rest of the filters.
fn apply_book_filters(
	query: Select<media::Entity>,
	scope: &BookScope,
	params: &BookFeedParams,
	with_lang: bool,
) -> Select<media::Entity> {
	let query = scope.apply(query);
	let query = match params.search() {
		Some(search) => query.filter(book_search_condition(&search)),
		None => query,
	};
	let query = match params.genre() {
		Some(genre) => {
			query.filter(list_has_element(media_metadata::Column::Genres, &genre))
		},
		None => query,
	};
	let query = match params.author() {
		Some(author) => {
			query.filter(list_has_element(media_metadata::Column::Writers, &author))
		},
		None => query,
	};
	match params.lang().filter(|_| with_lang) {
		Some(lang) => query.filter(lang_condition(&lang)),
		None => query,
	}
}

const LANG_EXPR: &str = "LOWER(SUBSTR(TRIM(\"media_metadata\".\"language\"), 1, 2))";

fn lang_condition(lang: &str) -> Condition {
	Condition::all().add(QExpr::cust(LANG_EXPR).eq(lang.to_string()))
}

fn apply_sort(
	query: Select<media::Entity>,
	sort: BookSort,
	search: Option<&str>,
) -> Select<media::Entity> {
	let title = QExpr::cust(
		"TRIM(COALESCE(NULLIF(TRIM(\"media_metadata\".\"title_sort\"), ''), NULLIF(TRIM(\"media_metadata\".\"title\"), ''), \"media\".\"name\")) COLLATE NOCASE",
	);
	match sort {
		BookSort::Relevance => {
			let pattern = format!(
				"%{}%",
				graphql::filter::keyword::escape_like_fragment(search.unwrap_or_default())
			);
			let rank = QExpr::cust_with_values(
				"CASE WHEN \"media_metadata\".\"title\" LIKE ? ESCAPE '\\' OR \"media\".\"name\" LIKE ? ESCAPE '\\' THEN 0 WHEN \"media_metadata\".\"writers\" LIKE ? ESCAPE '\\' THEN 1 WHEN \"media_metadata\".\"series\" LIKE ? ESCAPE '\\' THEN 2 ELSE 3 END",
				[pattern.clone(), pattern.clone(), pattern.clone(), pattern],
			);
			query
				.order_by(rank, Order::Asc)
				.order_by(title, Order::Asc)
				.order_by_asc(media::Column::Id)
		},
		BookSort::Title => query
			.order_by(title, Order::Asc)
			.order_by_asc(media::Column::Id),
		BookSort::Added => query
			.order_by_desc(media::Column::CreatedAt)
			.order_by_asc(media::Column::Id),
		BookSort::Series => media::Entity::order_in_series(
			query.order_by(
				QExpr::cust(
					"COALESCE(NULLIF(\"media_metadata\".\"series\", ''), '~') COLLATE NOCASE",
				),
				Order::Asc,
			),
		),
		BookSort::Author => query
			.order_by(
				QExpr::cust(
					"COALESCE(NULLIF(\"media_metadata\".\"writers\", ''), '~') COLLATE NOCASE",
				),
				Order::Asc,
			)
			.order_by(title, Order::Asc)
			.order_by_asc(media::Column::Id),
	}
}

/// Builds `path?k=v&...` for the given params, skipping empty values. Values are URL
/// encoded. `search` is never included (it's added by the search template).
fn path_with_params(path: &str, params: &[(&str, Option<String>)]) -> String {
	let query = params
		.iter()
		.filter_map(|(k, v)| {
			v.as_ref()
				.map(|v| format!("{k}={}", urlencoding::encode(v)))
		})
		.collect::<Vec<_>>()
		.join("&");
	if query.is_empty() {
		path.to_string()
	} else {
		format!("{path}?{query}")
	}
}

fn catalog_url(req: &AuthContext, path: &str) -> String {
	match req.api_key() {
		Some(api_key) => format!("/opds/{api_key}/v1.2/{path}"),
		None => format!("/opds/v1.2/{path}"),
	}
}

/// A `rel="search"` template link for `path` (which may already carry query params)
pub(crate) fn scoped_search_link(req: &AuthContext, path: &str) -> OpdsGenericLink {
	let separator = if path.contains('?') { '&' } else { '?' };
	OpdsGenericLink::search_template(
		catalog_url(req, &format!("{path}{separator}search={{searchTerms}}")),
		OpdsLinkType::Acquisition,
	)
}

fn language_name(code: &str) -> String {
	let name = match code {
		"es" => "Español",
		"en" => "English",
		"fr" => "Français",
		"de" => "Deutsch",
		"it" => "Italiano",
		"pt" => "Português",
		"ca" => "Català",
		"gl" => "Galego",
		"eu" => "Euskara",
		"la" => "Latina",
		"ja" => "日本語",
		"zh" => "中文",
		"ru" => "Русский",
		"nl" => "Nederlands",
		// ISO 639-2 `und`
		"un" => "Undetermined",
		_ => return code.to_uppercase(),
	};
	name.to_string()
}

/// Generates any book feed. Pagination is page-based (1-based unless `zero_based`).
pub(crate) async fn books_feed(
	ctx: &AppState,
	req: &AuthContext,
	scope: BookScope,
	pagination: OffsetPagination,
	params: BookFeedParams,
) -> APIResult<Xml> {
	let user = req.user();
	let conn = ctx.conn.as_ref();
	let sort = params.sort(&scope);

	let query = apply_book_filters(
		OPDSPublicationEntity::find_for_user(&user),
		&scope,
		&params,
		true,
	);
	let count = query.clone().count(conn).await?;
	let books = apply_sort(query, sort, params.search().as_deref())
		.offset(pagination.offset())
		.limit(pagination.limit())
		.into_model::<OPDSPublicationEntity>()
		.all(conn)
		.await?;

	let entries = futures_util::future::join_all(books.into_iter().map(|m| {
		OPDSEntryBuilder::<OPDSPublicationEntity>::new(m, req.api_key()).into_opds_entry()
	}))
	.await;

	// The params which define *what* is listed (not the page or the search), used to
	// build the facet and search links
	let default_sort = params.default_sort(&scope);
	let sort_param = (sort != default_sort).then(|| sort.key().to_string());
	let base = |sort: Option<String>, lang: Option<String>, search: Option<String>| {
		path_with_params(
			&scope.path(),
			&[
				("genre", params.genre()),
				("author", params.author()),
				("lang", lang),
				("sort", sort),
				("search", search),
			],
		)
	};
	// The feed's own path without the search (the paginator and the search template add
	// it), and the facet base which keeps the current search
	let feed_path = base(sort_param.clone(), params.lang(), None);
	let facet = |sort: Option<String>, lang: Option<String>| {
		catalog_url(req, &base(sort, lang, params.search()))
	};

	let mut title = scope.title();
	if let Some(genre) = params.genre() {
		title = format!("{title} · {genre}");
	}
	if let Some(author) = params.author() {
		title = format!("{title} · {author}");
	}
	if let Some(search) = params.search() {
		title = format!("{title} · “{search}”");
	}

	let mut extra_links = vec![scoped_search_link(req, &feed_path)];

	// Sort facets (best match only makes sense while searching)
	for option in BookSort::ALL {
		if option == BookSort::Relevance && params.search().is_none() {
			continue;
		}
		let key = (option != default_sort).then(|| option.key().to_string());
		extra_links.push(OpdsGenericLink::facet(
			"Sort",
			option.label().to_string(),
			facet(key, params.lang()),
			option == sort,
		));
	}

	// Language facets, computed for the current filters minus the language itself
	let languages: Vec<(Option<String>, i64)> =
		apply_book_filters(media::Entity::find_for_user(&user), &scope, &params, false)
			.select_only()
			.column_as(QExpr::cust(LANG_EXPR), "lang")
			.column_as(media::Column::Id.count(), "count")
			.group_by(QExpr::cust(LANG_EXPR))
			.order_by(QExpr::cust("COUNT(\"media\".\"id\")"), Order::Desc)
			.into_tuple()
			.all(conn)
			.await?;
	// Books without a (valid) language still count towards "All languages"
	let total: i64 = languages.iter().map(|(_, c)| c).sum();
	let languages = languages
		.into_iter()
		.filter_map(|(lang, count)| {
			lang.filter(|l| l.len() == 2 && l.chars().all(|c| c.is_ascii_alphabetic()))
				.map(|l| (l, count))
		})
		.collect::<Vec<_>>();
	if languages.len() > 1 || params.lang().is_some() {
		extra_links.push(
			OpdsGenericLink::facet(
				"Language",
				"All languages".to_string(),
				facet(sort_param.clone(), None),
				params.lang().is_none(),
			)
			.with_count(total as u64),
		);
		for (lang, count) in languages.into_iter().take(8) {
			let active = params.lang().as_deref() == Some(lang.as_str());
			extra_links.push(
				OpdsGenericLink::facet(
					"Language",
					language_name(&lang),
					facet(sort_param.clone(), Some(lang)),
					active,
				)
				.with_count(count as u64),
			);
		}
	}

	let feed = OPDSFeedBuilder::new(req.api_key()).paginated(OPDSFeedBuilderParams {
		id: scope.feed_id(),
		title,
		entries,
		href_postfix: feed_path,
		page_params: Some(OPDSFeedBuilderPageParams {
			page: pagination.page,
			page_size: pagination.limit(),
			zero_based: pagination.zero_based.unwrap_or(false),
			count,
		}),
		search: params.search(),
	})?;

	Ok(Xml(feed.with_extra_links(extra_links).build()?))
}

fn navigation_entry(
	req: &AuthContext,
	id: String,
	title: String,
	content: Option<String>,
	path: String,
	acquisition: bool,
) -> OpdsEntry {
	OpdsEntry::new(
		id,
		Utc::now().into(),
		title,
		None,
		content,
		None,
		Some(vec![OpdsLink {
			link_type: if acquisition {
				OpdsLinkType::Acquisition
			} else {
				OpdsLinkType::Navigation
			},
			rel: OpdsLinkRel::Subsection,
			href: catalog_url(req, &path),
		}]),
		None,
	)
}

fn simple_feed(
	req: &AuthContext,
	id: &str,
	title: String,
	self_path: &str,
	entries: Vec<OpdsEntry>,
) -> OpdsFeed {
	OpdsFeed::new(
		id.to_string(),
		title,
		Some(vec![
			OpdsLink {
				link_type: OpdsLinkType::Navigation,
				rel: OpdsLinkRel::ItSelf,
				href: catalog_url(req, self_path),
			},
			OpdsLink {
				link_type: OpdsLinkType::Navigation,
				rel: OpdsLinkRel::Start,
				href: catalog_url(req, "catalog"),
			},
		]),
		entries,
	)
}

fn can_access_smart_lists(user: &AuthUser) -> bool {
	user.has_permission(UserPermission::AccessSmartList)
}

/// Resolves a smart list into the subquery of the media IDs it matches
fn smart_list_media_ids(
	user: &AuthUser,
	list: &smart_list::Model,
) -> APIResult<SelectStatement> {
	let filters: Vec<SmartListFilterGroupInput> = serde_json::from_slice(&list.filters)
		.map_err(|e| {
		APIError::InternalServerError(format!("Invalid smart list filters: {e}"))
	})?;
	Ok(build_books_query(user, list.joiner, &filters, None)
		.select_only()
		.column(media::Column::Id)
		.into_query())
}

/// GET /opds/v1.2/smart-lists: every smart list the user can read, with book counts
pub(crate) async fn smart_lists_feed(
	ctx: &AppState,
	req: &AuthContext,
) -> APIResult<Xml> {
	let user = req.user();
	let conn = ctx.conn.as_ref();
	if !can_access_smart_lists(&user) {
		return Err(APIError::Forbidden(
			"You do not have permission to access smart lists".to_string(),
		));
	}

	let lists = smart_list::Entity::find_for_user(&user, false, false, None)
		.order_by_asc(smart_list::Column::Name)
		.all(conn)
		.await?;

	// Count every list's books concurrently, the shelf list is opened from slow e-readers
	let counts = futures_util::future::join_all(lists.iter().map(|list| {
		let user = &user;
		async move {
			let ids = smart_list_media_ids(user, list).ok()?;
			media::Entity::find()
				.filter(media::Column::Id.in_subquery(ids))
				.count(conn)
				.await
				.ok()
		}
	}))
	.await;

	let entries = lists
		.into_iter()
		.zip(counts)
		.map(|(list, count)| {
			let content = match (count, list.description.clone()) {
				(Some(count), Some(desc)) if !desc.is_empty() => {
					Some(format!("{count} books — {desc}"))
				},
				(Some(count), _) => Some(format!("{count} books")),
				(None, desc) => desc,
			};
			navigation_entry(
				req,
				format!("smartList:{}", list.id),
				list.name.clone(),
				content,
				format!("smart-lists/{}", list.id),
				true,
			)
		})
		.collect();

	let feed = simple_feed(
		req,
		"smartLists",
		"Shelves".to_string(),
		"smart-lists",
		entries,
	);
	Ok(Xml(feed.build()?))
}

/// GET /opds/v1.2/smart-lists/{id}: the books of a smart list
pub(crate) async fn smart_list_books_feed(
	ctx: &AppState,
	req: &AuthContext,
	id: String,
	pagination: OffsetPagination,
	params: BookFeedParams,
) -> APIResult<Xml> {
	let user = req.user();
	if !can_access_smart_lists(&user) {
		return Err(APIError::Forbidden(
			"You do not have permission to access smart lists".to_string(),
		));
	}
	let list = smart_list::Entity::find_by_id(&user, id.clone().into())
		.one(ctx.conn.as_ref())
		.await?
		.ok_or(APIError::NotFound("Smart list not found".to_string()))?;
	let media_ids = smart_list_media_ids(&user, &list)?;
	books_feed(
		ctx,
		req,
		BookScope::SmartList {
			id,
			name: list.name,
			media_ids,
		},
		pagination,
		params,
	)
	.await
}

/// GET /opds/v1.2/libraries/{id}/books
pub(crate) async fn library_books_feed(
	ctx: &AppState,
	req: &AuthContext,
	id: String,
	pagination: OffsetPagination,
	params: BookFeedParams,
) -> APIResult<Xml> {
	let library = library::Entity::find_for_user(&req.user())
		.filter(library::Column::Id.eq(&id))
		.one(ctx.conn.as_ref())
		.await?
		.ok_or(APIError::NotFound("Library not found".into()))?;
	books_feed(
		ctx,
		req,
		BookScope::Library {
			id,
			name: library.name,
		},
		pagination,
		params,
	)
	.await
}

/// GET /opds/v1.2/series/{id}
pub(crate) async fn series_books_feed(
	ctx: &AppState,
	req: &AuthContext,
	id: String,
	pagination: OffsetPagination,
	params: BookFeedParams,
) -> APIResult<Xml> {
	let series::ModelWithMetadata { series, metadata } =
		series::ModelWithMetadata::find_for_user(&req.user())
			.filter(series::Column::Id.eq(id.clone()))
			.into_model::<series::ModelWithMetadata>()
			.one(ctx.conn.as_ref())
			.await?
			.ok_or(APIError::NotFound(format!("Series {id} not found")))?;
	let title = metadata
		.and_then(|m| m.title.clone())
		.unwrap_or_else(|| series.name.clone());
	books_feed(
		ctx,
		req,
		BookScope::Series {
			id: series.id,
			title,
		},
		pagination,
		params,
	)
	.await
}

/// Which metadata index is being browsed
#[derive(Clone, Copy)]
pub(crate) enum ValueIndex {
	Authors,
	Genres,
}

impl ValueIndex {
	fn field(&self) -> MetadataFacetField {
		match self {
			ValueIndex::Authors => MetadataFacetField::Writers,
			ValueIndex::Genres => MetadataFacetField::Genres,
		}
	}

	fn path(&self) -> &'static str {
		match self {
			ValueIndex::Authors => "authors",
			ValueIndex::Genres => "genres",
		}
	}

	fn title(&self) -> &'static str {
		match self {
			ValueIndex::Authors => "Authors",
			ValueIndex::Genres => "Genres",
		}
	}

	fn book_param(&self) -> &'static str {
		match self {
			ValueIndex::Authors => "author",
			ValueIndex::Genres => "genre",
		}
	}
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ValueIndexParams {
	#[serde(default)]
	pub search: Option<String>,
	#[serde(default)]
	pub letter: Option<String>,
}

fn initial_of(value: &str) -> String {
	match sort_key(value).chars().next() {
		Some(c) if c.is_alphabetic() => c.to_uppercase().collect(),
		_ => "#".to_string(),
	}
}

/// GET /opds/v1.2/authors and /opds/v1.2/genres
///
/// - without params: a letter index (A, B, ..., #) with counts
/// - `?letter=X`: the values starting with that letter
/// - `?search=...`: the values containing the search
///
/// Every value links to the matching book feed (`books?author=...`/`books?genre=...`).
pub(crate) async fn value_index_feed(
	ctx: &AppState,
	req: &AuthContext,
	index: ValueIndex,
	pagination: OffsetPagination,
	params: ValueIndexParams,
) -> APIResult<Xml> {
	let user = req.user();
	let search = BookFeedParams::clean(&params.search);
	let letter = BookFeedParams::clean(&params.letter);

	let values = metadata_value_counts(
		ctx.conn.as_ref(),
		&user,
		index.field(),
		None,
		None,
		search.clone(),
	)
	.await?;

	let search_link = OpdsGenericLink::search_template(
		catalog_url(req, &format!("{}?search={{searchTerms}}", index.path())),
		OpdsLinkType::Navigation,
	);

	// Letter index
	if search.is_none() && letter.is_none() {
		let mut letters: BTreeMap<String, (u64, u64)> = BTreeMap::new();
		for value in &values {
			let entry = letters.entry(initial_of(&value.value)).or_default();
			entry.0 += 1;
			entry.1 += value.count;
		}
		let entries = letters
			.into_iter()
			.map(|(letter, (values, books))| {
				navigation_entry(
					req,
					format!("{}:{letter}", index.path()),
					letter.clone(),
					Some(format!(
						"{values} {} · {books} books",
						index.title().to_lowercase()
					)),
					path_with_params(index.path(), &[("letter", Some(letter))]),
					false,
				)
			})
			.collect();
		let feed = simple_feed(
			req,
			index.path(),
			index.title().to_string(),
			index.path(),
			entries,
		)
		.with_extra_links(vec![search_link]);
		return Ok(Xml(feed.build()?));
	}

	let values = values
		.into_iter()
		.filter(|v| {
			letter
				.as_ref()
				.is_none_or(|letter| initial_of(&v.value) == letter.to_uppercase())
		})
		.collect::<Vec<_>>();
	let count = values.len() as u64;
	let entries = values
		.into_iter()
		.skip(pagination.offset() as usize)
		.take(pagination.limit() as usize)
		.map(|value| {
			navigation_entry(
				req,
				format!("{}:{}", index.path(), value.value),
				value.value.clone(),
				Some(format!("{} books", value.count)),
				path_with_params("books", &[(index.book_param(), Some(value.value))]),
				true,
			)
		})
		.collect();

	let title = match (&letter, &search) {
		(_, Some(search)) => format!("{} · “{search}”", index.title()),
		(Some(letter), None) => format!("{} · {}", index.title(), letter.to_uppercase()),
		_ => index.title().to_string(),
	};
	let feed = OPDSFeedBuilder::new(req.api_key()).paginated(OPDSFeedBuilderParams {
		id: index.path().to_string(),
		title,
		entries,
		href_postfix: path_with_params(index.path(), &[("letter", letter)]),
		page_params: Some(OPDSFeedBuilderPageParams {
			page: pagination.page,
			page_size: pagination.limit(),
			zero_based: pagination.zero_based.unwrap_or(false),
			count,
		}),
		search,
	})?;
	Ok(Xml(feed.with_extra_links(vec![search_link]).build()?))
}
