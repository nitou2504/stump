//! Free-text search shared by the web UI (the `search` filter field) and OPDS.
//!
//! A query is split into words; a book matches when **every** word appears in at least one
//! of its fields (file name and path, title, series, writers, publisher, genres or tags), so
//! `pearson brown` finds a Pearson book by Brown in any field order. Matching ignores case
//! and accents (`fisica` finds `Física`).

use models::entity::{media_tag, tag};
use sea_orm::{
	sea_query::{Expr, Query, SelectStatement},
	Condition, ExprTrait,
};

use super::keyword::escape_like_fragment;

/// The most words a query is split into; the rest are ignored
const MAX_WORDS: usize = 8;

/// Accented characters folded by [`fold`] and [`folded_sql`]. SQLite's `lower()` only
/// handles ASCII, so the uppercase accented letters are listed too.
const ACCENTS: &[(char, char)] = &[
	('á', 'a'),
	('à', 'a'),
	('ä', 'a'),
	('â', 'a'),
	('ã', 'a'),
	('Á', 'a'),
	('À', 'a'),
	('Ä', 'a'),
	('Â', 'a'),
	('é', 'e'),
	('è', 'e'),
	('ë', 'e'),
	('ê', 'e'),
	('É', 'e'),
	('È', 'e'),
	('í', 'i'),
	('ì', 'i'),
	('ï', 'i'),
	('î', 'i'),
	('Í', 'i'),
	('ó', 'o'),
	('ò', 'o'),
	('ö', 'o'),
	('ô', 'o'),
	('õ', 'o'),
	('Ó', 'o'),
	('Ö', 'o'),
	('ú', 'u'),
	('ù', 'u'),
	('ü', 'u'),
	('û', 'u'),
	('Ú', 'u'),
	('Ü', 'u'),
	('ñ', 'n'),
	('Ñ', 'n'),
	('ç', 'c'),
	('Ç', 'c'),
];

/// Lowercases `value` and strips the accents in [`ACCENTS`], like [`folded_sql`] does in SQL
pub fn fold(value: &str) -> String {
	value
		.to_lowercase()
		.chars()
		.map(|c| {
			ACCENTS
				.iter()
				.find(|(accented, _)| *accented == c)
				.map_or(c, |(_, plain)| *plain)
		})
		.collect()
}

/// The words of a search query, folded and deduplicated. `_` separates words too, since
/// file names use it instead of spaces.
pub fn search_words(query: &str) -> Vec<String> {
	let mut words: Vec<String> = Vec::new();
	for word in fold(query).split(|c: char| c.is_whitespace() || c == '_') {
		let word = word.trim_matches(|c: char| !c.is_alphanumeric());
		if !word.is_empty() && !words.iter().any(|w| w == word) {
			words.push(word.to_string());
		}
		if words.len() == MAX_WORDS {
			break;
		}
	}
	words
}

/// `column` lowercased with its accents stripped, as an SQL expression
fn folded_sql(column: &str) -> String {
	ACCENTS
		.iter()
		.fold(format!("LOWER({column})"), |sql, (accented, plain)| {
			format!("REPLACE({sql}, '{accented}', '{plain}')")
		})
}

/// `folded(column) LIKE %word%`
fn column_contains(column: &str, word: &str) -> Expr {
	Expr::cust_with_values(
		format!("{} LIKE ? ESCAPE '\\'", folded_sql(column)),
		[format!("%{}%", escape_like_fragment(word))],
	)
}

/// The IDs of the books with a tag containing `word`
fn tag_contains_subquery(word: &str) -> SelectStatement {
	let tag_ids = Query::select()
		.column(tag::Column::Id)
		.from(tag::Entity)
		.and_where(column_contains(r#""tags"."name""#, word))
		.to_owned();
	Query::select()
		.distinct()
		.column(media_tag::Column::MediaId)
		.from(media_tag::Entity)
		.and_where(Expr::col(media_tag::Column::TagId).in_subquery(tag_ids))
		.to_owned()
}

/// The book fields searched, as SQL column references. The query must join `media_metadata`.
const MEDIA_COLUMNS: &[&str] = &[
	r#""media"."name""#,
	r#""media"."path""#,
	r#""media_metadata"."title""#,
	r#""media_metadata"."series""#,
	r#""media_metadata"."writers""#,
	r#""media_metadata"."publisher""#,
	r#""media_metadata"."genres""#,
];

/// Books matching every word of `query` (see the module docs). An empty query matches all.
pub fn media_search_condition(query: &str) -> Condition {
	search_words(query)
		.iter()
		.fold(Condition::all(), |all_words, word| {
			let any_field = MEDIA_COLUMNS
				.iter()
				.fold(Condition::any(), |any, column| {
					any.add(column_contains(column, word))
				})
				.add(
					Expr::col((
						models::entity::media::Entity,
						models::entity::media::Column::Id,
					))
					.in_subquery(tag_contains_subquery(word)),
				);
			all_words.add(any_field)
		})
}

/// The series fields searched. The query must join `series_metadata`.
const SERIES_COLUMNS: &[&str] = &[r#""series"."name""#, r#""series_metadata"."title""#];

/// Series matching every word of `query` in their name or title
pub fn series_search_condition(query: &str) -> Condition {
	search_words(query)
		.iter()
		.fold(Condition::all(), |all_words, word| {
			all_words.add(SERIES_COLUMNS.iter().fold(Condition::any(), |any, column| {
				any.add(column_contains(column, word))
			}))
		})
}

#[cfg(test)]
mod tests {
	use super::*;
	use models::entity::media;
	use sea_orm::{EntityTrait, QueryFilter, QueryTrait};

	#[test]
	fn folds_case_and_accents() {
		assert_eq!(fold("Física CUÁNTICA Ñandú"), "fisica cuantica nandu");
	}

	#[test]
	fn splits_queries_into_unique_words() {
		assert_eq!(search_words("  Pearson   brown "), vec!["pearson", "brown"]);
		assert_eq!(search_words("Catrin_Brown brown"), vec!["catrin", "brown"]);
		assert_eq!(search_words("¿Física?"), vec!["fisica"]);
		assert!(search_words("   ").is_empty());
	}

	#[test]
	fn every_word_must_match_some_field() {
		let sql = media::Entity::find()
			.filter(media_search_condition("pearson brown"))
			.build(sea_orm::DatabaseBackend::Sqlite)
			.to_string();
		assert!(sql.contains("'%pearson%'"), "{sql}");
		assert!(sql.contains("'%brown%'"), "{sql}");
		assert!(sql.contains(") AND ("), "{sql}");
		assert!(sql.contains(r#""media_metadata"."writers""#), "{sql}");
		assert!(sql.contains(r#""tags"."name""#), "{sql}");
	}

	#[test]
	fn empty_query_matches_everything() {
		let sql = media::Entity::find()
			.filter(media_search_condition(" "))
			.build(sea_orm::DatabaseBackend::Sqlite)
			.to_string();
		assert!(!sql.contains("LIKE"), "{sql}");
	}
}
