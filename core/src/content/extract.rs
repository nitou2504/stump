//! Reads a book's text, section by section

use std::path::Path;

use epub::doc::NavPoint;

use super::{
	truncate_chars, ContentError, ContentResult, MAX_BOOK_CHARS, MAX_SECTIONS,
	MAX_SECTION_CHARS,
};
use crate::{
	config::StumpConfig,
	media::processor::epub::open_epub,
	pdf_reader::{PageText, PdfReaderService, Priority},
	readium::search::extract_searchable_text,
};

/// Largest spine item read (bigger ones are left out, as the in-book search does)
const MAX_ITEM_BYTES: usize = 16 * 1024 * 1024;
/// Pages of a PDF read
const MAX_PDF_PAGES: i32 = 6000;
/// A PDF whose text layer has fewer chars than this in all has no text
const MIN_PDF_CHARS: usize = 20;

/// One section of a book
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
	pub title: String,
	pub level: i32,
	pub text: String,
	/// The PDF page it starts at
	pub page_start: Option<i32>,
	/// For a PDF section of several pages: `(char offset in the text, page label)` of each page
	pub page_marks: Vec<(usize, String)>,
	/// Where in the file the text came from (`e:<spine index>` for an EPUB, `p:<first>-<last>` for a
	/// PDF), so the text can be read from the file again: the index does not keep it
	pub source: Option<String>,
}

/// Where in a book a section's text is read from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRef {
	/// The EPUB's reading-order item
	Spine(usize),
	/// The PDF's pages, first to last (1-based)
	Pages(i32, i32),
}

impl SourceRef {
	pub fn encode(self) -> String {
		match self {
			Self::Spine(index) => format!("e:{index}"),
			Self::Pages(first, last) => format!("p:{first}-{last}"),
		}
	}

	pub fn parse(raw: &str) -> Option<Self> {
		if let Some(index) = raw.strip_prefix("e:") {
			return index.parse().ok().map(Self::Spine);
		}
		let (first, last) = raw.strip_prefix("p:")?.split_once('-')?;
		Some(Self::Pages(first.parse().ok()?, last.parse().ok()?))
	}
}

/// What reading a book found: its sections, none when the book has no text
#[derive(Debug, Clone, Default)]
pub struct Extracted {
	pub sections: Vec<Section>,
}

/// An EPUB's sections: the reading order's documents with text, titled from the table of contents
/// (blocking: run it on a thread of its own)
pub fn extract_epub(path: &str) -> ContentResult<Extracted> {
	let mut epub = open_epub(path).map_err(|e| ContentError::Extract(e.to_string()))?;
	let toc = flatten_toc(&epub.toc);
	let mut sections = Vec::new();
	let mut total = 0usize;
	for index in 0..epub.get_num_chapters() {
		if sections.len() >= MAX_SECTIONS || total >= MAX_BOOK_CHARS {
			break;
		}
		let Some((text, package_path)) = epub_chapter_text(&mut epub, index) else {
			continue;
		};
		total += text.chars().count();
		let (title, level) = toc_title(&toc, &package_path)
			.unwrap_or_else(|| (format!("Section {}", sections.len() + 1), 1));
		sections.push(Section {
			title,
			level,
			text,
			page_start: None,
			page_marks: Vec::new(),
			source: Some(SourceRef::Spine(index).encode()),
		});
	}
	Ok(Extracted { sections })
}

type OpenEpub = epub::doc::EpubDoc<std::io::BufReader<std::fs::File>>;

/// The text of the reading-order item `index` and its path in the package, `None` when it is not a
/// section (not a page of text, too big, empty). The ONE place that decides it, for the reading of
/// a whole book and for reading one section again: both give the same text.
fn epub_chapter_text(epub: &mut OpenEpub, index: usize) -> Option<(String, String)> {
	epub.set_current_chapter(index);
	let item = epub.spine.get(index)?;
	let idref = item.idref.clone();
	let resource = epub.resources.get(&idref)?;
	let package_path = resource.path.to_string_lossy().to_string();
	let mime = epub.get_current_mime().unwrap_or_default().to_ascii_lowercase();
	if !(mime.contains("xhtml") || mime == "text/html" || mime == "application/xml" || mime == "text/xml")
	{
		return None;
	}
	let (bytes, _) = epub.get_current()?;
	if bytes.is_empty() || bytes.len() > MAX_ITEM_BYTES {
		return None;
	}
	let mut text = extract_searchable_text(&bytes);
	if text.is_empty() {
		return None;
	}
	truncate_chars(&mut text, MAX_SECTION_CHARS);
	Some((text, package_path))
}

/// The text of the EPUB's reading-order items `spines`, read from the file again (blocking: run it
/// on a thread of its own). The book is opened once. An item that is no longer a section gives no
/// entry.
pub fn read_epub_items(
	path: &str,
	spines: &[usize],
) -> ContentResult<std::collections::HashMap<usize, String>> {
	let mut epub = open_epub(path).map_err(|e| ContentError::Extract(e.to_string()))?;
	let mut out = std::collections::HashMap::with_capacity(spines.len());
	for &index in spines {
		if index < epub.get_num_chapters() {
			if let Some((text, _)) = epub_chapter_text(&mut epub, index) {
				out.insert(index, text);
			}
		}
	}
	Ok(out)
}

/// The table of contents as `(path without fragment, label, depth)`
fn flatten_toc(toc: &[NavPoint]) -> Vec<(String, String, i32)> {
	let mut out = Vec::new();
	let mut pending: Vec<(&NavPoint, i32)> = toc.iter().rev().map(|n| (n, 1)).collect();
	while let Some((nav, depth)) = pending.pop() {
		let content = nav.content.to_string_lossy();
		let path = content.split('#').next().unwrap_or("").to_string();
		out.push((path, nav.label.trim().to_string(), depth));
		pending.extend(nav.children.iter().rev().map(|c| (c, depth + 1)));
	}
	out
}

fn toc_title(toc: &[(String, String, i32)], package_path: &str) -> Option<(String, i32)> {
	let wanted = package_path.trim_start_matches('/');
	toc.iter()
		.find(|(path, label, _)| {
			!label.is_empty() && path.trim_start_matches('/') == wanted
		})
		.map(|(_, label, depth)| (label.clone(), *depth))
}

/// One page's text: its runs joined by spaces, a hyphen at a line end joining the two halves
pub fn page_plain_text(page: &PageText) -> String {
	let mut out = String::new();
	for run in &page.runs {
		let chars: Vec<char> = run.text.chars().collect();
		let joins = run.eol
			&& chars.len() >= 2
			&& chars[chars.len() - 1] == '-'
			&& chars[chars.len() - 2].is_alphabetic();
		let usable = if joins { chars.len() - 1 } else { chars.len() };
		out.extend(chars.iter().take(usable));
		if !joins {
			out.push(' ');
		}
	}
	collapse_whitespace(&out)
}

fn collapse_whitespace(input: &str) -> String {
	let mut out = String::with_capacity(input.len());
	let mut last_space = true;
	for ch in input.chars() {
		if ch.is_whitespace() {
			if !last_space {
				out.push(' ');
				last_space = true;
			}
		} else {
			out.push(ch);
			last_space = false;
		}
	}
	out.trim_end().to_string()
}

/// A PDF section's text from its pages' texts (in page order): the non-empty ones joined by a
/// space and cut at [`MAX_SECTION_CHARS`], with each page's `(char offset, label)`. The ONE place
/// that builds it, for reading a whole book and for reading one section again.
fn join_pages<'a>(
	pages: impl Iterator<Item = (i32, &'a str)>,
	label: &dyn Fn(i32) -> String,
) -> (String, Vec<(usize, String)>) {
	let mut text = String::new();
	let mut marks = Vec::new();
	for (page, page_text) in pages {
		if page_text.is_empty() {
			continue;
		}
		if !text.is_empty() {
			text.push(' ');
		}
		marks.push((text.chars().count(), label(page)));
		text.push_str(page_text);
	}
	truncate_chars(&mut text, MAX_SECTION_CHARS);
	(text, marks)
}

/// The text of PDF sections, each given as its first and last page, read from the file again
pub async fn read_pdf_ranges(
	config: &StumpConfig,
	path: &Path,
	ranges: &[(i32, i32)],
) -> ContentResult<std::collections::HashMap<(i32, i32), String>> {
	let service = PdfReaderService::global(config);
	let mut out = std::collections::HashMap::with_capacity(ranges.len());
	for &(first, last) in ranges {
		let mut pages: Vec<(i32, String)> = Vec::new();
		for page in first.max(1)..=last.min(MAX_PDF_PAGES) {
			match service.text(path, page, Priority::Background).await {
				Ok(text) => pages.push((page, page_plain_text(&text))),
				Err(crate::pdf_reader::PdfReaderError::PageNotFound) => break,
				Err(error) => return Err(ContentError::Extract(error.to_string())),
			}
		}
		let (text, _) = join_pages(pages.iter().map(|(page, text)| (*page, text.as_str())), &|_| {
			String::new()
		});
		out.insert((first, last), text);
	}
	Ok(out)
}

/// A PDF's sections: its outline's entries (from each entry's page to the next entry's), or one
/// per page when it has none. A PDF with no text layer has no sections.
pub async fn extract_pdf(config: &StumpConfig, path: &Path) -> ContentResult<Extracted> {
	let service = PdfReaderService::global(config);
	let layout = service
		.layout(path)
		.await
		.map_err(|e| ContentError::Extract(e.to_string()))?;
	if !layout.has_text || layout.page_count < 1 {
		return Ok(Extracted::default());
	}
	let pages = layout.page_count.min(MAX_PDF_PAGES);
	let mut texts: Vec<String> = Vec::with_capacity(pages as usize);
	let mut total = 0usize;
	for page in 1..=pages {
		let text = match service.text(path, page, Priority::Background).await {
			Ok(text) => page_plain_text(&text),
			Err(crate::pdf_reader::PdfReaderError::PageNotFound) => break,
			Err(error) => return Err(ContentError::Extract(error.to_string())),
		};
		total += text.chars().count();
		texts.push(text);
		if total >= MAX_BOOK_CHARS {
			break;
		}
	}
	if total < MIN_PDF_CHARS {
		return Ok(Extracted::default());
	}
	let label = |page: i32| {
		layout
			.labels
			.as_ref()
			.and_then(|labels| labels.get(page as usize - 1))
			.filter(|l| !l.is_empty())
			.cloned()
			.unwrap_or_else(|| page.to_string())
	};

	// outline entries with a page, in page order, one per page
	let mut entries: Vec<(i32, String, i32)> = Vec::new();
	let mut pending: Vec<(&crate::pdf_reader::OutlineEntry, i32)> =
		layout.outline.iter().rev().map(|e| (e, 1)).collect();
	while let Some((entry, depth)) = pending.pop() {
		if let Some(page) = entry.page.filter(|p| *p >= 1 && *p <= texts.len() as i32) {
			entries.push((page, entry.title.trim().to_string(), depth));
		}
		pending.extend(entry.children.iter().rev().map(|c| (c, depth + 1)));
	}
	entries.sort_by_key(|(page, _, _)| *page);
	entries.dedup_by_key(|(page, _, _)| *page);

	let build = |first: i32, last: i32, title: String, level: i32| {
		let (text, marks) = join_pages(
			(first..=last).map(|page| (page, texts[page as usize - 1].as_str())),
			&label,
		);
		Section {
			title,
			level,
			text,
			page_start: Some(first),
			page_marks: marks,
			source: Some(SourceRef::Pages(first, last).encode()),
		}
	};

	let mut sections = Vec::new();
	if entries.is_empty() {
		for page in 1..=texts.len() as i32 {
			if sections.len() >= MAX_SECTIONS {
				break;
			}
			let section = build(page, page, format!("Page {}", label(page)), 1);
			if !section.text.is_empty() {
				sections.push(section);
			}
		}
	} else {
		let last_page = texts.len() as i32;
		if entries[0].0 > 1 {
			let section = build(1, entries[0].0 - 1, "Start".to_string(), 1);
			if !section.text.is_empty() {
				sections.push(section);
			}
		}
		for (i, (page, title, level)) in entries.iter().enumerate() {
			if sections.len() >= MAX_SECTIONS {
				break;
			}
			let end = entries.get(i + 1).map_or(last_page, |(next, _, _)| next - 1);
			let title = if title.is_empty() {
				format!("Page {}", label(*page))
			} else {
				title.clone()
			};
			let section = build(*page, end, title, *level);
			if !section.text.is_empty() {
				sections.push(section);
			}
		}
	}
	Ok(Extracted { sections })
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::pdf_reader::TextRun;

	fn run(text: &str, eol: bool) -> TextRun {
		TextRun {
			bounds: [0.0; 4],
			text: text.to_string(),
			first_char: 0,
			eol,
			char_spans: vec![],
		}
	}

	#[test]
	fn pdf_page_text_joins_runs_and_hyphenated_line_ends() {
		let page = PageText {
			width: 1.0,
			height: 1.0,
			runs: vec![run("An exam-", true), run("ple of   text", true)],
		};
		assert_eq!(page_plain_text(&page), "An example of text");
	}

	#[test]
	fn the_toc_titles_a_spine_item_by_its_path() {
		let toc = vec![
			("OEBPS/c1.xhtml".to_string(), "One".to_string(), 1),
			("OEBPS/c1.xhtml".to_string(), "One, part".to_string(), 2),
			("OEBPS/c2.xhtml".to_string(), "Two".to_string(), 2),
		];
		assert_eq!(
			toc_title(&toc, "OEBPS/c2.xhtml"),
			Some(("Two".to_string(), 2))
		);
		assert_eq!(toc_title(&toc, "OEBPS/c9.xhtml"), None);
	}

	#[test]
	fn an_epub_gives_sections_with_text() {
		let path = concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/integration-tests/data/book.epub"
		);
		let extracted = extract_epub(path).expect("should read the epub");
		assert!(!extracted.sections.is_empty());
		assert!(extracted.sections.iter().all(|s| !s.text.is_empty()));
		assert!(extracted.sections.iter().all(|s| !s.text.contains('<')));
	}

	#[test]
	fn a_section_read_again_is_the_text_that_was_indexed() {
		let path = concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/integration-tests/data/book.epub"
		);
		let extracted = extract_epub(path).expect("should read the epub");
		let spines: Vec<usize> = extracted
			.sections
			.iter()
			.map(|s| match SourceRef::parse(s.source.as_deref().expect("a source")) {
				Some(SourceRef::Spine(index)) => index,
				other => panic!("an EPUB's source is a spine item, not {other:?}"),
			})
			.collect();
		assert!(spines.windows(2).all(|w| w[0] < w[1]), "in reading order");
		// all at once (the book opened once), and one at a time
		let again = read_epub_items(path, &spines).expect("should read them again");
		for (section, spine) in extracted.sections.iter().zip(&spines) {
			assert_eq!(again.get(spine), Some(&section.text), "section at item {spine}");
			let alone = read_epub_items(path, &[*spine]).unwrap();
			assert_eq!(alone.get(spine), Some(&section.text));
		}
		// an item that is not a section, or not there, gives nothing
		let missing = read_epub_items(path, &[100_000]).unwrap();
		assert!(missing.is_empty());
	}

	#[test]
	fn pdf_sections_join_their_pages_the_same_way_whole_or_alone() {
		let pages = [(3, "uno"), (4, ""), (5, "dos tres")];
		let (text, marks) = join_pages(pages.iter().copied(), &|p| format!("p{p}"));
		assert_eq!(text, "uno dos tres");
		assert_eq!(marks, vec![(0, "p3".to_string()), (4, "p5".to_string())]);
		let (alone, _) = join_pages(pages.iter().copied(), &|_| String::new());
		assert_eq!(alone, text);
	}
}
