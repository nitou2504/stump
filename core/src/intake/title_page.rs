//! What a book's own title page says it is, for the online lookups when its metadata looks
//! like junk: a title that is a tool's or a file's name (`OUP Book`, `Subject:`,
//! `Microsoft Word - draft.doc`, empty), a creator that is a program or an account
//! (`pdfTeX`, `Administrator`), or a title and an author that were swapped. An EPUB's first
//! pages are read (an ePubLibre title page, a Library of Congress CIP block, or the title page's
//! first lines) and only searched with: nothing is written back, and an online record still
//! has to agree with what was searched before it is used.

use std::{
	io::Read,
	path::{Path, PathBuf},
	sync::LazyLock,
};

use regex::Regex;

use super::{
	lookup::LookupQuery,
	naming::{display_name, looks_like_person, parse_file_name, split_authors},
};

/// What looks wrong in a book's title and authors
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JunkMetadata {
	/// No title, or a tool's or a file's name
	pub title: bool,
	/// No author, or a program's or an account's name
	pub authors: bool,
	/// The title is a person's name and the author a title
	pub swapped: bool,
}

impl JunkMetadata {
	pub fn any(&self) -> bool {
		self.title || self.authors || self.swapped
	}
}

static JUNK_TITLE: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?i)^(?:oup book|e-?book|book\s?\d*|untitled|unknown|document\s?\d*|microsoft word.*|subject:?|title:?|creator:?|.*\.(?:pdf|docx?|indd|qxd|tex|rtf|epub)|\d{5,}|cover|portada|sin t[ií]tulo|t[ií]tulo)$",
	)
	.expect("valid regex")
});

static JUNK_AUTHOR: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?i)^(?:creator:.*|.*latex.*|.*pdftex.*|.*hyperref.*|.*acrobat.*|adobe.*|microsoft.*|calibre.*|administrator|admin|user|owner|unknown|author|autor|desconocido|[a-z]{1,2}|[a-z]+\d+)$",
	)
	.expect("valid regex")
});

/// Whether a book's title and authors look like junk (see the module's documentation)
pub fn junk_metadata(title: Option<&str>, authors: &[String]) -> JunkMetadata {
	let title = title.map(str::trim).unwrap_or_default();
	let authors: Vec<&str> = authors
		.iter()
		.map(|a| a.trim())
		.filter(|a| !a.is_empty())
		.collect();
	let joined = authors.join(" ");
	JunkMetadata {
		title: title.is_empty() || JUNK_TITLE.is_match(title),
		authors: authors.is_empty() || authors.iter().any(|a| JUNK_AUTHOR.is_match(a)),
		swapped: !title.is_empty()
			&& looks_like_person(title)
			&& (joined.split_whitespace().count() >= 5 || joined.contains(':')),
	}
}

/// The title and authors a title page gives
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TitlePage {
	pub title: Option<String>,
	pub authors: Vec<String>,
}

/// Pages that name other books (`Also by`, `Praise for`): their lines are no title
static OTHER_BOOKS: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?i)\b(?:also by|other (?:books|titles|works) by|by the same author|books by|praise for|del mismo autor|otros (?:libros|t[ií]tulos) de|otras obras de)\b",
	)
	.expect("valid regex")
});

/// Pages a converter or a library adds (`This book was produced in EPUB format by the Internet
/// Archive`): nothing on them is the book's
static NOTICE: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?i)(?:internet archive|hocr-to-epub|project gutenberg|this (?:e-?book|book|file) was (?:produced|created|converted)|converted to epub)",
	)
	.expect("valid regex")
});

/// Navigation lines of the first pages
static NAV: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?i)^(?:title page|begin reading|table of contents|contents|cover|copyright|dedication|epigraph|front matter|portada|cubierta|[ií]ndice|t[ií]tulo|cr[eé]ditos|sinopsis)$",
	)
	.expect("valid regex")
});

/// Lines of the first pages that are no title or author
static NOISE: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?i)(?:about the (?:author|pagination)|thank you for|www\.|https?:|^\d+$|^page [ivx\d]+$|\.qxd|^\d{1,2}/\d{1,2}/\d{2})",
	)
	.expect("valid regex")
});

/// Where the title page ends: the copyright page
static COPYRIGHT: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"(?i)(?:copyright|©|\bisbn\b|all rights|published by|first published|derechos reservados)")
		.expect("valid regex")
});

/// `by Richard P. Gabriel`, `Edited by ...`, `por Gabriel García Márquez`
static BY: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"(?i)^(?:(?:written|edited|translated|selected|illustrated|introduced)\s+)?(?:by|por)\s+(.+)$")
		.expect("valid regex")
});

/// Title page lines about the edition, not a person: `Second Edition`, `Volume 2`
static EDITION: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?i)\b(?:edition|edici[oó]n|volume|vol\.|tomo|series|serie|book \w+|libro \w+)\b",
	)
	.expect("valid regex")
});

/// Publishers' and places' lines on a title page
static PUBLISHER: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"(?i)\b(?:press|publish|books|editorial|ediciones|university|universidad|new york|london|oxford)\b")
		.expect("valid regex")
});

/// ePubLibre's title page: Author / Title / [subtitle] / [Series - N] / ePub r1.2
static EPUBLIBRE: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"(?i)^epub r\d").expect("valid regex"));

/// A CIP block (new style): `Names: Gabriel, Richard P., author.` / `Title: Patterns of
/// software : tales ... / Richard P. Gabriel.`
static CIP_NAMES: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"(?m)^Names?:\s*(?P<a>[^\n]+)\n(?:[^\n]*\n)?Title:\s*(?P<t>[^\n]+)")
		.expect("valid regex")
});

/// A CIP block (old style): `Library of Congress Cataloging-in-Publication Data` / author /
/// title
static CIP_OLD: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?i)cataloging[- ]in[- ]publication data\s*\n(?P<a>[^\n]+)\n(?P<t>[^\n]+)",
	)
	.expect("valid regex")
});

/// `PATTERNS OF SOFTWARE` -> `Patterns Of Software`; other text unchanged
fn norm_case(text: &str) -> String {
	let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
	if text.chars().any(char::is_lowercase) || !text.chars().any(char::is_alphabetic) {
		return text;
	}
	text.split(' ')
		.map(|word| {
			let mut chars = word.chars();
			match chars.next() {
				Some(first) => {
					first.to_uppercase().collect::<String>()
						+ &chars.as_str().to_lowercase()
				},
				None => String::new(),
			}
		})
		.collect::<Vec<_>>()
		.join(" ")
}

/// A CIP name, `Gabriel, Richard P., 1949- author.` -> `Richard P. Gabriel`
fn cip_name(raw: &str) -> String {
	static DATES: LazyLock<Regex> = LazyLock::new(|| {
		Regex::new(
			r"(?i),?\s*(?:\d{4}\s*[-–]\s*(?:\d{4})?|author|editor|translator)[.,\s]*.*$",
		)
		.expect("valid regex")
	});
	let name = DATES.replace(raw.trim(), "");
	let mut name = name.trim_matches([' ', ',']).to_string();
	// a closing full stop, not an initial's (`Richard P.`)
	if name.ends_with('.')
		&& name
			.split_whitespace()
			.last()
			.is_some_and(|w| w.chars().count() > 2)
	{
		name.pop();
	}
	display_name(&name)
}

/// A CIP title, `Patterns of software : tales from the software community / Richard P.
/// Gabriel.` -> `Patterns of software`
fn cip_title(raw: &str) -> Option<String> {
	let title = raw.split(" / ").next().unwrap_or(raw);
	let title = title.split(" : ").next().unwrap_or(title);
	let title = title.trim().trim_end_matches(['.', ' ']).trim();
	let mut chars = title.chars();
	let first = chars.next()?;
	Some(first.to_uppercase().collect::<String>() + chars.as_str())
}

/// The title and authors of a book's first pages, one page's lines per item (a page that
/// names other books, `Also by`, is skipped)
pub fn parse_title_page(pages: &[Vec<String>]) -> TitlePage {
	let lines: Vec<String> = pages
		.iter()
		.filter(|page| {
			!page
				.iter()
				.take(12)
				.any(|l| OTHER_BOOKS.is_match(l) || NOTICE.is_match(l))
		})
		.flatten()
		.map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
		.filter(|l| !l.is_empty() && !NAV.is_match(l))
		.take(400)
		.collect();

	// ePubLibre: Author / Title / ... / ePub r1.2
	if let Some(k) = lines.iter().take(12).position(|l| EPUBLIBRE.is_match(l)) {
		if k >= 2 {
			return TitlePage {
				title: Some(lines[1].clone()),
				authors: split_authors(&lines[0]),
			};
		}
	}

	let text = lines.join("\n");
	for cip in [&*CIP_NAMES, &*CIP_OLD] {
		if let Some(caps) = cip.captures(&text) {
			let author = cip_name(&caps["a"]);
			if let Some(title) = cip_title(&caps["t"]).filter(|_| !author.is_empty()) {
				return TitlePage {
					title: Some(title),
					authors: vec![author],
				};
			}
		}
	}

	// the title page: the lines before the copyright page. Its first line is the title; a
	// `by X` line names the authors, else the first name after the title does
	let stop = lines
		.iter()
		.take(80)
		.position(|l| COPYRIGHT.is_match(l))
		.unwrap_or(40)
		.min(40)
		.min(lines.len());
	let head: Vec<&String> = lines[..stop]
		.iter()
		.filter(|l| !NOISE.is_match(l))
		.take(15)
		.collect();
	let mut page = TitlePage::default();
	let by_line = head.iter().position(|l| BY.is_match(l));
	if let Some(caps) = by_line.and_then(|i| BY.captures(head[i])) {
		let names = norm_case(&caps[1]);
		if names.split_whitespace().count() <= 8 {
			page.authors = split_authors(&names);
		}
	}
	// a title is short and no sentence (a blurb's line, a notice)
	let title_like = |line: &str| {
		let words = line.split_whitespace().count();
		(1..=12).contains(&words)
			&& line.chars().count() <= 100
			&& line.chars().any(char::is_alphabetic)
			&& !(words >= 6 && line.ends_with('.'))
	};
	let title_at = head
		.iter()
		.enumerate()
		.position(|(i, l)| Some(i) != by_line && title_like(l));
	if let Some(at) = title_at {
		page.title = Some(norm_case(head[at]));
		if page.authors.is_empty() {
			page.authors = head[at + 1..]
				.iter()
				.map(|l| norm_case(l))
				.find(|l| {
					looks_like_person(l) && !PUBLISHER.is_match(l) && !EDITION.is_match(l)
				})
				.into_iter()
				.collect();
		}
	}
	page
}

/// HTML text as lines: block ends and line breaks become new lines, tags go, entities are
/// decoded
fn html_lines(html: &str) -> Vec<String> {
	static HEAD: LazyLock<Regex> =
		LazyLock::new(|| Regex::new(r"(?is)<head\b.*?</head>").expect("valid regex"));
	static BREAK: LazyLock<Regex> = LazyLock::new(|| {
		Regex::new(
			r"(?i)<br\s*/?>|</(?:p|div|h[1-6]|li|tr|td|blockquote|section|header)\s*>",
		)
		.expect("valid regex")
	});
	static TAG: LazyLock<Regex> =
		LazyLock::new(|| Regex::new(r"<[^>]*>").expect("valid regex"));
	let body = HEAD.replace(html, "");
	let broken = BREAK.replace_all(&body, "\n");
	let text = TAG.replace_all(&broken, " ");
	decode_entities(&text)
		.lines()
		.map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
		.filter(|l| !l.is_empty())
		.collect()
}

/// `&amp;`, `&nbsp;`, `&#8217;`, `&#x2019;` -> their characters
fn decode_entities(text: &str) -> String {
	static ENTITY: LazyLock<Regex> = LazyLock::new(|| {
		Regex::new(r"&(?:#(\d{1,7})|#[xX]([0-9a-fA-F]{1,6})|([a-zA-Z]{2,8}));")
			.expect("valid regex")
	});
	ENTITY
		.replace_all(text, |caps: &regex::Captures| {
			let code = caps
				.get(1)
				.and_then(|d| d.as_str().parse::<u32>().ok())
				.or_else(|| {
					caps.get(2)
						.and_then(|h| u32::from_str_radix(h.as_str(), 16).ok())
				});
			if let Some(code) = code {
				return char::from_u32(code).map(String::from).unwrap_or_default();
			}
			match caps.get(3).map(|n| n.as_str()) {
				Some("amp") => "&",
				Some("lt") => "<",
				Some("gt") => ">",
				Some("quot") => "\"",
				Some("apos") => "'",
				Some("nbsp") => " ",
				Some("rsquo" | "lsquo") => "'",
				Some("rdquo" | "ldquo") => "\"",
				Some("mdash") => "—",
				Some("ndash") => "–",
				_ => " ",
			}
			.to_string()
		})
		.into_owned()
}

/// The first pages of an EPUB in reading order (at most `max_pages` with text), as lines
fn epub_first_pages(path: &Path, max_pages: usize) -> Option<Vec<Vec<String>>> {
	static ITEM: LazyLock<Regex> = LazyLock::new(|| {
		Regex::new(r"(?is)<(?:\w+:)?item\b[^>]*>").expect("valid regex")
	});
	static ITEMREF: LazyLock<Regex> = LazyLock::new(|| {
		Regex::new(r#"(?is)<(?:\w+:)?itemref\b[^>]*\bidref=["']([^"']+)["']"#)
			.expect("valid regex")
	});
	static ATTR: LazyLock<Regex> = LazyLock::new(|| {
		Regex::new(r#"(?is)\b(id|href|media-type)=["']([^"']*)["']"#)
			.expect("valid regex")
	});
	let mut archive = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
	let opf_name = super::write::opf_path(&mut archive).ok()?;
	let mut opf = String::new();
	archive
		.by_name(&opf_name)
		.ok()?
		.read_to_string(&mut opf)
		.ok()?;
	let base = Path::new(&opf_name)
		.parent()
		.map(Path::to_path_buf)
		.unwrap_or_default();
	let manifest: Vec<(String, String, String)> = ITEM
		.find_iter(&opf)
		.map(|item| {
			let mut id = String::new();
			let mut href = String::new();
			let mut media_type = String::new();
			for caps in ATTR.captures_iter(item.as_str()) {
				match caps[1].to_lowercase().as_str() {
					"id" => id = caps[2].to_string(),
					"href" => href = caps[2].to_string(),
					_ => media_type = caps[2].to_lowercase(),
				}
			}
			(id, href, media_type)
		})
		.collect();
	let mut pages = vec![];
	for caps in ITEMREF.captures_iter(&opf).take(max_pages * 3) {
		if pages.len() >= max_pages {
			break;
		}
		let Some((_, href, media_type)) =
			manifest.iter().find(|(id, _, _)| *id == caps[1])
		else {
			continue;
		};
		if !media_type.contains("html") {
			continue;
		}
		let href = urlencoding::decode(href.split('#').next().unwrap_or(href))
			.map(|h| h.into_owned())
			.unwrap_or_else(|_| href.clone());
		let name = normalize_zip_path(&base.join(href));
		let mut html = String::new();
		let Ok(mut file) = archive.by_name(&name) else {
			continue;
		};
		// a title page is short: the start of a long chapter is enough
		if file
			.by_ref()
			.take(256 * 1024)
			.read_to_string(&mut html)
			.is_err()
		{
			continue;
		}
		let lines = html_lines(&html);
		if !lines.is_empty() {
			pages.push(lines.into_iter().take(150).collect());
		}
	}
	Some(pages)
}

/// `OEBPS/Text/../Images/a.xhtml` -> `OEBPS/Images/a.xhtml`, with `/` separators
fn normalize_zip_path(path: &Path) -> String {
	let mut parts: Vec<String> = vec![];
	for component in path.components() {
		match component {
			std::path::Component::ParentDir => {
				parts.pop();
			},
			std::path::Component::Normal(part) => {
				parts.push(part.to_string_lossy().into_owned())
			},
			_ => {},
		}
	}
	parts.join("/")
}

/// The title page of an EPUB file (its first pages with text)
pub fn read_title_page(path: &Path) -> Option<TitlePage> {
	let pages = epub_first_pages(path, 6)?;
	let page = parse_title_page(&pages);
	(page.title.is_some() || !page.authors.is_empty()).then_some(page)
}

/// Puts what a title page says into a query whose metadata looks like junk: its title for a
/// junk title, its authors for junk authors (else the junk authors are left out of the
/// search), both when the title and author were swapped (else they are swapped back). Whether
/// the query changed
pub fn apply_title_page(query: &mut LookupQuery, page: Option<&TitlePage>) -> bool {
	let junk = junk_metadata(query.title.as_deref(), &query.authors);
	if !junk.any() {
		return false;
	}
	let page_title = page
		.and_then(|p| p.title.clone())
		.filter(|t| !t.trim().is_empty());
	let page_authors = page.map(|p| p.authors.clone()).unwrap_or_default();
	if junk.swapped {
		let name = query.title.take().unwrap_or_default();
		let title = query.authors.join(" ");
		query.title = Some(page_title.unwrap_or(title));
		query.authors = if page_authors.is_empty() {
			vec![name]
		} else {
			page_authors
		};
		return true;
	}
	let mut changed = false;
	if junk.title {
		if let Some(title) = page_title {
			query.title = Some(title);
			changed = true;
		}
	}
	if junk.authors {
		if !page_authors.is_empty() {
			query.authors = page_authors;
			changed = true;
		} else if !query.authors.is_empty() {
			query.authors.clear();
			changed = true;
		}
	}
	changed
}

/// [`apply_title_page`] for a book file: the title page is read only when the metadata looks
/// like junk, and only from an EPUB
pub async fn repair_for_lookup(
	mut query: LookupQuery,
	path: &Path,
) -> (LookupQuery, bool) {
	if !junk_metadata(query.title.as_deref(), &query.authors).any() {
		return (query, false);
	}
	let is_epub = path
		.extension()
		.is_some_and(|e| e.eq_ignore_ascii_case("epub"));
	let page = if is_epub {
		let path: PathBuf = path.to_path_buf();
		tokio::task::spawn_blocking(move || read_title_page(&path))
			.await
			.ok()
			.flatten()
	} else {
		None
	};
	let mut changed = apply_title_page(&mut query, page.as_ref());
	// still no usable title: the file's name (`Is_Anyone_There.epub`), as *Add books* reads one
	if junk_metadata(query.title.as_deref(), &[]).title {
		let title = path
			.file_stem()
			.and_then(|stem| stem.to_str())
			.map(|stem| stem.trim_end_matches(".kepub"))
			.and_then(|stem| parse_file_name(stem).title)
			.filter(|title| !JUNK_TITLE.is_match(title));
		if let Some(title) = title {
			query.title = Some(title);
			changed = true;
		}
	}
	(query, changed)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn lines(text: &str) -> Vec<String> {
		text.lines().map(str::to_string).collect()
	}

	#[test]
	fn tells_junk_metadata() {
		let gabriel = vec!["Richard P. Gabriel".to_string()];
		assert!(!junk_metadata(Some("Patterns of Software"), &gabriel).any());
		assert!(junk_metadata(Some("OUP Book"), &gabriel).title);
		assert!(junk_metadata(Some("Subject:"), &gabriel).title);
		assert!(junk_metadata(Some("Microsoft Word - draft.doc"), &gabriel).title);
		assert!(junk_metadata(None, &gabriel).title);
		assert!(
			junk_metadata(
				Some("Patterns of Software"),
				&["pdfTeX-1.40.10".to_string()]
			)
			.authors
		);
		assert!(
			junk_metadata(Some("Patterns of Software"), &["Administrator".to_string()])
				.authors
		);
		assert!(junk_metadata(Some("Patterns of Software"), &[]).authors);
		// a title that is a name, an author that is a title
		let swapped = junk_metadata(
			Some("Robert Pantano"),
			&["Notes from the End of Everything".to_string()],
		);
		assert!(swapped.swapped);
		// real titles and authors
		assert!(
			!junk_metadata(Some("Brave New World"), &["Aldous Huxley".to_string()]).any()
		);
		assert!(!junk_metadata(Some("Beowulf"), &["Anonymous".to_string()]).any());
		assert!(
			!junk_metadata(Some("The Notebook"), &["Nicholas Sparks".to_string()]).any()
		);
		assert!(!junk_metadata(Some("1984"), &["George Orwell".to_string()]).any());
		assert!(
			junk_metadata(Some("9780195121230"), &["George Orwell".to_string()]).title
		);
	}

	#[test]
	fn reads_an_epublibre_title_page() {
		let page = parse_title_page(&[lines(
			"Umberto Eco\nEl nombre de la rosa\nePub r1.4\nTitivillus 27.05.2020",
		)]);
		assert_eq!(page.title.as_deref(), Some("El nombre de la rosa"));
		assert_eq!(page.authors, vec!["Umberto Eco"]);
	}

	#[test]
	fn reads_a_cip_block() {
		let page = parse_title_page(&[lines(
			"Copyright © 1996 by Oxford University Press\nLibrary of Congress Cataloging-in-Publication Data\nGabriel, Richard P.\nPatterns of software : tales from the software community / Richard P. Gabriel.\np. cm.",
		)]);
		assert_eq!(page.title.as_deref(), Some("Patterns of software"));
		assert_eq!(page.authors, vec!["Richard P. Gabriel"]);
		let page = parse_title_page(&[lines(
			"Names: Gabriel, Richard P., 1949- author.\nTitle: Patterns of software : tales from the software community / Richard P. Gabriel.\nDescription: New York : Oxford University Press, 1996.",
		)]);
		assert_eq!(page.title.as_deref(), Some("Patterns of software"));
		assert_eq!(page.authors, vec!["Richard P. Gabriel"]);
	}

	#[test]
	fn reads_the_first_lines_of_a_title_page() {
		let page = parse_title_page(&[
			// a page naming other books is no title page
			lines("Also by Richard P. Gabriel\nPerformance and Evaluation of Lisp Systems"),
			lines(
				"PATTERNS OF SOFTWARE\nTales from the Software Community\nRichard P. Gabriel\nNew York Oxford\nOxford University Press\n1996",
			),
			lines("Copyright © 1996 by Richard P. Gabriel"),
		]);
		assert_eq!(page.title.as_deref(), Some("Patterns Of Software"));
		assert_eq!(page.authors, vec!["Richard P. Gabriel"]);
		let page = parse_title_page(&[lines(
			"The Joy of Clojure\nSecond Edition\nby Michael Fogus and Chris Houser\nManning\nShelter Island",
		)]);
		assert_eq!(page.title.as_deref(), Some("The Joy of Clojure"));
		assert_eq!(page.authors, vec!["Michael Fogus", "Chris Houser"]);
	}

	#[test]
	fn skips_notices_and_sentences() {
		let page = parse_title_page(&[
			lines("This book was produced in EPUB format by the Internet Archive.\nThe book pages were scanned and converted to EPUB format automatically."),
			lines("Isaac Asimov calls this unusual collection speculative essays on the known and unknown, spanning a wide field."),
			lines("Is Anyone There?\nIsaac Asimov\nAce Books"),
		]);
		assert_eq!(page.title.as_deref(), Some("Is Anyone There?"));
		assert_eq!(page.authors, vec!["Isaac Asimov"]);
	}

	#[tokio::test]
	async fn falls_back_to_the_file_name_for_a_missing_title() {
		let query = LookupQuery::default();
		let (query, changed) =
			repair_for_lookup(query, Path::new("/lib/Isaac_Asimov/Is_Anyone_There.pdf"))
				.await;
		assert!(changed);
		assert_eq!(query.title.as_deref(), Some("Is Anyone There"));
		// a good title stays
		let query = LookupQuery {
			title: Some("Is Anyone There?".to_string()),
			authors: vec!["Isaac Asimov".to_string()],
			..Default::default()
		};
		let (query, changed) =
			repair_for_lookup(query, Path::new("/lib/Isaac_Asimov/Other_Name.pdf")).await;
		assert!(!changed);
		assert_eq!(query.title.as_deref(), Some("Is Anyone There?"));
	}

	#[test]
	fn reads_html_as_lines() {
		let html = r#"<html><head><title>Cover</title></head><body><h1 class="t">Patterns&#160;of Software</h1><p>Tales from the <i>Software</i> Community</p><p>Richard&nbsp;P. Gabriel<br/>Oxford</p></body></html>"#;
		assert_eq!(
			html_lines(html),
			vec![
				"Patterns of Software",
				"Tales from the Software Community",
				"Richard P. Gabriel",
				"Oxford"
			]
		);
		assert_eq!(
			normalize_zip_path(Path::new("OEBPS/Text/../Images/a.xhtml")),
			"OEBPS/Images/a.xhtml"
		);
	}

	#[test]
	fn repairs_only_junk_queries() {
		let page = TitlePage {
			title: Some("Patterns of Software".to_string()),
			authors: vec!["Richard P. Gabriel".to_string()],
		};
		let mut query = LookupQuery {
			title: Some("OUP Book".to_string()),
			authors: vec!["Richard P. Gabriel".to_string()],
			..Default::default()
		};
		assert!(apply_title_page(&mut query, Some(&page)));
		assert_eq!(query.title.as_deref(), Some("Patterns of Software"));
		assert_eq!(query.authors, vec!["Richard P. Gabriel"]);

		// good metadata is left alone
		let mut query = LookupQuery {
			title: Some("Brave New World".to_string()),
			authors: vec!["Aldous Huxley".to_string()],
			..Default::default()
		};
		assert!(!apply_title_page(&mut query, Some(&page)));
		assert_eq!(query.title.as_deref(), Some("Brave New World"));

		// junk authors without a title page: searched by the title alone
		let mut query = LookupQuery {
			title: Some("Patterns of Software".to_string()),
			authors: vec!["pdfTeX-1.40.10".to_string()],
			..Default::default()
		};
		assert!(apply_title_page(&mut query, None));
		assert!(query.authors.is_empty());

		// swapped, without a title page: swapped back
		let mut query = LookupQuery {
			title: Some("Robert Pantano".to_string()),
			authors: vec!["Notes from the End of Everything".to_string()],
			..Default::default()
		};
		assert!(apply_title_page(&mut query, None));
		assert_eq!(
			query.title.as_deref(),
			Some("Notes from the End of Everything")
		);
		assert_eq!(query.authors, vec!["Robert Pantano"]);
	}
}
