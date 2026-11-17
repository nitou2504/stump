//! Answers over the index. Every function here takes the ids (or catalog rows) the caller has
//! already decided the user may see, and never looks beyond them.

use std::time::{Duration, Instant};

use regex::RegexBuilder;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value};
use serde::Serialize;

use super::{
	catalog::CatalogRow, fold_mapped, fold_str, ContentError, ContentResult,
	MAX_SECTION_CHARS,
};

/// Longest `q`
pub const MAX_QUERY_CHARS: usize = 256;
/// Most terms of a query
pub const MAX_TERMS: usize = 12;
/// Longest regex pattern (and literal pattern)
pub const MAX_PATTERN_CHARS: usize = 200;
/// Compiled size of a regex, in bytes
const REGEX_SIZE_LIMIT: usize = 1 << 20;
/// Most hits one grep counts
pub const MAX_GREP_HITS: usize = 2000;
/// Sections one grep query reads (their texts are all that is in memory at once)
const GREP_SECTIONS_PER_QUERY: usize = 16;
/// How long one grep may run
const GREP_BUDGET: Duration = Duration::from_secs(5);
/// Largest page
pub const MAX_PAGE_SIZE: u32 = 50;
/// Largest `chars` of a read
pub const MAX_READ_CHARS: usize = 20_000;

fn stmt(sql: &str, values: Vec<Value>) -> Statement {
	Statement::from_sql_and_values(DbBackend::Sqlite, sql, values)
}

fn ids_json(ids: &[String]) -> Value {
	serde_json::to_string(ids).unwrap_or_else(|_| "[]".to_string()).into()
}

/// The words of a query: bare words, or `"quoted phrases"`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terms {
	/// As typed (without quotes)
	pub raw: Vec<String>,
	/// Folded: lowercase, no accents
	pub folded: Vec<String>,
	pub match_all: bool,
}

/// Splits `q` into terms. `None` when there is no word in it.
pub fn parse_terms(q: &str, match_all: bool) -> Result<Option<Terms>, ContentError> {
	if q.chars().count() > MAX_QUERY_CHARS {
		return Err(ContentError::Invalid(format!(
			"q is longer than {MAX_QUERY_CHARS} characters"
		)));
	}
	let mut raw: Vec<String> = Vec::new();
	let mut current = String::new();
	let mut quoted = false;
	let flush = |current: &mut String, raw: &mut Vec<String>| {
		let word = current
			.trim_matches(|c: char| !c.is_alphanumeric())
			.to_string();
		if word.chars().any(char::is_alphanumeric) {
			raw.push(word);
		}
		current.clear();
	};
	for ch in q.chars() {
		match ch {
			'"' => {
				flush(&mut current, &mut raw);
				quoted = !quoted;
			},
			c if c.is_whitespace() && !quoted => flush(&mut current, &mut raw),
			c => current.push(c),
		}
	}
	flush(&mut current, &mut raw);
	if raw.len() > MAX_TERMS {
		return Err(ContentError::Invalid(format!(
			"q has more than {MAX_TERMS} words"
		)));
	}
	if raw.is_empty() {
		return Ok(None);
	}
	let folded = raw.iter().map(|word| fold_str(word)).collect();
	Ok(Some(Terms {
		raw,
		folded,
		match_all,
	}))
}

impl Terms {
	/// The FTS5 expression: every term a quoted string (nothing in it is syntax), joined by OR
	/// or AND; `column` limits it to one column
	pub fn fts_expression(&self, column: Option<&str>) -> String {
		let joiner = if self.match_all { " AND " } else { " OR " };
		let body = self
			.raw
			.iter()
			.map(|word| format!("\"{}\"", word.replace('"', "")))
			.collect::<Vec<_>>()
			.join(joiner);
		match column {
			Some(column) => format!("{column} : ({body})"),
			None => body,
		}
	}
}

/// One passage of a section that answers a search
#[derive(Debug, Clone, Serialize)]
pub struct Passage {
	pub media_id: String,
	pub section: i64,
	pub section_title: String,
	pub score: f64,
	pub pos: usize,
	pub snippet: String,
	pub page_label: Option<String>,
}

/// Searches the text of `ids`. One best section per book, or (`per_book` false) every matching
/// section. Returns the total and the page
pub async fn search_text(
	conn: &DatabaseConnection,
	ids: &[String],
	terms: &Terms,
	per_book: bool,
	page: u32,
	page_size: u32,
) -> ContentResult<(u64, Vec<Passage>)> {
	if ids.is_empty() {
		return Ok((0, Vec::new()));
	}
	let expression = terms.fts_expression(None);
	let inner = "SELECT s.media_id AS media_id, s.id AS section_id, s.n AS n, s.title AS title,
			s.page_marks AS page_marks, bm25(content_fts, 5.0, 1.0) AS score
		 FROM content_fts CROSS JOIN content_sections s ON s.id = content_fts.rowid
		 WHERE content_fts MATCH ?1 AND s.media_id IN (SELECT value FROM json_each(?2))";
	let count_sql = if per_book {
		"SELECT COUNT(DISTINCT s.media_id) AS total
		 FROM content_fts JOIN content_sections s ON s.id = content_fts.rowid
		 WHERE content_fts MATCH ?1 AND s.media_id IN (SELECT value FROM json_each(?2))"
			.to_string()
	} else {
		"SELECT COUNT(*) AS total
		 FROM content_fts JOIN content_sections s ON s.id = content_fts.rowid
		 WHERE content_fts MATCH ?1 AND s.media_id IN (SELECT value FROM json_each(?2))"
			.to_string()
	};
	let total: i64 = conn
		.query_one_raw(stmt(
			&count_sql,
			vec![expression.clone().into(), ids_json(ids)],
		))
		.await?
		.map(|row| row.try_get("", "total"))
		.transpose()?
		.unwrap_or(0);
	let page_sql = if per_book {
		format!(
			"WITH hits AS MATERIALIZED ({inner})
			 SELECT media_id, section_id, n, title, page_marks, MIN(score) AS score FROM hits
			 GROUP BY media_id ORDER BY score ASC, media_id LIMIT ?3 OFFSET ?4"
		)
	} else {
		format!(
			"WITH hits AS MATERIALIZED ({inner})
			 SELECT * FROM hits ORDER BY score ASC, media_id, n LIMIT ?3 OFFSET ?4"
		)
	};
	let offset = (page.max(1) as i64 - 1) * page_size as i64;
	let rows = conn
		.query_all_raw(stmt(
			&page_sql,
			vec![
				expression.into(),
				ids_json(ids),
				(page_size as i64).into(),
				offset.into(),
			],
		))
		.await?;
	struct Hit {
		media_id: String,
		section_id: i64,
		section: i64,
		title: String,
		score: f64,
		marks: Option<String>,
	}
	let mut hits = Vec::with_capacity(rows.len());
	for row in rows {
		hits.push(Hit {
			media_id: row.try_get("", "media_id")?,
			section_id: row.try_get("", "section_id")?,
			section: row.try_get("", "n")?,
			title: row.try_get("", "title")?,
			score: row.try_get("", "score")?,
			marks: row.try_get("", "page_marks")?,
		});
	}
	let mut passages = Vec::with_capacity(hits.len());
	// one query per chunk of sections (and a chunk's texts are all that is in memory at once)
	while !hits.is_empty() {
		let chunk: Vec<Hit> = hits.drain(..SEARCH_TEXT_CHUNK.min(hits.len())).collect();
		let section_ids: Vec<i64> = chunk.iter().map(|h| h.section_id).collect();
		let mut texts = section_texts(conn, &section_ids).await?;
		let batch: Vec<(Hit, String)> = chunk
			.into_iter()
			.map(|h| {
				let text = texts.remove(&h.section_id).unwrap_or_default();
				(h, text)
			})
			.collect();
		let terms = terms.clone();
		// folding up to 2M chars a section is CPU work: not on the async runtime
		let located = tokio::task::spawn_blocking(move || {
			batch
				.into_iter()
				.map(|(hit, text)| {
					let (pos, snippet) = locate(&text, &terms);
					Passage {
						page_label: page_label_at(hit.marks.as_deref(), pos),
						media_id: hit.media_id,
						section: hit.section,
						section_title: hit.title,
						score: -hit.score,
						pos,
						snippet,
					}
				})
				.collect::<Vec<_>>()
		})
		.await
		.map_err(|_| ContentError::Extract("the search failed".to_string()))?;
		passages.extend(located);
	}
	Ok((total.max(0) as u64, passages))
}

/// Sections whose text one query fetches (the memory of a page's texts is bounded by this)
const SEARCH_TEXT_CHUNK: usize = 10;

/// The texts (at most [`MAX_SECTION_CHARS`] chars each) of the sections `ids`, in one query
async fn section_texts(
	conn: &DatabaseConnection,
	ids: &[i64],
) -> ContentResult<std::collections::HashMap<i64, String>> {
	let rows = conn
		.query_all_raw(stmt(
			"SELECT rowid AS id, substr(text, 1, ?2) AS text FROM content_fts
			 WHERE rowid IN (SELECT value FROM json_each(?1))",
			vec![
				serde_json::to_string(ids).unwrap_or_else(|_| "[]".to_string()).into(),
				(MAX_SECTION_CHARS as i64).into(),
			],
		))
		.await?;
	let mut texts = std::collections::HashMap::with_capacity(rows.len());
	for row in rows {
		texts.insert(row.try_get("", "id")?, row.try_get("", "text")?);
	}
	Ok(texts)
}

/// A section's text, `chars` characters from char `pos`
async fn section_text(
	conn: &DatabaseConnection,
	section_id: i64,
	pos: usize,
	chars: usize,
) -> ContentResult<String> {
	Ok(conn
		.query_one_raw(stmt(
			"SELECT substr(text, ?, ?) AS text FROM content_fts WHERE rowid = ?",
			vec![
				(pos as i64 + 1).into(),
				(chars as i64).into(),
				section_id.into(),
			],
		))
		.await?
		.map(|row| row.try_get::<String>("", "text"))
		.transpose()?
		.unwrap_or_default())
}

/// The label of the page the char at `pos` is on, from a section's page marks (`[[pos, label]]`)
pub fn page_label_at(marks: Option<&str>, pos: usize) -> Option<String> {
	let marks: Vec<(usize, String)> = serde_json::from_str(marks?).ok()?;
	marks
		.into_iter()
		.take_while(|(at, _)| *at <= pos)
		.last()
		.map(|(_, label)| label)
}

const SNIPPET_BEFORE: usize = 120;
const SNIPPET_AFTER: usize = 200;

/// A snippet of `chars` around `[start, end)`, cut at word boundaries
pub fn snippet_around(chars: &[char], start: usize, end: usize) -> String {
	let mut from = start.saturating_sub(SNIPPET_BEFORE);
	let mut to = (end + SNIPPET_AFTER).min(chars.len());
	while from > 0 && from < start && !chars[from - 1].is_whitespace() {
		from += 1;
	}
	while to < chars.len() && to > end && !chars[to].is_whitespace() {
		to -= 1;
	}
	let mut out = String::new();
	if from > 0 {
		out.push('…');
	}
	out.extend(chars[from..to].iter());
	if to < chars.len() {
		out.push('…');
	}
	out.trim().to_string()
}

/// Where in `text` the terms are best answered: the char offset of the match and a snippet. The
/// match chosen is where most different terms lie close together (the earliest on a tie)
pub fn locate(text: &str, terms: &Terms) -> (usize, String) {
	let chars: Vec<char> = text.chars().collect();
	let (folded, origin) = fold_mapped(text);
	// every place (in folded chars) a term starts, to a cap
	let mut found: Vec<(usize, usize, usize)> = Vec::new(); // (folded start, term index, length)
	for (index, term) in terms.folded.iter().enumerate() {
		let needle: Vec<char> = term.chars().collect();
		if needle.is_empty() || needle.len() > folded.len() {
			continue;
		}
		let mut seen = 0;
		let mut i = 0;
		while i + needle.len() <= folded.len() && seen < 200 {
			if folded[i] == needle[0] && folded[i..i + needle.len()] == needle[..] {
				found.push((i, index, needle.len()));
				seen += 1;
				i += needle.len();
			} else {
				i += 1;
			}
		}
	}
	if found.is_empty() {
		let end = chars.len().min(SNIPPET_AFTER);
		return (0, snippet_around(&chars, 0, end.min(1)));
	}
	found.sort();
	let mut best = (0usize, 0usize); // (distinct terms, index into found)
	for (i, (start, _, _)) in found.iter().enumerate() {
		let mut distinct = std::collections::HashSet::new();
		for (other, term, _) in &found {
			if other.abs_diff(*start) <= 150 {
				distinct.insert(*term);
			}
		}
		if distinct.len() > best.0 {
			best = (distinct.len(), i);
		}
	}
	let (start, _, length) = found[best.1];
	let begin = origin[start];
	let end = origin[(start + length - 1).min(origin.len() - 1)] + 1;
	(begin, snippet_around(&chars, begin, end))
}

/// A hit of the catalog search
#[derive(Debug, Clone)]
pub struct CatalogHit {
	pub index: usize,
	pub score: f64,
}

fn word_prefix_in(haystack: &str, term: &str) -> bool {
	let mut start = 0;
	while let Some(found) = haystack[start..].find(term) {
		let at = start + found;
		let before_ok = haystack[..at]
			.chars()
			.next_back()
			.is_none_or(|c| !c.is_alphanumeric());
		if before_ok {
			return true;
		}
		start = at + term.len().max(1);
		while start < haystack.len() && !haystack.is_char_boundary(start) {
			start += 1;
		}
	}
	false
}

/// The catalog search: title, series, authors, tags and the table of contents of `books`,
/// best first
pub async fn search_catalog(
	conn: &DatabaseConnection,
	books: &[CatalogRow],
	terms: &Terms,
) -> ContentResult<Vec<CatalogHit>> {
	if books.is_empty() {
		return Ok(Vec::new());
	}
	let ids: Vec<String> = books.iter().map(|b| b.media_id.clone()).collect();
	// for each term, the books with a section title that has it
	let mut in_toc: Vec<std::collections::HashSet<String>> = Vec::new();
	for word in &terms.raw {
		let single = Terms {
			raw: vec![word.clone()],
			folded: vec![fold_str(word)],
			match_all: false,
		};
		let rows = conn
			.query_all_raw(stmt(
				"SELECT DISTINCT s.media_id AS media_id
				 FROM content_fts JOIN content_sections s ON s.id = content_fts.rowid
				 WHERE content_fts MATCH ?1 AND s.media_id IN (SELECT value FROM json_each(?2))",
				vec![single.fts_expression(Some("title")).into(), ids_json(&ids)],
			))
			.await?;
		let mut set = std::collections::HashSet::new();
		for row in rows {
			set.insert(row.try_get::<String>("", "media_id")?);
		}
		in_toc.push(set);
	}
	let mut hits = Vec::new();
	for (index, book) in books.iter().enumerate() {
		let title = fold_str(&book.title);
		let series = fold_str(book.series.as_deref().unwrap_or(""));
		let authors = fold_str(&book.authors.join(" , "));
		let tags = fold_str(&book.tags.join(" , "));
		let mut score = 0.0;
		let mut matched = 0;
		for (i, term) in terms.folded.iter().enumerate() {
			let mut term_score = 0.0;
			if word_prefix_in(&title, term) {
				term_score += 3.0;
			}
			if word_prefix_in(&series, term) {
				term_score += 2.0;
			}
			if word_prefix_in(&authors, term) {
				term_score += 2.0;
			}
			if word_prefix_in(&tags, term) {
				term_score += 1.0;
			}
			if in_toc[i].contains(&book.media_id) {
				term_score += 1.0;
			}
			if term_score > 0.0 {
				matched += 1;
				score += term_score;
			}
		}
		let enough = if terms.match_all {
			matched == terms.folded.len()
		} else {
			matched > 0
		};
		if enough {
			hits.push(CatalogHit { index, score });
		}
	}
	hits.sort_by(|a, b| {
		b.score
			.partial_cmp(&a.score)
			.unwrap_or(std::cmp::Ordering::Equal)
			.then_with(|| books[a.index].title.cmp(&books[b.index].title))
	});
	Ok(hits)
}

/// A section of the list
#[derive(Debug, Clone, Serialize)]
pub struct SectionInfo {
	pub n: i64,
	pub title: String,
	pub level: i64,
	pub chars: i64,
	pub page_start: Option<i64>,
}

pub async fn list_sections(
	conn: &DatabaseConnection,
	media_id: &str,
	page: u32,
	page_size: u32,
) -> ContentResult<(u64, Vec<SectionInfo>)> {
	let total: i64 = conn
		.query_one_raw(stmt(
			"SELECT COUNT(*) AS total FROM content_sections WHERE media_id = ?",
			vec![media_id.into()],
		))
		.await?
		.map(|row| row.try_get("", "total"))
		.transpose()?
		.unwrap_or(0);
	let offset = (page.max(1) as i64 - 1) * page_size as i64;
	let rows = conn
		.query_all_raw(stmt(
			"SELECT n, title, level, chars, page_start FROM content_sections
			 WHERE media_id = ? ORDER BY n LIMIT ? OFFSET ?",
			vec![media_id.into(), (page_size as i64).into(), offset.into()],
		))
		.await?;
	let mut sections = Vec::with_capacity(rows.len());
	for row in rows {
		sections.push(SectionInfo {
			n: row.try_get("", "n")?,
			title: row.try_get("", "title")?,
			level: row.try_get("", "level")?,
			chars: row.try_get("", "chars")?,
			page_start: row.try_get("", "page_start")?,
		});
	}
	Ok((total.max(0) as u64, sections))
}

/// A piece of a section
#[derive(Debug, Clone, Serialize)]
pub struct ReadChunk {
	pub n: i64,
	pub title: String,
	pub text: String,
	pub pos: usize,
	pub next_pos: Option<usize>,
	pub chars_total: usize,
	pub page_labels: Vec<(usize, String)>,
}

/// Reads `chars` characters of section `n` from char `pos`. The piece ends at a word boundary when
/// it can. `None` when the book has no such section
pub async fn read_section(
	conn: &DatabaseConnection,
	media_id: &str,
	n: i64,
	pos: usize,
	chars: usize,
) -> ContentResult<Option<ReadChunk>> {
	let chars = chars.clamp(1, MAX_READ_CHARS);
	let Some(row) = conn
		.query_one_raw(stmt(
			"SELECT id, title, chars, page_marks FROM content_sections WHERE media_id = ? AND n = ?",
			vec![media_id.into(), n.into()],
		))
		.await?
	else {
		return Ok(None);
	};
	let id: i64 = row.try_get("", "id")?;
	let total: i64 = row.try_get("", "chars")?;
	let total = total.max(0) as usize;
	if pos > total {
		return Err(ContentError::Invalid(
			"pos is past the end of the section".to_string(),
		));
	}
	let mut text = section_text(conn, id, pos, chars).await?;
	let mut length = text.chars().count();
	if pos + length < total {
		// end at a word boundary when one lies in the last part of the piece
		let tail: Vec<char> = text.chars().collect();
		let floor = length.saturating_sub(length / 5 + 1);
		if let Some(cut) = (floor..length).rev().find(|i| tail[*i].is_whitespace()) {
			if cut > 0 {
				text = tail[..cut].iter().collect();
				length = cut;
			}
		}
	}
	let end = pos + length;
	let marks: Vec<(usize, String)> = row
		.try_get::<Option<String>>("", "page_marks")?
		.and_then(|raw| serde_json::from_str(&raw).ok())
		.unwrap_or_default();
	let mut page_labels = Vec::new();
	let mut before: Option<&(usize, String)> = None;
	for mark in &marks {
		if mark.0 <= pos {
			before = Some(mark);
		} else if mark.0 < end {
			page_labels.push(mark.clone());
		}
	}
	if let Some((_, label)) = before {
		page_labels.insert(0, (pos, label.clone()));
	}
	Ok(Some(ReadChunk {
		n,
		title: row.try_get("", "title")?,
		text,
		pos,
		next_pos: (end < total).then_some(end),
		chars_total: total,
		page_labels,
	}))
}

/// A hit of a grep
#[derive(Debug, Clone, Serialize)]
pub struct GrepHit {
	pub n: i64,
	pub pos: usize,
	pub snippet: String,
}

/// The pattern as a regex that cannot run away: a literal is escaped, a regex gets size limits
/// (and the engine has no backtracking, so no pattern takes more than linear time in the text).
/// Without `case`, matching is case-insensitive, and accents don't matter either: the text and the
/// pattern's non-ASCII letters are folded.
pub fn compile_pattern(
	pattern: &str,
	regex: bool,
	case: bool,
) -> Result<regex::Regex, ContentError> {
	if pattern.is_empty() {
		return Err(ContentError::Invalid("pattern is empty".to_string()));
	}
	if pattern.chars().count() > MAX_PATTERN_CHARS {
		return Err(ContentError::Invalid(format!(
			"pattern is longer than {MAX_PATTERN_CHARS} characters"
		)));
	}
	let source = if regex {
		pattern.to_string()
	} else {
		regex::escape(pattern)
	};
	let source = if case {
		source
	} else {
		source
			.chars()
			.map(|c| {
				if c.is_ascii() {
					c.to_string()
				} else {
					fold_str(&c.to_string())
				}
			})
			.collect()
	};
	RegexBuilder::new(&source)
		.case_insensitive(!case)
		.size_limit(REGEX_SIZE_LIMIT)
		.dfa_size_limit(REGEX_SIZE_LIMIT)
		.nest_limit(30)
		.build()
		.map_err(|error| {
			ContentError::Invalid(match error {
				regex::Error::CompiledTooBig(_) => "The pattern is too big".to_string(),
				_ => "The pattern is not a valid regular expression".to_string(),
			})
		})
}

/// Every match of `re` in `text` (`case` set: the text as it is; otherwise folded, and the hit's
/// place is mapped back to the text), at most `max` hits
pub fn grep_text(
	text: &str,
	re: &regex::Regex,
	case: bool,
	max: usize,
	deadline: Instant,
) -> ContentResult<Vec<(usize, String)>> {
	if Instant::now() > deadline {
		return Err(too_slow());
	}
	let chars: Vec<char> = text.chars().collect();
	let mut hits = Vec::new();
	if case {
		// byte offsets to char offsets, walking forward
		let mut byte_to_char = 0usize;
		let mut last_byte = 0usize;
		for (i, found) in re.find_iter(text).enumerate() {
			if i % DEADLINE_CHECK_EVERY == 0 && Instant::now() > deadline {
				return Err(too_slow());
			}
			if found.is_empty() {
				continue;
			}
			byte_to_char += text[last_byte..found.start()].chars().count();
			last_byte = found.start();
			let length = found.as_str().chars().count();
			hits.push((byte_to_char, snippet_around(&chars, byte_to_char, byte_to_char + length)));
			if hits.len() >= max {
				break;
			}
		}
		return Ok(hits);
	}
	let (folded, origin) = fold_mapped(text);
	let folded_text: String = folded.iter().collect();
	if Instant::now() > deadline {
		return Err(too_slow());
	}
	let mut byte_to_char = 0usize;
	let mut last_byte = 0usize;
	for (i, found) in re.find_iter(&folded_text).enumerate() {
		if i % DEADLINE_CHECK_EVERY == 0 && Instant::now() > deadline {
			return Err(too_slow());
		}
		if found.is_empty() {
			continue;
		}
		byte_to_char += folded_text[last_byte..found.start()].chars().count();
		last_byte = found.start();
		let length = found.as_str().chars().count();
		let begin = origin[byte_to_char];
		let end = origin[(byte_to_char + length - 1).min(origin.len() - 1)] + 1;
		hits.push((begin, snippet_around(&chars, begin, end)));
		if hits.len() >= max {
			break;
		}
	}
	Ok(hits)
}

/// Matches between two looks at the clock
const DEADLINE_CHECK_EVERY: usize = 256;

fn too_slow() -> ContentError {
	ContentError::Invalid("The search took too long; narrow it with section".to_string())
}

/// Greps a book's sections (one with `section`). Returns the total (capped at
/// [`MAX_GREP_HITS`]) and the hits of the page
pub async fn grep_book(
	conn: &DatabaseConnection,
	media_id: &str,
	re: &regex::Regex,
	case: bool,
	section: Option<i64>,
	page: u32,
	page_size: u32,
) -> ContentResult<(u64, Vec<GrepHit>)> {
	let deadline = Instant::now() + GREP_BUDGET;
	let mut all: Vec<GrepHit> = Vec::new();
	let mut last = -1i64;
	// keyset paging: a few sections per query (text included), regex work off the async runtime
	while all.len() < MAX_GREP_HITS {
		if Instant::now() > deadline {
			return Err(too_slow());
		}
		let rows = conn
			.query_all_raw(stmt(
				"SELECT s.n AS n, substr(f.text, 1, ?5) AS text
				 FROM content_sections s JOIN content_fts f ON f.rowid = s.id
				 WHERE s.media_id = ?1 AND (?2 IS NULL OR s.n = ?2) AND s.n > ?3
				 ORDER BY s.n LIMIT ?4",
				vec![
					media_id.into(),
					section.into(),
					last.into(),
					(GREP_SECTIONS_PER_QUERY as i64).into(),
					(MAX_SECTION_CHARS as i64).into(),
				],
			))
			.await?;
		if rows.is_empty() {
			break;
		}
		let mut batch: Vec<(i64, String)> = Vec::with_capacity(rows.len());
		for row in rows {
			batch.push((row.try_get("", "n")?, row.try_get("", "text")?));
		}
		last = batch.last().map(|(n, _)| *n).unwrap_or(last);
		let full = batch.len() == GREP_SECTIONS_PER_QUERY;
		let re = re.clone();
		let room = MAX_GREP_HITS - all.len();
		let found = tokio::task::spawn_blocking(move || -> ContentResult<Vec<GrepHit>> {
			let mut found = Vec::new();
			for (n, text) in batch {
				if found.len() >= room {
					break;
				}
				for (pos, snippet) in grep_text(&text, &re, case, room - found.len(), deadline)? {
					found.push(GrepHit { n, pos, snippet });
				}
			}
			Ok(found)
		})
		.await
		.map_err(|_| ContentError::Extract("the search failed".to_string()))??;
		all.extend(found);
		if !full {
			break;
		}
	}
	let total = all.len() as u64;
	let skip = (page.max(1) as usize - 1) * page_size as usize;
	Ok((
		total,
		all.into_iter().skip(skip).take(page_size as usize).collect(),
	))
}

/// The counts of the status page, over `books`
#[derive(Debug, Clone, Default, Serialize)]
pub struct StatusCounts {
	pub books: u64,
	pub indexed: u64,
	pub pending: u64,
	pub failed: u64,
}

pub async fn status_counts(
	conn: &DatabaseConnection,
	books: &[CatalogRow],
) -> ContentResult<StatusCounts> {
	let states: std::collections::HashMap<String, super::store::BookState> =
		super::store::book_states(conn)
			.await?
			.into_iter()
			.map(|s| (s.media_id.clone(), s))
			.collect();
	let mut counts = StatusCounts::default();
	for book in books.iter().filter(|b| b.kind.is_some()) {
		counts.books += 1;
		match states.get(&book.media_id) {
			Some(state) if state.extractor == super::EXTRACTOR_VERSION => {
				match state.status.as_str() {
					super::store::FAILED => counts.failed += 1,
					_ => counts.indexed += 1,
				}
			},
			_ => counts.pending += 1,
		}
	}
	Ok(counts)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn far() -> Instant {
		Instant::now() + Duration::from_secs(60)
	}

	#[test]
	fn grep_stops_once_its_deadline_has_passed() {
		let re = compile_pattern("a", false, false).unwrap();
		let text = "a ".repeat(10_000);
		let past = Instant::now() - Duration::from_secs(1);
		for case in [true, false] {
			let error = grep_text(&text, &re, case, 100, past).unwrap_err();
			assert!(matches!(error, ContentError::Invalid(m) if m.contains("took too long")));
		}
	}

	#[test]
	fn terms_are_words_or_quoted_phrases() {
		let terms = parse_terms("  Árbol \"la canción\" don't ", false).unwrap().unwrap();
		assert_eq!(terms.raw, vec!["Árbol", "la canción", "don't"]);
		assert_eq!(terms.folded, vec!["arbol", "la cancion", "don't"]);
		assert!(parse_terms("  ?! \"\" ", false).unwrap().is_none());
	}

	#[test]
	fn long_or_wordy_queries_are_refused() {
		assert!(parse_terms(&"a".repeat(MAX_QUERY_CHARS + 1), false).is_err());
		let many = (0..=MAX_TERMS).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
		assert!(parse_terms(&many, false).is_err());
	}

	#[test]
	fn the_fts_expression_has_no_syntax_from_the_user() {
		let terms = parse_terms("a AND b NEAR(c) x*", true).unwrap().unwrap();
		let expression = terms.fts_expression(None);
		assert!(expression.contains("\"AND\""));
		assert!(expression.contains("\"x\""));
		assert_eq!(
			parse_terms("uno dos", false).unwrap().unwrap().fts_expression(Some("title")),
			"title : (\"uno\" OR \"dos\")"
		);
	}

	#[test]
	fn locate_finds_the_densest_match_ignoring_case_and_accents() {
		let text = format!(
			"{} El Árbol grande y la canción del pájaro {}",
			"relleno ".repeat(100),
			"fin ".repeat(100)
		);
		let terms = parse_terms("ARBOL cancion", true).unwrap().unwrap();
		let (pos, snippet) = locate(&text, &terms);
		assert_eq!(text.chars().skip(pos).take(5).collect::<String>(), "Árbol");
		assert!(snippet.contains("canción"));
		assert!(snippet.starts_with('…') && snippet.ends_with('…'));
	}

	#[test]
	fn page_labels_come_from_the_marks() {
		let marks = r#"[[0,"i"],[100,"ii"]]"#;
		assert_eq!(page_label_at(Some(marks), 50).as_deref(), Some("i"));
		assert_eq!(page_label_at(Some(marks), 100).as_deref(), Some("ii"));
		assert_eq!(page_label_at(None, 5), None);
	}

	#[test]
	fn patterns_are_limited_and_cannot_run_away() {
		assert!(compile_pattern("", false, false).is_err());
		assert!(compile_pattern(&"a".repeat(MAX_PATTERN_CHARS + 1), false, false).is_err());
		assert!(compile_pattern("(unclosed", true, false).is_err());
		// too big once compiled
		assert!(compile_pattern("((a{100}){100}){100}", true, false).is_err());
		// the classic catastrophic pattern runs in linear time
		let re = compile_pattern("(a+)+$", true, false).unwrap();
		let text = format!("{}b", "a".repeat(200_000));
		let started = Instant::now();
		assert!(grep_text(&text, &re, false, 10, far()).unwrap().is_empty());
		assert!(started.elapsed() < Duration::from_secs(2));
		// a literal is not a pattern
		let literal = compile_pattern("a.c(", false, false).unwrap();
		assert_eq!(grep_text("xa.c(x abc(", &literal, false, 10, far()).unwrap().len(), 1);
	}

	#[test]
	fn grep_ignores_accents_and_case_unless_asked() {
		let text = "La Canción y la cancion";
		let re = compile_pattern("canción", false, false).unwrap();
		let hits = grep_text(text, &re, false, 10, far()).unwrap();
		assert_eq!(hits.iter().map(|h| h.0).collect::<Vec<_>>(), vec![3, 16]);
		let exact = compile_pattern("Canción", false, true).unwrap();
		assert_eq!(grep_text(text, &exact, true, 10, far()).unwrap().iter().map(|h| h.0).collect::<Vec<_>>(), vec![3]);
		let re = compile_pattern("ca.ci[oó]n", true, false).unwrap();
		assert_eq!(grep_text(text, &re, false, 10, far()).unwrap().len(), 2);
	}
}
