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

/// A search as the phrase it spells: [`fold`]ed, [`SEPARATORS`] as spaces, its words in order (repeats
/// too) and joined with one space. [`phrase_sql`] makes the same of a title, so the two can be compared.
pub fn search_phrase(query: &str) -> String {
	fold(query)
		.split(|c: char| c.is_whitespace() || SEPARATORS.contains(&c))
		.filter(|word| !word.is_empty())
		.collect::<Vec<_>>()
		.join(" ")
}

/// Characters treated as spaces between words, e.g. in `Física_(IB)` or `Asignatura: Física`
const SEPARATORS: &[char] = &[
	'_', '-', '(', ')', '[', ']', ':', ',', '.', ';', '/', '"', '«', '»', '¿', '?', '¡',
	'!', '—', '–', '·', '|',
];

/// `column` lowercased, without accents and with separators as spaces, as an SQL expression:
/// `fisica  diploma del ib` for `Física (Diploma del IB)` (a separator next to a space leaves two
/// spaces: see [`phrase_sql`])
fn spaced_sql(column: &str) -> String {
	let folded = ACCENTS
		.iter()
		.fold(format!("LOWER({column})"), |sql, (accented, plain)| {
			format!("REPLACE({sql}, '{accented}', '{plain}')")
		});
	SEPARATORS.iter().fold(folded, |sql, separator| {
		format!("REPLACE({sql}, '{separator}', ' ')")
	})
}

/// [`spaced_sql`] after a leading space, as an SQL expression: ` fisica  diploma del ib` for
/// `Física (Diploma del IB)`, so a word can be looked for as `% word%`
fn folded_sql(column: &str) -> String {
	format!("(' ' || {})", spaced_sql(column))
}

/// `column` as the phrase it spells, to compare with [`search_phrase`]: [`spaced_sql`] with runs of
/// spaces made one (up to eight in a row) and none at either end: `fisica diploma del ib`
fn phrase_sql(column: &str) -> String {
	let collapsed = (0..3).fold(spaced_sql(column), |sql, _| {
		format!("REPLACE({sql}, '  ', ' ')")
	});
	format!("TRIM({collapsed})")
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

/// How well the title of a book answers `query`, as an SQL sort key (smaller is better): `0` when the
/// title is the query, `1` when it starts with it, `2` when a word of it does, `3` for the other books
/// found (by their author, series, tags...). The title is the one the book is shown with: the
/// metadata title, else the file name. Case, accents and separators don't count, as in
/// [`media_search_condition`]. `None` when the query has no words, which matches everything and ranks
/// nothing. The query must join `media_metadata`.
pub fn media_relevance(query: &str) -> Option<Expr> {
	let words = search_words(query);
	if words.is_empty() {
		return None;
	}
	let phrase = search_phrase(query);
	let title = r#"COALESCE(NULLIF(TRIM("media_metadata"."title"), ''), "media"."name")"#;
	// a title that can't hold every word is rank 3 without being folded (the same cheap pre-check as the filter's)
	let holds_every_word = words
		.iter()
		.map(|_| format!("({title}) LIKE ? ESCAPE '\\'"))
		.collect::<Vec<_>>()
		.join(" AND ");
	let title_phrase = phrase_sql(title);
	let sql = format!(
		"CASE WHEN NOT ({holds_every_word}) THEN 3 \
		WHEN {title_phrase} = ? THEN 0 \
		WHEN {title_phrase} LIKE ? ESCAPE '\\' THEN 1 \
		WHEN (' ' || {title_phrase}) LIKE ? ESCAPE '\\' THEN 2 \
		ELSE 3 END"
	);
	let mut values: Vec<String> = words.iter().map(|word| loose_pattern(word)).collect();
	values.push(phrase.clone());
	values.push(format!("{}%", escape_like_fragment(&phrase)));
	values.push(format!("% {}%", escape_like_fragment(&phrase)));
	Some(Expr::cust_with_values(sql, values))
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
	use sea_orm::{EntityTrait, QueryFilter, QueryOrder, QueryTrait};

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
	fn a_search_is_the_phrase_it_spells() {
		assert_eq!(search_phrase("  Dune:  MESSIAH "), "dune messiah");
		assert_eq!(search_phrase("Física (IB)"), "fisica ib");
		// repeats stay: they are part of a title, unlike in the words of a filter
		assert_eq!(search_phrase("the the"), "the the");
		assert_eq!(search_phrase("  "), "");
	}

	#[test]
	fn relevance_needs_a_word_to_rank_by() {
		assert!(media_relevance("").is_none());
		assert!(media_relevance("   ").is_none());
		assert!(media_relevance("&&& ...").is_none());
		assert!(media_relevance("dune").is_some());
	}

	#[test]
	fn relevance_ranks_exact_then_prefix_then_word_start() {
		let sql = media::Entity::find()
			.order_by_asc(media_relevance("Dune Messiah").unwrap())
			.build(sea_orm::DatabaseBackend::Sqlite)
			.to_string();
		// each word is looked for loosely first, so most titles are never folded
		assert!(
			sql.contains(&format!("'{}'", loose_pattern("dune"))),
			"{sql}"
		);
		assert!(
			sql.contains(&format!("'{}'", loose_pattern("messiah"))),
			"{sql}"
		);
		// the title as the book is shown: the metadata title, else the file name
		assert!(
			sql.contains(
				r#"COALESCE(NULLIF(TRIM("media_metadata"."title"), ''), "media"."name")"#
			),
			"{sql}"
		);
		for (rank, comparison) in [
			(0, "= 'dune messiah'"),
			(1, "LIKE 'dune messiah%'"),
			(2, "LIKE '% dune messiah%'"),
		] {
			assert!(sql.contains(&comparison.to_string()), "{comparison}: {sql}");
			assert!(sql.contains(&format!("THEN {rank}")), "{rank}: {sql}");
		}
		assert!(sql.contains("ELSE 3 END"), "{sql}");
	}

	#[test]
	fn relevance_escapes_what_like_would_read_as_a_pattern() {
		let sql = media::Entity::find()
			.order_by_asc(media_relevance("50% off").unwrap())
			.build(sea_orm::DatabaseBackend::Sqlite)
			.to_string();
		assert!(sql.contains(r"LIKE '50\% off%'"), "{sql}");
		assert!(sql.contains(r"LIKE '% 50\% off%'"), "{sql}");
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
