//! Whole-book search over the text layers: accent- and case-insensitive substring matching, with
//! snippets, the PDFium char range and the boxes of each hit.

use serde::Serialize;

use super::text::{PageText, TextRun};

/// Accents folded away, like the fork's library search (`graphql::filter::search::fold`)
const ACCENTS: &[(char, char)] = &[
	('á', 'a'),
	('à', 'a'),
	('ä', 'a'),
	('â', 'a'),
	('ã', 'a'),
	('å', 'a'),
	('é', 'e'),
	('è', 'e'),
	('ë', 'e'),
	('ê', 'e'),
	('í', 'i'),
	('ì', 'i'),
	('ï', 'i'),
	('î', 'i'),
	('ó', 'o'),
	('ò', 'o'),
	('ö', 'o'),
	('ô', 'o'),
	('õ', 'o'),
	('ú', 'u'),
	('ù', 'u'),
	('ü', 'u'),
	('û', 'u'),
	('ñ', 'n'),
	('ç', 'c'),
	('ý', 'y'),
	('ÿ', 'y'),
];

/// Lowercases `ch` and strips its accent. Ligatures common in PDFs are expanded.
fn fold_char(ch: char, out: &mut String) {
	match ch {
		'ﬁ' => out.push_str("fi"),
		'ﬂ' => out.push_str("fl"),
		'ﬀ' => out.push_str("ff"),
		'ﬃ' => out.push_str("ffi"),
		'ﬄ' => out.push_str("ffl"),
		'’' | '‘' => out.push('\''),
		'“' | '”' => out.push('"'),
		_ => {
			for lower in ch.to_lowercase() {
				let plain = ACCENTS
					.iter()
					.find(|(accented, _)| *accented == lower)
					.map_or(lower, |(_, plain)| *plain);
				out.push(plain);
			}
		},
	}
}

/// The search query folded the same way as the page text, whitespace collapsed
pub fn fold_query(query: &str) -> String {
	let mut folded = String::new();
	for ch in query.trim().chars() {
		if ch.is_whitespace() {
			if !folded.ends_with(' ') {
				folded.push(' ');
			}
		} else {
			fold_char(ch, &mut folded);
		}
	}
	folded
}

/// One char of the searchable page text
struct Entry {
	ch: char,
	/// `(run index, char position in the run)`, or `None` for a separator between runs
	source: Option<(usize, usize)>,
}

/// One hit on a page
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchHit {
	pub page: i32,
	pub snippet: String,
	/// `[start, end)` of the hit in `snippet`, in UTF-16 code units
	#[serde(rename = "match")]
	pub match_range: [usize; 2],
	/// `[start, end)` PDFium char range on the page
	pub chars: [i32; 2],
	pub rects: Vec<HitRect>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HitRect {
	pub x: f32,
	pub y: f32,
	pub width: f32,
	pub height: f32,
}

const SNIPPET_CONTEXT: usize = 50;

/// The searchable text of a page: runs joined with single spaces (a hyphen at a line end joins
/// the two halves of the word), whitespace collapsed
fn page_entries(runs: &[TextRun]) -> Vec<Entry> {
	let mut entries: Vec<Entry> = Vec::new();
	for (run_index, run) in runs.iter().enumerate() {
		let chars: Vec<char> = run.text.chars().collect();
		let joins_next_word = run.eol
			&& chars.len() >= 2
			&& chars[chars.len() - 1] == '-'
			&& chars[chars.len() - 2].is_alphabetic();
		let usable = if joins_next_word {
			chars.len() - 1
		} else {
			chars.len()
		};
		for (position, ch) in chars.iter().take(usable).enumerate() {
			if ch.is_whitespace() {
				if entries.last().is_some_and(|entry| entry.ch == ' ')
					|| entries.is_empty()
				{
					continue;
				}
				entries.push(Entry {
					ch: ' ',
					source: Some((run_index, position)),
				});
			} else {
				entries.push(Entry {
					ch: *ch,
					source: Some((run_index, position)),
				});
			}
		}
		if !joins_next_word && entries.last().is_some_and(|entry| entry.ch != ' ') {
			entries.push(Entry {
				ch: ' ',
				source: None,
			});
		}
	}
	entries
}

/// Finds every hit of `folded_query` (from [`fold_query`]) on one page
pub fn search_page(page: i32, text: &PageText, folded_query: &str) -> Vec<SearchHit> {
	if folded_query.is_empty() || text.runs.is_empty() {
		return Vec::new();
	}
	let entries = page_entries(&text.runs);

	// the folded text, and for every folded char the entry it came from
	let mut folded: Vec<char> = Vec::with_capacity(entries.len());
	let mut origin: Vec<usize> = Vec::with_capacity(entries.len());
	let mut buffer = String::new();
	for (index, entry) in entries.iter().enumerate() {
		buffer.clear();
		fold_char(entry.ch, &mut buffer);
		for ch in buffer.chars() {
			folded.push(ch);
			origin.push(index);
		}
	}
	let needle: Vec<char> = folded_query.chars().collect();

	let mut hits = Vec::new();
	let mut start = 0;
	while start + needle.len() <= folded.len() {
		if folded[start..start + needle.len()] != needle[..] {
			start += 1;
			continue;
		}
		let first_entry = origin[start];
		let last_entry = origin[start + needle.len() - 1];
		hits.push(build_hit(page, text, &entries, first_entry, last_entry));
		start += needle.len();
	}
	hits
}

fn build_hit(
	page: i32,
	text: &PageText,
	entries: &[Entry],
	first: usize,
	last: usize,
) -> SearchHit {
	// snippet: the hit with some context, whitespace already collapsed
	let from = first.saturating_sub(SNIPPET_CONTEXT);
	let to = (last + 1 + SNIPPET_CONTEXT).min(entries.len());
	let mut snippet = String::new();
	let mut match_start = 0;
	let mut match_end = 0;
	if from > 0 {
		snippet.push('…');
	}
	for (index, entry) in entries.iter().enumerate().take(to).skip(from) {
		if index == first {
			match_start = snippet.encode_utf16().count();
		}
		snippet.push(entry.ch);
		if index == last {
			match_end = snippet.encode_utf16().count();
		}
	}
	if to < entries.len() {
		snippet.push('…');
	}
	let trimmed_start = snippet.len() - snippet.trim_start().len();
	let snippet_trimmed = snippet.trim().to_string();
	let shift = snippet[..trimmed_start].encode_utf16().count();

	// char range and boxes, per run
	let mut char_start = i32::MAX;
	let mut char_end = i32::MIN;
	let mut per_run: Vec<(usize, usize, usize)> = Vec::new();
	for entry in &entries[first..=last] {
		let Some((run_index, position)) = entry.source else {
			continue;
		};
		let run = &text.runs[run_index];
		let index = run.first_char + position as i32;
		char_start = char_start.min(index);
		char_end = char_end.max(index + 1);
		match per_run.last_mut() {
			Some((last_run, _, end)) if *last_run == run_index => *end = position + 1,
			_ => per_run.push((run_index, position, position + 1)),
		}
	}
	let rects = per_run
		.into_iter()
		.map(|(run_index, from, to)| {
			let [x, y, width, height] = text.span_rect(&text.runs[run_index], from, to);
			HitRect {
				x,
				y,
				width,
				height,
			}
		})
		.collect();

	SearchHit {
		page,
		snippet: snippet_trimmed,
		match_range: [
			match_start.saturating_sub(shift),
			match_end.saturating_sub(shift),
		],
		chars: if char_start <= char_end {
			[char_start, char_end]
		} else {
			[0, 0]
		},
		rects,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::pdf_reader::text::{group_runs, PageChar};

	fn page(lines: &[&str]) -> PageText {
		let mut chars = Vec::new();
		let mut index = 0;
		for (line_number, line) in lines.iter().enumerate() {
			for (i, ch) in line.chars().enumerate() {
				let left = 50.0 + i as f32 * 5.0;
				let top = 100.0 + line_number as f32 * 12.0;
				chars.push(PageChar {
					index,
					ch,
					bounds: Some([left, top, left + 5.0, top + 10.0]),
				});
				index += 1;
			}
			chars.push(PageChar {
				index,
				ch: '\r',
				bounds: None,
			});
			chars.push(PageChar {
				index: index + 1,
				ch: '\n',
				bounds: None,
			});
			index += 2;
		}
		PageText {
			width: 600.0,
			height: 800.0,
			runs: group_runs(&chars),
		}
	}

	#[test]
	fn finds_accent_and_case_insensitive_hits_with_boxes() {
		let text = page(&["La Física cuántica", "y la quimica"]);
		let hits = search_page(3, &text, &fold_query("FISICA"));
		assert_eq!(hits.len(), 1);
		let hit = &hits[0];
		assert_eq!(hit.page, 3);
		assert_eq!(hit.chars, [3, 9]);
		let matched: String = hit
			.snippet
			.encode_utf16()
			.skip(hit.match_range[0])
			.take(hit.match_range[1] - hit.match_range[0])
			.map(|unit| char::from_u32(unit as u32).unwrap())
			.collect();
		assert_eq!(matched, "Física");
		assert_eq!(hit.rects.len(), 1);
		assert!((hit.rects[0].x - (65.0 / 600.0)).abs() < 1e-4);
	}

	#[test]
	fn matches_across_lines_and_hyphenated_words() {
		let text = page(&["the commensura-", "bility of segments"]);
		let hits = search_page(1, &text, &fold_query("commensurability"));
		assert_eq!(hits.len(), 1);
		assert_eq!(hits[0].rects.len(), 2, "one box per line");

		let text = page(&["quick brown", "fox jumps"]);
		let hits = search_page(1, &text, &fold_query("brown  fox"));
		assert_eq!(hits.len(), 1);
		assert_eq!(hits[0].rects.len(), 2);
	}

	#[test]
	fn finds_every_hit_and_nothing_on_empty_pages() {
		let text = page(&["ab ab ab"]);
		assert_eq!(search_page(1, &text, &fold_query("ab")).len(), 3);
		let empty = PageText {
			width: 1.0,
			height: 1.0,
			runs: vec![],
		};
		assert!(search_page(1, &empty, "ab").is_empty());
	}
}
