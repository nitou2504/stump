//! Value + count listings for list-like metadata fields (genres, writers, language, ...).
//!
//! These power the "browse by genre/author" pages in the web client and the matching
//! OPDS navigation feeds, so the counting logic lives in a plain function that both can
//! call.

use std::collections::HashMap;

use async_graphql::{Context, Enum, Object, Result, SimpleObject, ID};
use models::entity::{media, media_metadata, user::AuthUser};
use sea_orm::{prelude::*, sea_query::SelectStatement, ConnectionTrait, QuerySelect};

use crate::{
	data::{AuthContext, CoreContext},
	filter::search::{search_phrase, search_words},
};

/// The metadata fields which can be listed with their book counts
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetadataFacetField {
	Genres,
	Writers,
	Language,
	Publisher,
	Series,
}

impl MetadataFacetField {
	pub fn column(&self) -> media_metadata::Column {
		match self {
			Self::Genres => media_metadata::Column::Genres,
			Self::Writers => media_metadata::Column::Writers,
			Self::Language => media_metadata::Column::Language,
			Self::Publisher => media_metadata::Column::Publisher,
			Self::Series => media_metadata::Column::Series,
		}
	}

	/// Whether the column stores a comma-separated list of values
	pub fn is_list(&self) -> bool {
		matches!(self, Self::Genres | Self::Writers)
	}
}

#[derive(SimpleObject, Clone, Debug)]
pub struct MetadataValueCount {
	pub value: String,
	pub count: u64,
}

/// Lists every distinct value of `field` visible to `user`, with the number of books
/// carrying it. Values are de-duplicated case-insensitively (the first-seen casing wins)
/// and sorted alphabetically, case-insensitively.
///
/// * `library_id` restricts the books to a single library
/// * `media_ids` restricts the books to an arbitrary subquery of media IDs (e.g. a smart
///   list)
/// * `search` keeps only values containing the (case-insensitive) search string
pub async fn metadata_value_counts<C: ConnectionTrait>(
	conn: &C,
	user: &AuthUser,
	field: MetadataFacetField,
	library_id: Option<String>,
	media_ids: Option<SelectStatement>,
	search: Option<String>,
) -> std::result::Result<Vec<MetadataValueCount>, DbErr> {
	let column = field.column();
	let mut query = media::Entity::find_for_user(user)
		.select_only()
		.column(column)
		.filter(column.is_not_null());
	if let Some(library_id) = library_id {
		query = query.filter(media::Column::LibraryId.eq(library_id));
	}
	if let Some(media_ids) = media_ids {
		query = query.filter(media::Column::Id.in_subquery(media_ids));
	}
	let rows: Vec<String> = query.into_tuple().all(conn).await?;

	let search = search
		.map(|s| s.trim().to_lowercase())
		.filter(|s| !s.is_empty());

	// lowercase key -> (display value, count)
	let mut counts: HashMap<String, (String, u64)> = HashMap::new();
	for row in rows {
		let values: Vec<&str> = if field.is_list() {
			row.split(',').collect()
		} else {
			vec![row.as_str()]
		};
		// A book listing the same value twice should still only count once
		let mut seen_in_row = Vec::with_capacity(values.len());
		for value in values {
			let value = value.trim();
			if value.is_empty() {
				continue;
			}
			let key = value.to_lowercase();
			if seen_in_row.contains(&key) {
				continue;
			}
			if let Some(search) = &search {
				if !key.contains(search.as_str()) {
					continue;
				}
			}
			seen_in_row.push(key.clone());
			counts
				.entry(key)
				.and_modify(|(_, count)| *count += 1)
				.or_insert_with(|| (value.to_string(), 1));
		}
	}

	let mut result = counts
		.into_iter()
		.map(|(key, (value, count))| {
			(sort_key(&key), MetadataValueCount { value, count })
		})
		.collect::<Vec<_>>();
	result.sort_by(|(a, _), (b, _)| a.cmp(b));
	Ok(result.into_iter().map(|(_, v)| v).collect())
}

/// A cheap accent-insensitive sort key so that e.g. `Ética` sorts next to `Etnografía`
/// instead of after `Zoología`
pub fn sort_key(value: &str) -> String {
	value
		.to_lowercase()
		.chars()
		.map(|c| match c {
			'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
			'é' | 'è' | 'ë' | 'ê' => 'e',
			'í' | 'ì' | 'ï' | 'î' => 'i',
			'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'ø' => 'o',
			'ú' | 'ù' | 'ü' | 'û' => 'u',
			'ñ' => 'n',
			'ç' => 'c',
			_ => c,
		})
		.collect()
}

/// The most characters of an author search that are used
const MAX_AUTHOR_SEARCH_CHARS: usize = 100;

/// The most authors [`MetadataFacetQuery::search_authors`] gives back
const MAX_AUTHOR_MATCHES: i32 = 20;

/// The `values` (a name and how many books carry it) that answer `search`, the best first, at
/// most `limit`. Every word of the search has to start a word of the name, ignoring case and
/// accents like the book search does. The best are the names that are the search, then the names
/// that start with it, then those with a word that starts with it, then the rest; the more books
/// first among equals, then alphabetically (see [`sort_key`]). A search with no words matches nothing.
pub fn best_matches(
	values: Vec<MetadataValueCount>,
	search: &str,
	limit: usize,
) -> Vec<MetadataValueCount> {
	let words = search_words(search);
	if words.is_empty() {
		return Vec::new();
	}
	let phrase = search_phrase(search);
	let mut matches: Vec<(u8, std::cmp::Reverse<u64>, String, MetadataValueCount)> = values
		.into_iter()
		.filter_map(|value| {
			let name = search_phrase(&value.value);
			let starts_a_word = |word: &String| {
				name.split(' ').any(|part| part.starts_with(word.as_str()))
			};
			if !words.iter().all(starts_a_word) {
				return None;
			}
			let rank = if name == phrase {
				0
			} else if name.starts_with(&phrase) {
				1
			} else if format!(" {name}").contains(&format!(" {phrase}")) {
				2
			} else {
				3
			};
			Some((rank, std::cmp::Reverse(value.count), name, value))
		})
		.collect();
	matches.sort_by(|a, b| (a.0, &a.1, &a.2).cmp(&(b.0, &b.1, &b.2)));
	matches.truncate(limit);
	matches.into_iter().map(|(.., value)| value).collect()
}

#[derive(Default)]
pub struct MetadataFacetQuery;

#[Object]
impl MetadataFacetQuery {
	/// List the distinct values of a metadata field with their book counts, e.g. every
	/// genre and how many books have it
	async fn metadata_value_counts(
		&self,
		ctx: &Context<'_>,
		field: MetadataFacetField,
		library_id: Option<ID>,
		search: Option<String>,
	) -> Result<Vec<MetadataValueCount>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(metadata_value_counts(
			conn,
			user,
			field,
			library_id.map(|id| id.to_string()),
			None,
			search,
		)
		.await?)
	}

	/// Authors for a search box: the writers of the books you can see whose name answers
	/// `search` (every word of it starts a word of the name; case and accents don't count), the
	/// best first (the name is the search, starts with it, has a word that starts with it, then
	/// the rest) and, among equals, the one with more books first. Each comes with how many of the
	/// books you can see they wrote: an author whose books are all hidden from you is not in the
	/// answer, and a hidden book is not in a count. At most `limit` (4 by default, 20 at most).
	async fn search_authors(
		&self,
		ctx: &Context<'_>,
		search: String,
		#[graphql(default = 4)] limit: i32,
	) -> Result<Vec<MetadataValueCount>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let search: String = search.chars().take(MAX_AUTHOR_SEARCH_CHARS).collect();
		if search_words(&search).is_empty() {
			return Ok(Vec::new());
		}
		let limit = limit.clamp(1, MAX_AUTHOR_MATCHES) as usize;

		// The same call the Browse page's list is made with, so the books it reads and counts
		// are the ones `find_for_user` lets this user see and no others
		let authors = metadata_value_counts(
			conn,
			user,
			MetadataFacetField::Writers,
			None,
			None,
			None,
		)
		.await?;
		Ok(best_matches(authors, &search, limit))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn author(value: &str, count: u64) -> MetadataValueCount {
		MetadataValueCount {
			value: value.to_string(),
			count,
		}
	}

	fn names(matches: Vec<MetadataValueCount>) -> Vec<String> {
		matches.into_iter().map(|m| m.value).collect()
	}

	#[test]
	fn the_best_name_comes_first_then_the_most_books() {
		let authors = vec![
			author("Zed Asimov", 40),
			author("Isaac Asimov", 148),
			author("Asimov", 1),
			author("Asimovich Rex", 2),
			author("Janet Asimov", 3),
		];
		// the name that is the search, the names that start with it (the one with more books
		// first), the names with a word that starts with it (more books first)
		assert_eq!(
			names(best_matches(authors, "asimov", 10)),
			["Asimov", "Asimovich Rex", "Isaac Asimov", "Zed Asimov", "Janet Asimov"]
		);
	}

	#[test]
	fn every_word_must_start_a_word_of_the_name_in_any_order() {
		let authors = vec![
			author("Isaac Asimov", 10),
			author("Isaac Newton", 5),
			author("Janet Asimov", 3),
		];
		assert_eq!(
			names(best_matches(authors.clone(), "isaac as", 10)),
			["Isaac Asimov"]
		);
		// the words may come in any order
		assert_eq!(
			names(best_matches(authors.clone(), "asimov isaac", 10)),
			["Isaac Asimov"]
		);
		// the middle of a word is not the start of one
		assert!(best_matches(authors, "simov", 10).is_empty());
	}

	#[test]
	fn case_and_accents_do_not_count() {
		let authors = vec![
			author("Gabriel García Márquez", 12),
			author("Ñoño Pérez", 2),
		];
		assert_eq!(
			names(best_matches(authors.clone(), "GARCIA marq", 10)),
			["Gabriel García Márquez"]
		);
		assert_eq!(names(best_matches(authors, "nono", 10)), ["Ñoño Pérez"]);
	}

	#[test]
	fn the_limit_keeps_the_best() {
		let authors = (0..10)
			.map(|n| author(&format!("Ann {n}"), n))
			.chain([author("Ann", 1)])
			.collect();
		let found = best_matches(authors, "ann", 3);
		assert_eq!(names(found), ["Ann", "Ann 9", "Ann 8"]);
	}

	#[test]
	fn a_search_without_words_matches_nothing() {
		let authors = vec![author("Isaac Asimov", 10)];
		assert!(best_matches(authors.clone(), "", 10).is_empty());
		assert!(best_matches(authors.clone(), "  ", 10).is_empty());
		assert!(best_matches(authors, "&&& ...", 10).is_empty());
	}
}
