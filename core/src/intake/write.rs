//! Writes the reviewed metadata into the book files, with the conventions Stump reads back,
//! so the library stays readable by KOReader and other readers and a rescan finds the same
//! values:
//!
//! - EPUB: `dc:title`, `dc:creator` (with file-as), `dc:subject` = the tags plus
//!   `bcurator:shelf` (the first big tag), `calibre:series` / `calibre:series_index` (and an
//!   EPUB 3 collection), `dc:language`, a missing `dc:description` / `dc:date`, and
//!   `typicalAgeRange`. The OPF is edited in place as text, must still parse, and the zip
//!   is rewritten with `mimetype` first and stored and every other entry copied unchanged.
//! - CBZ: `ComicInfo.xml` (Series, Number, Title, Writer, Genre, AgeRating, LanguageISO,
//!   Manga), every other entry copied unchanged.
//! - PDF: the Info dictionary (Title, Author, Subject, Keywords with the tags and Stump's
//!   keyword fields `bcurator:series=` / `series_index=` / `age=` / `rating=`, see
//!   [`crate::media::processor::pdf::KEYWORD_FIELD_PREFIX`]), appended as an incremental
//!   update so the rest of the file is untouched.

use std::{
	fs::File,
	io::{Read, Seek, SeekFrom, Write},
	path::{Path, PathBuf},
};

use models::shared::intake::IntakeMetadata;
use regex::Regex;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use super::library_index::is_facet;
use crate::media::processor::pdf::KEYWORD_FIELD_PREFIX;

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
	#[error("{0}")]
	Io(#[from] std::io::Error),
	#[error("{0}")]
	Zip(#[from] zip::result::ZipError),
	#[error("{0}")]
	Invalid(String),
}

pub type WriteResult<T> = Result<T, WriteError>;

/// The values written into a book file, derived from the reviewed metadata
#[derive(Debug, Clone, Default)]
pub struct FileMetadata {
	pub title: Option<String>,
	pub authors: Vec<String>,
	pub series: Option<String>,
	pub series_index: Option<f64>,
	pub language: Option<String>,
	pub year: Option<i32>,
	pub description: Option<String>,
	/// big tags, then small tags, then facets
	pub tags: Vec<String>,
	pub age_rating: Option<i32>,
	pub rating: Option<f64>,
	/// Manga reading direction in ComicInfo
	pub manga: bool,
}

impl FileMetadata {
	pub fn from_metadata(meta: &IntakeMetadata, manga: bool) -> Self {
		Self {
			title: meta.title.clone(),
			authors: meta.authors.clone(),
			series: meta.series.clone(),
			series_index: meta.series_index,
			language: meta.language.clone(),
			year: meta.year,
			description: meta.description.clone(),
			tags: order_tags(&meta.genres),
			age_rating: meta.age_rating,
			rating: meta.rating,
			manga,
		}
	}

	/// The first big tag (`bcurator:shelf`)
	pub fn shelf(&self) -> Option<&str> {
		self.tags
			.iter()
			.find(|t| !t.contains(':') && !is_facet(t))
			.map(String::as_str)
	}
}

/// Big tags, then `Big: Small` tags, then facets, keeping the order within each group
pub fn order_tags(tags: &[String]) -> Vec<String> {
	let mut seen = std::collections::HashSet::new();
	let unique: Vec<&String> = tags.iter().filter(|t| seen.insert(t.as_str())).collect();
	let big = unique.iter().filter(|t| !is_facet(t) && !t.contains(':'));
	let small = unique.iter().filter(|t| !is_facet(t) && t.contains(':'));
	let facets = unique.iter().filter(|t| is_facet(t));
	big.chain(small).chain(facets).map(|t| t.to_string()).collect()
}

pub(crate) fn num(v: f64) -> String {
	if v.fract() == 0.0 {
		format!("{}", v as i64)
	} else {
		format!("{v}")
	}
}

/// XML text escaping
pub fn esc(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
}

/// `Surname, Given` with name particles kept with the surname
pub fn file_as(name: &str) -> String {
	let words: Vec<&str> = name.split_whitespace().collect();
	if words.len() < 2 || name.contains(',') {
		return name.trim().to_string();
	}
	const PARTICLES: &[&str] = &["de", "del", "la", "las", "los", "van", "von", "da", "di", "du", "le"];
	let mut start = words.len() - 1;
	while start > 1 && PARTICLES.contains(&words[start - 1].to_lowercase().as_str()) {
		start -= 1;
	}
	format!("{}, {}", words[start..].join(" "), words[..start].join(" "))
}

pub(crate) fn temp_path(path: &Path) -> PathBuf {
	let name = path
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_default();
	path.with_file_name(format!(".{name}.writing"))
}

/// Copies every entry of `source` except `skip` into a new archive at `target`, then lets
/// `extra` add entries. `mimetype` goes first and stored when present.
pub(crate) fn rewrite_zip(
	source: &Path,
	target: &Path,
	skip: &dyn Fn(&str) -> bool,
	extra: &mut dyn FnMut(&mut ZipWriter<File>) -> WriteResult<()>,
) -> WriteResult<()> {
	let mut archive = ZipArchive::new(File::open(source)?)?;
	let mut writer = ZipWriter::new(File::create(target)?);
	if let Ok(mut mimetype) = archive.by_name("mimetype") {
		let mut buf = vec![];
		mimetype.read_to_end(&mut buf)?;
		writer.start_file(
			"mimetype",
			SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
		)?;
		writer.write_all(&buf)?;
	}
	for i in 0..archive.len() {
		let entry = archive.by_index_raw(i)?;
		let name = entry.name().to_string();
		if name == "mimetype" || skip(&name) {
			continue;
		}
		writer.raw_copy_file(entry)?;
	}
	extra(&mut writer)?;
	writer.finish()?;
	Ok(())
}

/// Replaces `path` by the finished `tmp`, keeping the original on failure
fn replace_with(tmp: &Path, path: &Path) -> WriteResult<()> {
	std::fs::rename(tmp, path)?;
	Ok(())
}

// ------------------------------------------------------------------ EPUB

pub(crate) fn opf_path(archive: &mut ZipArchive<File>) -> WriteResult<String> {
	let mut container = String::new();
	archive
		.by_name("META-INF/container.xml")?
		.read_to_string(&mut container)?;
	let re = Regex::new(r#"full-path=["']([^"']+)["']"#).expect("valid regex");
	re.captures(&container)
		.map(|c| c[1].to_string())
		.ok_or_else(|| WriteError::Invalid("EPUB container has no OPF path".into()))
}

/// Writes the metadata into an EPUB file (in place)
pub fn write_epub(path: &Path, meta: &FileMetadata) -> WriteResult<()> {
	let (opf_name, opf) = {
		let mut archive = ZipArchive::new(File::open(path)?)?;
		let name = opf_path(&mut archive)?;
		let mut bytes = vec![];
		archive.by_name(&name)?.read_to_end(&mut bytes)?;
		let text = String::from_utf8_lossy(&bytes)
			.trim_end_matches('\0')
			.to_string();
		(name, text)
	};
	let new_opf = edit_opf(&opf, meta)?;
	if new_opf == opf {
		return Ok(());
	}
	check_xml(&new_opf)?;

	let tmp = temp_path(path);
	let result = rewrite_zip(
		path,
		&tmp,
		&|name| name == opf_name,
		&mut |writer| {
			writer.start_file(
				opf_name.as_str(),
				SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
			)?;
			writer.write_all(new_opf.as_bytes())?;
			Ok(())
		},
	)
	.and_then(|_| {
		// the rewritten file must open and carry the new OPF
		let mut check = ZipArchive::new(File::open(&tmp)?)?;
		let mut text = String::new();
		check.by_name(&opf_name)?.read_to_string(&mut text)?;
		if text != new_opf {
			return Err(WriteError::Invalid("rewritten EPUB check failed".into()));
		}
		replace_with(&tmp, path)
	});
	if result.is_err() {
		let _ = std::fs::remove_file(&tmp);
	}
	result
}

pub(crate) fn check_xml(text: &str) -> WriteResult<()> {
	let mut reader = quick_xml::Reader::from_str(text);
	loop {
		match reader.read_event() {
			Ok(quick_xml::events::Event::Eof) => return Ok(()),
			Ok(_) => {},
			Err(error) => {
				return Err(WriteError::Invalid(format!("new OPF is not valid XML: {error}")))
			},
		}
	}
}

/// Edits the OPF text: the reviewed values replace what the file had
pub fn edit_opf(opf: &str, meta: &FileMetadata) -> WriteResult<String> {
	// the XML declaration must say UTF-8, the encoding we write
	let decl = Regex::new(r#"^(<\?xml[^>]*encoding=["'])[^"']+"#).expect("valid regex");
	let opf = decl.replace(opf, "${1}UTF-8").into_owned();

	let block = Regex::new(r"(?s)(<(?:\w+:)?metadata\b[^>]*>)(.*?)(</(?:\w+:)?metadata>)")
		.expect("valid regex");
	let Some(caps) = block.captures(&opf) else {
		return Err(WriteError::Invalid("OPF has no metadata block".into()));
	};
	let whole = caps.get(0).expect("match");
	let mut head = caps[1].to_string();
	let mut body = caps[2].to_string();
	let tail = caps[3].to_string();
	let package = Regex::new(r"<package\b[^>]*>")
		.expect("valid regex")
		.find(&opf)
		.map(|m| m.as_str().to_string())
		.unwrap_or_default();
	let epub3 = Regex::new(r#"version=["']3"#).expect("valid regex").is_match(&package);
	if !head.contains("xmlns:opf") && !package.contains("xmlns:opf") {
		head = format!("{} xmlns:opf=\"http://www.idpf.org/2007/opf\">", &head[..head.len() - 1]);
	}
	if !head.contains("xmlns:dc") && !package.contains("xmlns:dc") {
		head = format!(
			"{} xmlns:dc=\"http://purl.org/dc/elements/1.1/\">",
			&head[..head.len() - 1]
		);
	}

	let mut add: Vec<String> = vec![];
	let remove = |body: &mut String, pattern: &str| {
		let re = Regex::new(pattern).expect("valid regex");
		*body = re.replace_all(body, "").into_owned();
	};

	// title
	if let Some(title) = meta.title.as_deref().filter(|t| !t.trim().is_empty()) {
		let re = Regex::new(r"(?s)(<dc:title\b[^>]*>)(.*?)(</dc:title>)").expect("valid regex");
		if let Some(c) = re.captures(&body) {
			let current = html_unescape(c[2].trim());
			if current != title {
				let replaced = format!("{}{}{}", &c[1], esc(title), &c[3]);
				body = body.replacen(c.get(0).expect("match").as_str(), &replaced, 1);
			}
		} else {
			remove(&mut body, r"\s*<dc:title\b[^>]*/>");
			add.insert(0, format!("<dc:title>{}</dc:title>", esc(title)));
		}
	}

	// creators: replaced when the reviewed authors differ from the file's
	if !meta.authors.is_empty() {
		let creator =
			Regex::new(r"(?s)<dc:creator\b([^>]*)>(.*?)</dc:creator>").expect("valid regex");
		let current: Vec<String> = creator
			.captures_iter(&body)
			.filter(|c| !c[1].contains("role=\"") || c[1].contains("aut"))
			.map(|c| html_unescape(c[2].trim()))
			.collect();
		if current != meta.authors {
			let ids: Vec<String> = Regex::new(r#"<dc:creator\b[^>]*\bid=["']([^"']+)["']"#)
				.expect("valid regex")
				.captures_iter(&body)
				.map(|c| c[1].to_string())
				.collect();
			for id in ids {
				remove(
					&mut body,
					&format!(r#"(?s)\s*<meta\b[^>]*refines=["']#{}["'][^>]*>.*?</meta>"#, regex::escape(&id)),
				);
				remove(
					&mut body,
					&format!(r#"\s*<meta\b[^>]*refines=["']#{}["'][^>]*/>"#, regex::escape(&id)),
				);
			}
			remove(&mut body, r"(?s)\s*<dc:creator\b[^>]*>.*?</dc:creator>");
			remove(&mut body, r"\s*<dc:creator\b[^>]*/>");
			for (i, author) in meta.authors.iter().enumerate() {
				if epub3 {
					let id = format!("intake-creator-{}", i + 1);
					add.push(format!(
						"<dc:creator id=\"{id}\">{}</dc:creator><meta refines=\"#{id}\" property=\"file-as\">{}</meta><meta refines=\"#{id}\" property=\"role\" scheme=\"marc:relators\">aut</meta>",
						esc(author),
						esc(&file_as(author))
					));
				} else {
					add.push(format!(
						"<dc:creator opf:file-as=\"{}\" opf:role=\"aut\">{}</dc:creator>",
						esc(&file_as(author)),
						esc(author)
					));
				}
			}
		}
	}

	// tags own dc:subject (plus the first big tag as bcurator:shelf)
	if !meta.tags.is_empty() {
		remove(&mut body, r"\s*<dc:subject\b[^>]*>[^<]*</dc:subject>");
		remove(&mut body, r"\s*<dc:subject\b[^>]*/>");
		remove(&mut body, r#"\s*<meta name="bcurator:(?:estante|shelf)" content="[^"]*"\s*/>"#);
		for tag in &meta.tags {
			add.push(format!("<dc:subject>{}</dc:subject>", esc(tag)));
		}
		if let Some(shelf) = meta.shelf() {
			add.push(format!("<meta name=\"bcurator:shelf\" content=\"{}\"/>", esc(shelf)));
		}
	}

	// series
	if let Some(series) = meta.series.as_deref().filter(|s| !s.trim().is_empty()) {
		let ids: Vec<String> =
			Regex::new(r#"<meta[^>]*property=["']belongs-to-collection["'][^>]*id=["']([^"']+)["']"#)
				.expect("valid regex")
				.captures_iter(&body)
				.map(|c| c[1].to_string())
				.collect();
		for id in ids {
			remove(
				&mut body,
				&format!(r#"(?s)\s*<meta\b[^>]*refines=["']#{}["'][^>]*>.*?</meta>"#, regex::escape(&id)),
			);
		}
		remove(&mut body, r#"(?s)\s*<meta[^>]*property=["']belongs-to-collection["'][^>]*>.*?</meta>"#);
		remove(&mut body, r#"\s*<meta[^>]*name=["']calibre:series(?:_index)?["'][^>]*/>"#);
		add.push(format!("<meta name=\"calibre:series\" content=\"{}\"/>", esc(series)));
		if let Some(index) = meta.series_index {
			add.push(format!(
				"<meta name=\"calibre:series_index\" content=\"{}\"/>",
				num(index)
			));
		}
		if epub3 {
			let mut collection = format!(
				"<meta property=\"belongs-to-collection\" id=\"bcurator-series\">{}</meta><meta refines=\"#bcurator-series\" property=\"collection-type\">series</meta>",
				esc(series)
			);
			if let Some(index) = meta.series_index {
				collection.push_str(&format!(
					"<meta refines=\"#bcurator-series\" property=\"group-position\">{}</meta>",
					num(index)
				));
			}
			add.push(collection);
		}
	}

	// language: one plain code
	if let Some(language) = meta.language.as_deref().filter(|l| !l.is_empty()) {
		let re = Regex::new(r"<dc:language\b[^>]*>([^<]*)</dc:language>").expect("valid regex");
		let current: Vec<String> = re.captures_iter(&body).map(|c| c[1].trim().to_string()).collect();
		if current != [language.to_string()] {
			remove(&mut body, r"\s*<dc:language\b[^>]*>[^<]*</dc:language>");
			remove(&mut body, r"\s*<dc:language\b[^>]*/>");
			add.push(format!("<dc:language>{}</dc:language>", esc(language)));
		}
	}

	// a description only when the file has (almost) none
	if let Some(description) = meta.description.as_deref().filter(|d| !d.trim().is_empty()) {
		let re = Regex::new(r"(?s)<dc:description\b[^>]*>(.*?)</dc:description>").expect("valid regex");
		let words: usize = re
			.captures_iter(&body)
			.map(|c| c[1].split_whitespace().count())
			.sum();
		if words < 15 {
			remove(&mut body, r"(?s)\s*<dc:description\b[^>]*>.*?</dc:description>");
			remove(&mut body, r"\s*<dc:description\b[^>]*/>");
			add.push(format!("<dc:description>{}</dc:description>", esc(description)));
		}
	}

	// a publication year only when the file has no publication date
	if let Some(year) = meta.year {
		let re = Regex::new(r"<dc:date\b([^>]*)>([^<]*)</dc:date>").expect("valid regex");
		let has_date = re
			.captures_iter(&body)
			.any(|c| !c[1].to_lowercase().contains("modification") && c[2].trim().len() >= 4);
		if !has_date {
			add.push(format!("<dc:date>{year}</dc:date>"));
		}
	}

	if let Some(age) = meta.age_rating {
		remove(&mut body, r#"\s*<meta[^>]*name="typicalAgeRange"[^>]*/>"#);
		let value = if age >= 18 { "18+" } else { "All ages" };
		add.push(format!("<meta name=\"typicalAgeRange\" content=\"{value}\"/>"));
	}

	if add.is_empty() && body == caps[2] && head == caps[1] {
		return Ok(opf);
	}
	let body = format!("{}\n    {}\n  ", body.trim_end(), add.join("\n    "));
	Ok(format!(
		"{}{}{}{}{}",
		&opf[..whole.start()],
		head,
		body,
		tail,
		&opf[whole.end()..]
	))
}

pub(crate) fn html_unescape(s: &str) -> String {
	s.replace("&lt;", "<")
		.replace("&gt;", ">")
		.replace("&quot;", "\"")
		.replace("&apos;", "'")
		.replace("&#39;", "'")
		.replace("&amp;", "&")
}

// ------------------------------------------------------------------ CBZ

/// The ComicInfo.xml written for a book
pub fn comic_info(meta: &FileMetadata) -> String {
	let writer = meta.authors.first().cloned().unwrap_or_default();
	let mut x = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ComicInfo>");
	if let Some(series) = &meta.series {
		x.push_str(&format!("<Series>{}</Series>", esc(series)));
	}
	if let Some(index) = meta.series_index {
		x.push_str(&format!("<Number>{}</Number>", num(index)));
	}
	if let Some(title) = &meta.title {
		x.push_str(&format!("<Title>{}</Title>", esc(title)));
	}
	if !writer.is_empty() {
		x.push_str(&format!(
			"<Writer>{}</Writer><Penciller>{}</Penciller>",
			esc(&writer),
			esc(&writer)
		));
	}
	if let Some(description) = &meta.description {
		x.push_str(&format!("<Summary>{}</Summary>", esc(description)));
	}
	if let Some(year) = meta.year {
		x.push_str(&format!("<Year>{year}</Year>"));
	}
	if let Some(age) = meta.age_rating {
		x.push_str(if age >= 18 {
			"<AgeRating>Adults Only 18+</AgeRating>"
		} else {
			"<AgeRating>Everyone</AgeRating>"
		});
	}
	if !meta.tags.is_empty() {
		x.push_str(&format!("<Genre>{}</Genre>", esc(&meta.tags.join(", "))));
	}
	x.push_str(&format!(
		"<LanguageISO>{}</LanguageISO>",
		esc(meta.language.as_deref().unwrap_or("en"))
	));
	if meta.manga {
		x.push_str("<Manga>YesAndRightToLeft</Manga>");
	}
	x.push_str("</ComicInfo>");
	x
}

/// Writes (or replaces) ComicInfo.xml in a CBZ file (in place)
pub fn write_cbz(path: &Path, meta: &FileMetadata) -> WriteResult<()> {
	let info = comic_info(meta);
	let tmp = temp_path(path);
	let result = rewrite_zip(
		path,
		&tmp,
		&|name| name.eq_ignore_ascii_case("comicinfo.xml"),
		&mut |writer| {
			writer.start_file(
				"ComicInfo.xml",
				SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
			)?;
			writer.write_all(info.as_bytes())?;
			Ok(())
		},
	)
	.and_then(|_| {
		let original = ZipArchive::new(File::open(path)?)?;
		let original_count = original.len()
			- original
				.file_names()
				.filter(|n| n.eq_ignore_ascii_case("comicinfo.xml"))
				.count();
		let mut check = ZipArchive::new(File::open(&tmp)?)?;
		let mut text = String::new();
		check.by_name("ComicInfo.xml")?.read_to_string(&mut text)?;
		if check.len() != original_count + 1 || text != info {
			return Err(WriteError::Invalid("rewritten CBZ check failed".into()));
		}
		replace_with(&tmp, path)
	});
	if result.is_err() {
		let _ = std::fs::remove_file(&tmp);
	}
	result
}

// ------------------------------------------------------------------ PDF

/// The PDF Keywords written for a book: the tags, then Stump's keyword fields
pub fn pdf_keywords(meta: &FileMetadata) -> String {
	let mut keywords: Vec<String> = meta.tags.clone();
	if let Some(series) = &meta.series {
		keywords.push(format!("{KEYWORD_FIELD_PREFIX}series={}", series.replace(';', " ·")));
		if let Some(index) = meta.series_index {
			keywords.push(format!("{KEYWORD_FIELD_PREFIX}series_index={}", num(index)));
		}
	}
	if let Some(age) = meta.age_rating {
		keywords.push(format!(
			"{KEYWORD_FIELD_PREFIX}age={}",
			if age >= 18 { "18+" } else { "All ages" }
		));
	}
	if let Some(rating) = meta.rating {
		keywords.push(format!("{KEYWORD_FIELD_PREFIX}rating={rating:.2}"));
	}
	// Stump splits Keywords on commas: a comma inside a value becomes " ·"
	let mut seen = std::collections::HashSet::new();
	keywords
		.into_iter()
		.map(|k| k.replace(", ", " · ").replace(',', " ·"))
		.filter(|k| seen.insert(k.clone()))
		.collect::<Vec<_>>()
		.join(", ")
}

/// A PDF text string: literal when ASCII, else UTF-16BE hex with a BOM
pub(crate) fn pdf_string(text: &str) -> String {
	if text.is_ascii() {
		let escaped = text
			.replace('\\', "\\\\")
			.replace('(', "\\(")
			.replace(')', "\\)")
			.replace('\r', "\\r")
			.replace('\n', "\\n");
		format!("({escaped})")
	} else {
		let mut hex = String::from("<FEFF");
		for unit in text.encode_utf16() {
			hex.push_str(&format!("{unit:04X}"));
		}
		hex.push('>');
		hex
	}
}

/// The parts of the last trailer an incremental update needs
#[derive(Debug, Default, PartialEq)]
struct Trailer {
	root: String,
	size: u64,
	id: Option<String>,
	encrypted: bool,
	/// The last cross-reference section is a stream (PDF 1.5+)
	xref_stream: bool,
	prev: u64,
}

fn find_last(haystack: &[u8], needle: &[u8]) -> Option<usize> {
	haystack.windows(needle.len()).rposition(|w| w == needle)
}

/// The raw text of a dictionary value: `12 0 R`, `[<..><..>]`, a number
fn dict_value(dict: &str, key: &str) -> Option<String> {
	let re = Regex::new(&format!(r"/{key}(?:\s*|\b)(\d+\s+\d+\s+R|\d+|\[[^\]]*\])")).ok()?;
	re.captures(dict).map(|c| c[1].trim().to_string())
}

fn read_trailer(file: &mut File) -> WriteResult<Trailer> {
	let len = file.metadata()?.len();
	let tail_len = len.min(4096);
	file.seek(SeekFrom::Start(len - tail_len))?;
	let mut tail = vec![0; tail_len as usize];
	file.read_exact(&mut tail)?;
	let at = find_last(&tail, b"startxref")
		.ok_or_else(|| WriteError::Invalid("PDF has no startxref".into()))?;
	let after = String::from_utf8_lossy(&tail[at + 9..]);
	let prev: u64 = after
		.split_whitespace()
		.next()
		.and_then(|n| n.parse().ok())
		.ok_or_else(|| WriteError::Invalid("PDF startxref is unreadable".into()))?;
	if prev >= len {
		return Err(WriteError::Invalid("PDF startxref points past the end".into()));
	}

	file.seek(SeekFrom::Start(prev))?;
	let mut chunk = vec![0; (len - prev).min(256 * 1024) as usize];
	file.read_exact(&mut chunk)?;
	let text = String::from_utf8_lossy(&chunk);
	let (dict, xref_stream) = if text.trim_start().starts_with("xref") {
		// a classic xref table takes 20 bytes per object, so the trailer after it can lie far
		// beyond the first chunk; it ends right before `startxref`, which is in the tail
		let tail_start = len - tail_len;
		let in_tail = find_last(&tail[..at], b"trailer")
			.filter(|&t| tail_start + t as u64 >= prev)
			.map(|t| String::from_utf8_lossy(&tail[t..at]).into_owned());
		let dict = match in_tail {
			Some(dict) => dict,
			None => {
				if let Some(start) = text.find("trailer") {
					text[start..].to_string()
				} else {
					// a trailer far from both ends: read the whole xref section
					let section_len = (len - prev).min(64 * 1024 * 1024);
					file.seek(SeekFrom::Start(prev))?;
					let mut section = vec![0; section_len as usize];
					file.read_exact(&mut section)?;
					let start = find_last(&section, b"trailer").ok_or_else(|| {
						WriteError::Invalid("PDF has no trailer".into())
					})?;
					String::from_utf8_lossy(&section[start..]).into_owned()
				}
			},
		};
		(dict, false)
	} else {
		let end = text.find("stream").unwrap_or(text.len());
		(text[..end].to_string(), true)
	};
	let root = dict_value(&dict, "Root")
		.ok_or_else(|| WriteError::Invalid("PDF trailer has no Root".into()))?;
	let size = dict_value(&dict, "Size")
		.and_then(|s| s.parse().ok())
		.ok_or_else(|| WriteError::Invalid("PDF trailer has no Size".into()))?;
	Ok(Trailer {
		root,
		size,
		id: dict_value(&dict, "ID"),
		encrypted: dict.contains("/Encrypt"),
		xref_stream,
		prev,
	})
}

/// Values of the Info dictionary to keep (read by Stump's PDF reader before writing)
#[derive(Debug, Clone, Default)]
pub struct KeptPdfInfo {
	pub creator: Option<String>,
	pub producer: Option<String>,
	pub creation_date: Option<String>,
}

/// The Info dictionary entries written for a book (values already encoded as PDF strings)
pub fn pdf_info_entries(meta: &FileMetadata, kept: &KeptPdfInfo) -> Vec<(String, String)> {
	let mut info: Vec<(String, String)> = vec![];
	if let Some(title) = &meta.title {
		info.push(("Title".into(), pdf_string(title)));
	}
	if !meta.authors.is_empty() {
		// Stump splits writers on commas only
		info.push(("Author".into(), pdf_string(&meta.authors.join(", "))));
	}
	if let Some(description) = &meta.description {
		info.push(("Subject".into(), pdf_string(description)));
	}
	let keywords = pdf_keywords(meta);
	if !keywords.is_empty() {
		info.push(("Keywords".into(), pdf_string(&keywords)));
	}
	kept.push_entries(&mut info);
	info
}

impl KeptPdfInfo {
	/// Adds the kept values (and a fresh ModDate) to Info entries
	pub fn push_entries(&self, info: &mut Vec<(String, String)>) {
		if let Some(v) = &self.creator {
			info.push(("Creator".into(), pdf_string(v)));
		}
		if let Some(v) = &self.producer {
			info.push(("Producer".into(), pdf_string(v)));
		}
		if let Some(v) = &self.creation_date {
			info.push(("CreationDate".into(), pdf_string(v)));
		}
		info.push((
			"ModDate".into(),
			pdf_string(&chrono::Utc::now().format("D:%Y%m%d%H%M%SZ").to_string()),
		));
	}
}

/// Appends a new Info dictionary to a PDF as an incremental update (in place)
pub fn write_pdf(path: &Path, meta: &FileMetadata, kept: &KeptPdfInfo) -> WriteResult<()> {
	append_pdf_info(path, &pdf_info_entries(meta, kept))
}

/// Appends `info` (keys and PDF-encoded values) as the PDF's new Info dictionary, in an
/// incremental update: the rest of the file is untouched
pub fn append_pdf_info(path: &Path, info: &[(String, String)]) -> WriteResult<()> {
	let mut file = std::fs::OpenOptions::new().read(true).write(true).open(path)?;
	let trailer = read_trailer(&mut file)?;
	if trailer.encrypted {
		return Err(WriteError::Invalid("encrypted PDFs are left untouched".into()));
	}

	let len = file.metadata()?.len();
	// a new line first in case the file does not end with one
	let mut update: Vec<u8> = b"\n".to_vec();
	let info_number = trailer.size;
	let info_offset = len + update.len() as u64;
	update.extend_from_slice(format!("{info_number} 0 obj\n<<").as_bytes());
	for (key, value) in info {
		update.extend_from_slice(format!(" /{key} {value}").as_bytes());
	}
	update.extend_from_slice(b" >>\nendobj\n");

	let id = trailer
		.id
		.as_deref()
		.map(|id| format!(" /ID {id}"))
		.unwrap_or_default();
	if trailer.xref_stream {
		let xref_number = info_number + 1;
		let xref_offset = len + update.len() as u64;
		let mut entries: Vec<u8> = vec![];
		for offset in [info_offset, xref_offset] {
			entries.push(1);
			entries.extend_from_slice(&(offset as u32).to_be_bytes());
			entries.extend_from_slice(&0u16.to_be_bytes());
		}
		update.extend_from_slice(
			format!(
				"{xref_number} 0 obj\n<< /Type /XRef /Size {} /Root {} /Info {info_number} 0 R /Prev {}{id} /W [1 4 2] /Index [{info_number} 2] /Length {} >>\nstream\n",
				xref_number + 1,
				trailer.root,
				trailer.prev,
				entries.len()
			)
			.as_bytes(),
		);
		update.extend_from_slice(&entries);
		update.extend_from_slice(
			format!("\nendstream\nendobj\nstartxref\n{xref_offset}\n%%EOF\n").as_bytes(),
		);
	} else {
		let xref_offset = len + update.len() as u64;
		update.extend_from_slice(
			format!(
				"xref\n{info_number} 1\n{info_offset:010} 00000 n \ntrailer\n<< /Size {} /Root {} /Info {info_number} 0 R /Prev {}{id} >>\nstartxref\n{xref_offset}\n%%EOF\n",
				info_number + 1,
				trailer.root,
				trailer.prev
			)
			.as_bytes(),
		);
	}
	if info_offset > u32::MAX as u64 {
		return Err(WriteError::Invalid("PDF too large for an incremental update".into()));
	}
	file.seek(SeekFrom::End(0))?;
	file.write_all(&update)?;
	file.sync_all()?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn meta() -> FileMetadata {
		FileMetadata {
			title: Some("El nombre de la rosa".into()),
			authors: vec!["Umberto Eco".into()],
			series: Some("Saga & Co".into()),
			series_index: Some(2.0),
			language: Some("es".into()),
			year: Some(1980),
			description: None,
			tags: order_tags(&[
				"Origin: Italian".into(),
				"Mystery & Thriller: Historical Mystery".into(),
				"Historical Fiction".into(),
				"Mystery & Thriller".into(),
			]),
			age_rating: None,
			rating: Some(4.25),
			manga: false,
		}
	}

	#[test]
	fn orders_tags_big_small_facets() {
		assert_eq!(
			meta().tags,
			vec![
				"Historical Fiction",
				"Mystery & Thriller",
				"Mystery & Thriller: Historical Mystery",
				"Origin: Italian"
			]
		);
		assert_eq!(meta().shelf(), Some("Historical Fiction"));
	}

	#[test]
	fn sorts_names_like_file_as() {
		assert_eq!(file_as("Umberto Eco"), "Eco, Umberto");
		assert_eq!(file_as("José Joaquín de Olmedo"), "de Olmedo, José Joaquín");
		assert_eq!(file_as("Plato"), "Plato");
		assert_eq!(file_as("Eco, Umberto"), "Eco, Umberto");
	}

	#[test]
	fn edits_an_epub2_opf() {
		let opf = r#"<?xml version="1.0" encoding="iso-8859-1"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Il nome della rosa</dc:title>
    <dc:creator opf:role="aut">Eco, Umberto</dc:creator>
    <dc:subject>Novela</dc:subject>
    <dc:language>es-ES</dc:language>
    <dc:identifier id="id">urn:uuid:1</dc:identifier>
    <meta name="calibre:series" content="Old"/>
  </metadata>
  <manifest/><spine/>
</package>"#;
		let new = edit_opf(opf, &meta()).unwrap();
		assert!(new.starts_with(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
		assert!(new.contains("<dc:title>El nombre de la rosa</dc:title>"));
		assert!(new.contains(r#"<dc:creator opf:file-as="Eco, Umberto" opf:role="aut">Umberto Eco</dc:creator>"#));
		assert!(!new.contains("Novela"));
		assert!(new.contains("<dc:subject>Historical Fiction</dc:subject>"));
		assert!(new.contains(r#"<meta name="bcurator:shelf" content="Historical Fiction"/>"#));
		assert!(new.contains(r#"<meta name="calibre:series" content="Saga &amp; Co"/>"#));
		assert!(new.contains(r#"<meta name="calibre:series_index" content="2"/>"#));
		assert!(!new.contains("content=\"Old\""));
		assert!(new.contains("<dc:language>es</dc:language>"));
		assert!(new.contains("<dc:date>1980</dc:date>"));
		assert!(!new.contains("belongs-to-collection"));
		check_xml(&new).unwrap();
	}

	#[test]
	fn edits_an_epub3_opf_with_refines() {
		let opf = r##"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>El nombre de la rosa</dc:title>
    <dc:creator id="c1">U. Eco</dc:creator>
    <meta refines="#c1" property="file-as">Eco, U.</meta>
    <dc:date>1980-01-01</dc:date>
    <dc:language>es</dc:language>
  </metadata>
</package>"##;
		let new = edit_opf(opf, &meta()).unwrap();
		assert!(!new.contains("U. Eco"));
		assert!(!new.contains("refines=\"#c1\""));
		assert!(new.contains(r##"<dc:creator id="intake-creator-1">Umberto Eco</dc:creator><meta refines="#intake-creator-1" property="file-as">Eco, Umberto</meta>"##));
		assert!(new.contains(r#"<meta property="belongs-to-collection" id="bcurator-series">Saga &amp; Co</meta>"#));
		assert!(new.contains(r##"<meta refines="#bcurator-series" property="group-position">2</meta>"##));
		// a publication date is already there
		assert!(!new.contains("<dc:date>1980</dc:date>"));
		check_xml(&new).unwrap();
	}

	#[test]
	fn builds_pdf_keywords_with_the_keyword_fields() {
		let mut m = meta();
		m.tags.push("Viajes, deporte y juegos".into());
		m.age_rating = Some(18);
		assert_eq!(
			pdf_keywords(&m),
			"Historical Fiction, Mystery & Thriller, Mystery & Thriller: Historical Mystery, Origin: Italian, Viajes · deporte y juegos, bcurator:series=Saga & Co, bcurator:series_index=2, bcurator:age=18+, bcurator:rating=4.25"
		);
	}

	#[test]
	fn encodes_pdf_strings() {
		assert_eq!(pdf_string("A (b)"), r"(A \(b\))");
		assert_eq!(pdf_string("é"), "<FEFF00E9>");
	}

	#[test]
	fn builds_comic_info() {
		let mut m = meta();
		m.manga = true;
		m.tags = vec!["Manga & Comics".into()];
		let xml = comic_info(&m);
		assert!(xml.contains("<Series>Saga &amp; Co</Series><Number>2</Number>"));
		assert!(xml.contains("<Genre>Manga &amp; Comics</Genre>"));
		assert!(xml.contains("<Manga>YesAndRightToLeft</Manga>"));
		assert!(xml.contains("<LanguageISO>es</LanguageISO>"));
	}

	/// a minimal classic PDF, updated twice: each update chains to the previous one
	#[test]
	fn appends_an_info_update_to_a_classic_pdf() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("a.pdf");
		let body = "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n";
		let xref_at = body.len();
		let pdf = format!(
			"{body}xref\n0 3\n0000000000 65535 f \n0000000009 00000 n \n0000000058 00000 n \ntrailer\n<< /Size 3 /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n"
		);
		std::fs::write(&path, &pdf).unwrap();
		write_pdf(&path, &meta(), &KeptPdfInfo::default()).unwrap();
		let updated = std::fs::read_to_string(&path).unwrap();
		assert!(updated.starts_with(&pdf));
		assert!(updated.contains("3 0 obj\n<< /Title (El nombre de la rosa) /Author (Umberto Eco)"));
		assert!(updated.contains(&format!("/Size 4 /Root 1 0 R /Info 3 0 R /Prev {xref_at}")));

		let mut file = File::open(&path).unwrap();
		let trailer = read_trailer(&mut file).unwrap();
		assert_eq!(trailer.size, 4);
		assert_eq!(trailer.root, "1 0 R");
		write_pdf(&path, &meta(), &KeptPdfInfo::default()).unwrap();
		let mut file = File::open(&path).unwrap();
		assert_eq!(read_trailer(&mut file).unwrap().size, 5);
	}

	/// a classic PDF whose xref table (20 bytes an object) is far longer than the 256 KiB read
	/// after `startxref`: its trailer is found next to `startxref` (`PDF has no trailer` before)
	fn large_classic_pdf(objects: usize) -> (Vec<u8>, usize) {
		let mut pdf = b"%PDF-1.4\n".to_vec();
		let mut offsets = vec![];
		for n in 1..objects {
			offsets.push(pdf.len());
			let body = match n {
				1 => "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
				2 => "<< /Type /Pages /Kids [] /Count 0 >>".to_string(),
				_ => "null".to_string(),
			};
			pdf.extend_from_slice(format!("{n} 0 obj\n{body}\nendobj\n").as_bytes());
		}
		let xref_at = pdf.len();
		pdf.extend_from_slice(
			format!("xref\n0 {objects}\n0000000000 65535 f \n").as_bytes(),
		);
		for offset in offsets {
			pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
		}
		pdf.extend_from_slice(
			format!(
				"trailer\n<< /Size {objects} /Root 1 0 R /ID [<0A><0B>] >>\nstartxref\n{xref_at}\n%%EOF\n"
			)
			.as_bytes(),
		);
		(pdf, xref_at)
	}

	#[test]
	fn reads_the_trailer_after_a_large_classic_xref() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("large.pdf");
		let (pdf, xref_at) = large_classic_pdf(20_000);
		assert!(pdf.len() - xref_at > 256 * 1024);
		std::fs::write(&path, &pdf).unwrap();

		let mut file = File::open(&path).unwrap();
		let trailer = read_trailer(&mut file).unwrap();
		assert_eq!(trailer.size, 20_000);
		assert_eq!(trailer.root, "1 0 R");
		assert_eq!(trailer.id.as_deref(), Some("[<0A><0B>]"));
		assert!(!trailer.xref_stream);
		assert_eq!(trailer.prev, xref_at as u64);

		write_pdf(&path, &meta(), &KeptPdfInfo::default()).unwrap();
		let updated = std::fs::read(&path).unwrap();
		assert!(updated.starts_with(&pdf));
		let update = String::from_utf8_lossy(&updated[pdf.len()..]);
		assert!(update.contains("20000 0 obj\n<< /Title (El nombre de la rosa)"));
		assert!(update.contains(&format!(
			"/Size 20001 /Root 1 0 R /Info 20000 0 R /Prev {xref_at} /ID [<0A><0B>]"
		)));

		// the update's own trailer is small and read next; a second update chains to it
		let mut file = File::open(&path).unwrap();
		let second = read_trailer(&mut file).unwrap();
		assert_eq!(second.size, 20_001);
		assert!(second.prev > xref_at as u64);
		write_pdf(&path, &meta(), &KeptPdfInfo::default()).unwrap();
		let mut file = File::open(&path).unwrap();
		let third = read_trailer(&mut file).unwrap();
		assert_eq!(third.size, 20_002);
		assert_eq!(third.root, "1 0 R");
	}
}
