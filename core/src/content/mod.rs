//! The content index behind `/api/v2/content`: the text of every EPUB and PDF, per section, kept in
//! SQLite tables and an FTS5 table (migration `m20261117_000000_content_index`) so other programs
//! (the household's assistant) can search the books, list their sections, read a passage and grep it
//! without opening the files.
//!
//! - [`extract`] reads a book's sections: an EPUB's reading order (with [`crate::readium`]'s text
//!   extraction), a PDF's outline sections or pages (from the text layer the PDF reader serves).
//!   A scan without a text layer has no text, and that is all there is to say about it.
//! - [`store`] writes them (and reads the index's state)
//! - [`text`] reads a section's text from the book's file again: the index (`content_idx`, an FTS5
//!   table without content) keeps the words and their positions, not a second copy of the books
//! - [`job`] is the Stump job that does it for the books that are new, changed or never indexed,
//!   at idle priority, skipping what is unchanged
//! - [`query`] answers the searches over the allowed books. Who may read what is decided by the
//!   caller (the router works out the books the user can see, with the same filters as every other
//!   book query); everything here only looks at the ids it is given.
//!
//! The index is SQLite only (FTS5, `json_each`): on another database the migration does nothing.

pub mod catalog;
pub mod extract;
pub mod job;
pub mod query;
pub mod store;
pub mod text;

use sea_orm::DbErr;
use thiserror::Error;

use crate::pdf_reader::fold_char;

/// Bumped when what the extractors produce changes, so every book is read again
pub const EXTRACTOR_VERSION: i32 = 1;

/// How a book's index rows are kept. 0: moved from the first index (which kept the text) by the
/// `content_contentless` migration: its sections have no `source`. 1: read with `source`s. The job
/// reads a book of an older layout again (quietly: such a book is as searchable as any).
pub const LAYOUT_VERSION: i32 = 1;

/// Characters kept of one section (the rest of a longer one is cut off)
pub const MAX_SECTION_CHARS: usize = 2_000_000;
/// Characters kept of one book
pub const MAX_BOOK_CHARS: usize = 40_000_000;
/// Seconds one book's reading may take; past it the book is recorded as failed and the run goes on
pub const EXTRACT_TIMEOUT_SECS: u64 = 300;
/// Sections kept of one book
pub const MAX_SECTIONS: usize = 20_000;

#[derive(Debug, Error)]
pub enum ContentError {
	#[error("{0}")]
	Db(#[from] DbErr),
	/// The file could not be read as a book (the reason is kept for the status page)
	#[error("{0}")]
	Extract(String),
	/// The request is not one the index answers (a pattern that is too big, ...)
	#[error("{0}")]
	Invalid(String),
	/// The book's file is not there to be read (a section's text comes from it)
	#[error("{0}")]
	Unavailable(String),
}

pub type ContentResult<T> = Result<T, ContentError>;

/// What kind of book a file is, by its extension: only these have text to index
pub fn kind_of(extension: &str, path: &str) -> Option<&'static str> {
	let from = |ext: &str| match ext.to_ascii_lowercase().as_str() {
		"epub" => Some("epub"),
		"pdf" => Some("pdf"),
		_ => None,
	};
	from(extension).or_else(|| {
		std::path::Path::new(path)
			.extension()
			.and_then(|ext| ext.to_str())
			.and_then(from)
	})
}

/// The text folded for matching: lowercase, without accents (the same folding as the PDF reader's
/// search)
pub fn fold_str(text: &str) -> String {
	let mut out = String::with_capacity(text.len());
	for ch in text.chars() {
		fold_char(ch, &mut out);
	}
	out
}

/// The text folded, as chars, and for each folded char the index (in chars) of the char of `text` it
/// came from
pub fn fold_mapped(text: &str) -> (Vec<char>, Vec<usize>) {
	let mut folded = Vec::with_capacity(text.len());
	let mut origin = Vec::with_capacity(text.len());
	let mut buffer = String::new();
	for (index, ch) in text.chars().enumerate() {
		buffer.clear();
		fold_char(ch, &mut buffer);
		for folded_char in buffer.chars() {
			folded.push(folded_char);
			origin.push(index);
		}
	}
	(folded, origin)
}

/// Cuts `text` to at most `max` chars
pub fn truncate_chars(text: &mut String, max: usize) {
	if let Some((byte, _)) = text.char_indices().nth(max) {
		text.truncate(byte);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn kinds_come_from_the_extension() {
		assert_eq!(kind_of("epub", "/a/b.epub"), Some("epub"));
		assert_eq!(kind_of("PDF", "/a/b"), Some("pdf"));
		assert_eq!(kind_of("", "/a/b.Epub"), Some("epub"));
		assert_eq!(kind_of("cbz", "/a/b.cbz"), None);
	}

	#[test]
	fn folding_drops_case_and_accents_and_maps_back() {
		assert_eq!(fold_str("Canción ÁRBOL ñandú"), "cancion arbol nandu");
		let (folded, origin) = fold_mapped("ﬁn Él");
		assert_eq!(folded.iter().collect::<String>(), "fin el");
		assert_eq!(origin[0], 0);
		assert_eq!(origin[1], 0);
		assert_eq!(origin[2], 1);
		assert_eq!(*origin.last().unwrap(), 4);
	}

	#[test]
	fn truncation_counts_chars_not_bytes() {
		let mut text = "ñandú".to_string();
		truncate_chars(&mut text, 3);
		assert_eq!(text, "ñan");
		truncate_chars(&mut text, 10);
		assert_eq!(text, "ñan");
	}
}
