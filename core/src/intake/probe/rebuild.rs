//! The book that is filed for somebody who needs approval is **written anew** from what Stump's own
//! reader makes of the one that was sent (see [`super::sanitize`] for why).
//!
//! The package is read the way the server's reader reads it: with `open_epub` (epub-rs), which is
//! what decides which files are the book's resources, what type each has, which are its pages and
//! what its table of contents is. Nothing of that reading is left to a second reader. What the
//! book is made of then goes into a new archive:
//!
//! - a `mimetype`, a `META-INF/container.xml` and a package (`content.opf`), a table of contents
//!   (`toc.ncx`) and a navigation document (`nav.xhtml`), all generated here from the title,
//!   authors, language, publisher, date, subjects, series and cover the reader found, the
//!   resources it listed, its reading order and its table of contents (no deeper than
//!   [`MAX_TOC_DEPTH`] levels, no longer than the limit);
//! - every page and every SVG file, rebuilt by [`super::sanitize`]; every style sheet, rebuilt the
//!   same way; every raster image (checked, and kept as it is) and every font (checked by its first
//!   bytes, and kept as it is);
//! - nothing else: not a script, not a page that the package doesn't list, not a file with a name
//!   that a path or an address can't carry (`%`, `#`, `?`, white space that isn't a space...).
//!
//! What a file is, is decided by its name's extension **and** its bytes, never by the type that the
//! package declares for it. The new package declares the type that the file was found to be.

use std::{
	collections::{BTreeMap, HashMap, HashSet},
	fs::File,
	io::Write,
	path::Path,
};

use epub::doc::NavPoint;
use ring::digest;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use super::{
	check::{self, image_kind, ImageKind},
	contract::{code, is_safe_relative, ActiveItem, ActiveSummary, Limits, Reason},
	job::hex,
	sanitize::{self, Findings, Refusal, Settings},
	zipscan::{printable, ZipReport},
};

/// How deep the table of contents of a rebuilt book goes
pub const MAX_TOC_DEPTH: usize = 8;
const MAX_ACTIVE_ITEMS: usize = 30;
const IMAGE_HEAD_BYTES: usize = 8 * 1024 * 1024;

const PAGE_EXTENSIONS: &[&str] = &["xhtml", "html", "htm", "xht"];
const FONT_EXTENSIONS: &[&str] = &["ttf", "otf", "woff", "woff2"];
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "avif"];

/// What a rebuild made of a book
#[derive(Debug, Clone, Default)]
pub struct Rebuilt {
	/// What was taken out of the pages, and where
	pub active: ActiveSummary,
	/// What the approver should be told
	pub warnings: Vec<Reason>,
	/// Pages in the reading order of the new book
	pub pages: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
	Page,
	Svg,
	Css,
	Image(ImageKind),
	Font(&'static str),
}

impl Class {
	fn media_type(self) -> &'static str {
		match self {
			Class::Page => "application/xhtml+xml",
			Class::Svg => "image/svg+xml",
			Class::Css => "text/css",
			Class::Image(ImageKind::Jpeg) => "image/jpeg",
			Class::Image(ImageKind::Png) => "image/png",
			Class::Image(ImageKind::Gif) => "image/gif",
			Class::Image(ImageKind::Webp) => "image/webp",
			Class::Image(ImageKind::Avif) => "image/avif",
			Class::Font(media_type) => media_type,
		}
	}

	fn in_reading_order(self) -> bool {
		matches!(self, Class::Page | Class::Svg | Class::Image(_))
	}
}

fn class_is_page(ext: &str) -> bool {
	PAGE_EXTENSIONS.contains(&ext) || ext == "svg"
}

fn extension(name: &str) -> String {
	name.rsplit('/')
		.next()
		.and_then(|file| file.rsplit_once('.'))
		.map(|(_, ext)| ext.to_ascii_lowercase())
		.unwrap_or_default()
}

fn folder(name: &str) -> &str {
	name.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// A name that a path, an address and a package can all carry as it is
fn carries(name: &str) -> bool {
	is_safe_relative(name)
		&& name.len() <= 500
		&& !name.chars().any(|c| {
			c.is_control() || matches!(c, '%' | '#' | '?' | '"' | '<' | '>' | '|' | '*' | ':')
		})
}

fn font_media_type(head: &[u8]) -> Option<&'static str> {
	if head.starts_with(&[0, 1, 0, 0]) || head.starts_with(b"true") {
		Some("font/ttf")
	} else if head.starts_with(b"OTTO") {
		Some("font/otf")
	} else if head.starts_with(b"wOFF") {
		Some("font/woff")
	} else if head.starts_with(b"wOF2") {
		Some("font/woff2")
	} else {
		None
	}
}

fn text(value: &str, cap: usize) -> String {
	let cleaned: String = value
		.chars()
		.filter(|c| matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'))
		.collect();
	cleaned.trim().chars().take(cap).collect()
}

fn xml(value: &str) -> String {
	let mut out = String::new();
	for c in value.chars() {
		match c {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			'"' => out.push_str("&quot;"),
			c => out.push(c),
		}
	}
	out
}

/// The properties of a spine item that say which side of a spread a page is on, or how it is laid
/// out: nothing else of what the package says of a page is kept
fn spine_properties(raw: &str) -> String {
	raw.split_ascii_whitespace()
		.filter(|token| {
			(token.starts_with("page-spread-") || token.starts_with("rendition:"))
				&& token.len() < 60
				&& token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ':')
		})
		.take(6)
		.collect::<Vec<_>>()
		.join(" ")
}

/// `page-progression-direction` of the spine in the text of a package: `rtl` or `ltr`
fn reading_direction(opf: &str) -> Option<&'static str> {
	let start = opf.find("<spine")?;
	let tag = &opf[start..start + opf[start..].find('>')?];
	let at = tag.find("page-progression-direction")?;
	let value = &tag[at..];
	let quote = value.find(['"', '\''])?;
	let value = &value[quote + 1..];
	match &value[..value.find(['"', '\''])?] {
		"rtl" => Some("rtl"),
		"ltr" => Some("ltr"),
		_ => None,
	}
}

fn stored() -> SimpleFileOptions {
	SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
}

fn deflated() -> SimpleFileOptions {
	SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

fn fail(detail: impl Into<String>) -> Reason {
	Reason::new(code::OUTPUT_CHECK_FAILED, detail)
}

/// One entry of the new table of contents
struct TocNode {
	label: String,
	href: String,
	children: Vec<TocNode>,
}

fn toc_nodes(
	points: &[NavPoint],
	depth: usize,
	budget: &mut u64,
	target_of: &dyn Fn(&str) -> Option<String>,
) -> Vec<TocNode> {
	let mut out = Vec::new();
	for point in points {
		if *budget == 0 {
			break;
		}
		let path = point.content.to_string_lossy().to_string();
		let (file, fragment) = match path.split_once('#') {
			Some((file, fragment)) => (file.to_string(), Some(fragment.to_string())),
			None => (path.clone(), None),
		};
		let children = if depth + 1 < MAX_TOC_DEPTH {
			toc_nodes(&point.children, depth + 1, budget, target_of)
		} else {
			vec![]
		};
		match target_of(&file) {
			Some(name) if *budget > 0 => {
				*budget -= 1;
				let mut href = name;
				if let Some(fragment) = fragment.filter(|f| {
					!f.is_empty() && f.len() < 200 && !f.chars().any(|c| c.is_control() || c.is_whitespace())
				}) {
					href.push('#');
					href.push_str(&fragment);
				}
				out.push(TocNode {
					label: text(&point.label, 300),
					href,
					children,
				});
			},
			// an entry that leads nowhere in the new book: what is under it still does
			_ => out.extend(children),
		}
	}
	out
}

fn ncx_points(nodes: &[TocNode], out: &mut String, order: &mut u64) {
	for node in nodes {
		*order += 1;
		out.push_str(&format!(
			"<navPoint id=\"n{order}\" playOrder=\"{order}\"><navLabel><text>{}</text></navLabel><content src=\"{}\"/>",
			xml(&node.label),
			xml(&node.href)
		));
		ncx_points(&node.children, out, order);
		out.push_str("</navPoint>");
	}
}

fn nav_items(nodes: &[TocNode], out: &mut String) {
	out.push_str("<ol>");
	for node in nodes {
		out.push_str(&format!(
			"<li><a href=\"{}\">{}</a>",
			xml(&node.href),
			xml(&node.label)
		));
		if !node.children.is_empty() {
			nav_items(&node.children, out);
		}
		out.push_str("</li>");
	}
	out.push_str("</ol>");
}

fn depth_of(nodes: &[TocNode]) -> usize {
	nodes
		.iter()
		.map(|n| 1 + depth_of(&n.children))
		.max()
		.unwrap_or(0)
}

/// Where the bytes of a kept file come from: made here (a page, a style sheet), or copied from the
/// entry of the book it was in (an image, a font: never held in memory)
enum Source {
	Made(Vec<u8>),
	Entry(usize),
}

/// A kept file
struct Kept {
	class: Class,
	size: u64,
	source: Source,
}

/// Rebuilds the book at `input` into `output`
pub fn rebuild(
	input: &Path,
	output: &Path,
	report: &ZipReport,
	limits: &Limits,
) -> Result<Rebuilt, Reason> {
	let set = Settings::from_limits(limits);
	let doc = crate::media::processor::epub::open_epub(input).map_err(|error| {
		Reason::new(
			code::UNREADABLE,
			format!("Stump can't open the book's package: {error}"),
		)
	})?;
	if doc.resources.len() as u64 > limits.max_entries {
		return Err(Reason::new(
			code::TOO_MANY_ENTRIES,
			"The package lists more items than a book has",
		));
	}
	if doc.spine.len() as u64 > limits.max_spine_items {
		return Err(Reason::new(
			code::TOO_MANY_SPINE_ITEMS,
			format!("The book has more than {} pages in its spine", limits.max_spine_items),
		));
	}
	let mut archive = ZipArchive::new(
		File::open(input).map_err(|e| Reason::new(code::UNREADABLE, e.to_string()))?,
	)
	.map_err(|e| Reason::new(code::UNREADABLE, e.to_string()))?;
	let index_of: HashMap<&str, usize> = report
		.entries
		.iter()
		.enumerate()
		.filter(|(_, e)| !e.is_dir)
		.map(|(i, e)| (e.name.as_str(), i))
		.collect();

	// what the reader lists, in a fixed order, each file once
	let mut listed: Vec<(String, String)> = doc
		.resources
		.iter()
		.map(|(id, resource)| (resource.path.to_string_lossy().to_string(), id.clone()))
		.collect();
	listed.sort();

	let mut kept: BTreeMap<String, Kept> = BTreeMap::new();
	let mut name_of_id: HashMap<String, String> = HashMap::new();
	let mut findings_of: Vec<(String, Findings)> = vec![];
	let mut remote_refs = 0u64;
	let (mut dropped_pages, mut dropped_files, mut odd_names) = (0u64, 0u64, 0u64);
	let mut unpacked = 0u64;
	let mut seen: HashSet<String> = HashSet::new();

	for (path, id) in &listed {
		let Some(&index) = check::served_entries(path, &index_of).first() else {
			// listed, and not in the archive: the reader skips it too
			continue;
		};
		let entry = &report.entries[index];
		let name = entry.name.clone();
		name_of_id.insert(id.clone(), name.clone());
		if !seen.insert(name.clone()) {
			continue;
		}
		if !carries(&name) {
			odd_names += 1;
			continue;
		}
		let ext = extension(&name);
		if ext == "ncx" {
			// the table of contents is made here, from what the reader read of it
			continue;
		}
		let page_like = PAGE_EXTENSIONS.contains(&ext.as_str());
		let (cap, wanted) = if page_like || ext == "svg" || ext == "css" {
			(set.max_input as u64, true)
		} else if IMAGE_EXTENSIONS.contains(&ext.as_str()) || FONT_EXTENSIONS.contains(&ext.as_str()) {
			(limits.max_entry_bytes, true)
		} else {
			(0, false)
		};
		if !wanted {
			dropped_files += 1;
			continue;
		}
		if entry.size > cap {
			if page_like || ext == "svg" || ext == "css" {
				dropped_pages += 1;
			} else {
				dropped_files += 1;
			}
			continue;
		}
		let base = folder(&name).to_string();
		let (class, source) = if page_like || ext == "svg" || ext == "css" {
			let bytes = check::read_entry(&mut archive, index, &name, cap)?;
			if page_like && image_kind(&bytes).is_some() {
				dropped_files += 1;
				continue;
			}
			let made = if page_like {
				sanitize::clean_page(&bytes, &base, set).map(|p| (Class::Page, p))
			} else if ext == "svg" {
				sanitize::clean_svg(&bytes, &base, set).map(|p| (Class::Svg, p))
			} else {
				sanitize::clean_css(&bytes, &base, set).map(|p| (Class::Css, p))
			};
			match made {
				Ok((class, page)) => {
					remote_refs += page.remote_refs;
					if class != Class::Css {
						findings_of.push((name.clone(), page.findings));
					}
					(class, Source::Made(page.bytes))
				},
				Err(Refusal::TooBig | Refusal::TooDeep | Refusal::NotADocument) => {
					if class_is_page(&ext) {
						dropped_pages += 1;
					} else {
						dropped_files += 1;
					}
					continue;
				},
			}
		} else if FONT_EXTENSIONS.contains(&ext.as_str()) {
			let head = check::read_head(&mut archive, index, &name, 16)?;
			match font_media_type(&head) {
				Some(media_type) => (Class::Font(media_type), Source::Entry(index)),
				// a font that isn't one (or one that is obfuscated: it is no use without the key
				// that this book doesn't carry on)
				None => {
					dropped_files += 1;
					continue;
				},
			}
		} else {
			// an image is looked at by its first bytes, and copied from the book when it is written
			let head = check::read_head(&mut archive, index, &name, IMAGE_HEAD_BYTES as u64)?;
			match image_kind(&head) {
				Some(kind) => {
					check::check_image(&name, &head, false, limits)?;
					(Class::Image(kind), Source::Entry(index))
				},
				None => {
					dropped_files += 1;
					continue;
				},
			}
		};
		let size = match &source {
			Source::Made(bytes) => bytes.len() as u64,
			Source::Entry(_) => entry.size,
		};
		unpacked += size;
		if unpacked > limits.max_unpacked_bytes || kept.len() as u64 >= limits.max_entries {
			return Err(Reason::new(
				code::TOO_MANY_ENTRIES,
				"The rebuilt book would be bigger than a book may be",
			));
		}
		kept.insert(name.clone(), Kept { class, size, source });
	}

	// ids of the new package
	let mut id_of_name: HashMap<&str, String> = HashMap::new();
	for (number, name) in kept.keys().enumerate() {
		id_of_name.insert(name.as_str(), format!("r{}", number + 1));
	}
	let name_for_path = |path: &str| -> Option<String> {
		let index = *check::served_entries(path, &index_of).first()?;
		let name = &report.entries[index].name;
		kept.contains_key(name).then(|| name.clone())
	};

	// the reading order
	let mut spine: Vec<(String, bool, String)> = vec![];
	let mut in_spine: HashSet<String> = HashSet::new();
	for item in &doc.spine {
		let Some(name) = name_of_id.get(&item.idref) else {
			continue;
		};
		let Some(entry) = kept.get(name) else {
			continue;
		};
		if entry.class.in_reading_order() && in_spine.insert(name.clone()) {
			let properties = item.properties.as_deref().map(spine_properties).unwrap_or_default();
			spine.push((name.clone(), item.linear, properties));
		}
	}
	if !spine.iter().any(|(name, _, _)| kept[name].class == Class::Page) {
		return Err(Reason::new(
			code::NO_READABLE_PAGES,
			"Nothing in the book can be read as a page",
		));
	}

	// the table of contents
	let mut budget = limits.max_toc_entries;
	let target = |path: &str| name_for_path(path).filter(|name| kept[name].class.in_reading_order());
	let mut toc = toc_nodes(&doc.toc, 0, &mut budget, &target);
	let title = doc
		.metadata
		.iter()
		.find(|m| m.property == "title")
		.map(|m| text(&m.value, 500))
		.filter(|t| !t.is_empty())
		.unwrap_or_else(|| "Untitled".to_string());
	if toc.is_empty() {
		toc.push(TocNode {
			label: title.clone(),
			href: spine[0].0.clone(),
			children: vec![],
		});
	}

	// the direction the pages go, which only the package's own text says (the reader doesn't keep it)
	let direction = doc
		.root_file
		.to_str()
		.and_then(|name| index_of.get(name))
		.and_then(|&index| check::read_entry(&mut archive, index, "the package", limits.max_xml_bytes).ok())
		.and_then(|opf| reading_direction(&String::from_utf8_lossy(&opf)));

	// the package
	let values = |property: &str, max: usize, cap: usize| -> Vec<String> {
		doc.metadata
			.iter()
			.filter(|m| m.property == property)
			.map(|m| text(&m.value, cap))
			.filter(|v| !v.is_empty())
			.take(max)
			.collect()
	};
	let language = values("language", 1, 35)
		.into_iter()
		.next()
		.unwrap_or_else(|| "und".to_string());
	let creators = values("creator", 50, 300);
	let subjects = values("subject", 50, 200);
	let publisher = values("publisher", 1, 300).into_iter().next();
	let date = values("date", 1, 40).into_iter().next();
	let description = values("description", 1, 20_000).into_iter().next();
	let series = values("calibre:series", 1, 300).into_iter().next();
	let series_index = values("calibre:series_index", 1, 20).into_iter().next();
	let total: u64 = kept.values().map(|k| k.size).sum();
	let identifier = {
		let mut seed = format!("{title}\0{}\0{}\0{total}", creators.join("\0"), kept.len());
		for name in kept.keys() {
			seed.push('\0');
			seed.push_str(name);
		}
		format!(
			"urn:stump:{}",
			&hex(digest::digest(&digest::SHA256, seed.as_bytes()).as_ref())[..32]
		)
	};
	let cover = doc
		.get_cover_id()
		.and_then(|id| name_of_id.get(&id).cloned())
		.filter(|name| matches!(kept.get(name).map(|k| k.class), Some(Class::Image(_))));

	// names of the files that are made here: none may be a name of the book's own
	let free = |wanted: &str| {
		let (stem, ext) = wanted.rsplit_once('.').unwrap_or((wanted, ""));
		let mut name = wanted.to_string();
		let mut n = 0;
		while kept.contains_key(&name) {
			n += 1;
			name = format!("{stem}-{n}.{ext}");
		}
		name
	};
	let (opf_name, ncx_name, nav_name) = (free("content.opf"), free("toc.ncx"), free("nav.xhtml"));

	let mut opf = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"bookid\" prefix=\"rendition: http://www.idpf.org/vocab/rendition/#\" xml:lang=\"en\">\n<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n");
	opf.push_str(&format!("<dc:identifier id=\"bookid\">{}</dc:identifier>\n", xml(&identifier)));
	opf.push_str(&format!("<dc:title>{}</dc:title>\n", xml(&title)));
	opf.push_str(&format!("<dc:language>{}</dc:language>\n", xml(&language)));
	for creator in &creators {
		opf.push_str(&format!("<dc:creator>{}</dc:creator>\n", xml(creator)));
	}
	if let Some(publisher) = &publisher {
		opf.push_str(&format!("<dc:publisher>{}</dc:publisher>\n", xml(publisher)));
	}
	if let Some(date) = &date {
		opf.push_str(&format!("<dc:date>{}</dc:date>\n", xml(date)));
	}
	for subject in &subjects {
		opf.push_str(&format!("<dc:subject>{}</dc:subject>\n", xml(subject)));
	}
	if let Some(description) = &description {
		opf.push_str(&format!("<dc:description>{}</dc:description>\n", xml(description)));
	}
	if let Some(series) = &series {
		opf.push_str(&format!("<meta name=\"calibre:series\" content=\"{}\"/>\n", xml(series)));
		if let Some(index) = &series_index {
			opf.push_str(&format!("<meta name=\"calibre:series_index\" content=\"{}\"/>\n", xml(index)));
		}
	}
	// what makes a fixed-layout book (a picture book, a cookbook, a comic) one, and a manga read
	// from the right
	for meta in doc.metadata.iter().take(2_000) {
		match meta.property.as_str() {
			"rendition:layout" | "rendition:orientation" | "rendition:spread" => {
				let value = text(&meta.value, 40);
				if value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !value.is_empty() {
					opf.push_str(&format!(
						"<meta property=\"{}\">{}</meta>\n",
						meta.property,
						xml(&value)
					));
				}
			},
			"fixed-layout" | "original-resolution" | "primary-writing-mode" | "orientation-lock" => {
				let value = text(&meta.value, 40);
				if value.chars().all(|c| c.is_ascii_alphanumeric() || "-x_.".contains(c)) && !value.is_empty() {
					opf.push_str(&format!(
						"<meta name=\"{}\" content=\"{}\"/>\n",
						meta.property,
						xml(&value)
					));
				}
			},
			_ => {},
		}
	}
	if let Some(cover) = &cover {
		opf.push_str(&format!("<meta name=\"cover\" content=\"{}\"/>\n", id_of_name[cover.as_str()]));
	}
	opf.push_str("<meta property=\"dcterms:modified\">2000-01-01T00:00:00Z</meta>\n</metadata>\n<manifest>\n");
	opf.push_str(&format!("<item id=\"ncx\" href=\"{}\" media-type=\"application/x-dtbncx+xml\"/>\n", xml(&ncx_name)));
	opf.push_str(&format!("<item id=\"nav\" href=\"{}\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n", xml(&nav_name)));
	for (name, entry) in &kept {
		let properties = if cover.as_deref() == Some(name.as_str()) {
			" properties=\"cover-image\""
		} else {
			""
		};
		opf.push_str(&format!(
			"<item id=\"{}\" href=\"{}\" media-type=\"{}\"{properties}/>\n",
			id_of_name[name.as_str()],
			xml(name),
			entry.class.media_type()
		));
	}
	opf.push_str(&format!(
		"</manifest>\n<spine toc=\"ncx\"{}>\n",
		direction
			.map(|d| format!(" page-progression-direction=\"{d}\""))
			.unwrap_or_default()
	));
	for (name, linear, properties) in &spine {
		opf.push_str(&format!(
			"<itemref idref=\"{}\"{}{}/>\n",
			id_of_name[name.as_str()],
			if *linear { "" } else { " linear=\"no\"" },
			if properties.is_empty() {
				String::new()
			} else {
				format!(" properties=\"{properties}\"")
			}
		));
	}
	opf.push_str("</spine>\n</package>\n");

	let mut ncx = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\" version=\"2005-1\">\n<head>");
	ncx.push_str(&format!(
		"<meta name=\"dtb:uid\" content=\"{}\"/><meta name=\"dtb:depth\" content=\"{}\"/><meta name=\"dtb:totalPageCount\" content=\"0\"/><meta name=\"dtb:maxPageNumber\" content=\"0\"/></head>\n",
		xml(&identifier),
		depth_of(&toc)
	));
	ncx.push_str(&format!("<docTitle><text>{}</text></docTitle>\n<navMap>\n", xml(&title)));
	let mut order = 0u64;
	ncx_points(&toc, &mut ncx, &mut order);
	ncx.push_str("</navMap>\n</ncx>\n");

	let mut nav = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><head><title>");
	nav.push_str(&xml(&title));
	nav.push_str("</title></head><body><nav epub:type=\"toc\" id=\"toc\"><h1>");
	nav.push_str(&xml(&title));
	nav.push_str("</h1>");
	nav_items(&toc, &mut nav);
	nav.push_str("</nav></body></html>\n");

	let container = format!(
		"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n<rootfiles><rootfile full-path=\"{}\" media-type=\"application/oebps-package+xml\"/></rootfiles>\n</container>\n",
		xml(&opf_name)
	);

	// the archive
	let io = |error: &dyn std::fmt::Display| fail(format!("The book could not be rebuilt: {error}"));
	let mut writer = ZipWriter::new(File::create(output).map_err(|e| io(&e))?);
	writer.start_file("mimetype", stored()).map_err(|e| io(&e))?;
	writer.write_all(b"application/epub+zip").map_err(|e| io(&e))?;
	for (name, bytes) in [
		("META-INF/container.xml", container.as_bytes()),
		(opf_name.as_str(), opf.as_bytes()),
		(ncx_name.as_str(), ncx.as_bytes()),
		(nav_name.as_str(), nav.as_bytes()),
	] {
		writer.start_file(name, deflated()).map_err(|e| io(&e))?;
		writer.write_all(bytes).map_err(|e| io(&e))?;
	}
	for (name, entry) in &kept {
		let options = match entry.class {
			Class::Image(_) | Class::Font(_) => stored(),
			_ => deflated(),
		};
		writer.start_file(name.as_str(), options).map_err(|e| io(&e))?;
		match &entry.source {
			Source::Made(bytes) => writer.write_all(bytes).map_err(|e| io(&e))?,
			Source::Entry(index) => {
				let mut file = archive.by_index(*index).map_err(|e| io(&e))?;
				std::io::copy(&mut file, &mut writer).map_err(|e| io(&e))?;
			},
		}
	}
	writer.finish().map_err(|e| io(&e))?;

	// what was written reads back the way it was meant to
	let back = crate::media::processor::epub::open_epub(output).map_err(|e| {
		fail(format!("The rebuilt book can't be opened: {e}"))
	})?;
	let written: HashSet<&str> = kept.keys().map(String::as_str).collect();
	let reads_back = back.root_file.to_str() == Some(opf_name.as_str())
		&& back.spine.len() == spine.len()
		&& back.resources.len() == kept.len() + 2
		&& back.resources.values().all(|resource| {
			let path = resource.path.to_string_lossy();
			written.contains(&*path) || path == ncx_name || path == nav_name
		});
	if !reads_back {
		return Err(fail("The rebuilt book does not read back the way it was written"));
	}

	// what the approver is told
	let mut summary = ActiveSummary {
		remote_refs,
		..ActiveSummary::default()
	};
	for (name, findings) in &findings_of {
		for (kind, count) in findings {
			summary.found += count;
			if summary.items.len() < MAX_ACTIVE_ITEMS {
				summary.items.push(ActiveItem {
					kind: kind.clone(),
					entry: printable(name),
					count: *count,
				});
			}
		}
	}
	summary.stripped = summary.found > 0;
	let mut warnings = vec![];
	if dropped_pages > 0 {
		warnings.push(Reason::new(
			code::MALFORMED_XHTML,
			format!("{dropped_pages} pages could not be read as pages and are left out of the book"),
		));
	}
	if dropped_files + odd_names > 0 {
		warnings.push(Reason::new(
			code::DROPPED_FILES,
			format!(
				"{} files of kinds a book is not filed with (scripts, audio, video, other data) or with names that can't be used are left out",
				dropped_files + odd_names
			),
		));
	}
	Ok(Rebuilt {
		active: summary,
		warnings,
		pages: spine.len() as u64,
	})
}

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use super::*;
	use crate::intake::probe::zipscan;

	fn png(width: u32, height: u32) -> Vec<u8> {
		let image = image::RgbImage::new(width, height);
		let mut out = Cursor::new(Vec::new());
		image.write_to(&mut out, image::ImageFormat::Png).unwrap();
		out.into_inner()
	}

	const CONTAINER: &str = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;

	fn opf(items: &str, spine: &str) -> String {
		format!(
			r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:1</dc:identifier><dc:title>A Book</dc:title><dc:creator>Ana Writer</dc:creator><dc:language>es</dc:language><dc:description>Plain</dc:description></metadata><manifest>{items}</manifest><spine toc="ncx">{spine}</spine></package>"#
		)
	}

	fn ncx(points: &str) -> String {
		format!(
			r#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head/><docTitle><text>A Book</text></docTitle><navMap>{points}</navMap></ncx>"#
		)
	}

	const PAGE: &str = r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>One</title><link rel="stylesheet" type="text/css" href="s.css"/></head><body><h1 id="t">Uno</h1><p>hello<a id="n"/> <a epub:type="noteref" href="c2.xhtml#x">1</a></p><img src="i.png" alt="x"/></body></html>"#;

	struct Made {
		_dir: tempfile::TempDir,
		out: std::path::PathBuf,
		result: Result<Rebuilt, Reason>,
	}

	impl Made {
		fn entries(&self) -> BTreeMap<String, Vec<u8>> {
			let mut archive = ZipArchive::new(File::open(&self.out).unwrap()).unwrap();
			let mut all = BTreeMap::new();
			for i in 0..archive.len() {
				let mut file = archive.by_index(i).unwrap();
				let mut bytes = Vec::new();
				std::io::Read::read_to_end(&mut file, &mut bytes).unwrap();
				all.insert(file.name().to_string(), bytes);
			}
			all
		}

		fn text(&self, name: &str) -> String {
			String::from_utf8(self.entries()[name].clone()).unwrap()
		}
	}

	fn build(container: &str, files: &[(&str, Vec<u8>)], limits: &Limits) -> Made {
		let dir = tempfile::tempdir().unwrap();
		let input = dir.path().join("in.epub");
		let mut writer = ZipWriter::new(File::create(&input).unwrap());
		writer.start_file("mimetype", stored()).unwrap();
		writer.write_all(b"application/epub+zip").unwrap();
		writer
			.start_file("META-INF/container.xml", deflated())
			.unwrap();
		writer.write_all(container.as_bytes()).unwrap();
		for (name, bytes) in files {
			writer.start_file(*name, deflated()).unwrap();
			writer.write_all(bytes).unwrap();
		}
		writer.finish().unwrap();
		let report = zipscan::scan(&input, limits).expect("a zip");
		let out = dir.path().join("out.epub");
		let result = rebuild(&input, &out, &report, limits);
		Made {
			_dir: dir,
			out,
			result,
		}
	}

	fn depth(points: &[NavPoint]) -> usize {
		points.iter().map(|p| 1 + depth(&p.children)).max().unwrap_or(0)
	}

	fn count(points: &[NavPoint]) -> usize {
		points.iter().map(|p| 1 + count(&p.children)).sum()
	}

	#[test]
	fn a_book_is_written_anew_and_reads_back_the_way_it_was_read() {
		let items = r#"<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/><item id="c2" href="c2.xhtml" media-type="application/xhtml+xml"/><item id="css" href="s.css" media-type="text/css"/><item id="img" href="i.png" media-type="image/png" properties="cover-image"/><item id="f" href="f.ttf" media-type="font/ttf"/>"#;
		let spine = r#"<itemref idref="c1"/><itemref idref="c2" linear="no"/>"#;
		let toc = ncx(r#"<navPoint id="a" playOrder="1"><navLabel><text>Uno</text></navLabel><content src="c1.xhtml"/><navPoint id="b" playOrder="2"><navLabel><text>Dos</text></navLabel><content src="c2.xhtml#x"/></navPoint></navPoint>"#);
		let image = png(4, 4);
		let made = build(
			CONTAINER,
			&[
				("OEBPS/content.opf", opf(items, spine).into_bytes()),
				("OEBPS/toc.ncx", toc.into_bytes()),
				("OEBPS/c1.xhtml", PAGE.as_bytes().to_vec()),
				("OEBPS/c2.xhtml", PAGE.replace("Uno", "Dos").into_bytes()),
				(
					"OEBPS/s.css",
					b"p{color:red}@import url(http://e.example/a.css);".to_vec(),
				),
				("OEBPS/i.png", image.clone()),
				("OEBPS/f.ttf", [vec![0u8, 1, 0, 0], vec![0; 100]].concat()),
			],
			&Limits::default(),
		);
		let rebuilt = made.result.as_ref().expect("rebuilt");
		assert_eq!(rebuilt.pages, 2);
		assert!(rebuilt.warnings.is_empty(), "{:?}", rebuilt.warnings);
		assert_eq!(rebuilt.active.found, 0);
		// the mimetype comes first, stored
		let mut archive = ZipArchive::new(File::open(&made.out).unwrap()).unwrap();
		let first = archive.by_index(0).unwrap();
		assert_eq!(first.name(), "mimetype");
		assert_eq!(first.compression(), CompressionMethod::Stored);
		assert_eq!(first.size(), 20);
		drop(first);
		drop(archive);
		let entries = made.entries();
		assert_eq!(entries["OEBPS/i.png"], image);
		let css = made.text("OEBPS/s.css");
		assert!(css.contains("p{color:red}") && !css.contains("import"), "{css}");
		let page = made.text("OEBPS/c1.xhtml");
		for kept in [
			"<h1 id=\"t\">Uno</h1>",
			"<a id=\"n\"></a>",
			"epub:type=\"noteref\"",
			"href=\"c2.xhtml#x\"",
			"<img src=\"i.png\" alt=\"x\"/>",
			"href=\"s.css\"",
		] {
			assert!(page.contains(kept), "{kept} is not in {page}");
		}
		// what Stump's reader makes of what was written
		let book = crate::media::processor::epub::open_epub(&made.out).unwrap();
		assert_eq!(book.spine.len(), 2);
		assert!(!book.spine[1].linear);
		assert_eq!(book.resources.len(), 7);
		assert_eq!(book.mdata("title").unwrap().value, "A Book");
		assert!(book.get_cover_id().is_some());
		assert_eq!(book.toc[0].label, "Uno");
		assert_eq!(book.toc[0].children[0].label, "Dos");
		assert!(book.toc[0].children[0]
			.content
			.to_string_lossy()
			.ends_with("OEBPS/c2.xhtml#x"));
	}

	#[test]
	fn the_ways_round_a_checker_put_no_script_in_the_book() {
		let hidden = r#"<?xml version="1.0"?><!DOCTYPE html SYSTEM "<"><html xmlns="http://www.w3.org/1999/xhtml"><head><title>t</title></head><body><p>one</p><script>alert(1)</script><img src="x.png" onerror="alert(2)"/></body></html><!-- > -->"#;
		let scripted = r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>two</p><script>alert(3)</script></body></html>"#;
		let decoy = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><ROOTFILE full-path="evil.opf" media-type="application/oebps-package+xml"/><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
		let items = r#"<item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/><item id="p" href="p.dat" media-type="text/html"/><x id="d" href="d.dat" media-type="text/html"/><item id="e" Media-Type="image/png" media-type="text/html" href="e.dat"/><item id="g" href="g.dat" media-type="text/html&nbsp;"/><item id="q" href="a?b.xhtml" media-type="application/xhtml+xml"/><item id="m" href="../META-INF/m.xhtml" media-type="application/xhtml+xml"/><item id="js" href="app.js" media-type="text/javascript"/>"#;
		let spine = r#"<itemref idref="c1"/><itemref idref="p"/><itemref idref="d"/><itemref idref="e"/><itemref idref="g"/><itemref idref="q"/><itemref idref="m"/>"#;
		let made = build(
			decoy,
			&[
				("OEBPS/content.opf", opf(items, spine).into_bytes()),
				("OEBPS/c1.xhtml", hidden.as_bytes().to_vec()),
				("OEBPS/p.dat", b"x<img src=x onerror=alert(4)>".to_vec()),
				("OEBPS/d.dat", scripted.as_bytes().to_vec()),
				("OEBPS/e.dat", scripted.as_bytes().to_vec()),
				("OEBPS/g.dat", scripted.as_bytes().to_vec()),
				("OEBPS/a?b.xhtml", scripted.as_bytes().to_vec()),
				("META-INF/m.xhtml", scripted.as_bytes().to_vec()),
				("OEBPS/app.js", b"alert(5)".to_vec()),
				("evil.html", scripted.as_bytes().to_vec()),
				("OEBPS/evil.svg", b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(6)</script></svg>".to_vec()),
			],
			&Limits::default(),
		);
		let rebuilt = made.result.as_ref().expect("rebuilt");
		let entries = made.entries();
		let names: Vec<&str> = entries.keys().map(String::as_str).collect();
		assert_eq!(
			names,
			[
				"META-INF/container.xml",
				"META-INF/m.xhtml",
				"OEBPS/c1.xhtml",
				"content.opf",
				"mimetype",
				"nav.xhtml",
				"toc.ncx"
			]
		);
		for (name, bytes) in &entries {
			let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
			for bad in ["alert(", "onerror", "<script", "rootfile full-path=\"evil"] {
				assert!(!text.contains(bad), "{bad} is in {name}: {text}");
			}
		}
		assert!(made.text("OEBPS/c1.xhtml").contains("<p>one</p>"));
		assert_eq!(rebuilt.pages, 2);
		// the script and the handler of the first page, the script of the page in META-INF
		assert_eq!(rebuilt.active.found, 3);
		assert!(rebuilt.warnings.iter().any(|w| w.code == code::DROPPED_FILES));
	}

	#[test]
	fn a_book_with_no_page_that_can_be_read_is_not_made() {
		let items = r#"<item id="js" href="app.js" media-type="text/javascript"/>"#;
		let made = build(
			CONTAINER,
			&[
				("OEBPS/content.opf", opf(items, r#"<itemref idref="js"/>"#).into_bytes()),
				("OEBPS/app.js", b"alert(1)".to_vec()),
			],
			&Limits::default(),
		);
		assert_eq!(made.result.unwrap_err().code, code::NO_READABLE_PAGES);
		assert!(!made.out.exists());
	}

	#[test]
	fn a_table_of_contents_is_written_no_deeper_and_no_longer_than_the_limits() {
		let items = r#"<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/>"#;
		let mut points = String::new();
		for n in 0..50 {
			points.push_str(&format!(
				"<navPoint id=\"p{n}\" playOrder=\"{}\"><navLabel><text>L{n}</text></navLabel><content src=\"c1.xhtml#a{n}\"/>",
				n + 1
			));
		}
		points.push_str(&"</navPoint>".repeat(50));
		let files = [
			("OEBPS/content.opf", opf(items, r#"<itemref idref="c1"/>"#).into_bytes()),
			("OEBPS/toc.ncx", ncx(&points).into_bytes()),
			("OEBPS/c1.xhtml", PAGE.as_bytes().to_vec()),
		];
		let made = build(CONTAINER, &files, &Limits::default());
		made.result.as_ref().expect("rebuilt");
		let book = crate::media::processor::epub::open_epub(&made.out).unwrap();
		assert_eq!(depth(&book.toc), MAX_TOC_DEPTH);
		let tight = Limits {
			max_toc_entries: 5,
			..Limits::default()
		};
		let made = build(CONTAINER, &files, &tight);
		made.result.as_ref().expect("rebuilt");
		let book = crate::media::processor::epub::open_epub(&made.out).unwrap();
		assert_eq!(count(&book.toc), 5);
	}

	#[test]
	fn an_image_over_the_limits_is_refused_and_leaves_nothing() {
		let items = r#"<item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/><item id="img" href="i.png" media-type="image/png"/>"#;
		let tight = Limits {
			max_image_pixels: 100,
			..Limits::default()
		};
		let made = build(
			CONTAINER,
			&[
				("OEBPS/content.opf", opf(items, r#"<itemref idref="c1"/>"#).into_bytes()),
				("OEBPS/c1.xhtml", PAGE.as_bytes().to_vec()),
				("OEBPS/i.png", png(20, 20)),
			],
			&tight,
		);
		assert!(made.result.is_err());
		assert!(!made.out.exists());
	}

	#[test]
	fn svg_and_style_sheets_are_rebuilt_and_a_font_that_is_not_one_is_left_out() {
		let items = r#"<item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/><item id="v" href="c.svg" media-type="image/svg+xml"/><item id="css" href="s.css" media-type="text/css"/><item id="f" href="f.ttf" media-type="font/ttf"/>"#;
		let made = build(
			CONTAINER,
			&[
				("OEBPS/content.opf", opf(items, r#"<itemref idref="c1"/>"#).into_bytes()),
				("OEBPS/c1.xhtml", PAGE.as_bytes().to_vec()),
				(
					"OEBPS/c.svg",
					b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"><script>alert(1)</script><rect width=\"1\" height=\"1\" onclick=\"x\"/></svg>".to_vec(),
				),
				(
					"OEBPS/s.css",
					b"@import 'x.css'; a{background:url(http://e.example/x)} b{color:blue}".to_vec(),
				),
				("OEBPS/f.ttf", b"<html><script>alert(2)</script>".to_vec()),
			],
			&Limits::default(),
		);
		let rebuilt = made.result.as_ref().expect("rebuilt");
		let entries = made.entries();
		assert!(!entries.contains_key("OEBPS/f.ttf"));
		let svg = made.text("OEBPS/c.svg");
		assert!(svg.contains("<rect") && !svg.contains("script") && !svg.contains("onclick"));
		let css = made.text("OEBPS/s.css");
		assert!(css.contains("b{color:blue}") && !css.contains("e.example") && !css.contains("import"));
		assert_eq!(rebuilt.active.found, 2);
		assert!(rebuilt.warnings.iter().any(|w| w.code == code::DROPPED_FILES));
	}

	#[test]
	fn names_with_spaces_and_letters_of_any_script_survive_and_names_that_cannot_are_left_out() {
		let items = r#"<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="c1" href="Ca pítulo 1.xhtml" media-type="application/xhtml+xml"/><item id="c2" href="a%20b.xhtml" media-type="application/xhtml+xml"/>"#;
		let toc = ncx(r#"<navPoint id="a" playOrder="1"><navLabel><text>Capítulo</text></navLabel><content src="Ca pítulo 1.xhtml"/></navPoint>"#);
		let made = build(
			CONTAINER,
			&[
				("OEBPS/content.opf", opf(items, r#"<itemref idref="c1"/><itemref idref="c2"/>"#).into_bytes()),
				("OEBPS/toc.ncx", toc.into_bytes()),
				("OEBPS/Ca pítulo 1.xhtml", PAGE.as_bytes().to_vec()),
				("OEBPS/a%20b.xhtml", PAGE.as_bytes().to_vec()),
			],
			&Limits::default(),
		);
		let rebuilt = made.result.as_ref().expect("rebuilt");
		assert_eq!(rebuilt.pages, 1);
		assert!(made.entries().contains_key("OEBPS/Ca pítulo 1.xhtml"));
		assert!(!made.entries().contains_key("OEBPS/a%20b.xhtml"));
		let book = crate::media::processor::epub::open_epub(&made.out).unwrap();
		assert_eq!(
			book.toc[0].content.to_string_lossy(),
			"OEBPS/Ca pítulo 1.xhtml"
		);
	}

	fn jpeg_with_a_long_profile(profile_bytes: usize) -> Vec<u8> {
		let mut out = vec![0xFF, 0xD8];
		let mut left = profile_bytes;
		while left > 0 {
			let n = left.min(60_000);
			out.extend([0xFF, 0xE2]);
			out.extend(((n + 2) as u16).to_be_bytes());
			out.extend(std::iter::repeat(0u8).take(n));
			left -= n;
		}
		out.extend([0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x10, 0x00, 0x10, 0x03, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
		out.extend([0xFF, 0xD9]);
		out
	}

	#[test]
	fn an_image_with_a_long_profile_before_its_size_is_kept_and_too_many_pages_are_refused() {
		let items = r#"<item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/><item id="img" href="i.jpg" media-type="image/jpeg"/>"#;
		let made = build(
			CONTAINER,
			&[
				("OEBPS/content.opf", opf(items, r#"<itemref idref="c1"/>"#).into_bytes()),
				("OEBPS/c1.xhtml", PAGE.as_bytes().to_vec()),
				("OEBPS/i.jpg", jpeg_with_a_long_profile(400_000)),
			],
			&Limits::default(),
		);
		made.result.as_ref().expect("rebuilt");
		assert!(made.entries().contains_key("OEBPS/i.jpg"));
		let items = r#"<item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/>"#;
		let tight = Limits {
			max_spine_items: 3,
			..Limits::default()
		};
		let made = build(
			CONTAINER,
			&[
				("OEBPS/content.opf", opf(items, &r#"<itemref idref="c1"/>"#.repeat(5)).into_bytes()),
				("OEBPS/c1.xhtml", PAGE.as_bytes().to_vec()),
			],
			&tight,
		);
		assert_eq!(made.result.unwrap_err().code, code::TOO_MANY_SPINE_ITEMS);
		assert!(!made.out.exists());
	}

	#[test]
	fn a_fixed_layout_book_stays_one_and_a_manga_still_goes_from_the_right() {
		let package = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id" prefix="rendition: http://www.idpf.org/vocab/rendition/#"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">x</dc:identifier><dc:title>Picture book</dc:title><dc:language>en</dc:language><meta property="rendition:layout">pre-paginated</meta><meta property="rendition:spread">landscape</meta><meta name="fixed-layout" content="true"/></metadata><manifest><item id="p1" href="p1.xhtml" media-type="application/xhtml+xml"/><item id="p2" href="p2.xhtml" media-type="application/xhtml+xml"/></manifest><spine page-progression-direction="rtl"><itemref idref="p1" properties="page-spread-right scripted"/><itemref idref="p2" properties="rendition:page-spread-left"/></spine></package>"#;
		let made = build(
			CONTAINER,
			&[
				("OEBPS/content.opf", package.as_bytes().to_vec()),
				("OEBPS/p1.xhtml", PAGE.as_bytes().to_vec()),
				("OEBPS/p2.xhtml", PAGE.as_bytes().to_vec()),
			],
			&Limits::default(),
		);
		made.result.as_ref().expect("rebuilt");
		let out = made.text("content.opf");
		assert!(out.contains("page-progression-direction=\"rtl\""), "{out}");
		let rendition = crate::readium::generator::read_package_rendition(&out);
		assert!(rendition.fixed);
		assert_eq!(rendition.page_spreads.len(), 2);
		assert!(!out.contains("scripted"));
	}

	#[test]
	fn a_page_that_is_too_big_is_left_out_and_told_and_the_rest_is_made() {
		let items = r#"<item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/><item id="c2" href="c2.xhtml" media-type="application/xhtml+xml"/>"#;
		// text that doesn't compress (the archive is held to a ratio)
		let mut state = 0x2545_f491_4f6c_dd1du64;
		let mut big = String::from("<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>");
		while big.len() < 1_600_000 {
			state ^= state << 13;
			state ^= state >> 7;
			state ^= state << 17;
			big.push((b'a' + (state % 26) as u8) as char);
		}
		big.push_str("</p></body></html>");
		let tight = Limits {
			max_xml_bytes: 1,
			..Limits::default()
		};
		let made = build(
			CONTAINER,
			&[
				("OEBPS/content.opf", opf(items, r#"<itemref idref="c1"/><itemref idref="c2"/>"#).into_bytes()),
				("OEBPS/c1.xhtml", PAGE.as_bytes().to_vec()),
				("OEBPS/c2.xhtml", big.into_bytes()),
			],
			&tight,
		);
		let rebuilt = made.result.as_ref().expect("rebuilt");
		assert_eq!(rebuilt.pages, 1);
		assert!(rebuilt.warnings.iter().any(|w| w.code == code::MALFORMED_XHTML));
	}

	#[test]
	fn a_dictionary_sized_book_is_made_and_the_readers_routes_run_on_it_in_bounded_time() {
		// 8,232 pages is the biggest real book of the owner's, a dictionary; the reader makes its
		// manifest and its positions for every request that asks, so what they cost is checked on the
		// book that was made, not guessed from what was sent
		const PAGES: usize = 6_000;
		let mut items = String::from(
			r#"<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>"#,
		);
		let (mut spine, mut points) = (String::new(), String::new());
		let mut files: Vec<(String, Vec<u8>)> = Vec::new();
		for n in 0..PAGES {
			items.push_str(&format!(
				"<item id=\"c{n}\" href=\"c{n}.xhtml\" media-type=\"application/xhtml+xml\"/>"
			));
			spine.push_str(&format!("<itemref idref=\"c{n}\"/>"));
			points.push_str(&format!(
				"<navPoint id=\"p{n}\" playOrder=\"{}\"><navLabel><text>Entry {n}</text></navLabel><content src=\"c{n}.xhtml\"/></navPoint>",
				n + 1
			));
			files.push((
				format!("OEBPS/c{n}.xhtml"),
				format!(
					"<?xml version=\"1.0\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>t</title></head><body><p>Entry {n}</p></body></html>"
				)
				.into_bytes(),
			));
		}
		let mut all = vec![
			("OEBPS/content.opf".to_string(), opf(&items, &spine).into_bytes()),
			("OEBPS/toc.ncx".to_string(), ncx(&points).into_bytes()),
		];
		all.extend(files);
		let refs: Vec<(&str, Vec<u8>)> = all.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
		let made = build(CONTAINER, &refs, &Limits::default());
		let rebuilt = made.result.as_ref().expect("a dictionary is made");
		assert_eq!(rebuilt.pages as usize, PAGES);
		let started = std::time::Instant::now();
		super::super::check::check_reader_routes(&made.out).expect("the routes run on it");
		// a debug build, and a table of contents found for every page by its path at once (a search
		// of the whole table for every page is quadratic, and this is the size where that shows)
		assert!(started.elapsed().as_secs() < 30, "{:?}", started.elapsed());
	}

	/// The rebuild on a real library (`STUMP_PROBE_CALIBRATE_DIR`, at most
	/// `STUMP_PROBE_CALIBRATE_MAX` EPUBs, spread over it): every book of the owner's that a rebuild
	/// refuses, or loses pages of, for no good reason is a rebuild that is too strict
	#[test]
	#[ignore]
	fn rebuild_on_a_real_library() {
		let Ok(dir) = std::env::var("STUMP_PROBE_CALIBRATE_DIR") else {
			return;
		};
		let max: usize = std::env::var("STUMP_PROBE_CALIBRATE_MAX")
			.ok()
			.and_then(|v| v.parse().ok())
			.unwrap_or(60);
		let mut found: Vec<std::path::PathBuf> = walkdir::WalkDir::new(&dir)
			.into_iter()
			.flatten()
			.filter(|e| e.file_type().is_file())
			.map(|e| e.into_path())
			.filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("epub")))
			.collect();
		found.sort();
		let step = (found.len() / max.max(1)).max(1);
		let limits = Limits::default();
		let out = tempfile::tempdir().unwrap();
		let (mut ok, mut refused, mut pages_in, mut pages_out) = (0u64, 0u64, 0u64, 0u64);
		let mut by_code: BTreeMap<String, u64> = BTreeMap::new();
		let (mut warned_pages, mut warned_files, mut slowest) = (0u64, 0u64, (0u128, String::new()));
		for path in found.iter().step_by(step).take(max) {
			let started = std::time::Instant::now();
			let Ok(report) = zipscan::scan(path, &limits) else {
				continue;
			};
			let spine_in = crate::media::processor::epub::open_epub(path)
				.map(|b| b.spine.len() as u64)
				.unwrap_or(0);
			let target = out.path().join("out.epub");
			match rebuild(path, &target, &report, &limits) {
				Ok(made) => {
					ok += 1;
					pages_in += spine_in;
					pages_out += made.pages;
					warned_pages += u64::from(made.warnings.iter().any(|w| w.code == code::MALFORMED_XHTML));
					warned_files += u64::from(made.warnings.iter().any(|w| w.code == code::DROPPED_FILES));
				},
				Err(reason) => {
					refused += 1;
					*by_code.entry(reason.code.clone()).or_default() += 1;
					eprintln!("refused {}: {} {}", path.display(), reason.code, reason.detail);
				},
			}
			let took = started.elapsed().as_millis();
			if took > slowest.0 {
				slowest = (took, path.display().to_string());
			}
		}
		eprintln!(
			"rebuilt {ok}, refused {refused} {by_code:?}; pages {pages_out} of {pages_in}; books told about pages {warned_pages}, about files {warned_files}; slowest {} ms ({})",
			slowest.0, slowest.1
		);
		assert!(!by_code.contains_key(code::OUTPUT_CHECK_FAILED), "{by_code:?}");
		assert!(pages_out * 100 >= pages_in * 95, "{pages_out} pages of {pages_in}");
	}
}
