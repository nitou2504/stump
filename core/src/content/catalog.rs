//! The books a user may see through the content API, with the scope filters every endpoint takes.
//!
//! One function, [`visible_books`], decides: the books [`media::Entity::find_for_user`] returns (the
//! library access, the age restriction and the removed books, exactly as for every other book
//! query), then the scope (`tags_any`, `tags_none`, `exclude`). Every endpoint (search, sections,
//! read, grep, tags, status) asks it, so none can see what another hides.

use std::collections::{HashMap, HashSet};

use models::entity::{library, media, media_metadata, user::AuthUser};
use sea_orm::{
	prelude::*, ConnectionTrait, DbBackend, FromQueryResult, QuerySelect, Statement,
};

use super::{fold_str, kind_of, ContentResult};

/// Most media ids `exclude` may hold
pub const MAX_EXCLUDE: usize = 500;
/// Most tags a tag list may hold
pub const MAX_TAG_FILTERS: usize = 100;

/// The filters iaboy puts on every call
#[derive(Debug, Clone, Default)]
pub struct Scope {
	/// Only books with at least one of these tags (empty: no filter)
	pub tags_any: Vec<String>,
	/// Never books with any of these tags
	pub tags_none: Vec<String>,
	/// Never these books
	pub exclude: HashSet<String>,
}

fn split_list(value: &str) -> Vec<String> {
	value
		.split(',')
		.map(str::trim)
		.filter(|item| !item.is_empty())
		.map(str::to_string)
		.collect()
}

impl Scope {
	/// Reads the comma lists of a request. Errors with the reason when a list is too long
	pub fn parse(
		tags_any: Option<&str>,
		tags_none: Option<&str>,
		exclude: Option<&str>,
	) -> Result<Self, String> {
		let tags = |value: Option<&str>| -> Result<Vec<String>, String> {
			let list = split_list(value.unwrap_or(""));
			if list.len() > MAX_TAG_FILTERS {
				return Err(format!("At most {MAX_TAG_FILTERS} tags per filter"));
			}
			if list.iter().any(|tag| tag.chars().count() > 200) {
				return Err("A tag is too long".to_string());
			}
			Ok(list.iter().map(|tag| fold_str(tag)).collect())
		};
		let exclude: Vec<String> = split_list(exclude.unwrap_or(""));
		if exclude.len() > MAX_EXCLUDE {
			return Err(format!("At most {MAX_EXCLUDE} ids in exclude"));
		}
		if exclude.iter().any(|id| id.len() > 100) {
			return Err("An id in exclude is too long".to_string());
		}
		Ok(Self {
			tags_any: tags(tags_any)?,
			tags_none: tags(tags_none)?,
			exclude: exclude.into_iter().collect(),
		})
	}

	/// Whether a book with these tags passes (tags are compared folded)
	pub fn allows(&self, media_id: &str, tags: &[String]) -> bool {
		if self.exclude.contains(media_id) {
			return false;
		}
		if self.tags_any.is_empty() && self.tags_none.is_empty() {
			return true;
		}
		let folded: Vec<String> = tags.iter().map(|tag| fold_str(tag)).collect();
		if !self.tags_any.is_empty()
			&& !self.tags_any.iter().any(|wanted| folded.contains(wanted))
		{
			return false;
		}
		!self.tags_none.iter().any(|blocked| folded.contains(blocked))
	}
}

/// A book the user may see
#[derive(Debug, Clone)]
pub struct CatalogRow {
	pub media_id: String,
	pub title: String,
	pub series: Option<String>,
	pub authors: Vec<String>,
	pub language: Option<String>,
	pub library_id: String,
	pub library: String,
	pub tags: Vec<String>,
	pub path: String,
	/// `epub` or `pdf` for the books with text to index
	pub kind: Option<&'static str>,
}

#[derive(FromQueryResult)]
struct Row {
	id: String,
	name: String,
	path: String,
	extension: String,
	library_id: Option<String>,
	title: Option<String>,
	series: Option<String>,
	writers: Option<String>,
	genres: Option<String>,
	language: Option<String>,
}

/// The books `user` may see that pass `scope`; `only` limits it to one book, `path` to the book
/// at that path (the scope is applied to a lookup by path too)
pub async fn visible_books(
	conn: &DatabaseConnection,
	user: &AuthUser,
	scope: &Scope,
	only: Option<&str>,
	path: Option<&str>,
) -> ContentResult<Vec<CatalogRow>> {
	let mut select = media::Entity::find_for_user(user);
	if let Some(id) = only {
		select = select.filter(media::Column::Id.eq(id));
	}
	if let Some(path) = path {
		select = select.filter(media::Column::Path.eq(path));
	}
	if !scope.exclude.is_empty() {
		select = select.filter(
			media::Column::Id.is_not_in(scope.exclude.iter().cloned().collect::<Vec<_>>()),
		);
	}
	let rows = select
		.select_only()
		.column(media::Column::Id)
		.column(media::Column::Name)
		.column(media::Column::Path)
		.column(media::Column::Extension)
		.column(media::Column::LibraryId)
		.column(media_metadata::Column::Title)
		.column(media_metadata::Column::Series)
		.column(media_metadata::Column::Writers)
		.column(media_metadata::Column::Genres)
		.column(media_metadata::Column::Language)
		.into_model::<Row>()
		.all(conn)
		.await?;
	if rows.is_empty() {
		return Ok(Vec::new());
	}

	let libraries: HashMap<String, String> = library::Entity::find()
		.select_only()
		.column(library::Column::Id)
		.column(library::Column::Name)
		.into_tuple::<(String, String)>()
		.all(conn)
		.await?
		.into_iter()
		.collect();

	// tags set by hand (`media_tags`) count with the genres
	let mut extra: HashMap<String, Vec<String>> = HashMap::new();
	let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
	let tag_rows = conn
		.query_all_raw(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			"SELECT mt.media_id AS media_id, t.name AS name FROM media_tags mt
			 JOIN tags t ON t.id = mt.tag_id
			 WHERE mt.media_id IN (SELECT value FROM json_each(?))",
			[serde_json::to_string(&ids).unwrap_or_default().into()],
		))
		.await?;
	for row in tag_rows {
		let media_id: String = row.try_get("", "media_id")?;
		let name: String = row.try_get("", "name")?;
		extra.entry(media_id).or_default().push(name);
	}

	let mut books = Vec::with_capacity(rows.len());
	for row in rows {
		let mut tags = split_list(row.genres.as_deref().unwrap_or(""));
		for tag in extra.remove(&row.id).unwrap_or_default() {
			if !tags.iter().any(|t| fold_str(t) == fold_str(&tag)) {
				tags.push(tag);
			}
		}
		if !scope.allows(&row.id, &tags) {
			continue;
		}
		let library_id = row.library_id.unwrap_or_default();
		books.push(CatalogRow {
			title: row
				.title
				.filter(|t| !t.trim().is_empty())
				.unwrap_or(row.name),
			series: row.series.filter(|s| !s.trim().is_empty()),
			authors: split_list(row.writers.as_deref().unwrap_or("")),
			language: row.language.filter(|l| !l.trim().is_empty()),
			library: libraries.get(&library_id).cloned().unwrap_or_default(),
			library_id,
			kind: kind_of(&row.extension, &row.path),
			path: row.path,
			media_id: row.id,
			tags,
		});
	}
	Ok(books)
}

/// The tags of `books` with how many books carry each (the first spelling met names a tag; tags
/// compare folded), most used first
pub fn tag_counts(books: &[CatalogRow]) -> Vec<(String, u64)> {
	let mut counts: HashMap<String, (String, u64)> = HashMap::new();
	for book in books {
		let mut seen = HashSet::new();
		for tag in &book.tags {
			let key = fold_str(tag);
			if !seen.insert(key.clone()) {
				continue;
			}
			counts.entry(key).or_insert_with(|| (tag.clone(), 0)).1 += 1;
		}
	}
	let mut list: Vec<(String, u64)> = counts.into_values().collect();
	list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
	list
}

#[cfg(test)]
mod tests {
	use super::*;

	fn tags(list: &[&str]) -> Vec<String> {
		list.iter().map(|t| String::from(*t)).collect()
	}

	#[test]
	fn scope_compares_tags_without_case_or_accents() {
		let scope = Scope::parse(Some("Estante: Poesía, Otro"), None, None).unwrap();
		assert!(scope.allows("a", &tags(&["estante: poesia"])));
		assert!(!scope.allows("a", &tags(&["Estante: Novela"])));
		assert!(!scope.allows("a", &[]));
	}

	#[test]
	fn blocked_tags_and_excluded_ids_always_win() {
		let scope = Scope::parse(Some("a"), Some("B, Ñandú"), Some("x1, x2")).unwrap();
		assert!(scope.allows("m", &tags(&["a"])));
		assert!(!scope.allows("m", &tags(&["a", "b"])));
		assert!(!scope.allows("m", &tags(&["A", "nandu"])));
		assert!(!scope.allows("x2", &tags(&["a"])));
		let only_blocked = Scope::parse(None, Some("b"), None).unwrap();
		assert!(only_blocked.allows("m", &tags(&["a"])));
		assert!(only_blocked.allows("m", &[]));
		assert!(!only_blocked.allows("m", &tags(&["B"])));
	}

	#[test]
	fn the_lists_are_capped() {
		let ids = (0..=MAX_EXCLUDE)
			.map(|i| i.to_string())
			.collect::<Vec<_>>()
			.join(",");
		assert!(Scope::parse(None, None, Some(&ids)).is_err());
		let ok = (0..MAX_EXCLUDE)
			.map(|i| i.to_string())
			.collect::<Vec<_>>()
			.join(",");
		assert!(Scope::parse(None, None, Some(&ok)).is_ok());
		let many = (0..=MAX_TAG_FILTERS)
			.map(|i| format!("t{i}"))
			.collect::<Vec<_>>()
			.join(",");
		assert!(Scope::parse(Some(&many), None, None).is_err());
	}

	#[test]
	fn tag_counts_count_each_book_once_and_fold_spellings() {
		let book = |id: &str, t: &[&str]| CatalogRow {
			media_id: id.to_string(),
			title: id.to_string(),
			series: None,
			authors: vec![],
			language: None,
			library_id: "l".into(),
			library: "L".into(),
			tags: tags(t),
			path: String::new(),
			kind: None,
		};
		let counts = tag_counts(&[
			book("1", &["Poesía", "poesia"]),
			book("2", &["POESIA", "Novela"]),
		]);
		assert_eq!(counts, vec![("Poesía".to_string(), 2), ("Novela".to_string(), 1)]);
	}
}
