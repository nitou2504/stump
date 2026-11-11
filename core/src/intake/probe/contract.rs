//! What the server and the isolated checker (the *probe*) say to each other.
//!
//! The server never reads a byte of a book that somebody who needs approval uploaded. It writes
//! a **request** (which staged file, the limits, what to do with it), the probe reads the file in a
//! process of its own (no network, a memory ceiling, a deadline), and answers with a **verdict**.
//! The verdict is untrusted data to the server: it is validated before anything in it is used (see
//! `intake::queue`).
//!
//! Both are small JSON files. A request is at most [`MAX_REQUEST_BYTES`], a verdict at most
//! [`MAX_VERDICT_BYTES`], and both refuse fields they don't know.

use serde::{Deserialize, Serialize};

use crate::metadata::media::ProcessedMediaMetadata;

/// The version of this contract: a probe and a server that disagree don't talk
pub const CONTRACT_VERSION: u32 = 1;
/// The most bytes of a request
pub const MAX_REQUEST_BYTES: u64 = 256 * 1024;
/// The most bytes of a verdict that the server reads
pub const MAX_VERDICT_BYTES: u64 = 1024 * 1024;

/// The reason codes (see the design: `research/2026-11-10-upload-format-hardening.md`, section 3)
pub mod code {
	pub const TRUNCATED: &str = "TRUNCATED";
	pub const UNREADABLE: &str = "UNREADABLE";
	pub const AMBIGUOUS_ZIP: &str = "AMBIGUOUS_ZIP";
	pub const TOO_MANY_ENTRIES: &str = "TOO_MANY_ENTRIES";
	pub const ENCRYPTED: &str = "ENCRYPTED";
	pub const UNSUPPORTED_COMPRESSION: &str = "UNSUPPORTED_COMPRESSION";
	pub const ENTRY_TOO_LARGE: &str = "ENTRY_TOO_LARGE";
	pub const UNPACKED_TOO_LARGE: &str = "UNPACKED_TOO_LARGE";
	pub const RATIO: &str = "RATIO";
	pub const BAD_PATH: &str = "BAD_PATH";
	pub const DUPLICATE_NAME: &str = "DUPLICATE_NAME";
	pub const SYMLINK: &str = "SYMLINK";
	pub const HEADER_MISMATCH: &str = "HEADER_MISMATCH";
	pub const OVERLAPPING_ENTRIES: &str = "OVERLAPPING_ENTRIES";
	pub const SIZE_MISMATCH: &str = "SIZE_MISMATCH";
	pub const NESTED_ARCHIVE: &str = "NESTED_ARCHIVE";
	pub const NO_PACKAGE: &str = "NO_PACKAGE";
	pub const NO_READABLE_PAGES: &str = "NO_READABLE_PAGES";
	pub const XML_ENTITY: &str = "XML_ENTITY";
	pub const XML_TOO_DEEP: &str = "XML_TOO_DEEP";
	pub const TOO_MANY_SPINE_ITEMS: &str = "TOO_MANY_SPINE_ITEMS";
	pub const DRM: &str = "DRM";
	pub const TOO_MANY_PAGES: &str = "TOO_MANY_PAGES";
	pub const UNSUPPORTED_IMAGE: &str = "UNSUPPORTED_IMAGE";
	pub const IMAGE_TOO_LARGE: &str = "IMAGE_TOO_LARGE";
	pub const IMAGE_UNDECODABLE: &str = "IMAGE_UNDECODABLE";
	pub const PDF_UNREADABLE: &str = "PDF_UNREADABLE";
	pub const PAGE_SIZE: &str = "PAGE_SIZE";
	pub const MULTIVOLUME: &str = "MULTIVOLUME";
	pub const UNKNOWN_SIZE: &str = "UNKNOWN_SIZE";
	pub const TYPE_MISMATCH: &str = "TYPE_MISMATCH";
	pub const ACTIVE_CONTENT: &str = "ACTIVE_CONTENT";
	pub const ACTIVE_CONTENT_UNSTRIPPABLE: &str = "ACTIVE_CONTENT_UNSTRIPPABLE";
	pub const TIMEOUT: &str = "TIMEOUT";
	pub const CRASHED: &str = "CRASHED";
	pub const TOO_DEMANDING: &str = "TOO_DEMANDING";
	pub const OUTPUT_CHECK_FAILED: &str = "OUTPUT_CHECK_FAILED";
	pub const INPUT_CHANGED: &str = "INPUT_CHANGED";
	pub const DESCRIPTION: &str = "DESCRIPTION";
	pub const VERDICT_INVALID: &str = "VERDICT_INVALID";
	// warnings
	pub const MIMETYPE_ORDER: &str = "MIMETYPE_ORDER";
	pub const CASE_DUPLICATE: &str = "CASE_DUPLICATE";
	pub const REMOTE_RESOURCES: &str = "REMOTE_RESOURCES";
	pub const MALFORMED_XHTML: &str = "MALFORMED_XHTML";
	pub const DROPPED_FILES: &str = "DROPPED_FILES";
	pub const CONVERTED_CBR: &str = "CONVERTED_CBR";
	pub const NO_COVER: &str = "NO_COVER";
	pub const PAGE_COUNT: &str = "PAGE_COUNT";
}

/// The limits one check is held to. They come in each request from the server (its configuration),
/// and the probe holds them to the ceilings compiled into it ([`Limits::clamped`])
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
	/// Entries in one archive
	pub max_entries: u64,
	/// Bytes of one entry, counted as it is unpacked
	pub max_entry_bytes: u64,
	/// Bytes of all the entries of one archive together
	pub max_unpacked_bytes: u64,
	/// How many times over an entry of at least 1 MiB may inflate
	pub max_ratio: u64,
	/// Pages of a book
	pub max_pages: u64,
	/// Pixels of one image
	pub max_image_pixels: u64,
	/// Bytes of a metadata text (an OPF, `ComicInfo.xml`)
	pub max_xml_bytes: u64,
	/// How deep an XML text may nest
	pub max_xml_depth: u32,
	/// Items in the spine of an EPUB
	pub max_spine_items: u64,
	/// Entries (all the levels together) of the table of contents of an EPUB
	pub max_toc_entries: u64,
	/// Bytes of the file itself
	pub max_file_bytes: u64,
	/// Seconds a check may take, and seconds more for each MB of the file
	pub timeout_secs: u64,
	pub timeout_secs_per_mb: u64,
	/// MB of memory (`RLIMIT_DATA`) the checking process may use
	pub mem_mb: u64,
}

const MIB: u64 = 1024 * 1024;

impl Default for Limits {
	fn default() -> Self {
		Self {
			max_entries: 20_000,
			max_entry_bytes: 64 * MIB,
			max_unpacked_bytes: 2048 * MIB,
			max_ratio: 200,
			max_pages: 10_000,
			max_image_pixels: 50_000_000,
			// the biggest package or table of contents of 8,068 real books is 1 MB: the server's
			// reader builds a tree of it for every request, so what it may be is a few times that
			max_xml_bytes: 4 * MIB,
			max_xml_depth: 256,
			// 8,232 pages in the biggest real book (a dictionary), 3,798 in the next
			max_spine_items: 10_000,
			// 5,300 entries in the biggest real table of contents
			max_toc_entries: 10_000,
			max_file_bytes: 2048 * MIB,
			timeout_secs: 120,
			timeout_secs_per_mb: 1,
			mem_mb: 768,
		}
	}
}

impl Limits {
	/// The ceilings no request can raise: what the probe is built to be safe with
	pub fn hard() -> Self {
		Self {
			max_entries: 65_535,
			max_entry_bytes: 512 * MIB,
			max_unpacked_bytes: 8192 * MIB,
			max_ratio: 1_000,
			max_pages: 20_000,
			max_image_pixels: 200_000_000,
			max_xml_bytes: 64 * MIB,
			max_xml_depth: 1_024,
			max_spine_items: 50_000,
			max_toc_entries: 50_000,
			max_file_bytes: 4096 * MIB,
			timeout_secs: 1_800,
			timeout_secs_per_mb: 10,
			mem_mb: 900,
		}
	}

	/// Each limit as the smaller of what was asked and the ceiling; a limit of 0 is the default
	pub fn clamped(&self) -> Self {
		let (hard, default) = (Self::hard(), Self::default());
		let pick = |asked: u64, hard: u64, default: u64| {
			if asked == 0 {
				default
			} else {
				asked.min(hard)
			}
		};
		Self {
			max_entries: pick(self.max_entries, hard.max_entries, default.max_entries),
			max_entry_bytes: pick(
				self.max_entry_bytes,
				hard.max_entry_bytes,
				default.max_entry_bytes,
			),
			max_unpacked_bytes: pick(
				self.max_unpacked_bytes,
				hard.max_unpacked_bytes,
				default.max_unpacked_bytes,
			),
			max_ratio: pick(self.max_ratio, hard.max_ratio, default.max_ratio),
			max_pages: pick(self.max_pages, hard.max_pages, default.max_pages),
			max_image_pixels: pick(
				self.max_image_pixels,
				hard.max_image_pixels,
				default.max_image_pixels,
			),
			max_xml_bytes: pick(
				self.max_xml_bytes,
				hard.max_xml_bytes,
				default.max_xml_bytes,
			),
			max_xml_depth: u32::try_from(pick(
				u64::from(self.max_xml_depth),
				u64::from(hard.max_xml_depth),
				u64::from(default.max_xml_depth),
			))
			.unwrap_or(default.max_xml_depth),
			max_spine_items: pick(
				self.max_spine_items,
				hard.max_spine_items,
				default.max_spine_items,
			),
			max_toc_entries: pick(
				self.max_toc_entries,
				hard.max_toc_entries,
				default.max_toc_entries,
			),
			max_file_bytes: pick(
				self.max_file_bytes,
				hard.max_file_bytes,
				default.max_file_bytes,
			),
			timeout_secs: pick(
				self.timeout_secs,
				hard.timeout_secs,
				default.timeout_secs,
			),
			timeout_secs_per_mb: pick(
				self.timeout_secs_per_mb,
				hard.timeout_secs_per_mb,
				default.timeout_secs_per_mb,
			),
			mem_mb: pick(self.mem_mb, hard.mem_mb, default.mem_mb),
		}
	}

	/// How long a check of a file of `size` bytes may take, in seconds
	pub fn deadline_secs(&self, size: u64) -> u64 {
		let limits = self.clamped();
		limits
			.timeout_secs
			.saturating_add(limits.timeout_secs_per_mb.saturating_mul(size / MIB))
			.min(Self::hard().timeout_secs)
	}
}

/// What to do with the active content of an EPUB (scripts and the like)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActiveContent {
	/// Take it out: the book is written anew, without a page of the one that was sent
	/// (`probe::rebuild`)
	#[default]
	Strip,
	/// Refuse the book
	Block,
	/// File it as it is, and say it carries some
	Warn,
}

/// What a check is for
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
	/// Look at a file that was just uploaded
	Screen,
	/// Make the file that goes into the library: write the reviewed metadata, rebuild the archive,
	/// take out the active content, and check what came out
	File,
}

/// What the server asks of the probe
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
	pub v: u32,
	/// The ticket: a UUID
	pub id: String,
	pub kind: Kind,
	/// The staged file, relative to the folder of the inbox (`<config>/intake`, `/intake` in the
	/// probe's container). Never absolute, never with `..`
	pub input: String,
	/// The name it was uploaded with
	pub name: String,
	/// What the server counted as it arrived
	pub size: u64,
	pub sha256: String,
	pub limits: Limits,
	/// The uploader may file books themselves (the server owner, or somebody who manages
	/// libraries): what can't be cleaned is told to them, not refused
	#[serde(default)]
	pub approver: bool,
	pub active_content: ActiveContent,
	/// Convert a RAR comic to a ZIP one when filing it (UnRAR then never reads it in the server)
	pub cbr_to_cbz: bool,
	/// For a file job: what to write
	pub file: Option<FileJob>,
}

/// What a file job writes into the book
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileJob {
	pub metadata: crate::intake::write::FileMetadata,
}

/// How a check ended
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
	/// The file is fine, or fine once the active content is taken out
	Ok,
	/// It is not accepted: see the reasons
	Reject,
	/// The checker could not do it now (it was short of memory): the server asks again
	Retry,
}

/// Why a file is not accepted, or what a book carries that the server should say
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reason {
	pub code: String,
	pub detail: String,
}

impl Reason {
	pub fn new(code: &str, detail: impl Into<String>) -> Self {
		Self {
			code: code.to_string(),
			detail: detail.into(),
		}
	}
}

/// What the probe found of active content
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ActiveSummary {
	/// How many constructs were found (or taken out), in all
	pub found: u64,
	/// The files they are in (the first few), with what each holds
	pub items: Vec<ActiveItem>,
	/// They were taken out (a file job), or would be (a screening)
	pub stripped: bool,
	/// How many `http(s)` references to resources the book makes (information: they are inert)
	pub remote_refs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveItem {
	/// `script`, `iframe`, `onclick`...
	pub kind: String,
	pub entry: String,
	pub count: u64,
}

/// What a book says about itself that the server uses: the fields of Stump's own metadata that the
/// inbox reads, as plain values (Stump's type has readers of its own for files, which a verdict
/// doesn't go through)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BookMetadata {
	pub format: Option<String>,
	pub title: Option<String>,
	pub series: Option<String>,
	pub number: Option<f64>,
	pub summary: Option<String>,
	pub age_rating: Option<i32>,
	pub rating: Option<f64>,
	pub genres: Option<Vec<String>>,
	pub tags: Option<Vec<String>>,
	pub language: Option<String>,
	pub year: Option<i32>,
	pub identifier_isbn: Option<String>,
	pub writers: Option<Vec<String>>,
	pub publisher: Option<String>,
}

impl BookMetadata {
	pub fn from_processed(metadata: &ProcessedMediaMetadata) -> Self {
		let mut mine = Self {
			format: metadata.format.clone(),
			title: metadata.title.clone(),
			series: metadata.series.clone(),
			number: metadata.number,
			summary: metadata.summary.clone(),
			age_rating: metadata.age_rating,
			rating: metadata.rating,
			genres: metadata.genres.clone(),
			tags: metadata.tags.clone(),
			language: metadata.language.clone(),
			year: metadata.year,
			identifier_isbn: metadata.identifier_isbn.clone(),
			writers: metadata.writers.clone(),
			publisher: metadata.publisher.clone(),
		};
		mine.cap();
		mine
	}

	/// Cuts the texts and lists to what a book can have (what the probe says about a book is
	/// somebody else's text, and the server cuts it again)
	pub fn cap(&mut self) {
		if let Ok(mut value) = serde_json::to_value(&*self) {
			cap_json(&mut value);
			if let Ok(capped) = serde_json::from_value(value) {
				*self = capped;
			}
		}
		// then each field to what that field can be (the design's caps): a title is 300 characters,
		// there are 50 writers of 200, 100 genres and tags of 100, and so on, so that what is
		// kept of a book is not what its file chose to say
		let text = |field: &mut Option<String>, chars: usize| {
			if let Some(value) = field {
				cut_chars(value, chars);
			}
		};
		text(&mut self.format, 20);
		text(&mut self.title, 300);
		text(&mut self.series, 300);
		text(&mut self.summary, MAX_TEXT_CHARS);
		text(&mut self.language, 35);
		text(&mut self.identifier_isbn, 64);
		text(&mut self.publisher, 300);
		let list = |field: &mut Option<Vec<String>>, count: usize, chars: usize| {
			if let Some(items) = field {
				items.truncate(count);
				items.iter_mut().for_each(|item| cut_chars(item, chars));
			}
		};
		list(&mut self.writers, 50, 200);
		list(&mut self.genres, 100, 100);
		list(&mut self.tags, 100, 100);
	}

	/// As Stump's own type, for the code that reads an uploaded file the same way for every book
	pub fn into_processed(self) -> ProcessedMediaMetadata {
		ProcessedMediaMetadata {
			format: self.format,
			title: self.title,
			series: self.series,
			number: self.number,
			summary: self.summary,
			age_rating: self.age_rating,
			rating: self.rating,
			genres: self.genres,
			tags: self.tags,
			language: self.language,
			year: self.year,
			identifier_isbn: self.identifier_isbn,
			writers: self.writers,
			publisher: self.publisher,
			..ProcessedMediaMetadata::default()
		}
	}
}

/// The checked file as the probe saw it
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Book {
	/// `epub`, `cbz`, `cbr` or `pdf`, by what the file is
	pub format: String,
	/// Of the file that was checked
	pub sha256: String,
	pub size: u64,
	/// Stump's own file hash (it finds copies of a book with it)
	pub stump_hash: Option<String>,
	pub pages: i32,
	pub entries: u64,
	pub unpacked_bytes: u64,
	pub max_entry_bytes: u64,
	/// What the embedded metadata says
	pub metadata: Option<BookMetadata>,
	/// An EPUB's original title (`dc:title` of a translation's source)
	pub original_title: Option<String>,
	/// What an EPUB's title page says
	pub title_page_title: Option<String>,
	pub title_page_authors: Vec<String>,
	pub active_content: ActiveSummary,
	/// For a file job: the file that is to go in the library
	pub output: Option<Output>,
}

/// The file a file job made
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
	/// Relative to the probe's `out` folder: `<ticket>/000.cbz`
	pub path: String,
	pub format: String,
	pub sha256: String,
	pub size: u64,
	/// The format of the input, when it was converted (`cbr`)
	pub converted_from: Option<String>,
	pub pages: i32,
}

/// What it cost
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Usage {
	pub peak_rss_kb: u64,
	pub cpu_ms: u64,
	pub wall_ms: u64,
}

/// The probe's answer
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verdict {
	pub v: u32,
	pub id: String,
	pub kind: Kind,
	/// The revision of the build that checked: a probe and a server of different builds are not
	/// the same code, and a pass of the one means nothing to the other
	pub probe: String,
	pub status: Status,
	pub reasons: Vec<Reason>,
	pub warnings: Vec<Reason>,
	pub book: Option<Book>,
	pub usage: Usage,
}

impl Verdict {
	pub fn rejected(request: &Request, reasons: Vec<Reason>) -> Self {
		Self {
			v: CONTRACT_VERSION,
			id: request.id.clone(),
			kind: request.kind,
			probe: probe_revision().to_string(),
			status: Status::Reject,
			reasons,
			warnings: vec![],
			book: None,
			usage: Usage::default(),
		}
	}
}

/// The revision of this build
pub fn probe_revision() -> &'static str {
	env!("GIT_REV")
}

/// The longest text of a metadata field, in characters
pub const MAX_TEXT_CHARS: usize = 20_000;
/// The most items of a list in the metadata
pub const MAX_LIST_ITEMS: usize = 500;

/// Cuts every text and list of a JSON value to what a book can have, and takes the control
/// characters out of the texts: what the probe says about a book is somebody else's text
pub fn cap_json(value: &mut serde_json::Value) {
	use serde_json::Value;
	match value {
		Value::String(text) => {
			let cleaned: String = text
				.chars()
				.filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
				.take(MAX_TEXT_CHARS)
				.collect();
			*text = cleaned;
		},
		Value::Array(items) => {
			items.truncate(MAX_LIST_ITEMS);
			items.iter_mut().for_each(cap_json);
		},
		Value::Object(fields) => fields.values_mut().for_each(cap_json),
		_ => {},
	}
}

/// A text cut to `chars` characters (its newlines stay)
pub fn cut_chars(text: &mut String, chars: usize) {
	if let Some((at, _)) = text.char_indices().nth(chars) {
		text.truncate(at);
	}
}

/// A ticket id: the letters of a UUID, nothing a path could use
pub fn is_valid_ticket(id: &str) -> bool {
	!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// A path that is relative and stays inside what it is relative to
pub fn is_safe_relative(path: &str) -> bool {
	!path.is_empty()
		&& path.len() <= 1024
		&& !path.starts_with('/')
		&& !path.contains('\0')
		&& !path.contains('\\')
		&& path
			.split('/')
			.all(|part| !part.is_empty() && part != "." && part != "..")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn limits_are_held_to_the_ceilings_and_zero_is_the_default() {
		let asked = Limits {
			max_entries: u64::MAX,
			max_pages: 0,
			timeout_secs: 1_000_000,
			mem_mb: 10_000,
			..Limits::default()
		};
		let held = asked.clamped();
		assert_eq!(held.max_entries, Limits::hard().max_entries);
		assert_eq!(held.max_pages, Limits::default().max_pages);
		assert_eq!(held.timeout_secs, Limits::hard().timeout_secs);
		assert_eq!(held.mem_mb, Limits::hard().mem_mb);
		// the deadline grows with the file, up to the ceiling
		let limits = Limits::default();
		assert_eq!(limits.deadline_secs(0), 120);
		assert_eq!(limits.deadline_secs(300 * MIB), 420);
		assert_eq!(limits.deadline_secs(u64::MAX), Limits::hard().timeout_secs);
	}

	#[test]
	fn what_a_book_says_about_itself_is_cut_to_what_each_field_can_be() {
		let long = |n: usize| "é".repeat(n);
		let mut metadata = BookMetadata {
			format: Some(long(100)),
			title: Some(long(5_000)),
			series: Some(long(5_000)),
			summary: Some(format!("{}\n{}", long(30_000), long(30_000))),
			language: Some(long(100)),
			identifier_isbn: Some(long(500)),
			publisher: Some(long(5_000)),
			writers: Some(vec![long(5_000); 400]),
			genres: Some(vec![long(500); 400]),
			tags: Some(vec![long(500); 400]),
			..BookMetadata::default()
		};
		metadata.cap();
		let chars =
			|text: &Option<String>| text.as_deref().map_or(0, |t| t.chars().count());
		assert_eq!(chars(&metadata.format), 20);
		assert_eq!(chars(&metadata.title), 300);
		assert_eq!(chars(&metadata.series), 300);
		assert_eq!(chars(&metadata.summary), MAX_TEXT_CHARS);
		assert_eq!(chars(&metadata.language), 35);
		assert_eq!(chars(&metadata.identifier_isbn), 64);
		assert_eq!(chars(&metadata.publisher), 300);
		let writers = metadata.writers.as_ref().unwrap();
		assert!(writers.len() == 50 && writers.iter().all(|w| w.chars().count() == 200));
		for list in [&metadata.genres, &metadata.tags] {
			let list = list.as_ref().unwrap();
			assert!(list.len() == 100 && list.iter().all(|t| t.chars().count() == 100));
		}
		// a text with a line break keeps it, and one shorter than the cap is not touched
		let mut text = "a\nb".to_string();
		cut_chars(&mut text, 10);
		assert_eq!(text, "a\nb");
		cut_chars(&mut text, 2);
		assert_eq!(text, "a\n");
	}

	#[test]
	fn paths_stay_inside() {
		assert!(is_safe_relative("user/item/001_book.epub"));
		for bad in [
			"",
			"/etc/passwd",
			"../x",
			"a/../x",
			"a//b",
			"./a",
			"a\\b",
			"a\0b",
		] {
			assert!(!is_safe_relative(bad), "{bad:?}");
		}
	}

	#[test]
	fn a_request_with_a_field_nobody_knows_is_refused() {
		let json = r#"{"v":1,"id":"x","kind":"screen","input":"a/b","name":"b","size":1,
			"sha256":"","limits":{},"active_content":"strip","cbr_to_cbz":true,"file":null,"extra":1}"#;
		assert!(serde_json::from_str::<Request>(json).is_err());
		let json = r#"{"v":1,"id":"x","kind":"screen","input":"a/b","name":"b","size":1,
			"sha256":"","limits":{},"active_content":"strip","cbr_to_cbz":true,"file":null}"#;
		let request: Request = serde_json::from_str(json).expect("a request");
		assert_eq!(request.limits, Limits::default());
	}
}
