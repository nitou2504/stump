//! Free-text search shared by the web UI (the `search` filter field) and OPDS.
//!
//! A query is split into words; a book matches when **every** word starts a word in at least
//! one of its fields (file name, title, series, writers, publisher, genres or tags), so
//! `brown chemistry` finds a chemistry book by Brown in any field order and `quim` finds
//! `Química`, while `ib` doesn't match inside `biblioteca`. Matching ignores case and
//! accents (`fisica` finds `Física`).

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

/// The words of a search query, folded and deduplicated. [`SEPARATORS`] split words like
/// spaces, as they do in the searched text.
pub fn search_words(query: &str) -> Vec<String> {
	let mut words: Vec<String> = Vec::new();
	for word in fold(query).split(|c: char| c.is_whitespace() || SEPARATORS.contains(&c))
	{
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

/// Characters treated as spaces between words, e.g. in `Física_(IB)` or `Asignatura: Física`
const SEPARATORS: &[char] = &[
	'_', '-', '(', ')', '[', ']', ':', ',', '.', ';', '/', '"', '«', '»', '¿', '?', '¡',
	'!', '—', '–', '·', '|',
];

/// `column` lowercased, without accents and with separators as spaces, after a leading
/// space, as an SQL expression: ` fisica diploma del ib ` for `Física (Diploma del IB)`
fn folded_sql(column: &str) -> String {
	let folded = ACCENTS
		.iter()
		.fold(format!("LOWER({column})"), |sql, (accented, plain)| {
			format!("REPLACE({sql}, '{accented}', '{plain}')")
		});
	let spaced = SEPARATORS.iter().fold(folded, |sql, separator| {
		format!("REPLACE({sql}, '{separator}', ' ')")
	});
	format!("(' ' || {spaced})")
}

/// The columns joined into one text, so it is folded once per row and word
fn concat_sql(columns: &[&str]) -> String {
	columns
		.iter()
		.map(|column| format!("COALESCE({column}, '')"))
		.collect::<Vec<_>>()
		.join(" || ' ' || ")
}

/// A cheap pre-check for [`text_contains_word`]: `%word%` on the raw text with the letters
/// that may carry an accent (`a e i o u n c`) as `_` wildcards, so `fisica` checks
/// `%f_s___%`. It skips folding most rows, which is what makes the search fast.
fn loose_pattern(word: &str) -> String {
	let loose: String = escape_like_fragment(word)
		.chars()
		.map(|c| if "aeiounc".contains(c) { '_' } else { c })
		.collect();
	format!("%{loose}%")
}

/// Whether some word of the `columns` (taken as one text) starts with `word`
fn text_contains_word(columns: &[&str], word: &str) -> Expr {
	let concat = concat_sql(columns);
	Expr::cust_with_values(
		format!(
			"(({concat}) LIKE ? ESCAPE '\\' AND {} LIKE ? ESCAPE '\\')",
			folded_sql(&format!("({concat})"))
		),
		[
			loose_pattern(word),
			format!("% {}%", escape_like_fragment(word)),
		],
	)
}

/// The IDs of the books with a tag containing `word`
fn tag_contains_subquery(word: &str) -> SelectStatement {
	let tag_ids = Query::select()
		.column(tag::Column::Id)
		.from(tag::Entity)
		.and_where(text_contains_word(&[r#""tags"."name""#], word))
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
			let any_field = Condition::any()
				.add(text_contains_word(MEDIA_COLUMNS, word))
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
			all_words.add(text_contains_word(SERIES_COLUMNS, word))
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
	fn loose_pattern_wildcards_accentable_letters() {
		assert_eq!(loose_pattern("fisica"), "%f_s___%");
		assert_eq!(loose_pattern("50%"), r"%50\%%");
	}

	#[test]
	fn splits_queries_into_unique_words() {
		assert_eq!(search_words("  Pearson   brown "), vec!["pearson", "brown"]);
		assert_eq!(search_words("Catrin_Brown brown"), vec!["catrin", "brown"]);
		assert_eq!(search_words("¿Física?"), vec!["fisica"]);
		assert_eq!(search_words("Física (IB)"), vec!["fisica", "ib"]);
		assert!(search_words("   ").is_empty());
	}

	#[test]
	fn every_word_must_match_some_field() {
		let sql = media::Entity::find()
			.filter(media_search_condition("pearson brown"))
			.build(sea_orm::DatabaseBackend::Sqlite)
			.to_string();
		assert!(sql.contains("'% pearson%'"), "{sql}");
		assert!(sql.contains("'% brown%'"), "{sql}");
		assert!(sql.contains("'%p__rs__%'"), "{sql}");
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
