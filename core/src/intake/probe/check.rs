//! The checks of each format, run by the probe's child on a file it has not read before.
//!
//! Every limit comes from the request. What is checked here is what Stump's own readers would later
//! do with the file in the server: so the checks read it the way they do, under the same caps, and a
//! file that gets through is one that those readers finished on.

use std::{
	collections::HashMap,
	fs::File,
	io::{Cursor, Read},
	path::Path,
};

use quick_xml::{events::Event, Reader};
use zip::ZipArchive;

use super::{
	active::{self, Outcome},
	contract::{
		code, is_safe_relative, ActiveContent, ActiveItem, ActiveSummary, Limits, Reason,
	},
	zipscan::{name_problem, printable, ZipReport},
};
use crate::fs_utils::bounded;

const MIB: u64 = 1024 * 1024;
/// The most bytes of an image that are looked at for its size (a JPEG's can be after a long profile)
const IMAGE_HEAD_BYTES: u64 = 256 * 1024;
/// The side of an image that no book has
const MAX_IMAGE_SIDE: u64 = 30_000;
/// How many places that carry active content are listed
const MAX_ACTIVE_ITEMS: usize = 30;

/// The extensions of the entries that are the pages of a comic
pub const IMAGE_EXTENSIONS: &[&str] = &[
	"jpg", "jpeg", "png", "gif", "webp", "avif", "jxl", "bmp", "tif", "tiff", "heic",
];

pub fn extension_of(name: &str) -> String {
	name.rsplit_once('.')
		.map(|(_, extension)| extension.to_ascii_lowercase())
		.unwrap_or_default()
}

pub fn is_image_name(name: &str) -> bool {
	IMAGE_EXTENSIONS.contains(&extension_of(name).as_str())
}

/// Whether a name is one Stump skips (a hidden file, a macOS resource fork)
pub fn is_hidden(name: &str) -> bool {
	name.split(['/', '\\'])
		.any(|part| part.starts_with('.') || part == "__MACOSX")
}

/// The kinds of image a book may carry that the server decodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
	Jpeg,
	Png,
	Gif,
	Webp,
	/// Looked at by its header only: it is never decoded by the server
	Avif,
}

/// What an image is, by its first bytes (never by its name)
pub fn image_kind(head: &[u8]) -> Option<ImageKind> {
	if head.starts_with(b"\xff\xd8\xff") {
		Some(ImageKind::Jpeg)
	} else if head.starts_with(b"\x89PNG\r\n\x1a\n") {
		Some(ImageKind::Png)
	} else if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
		Some(ImageKind::Gif)
	} else if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WEBP" {
		Some(ImageKind::Webp)
	} else if head.len() >= 12
		&& &head[4..8] == b"ftyp"
		&& matches!(&head[8..12], b"avif" | b"avis")
	{
		Some(ImageKind::Avif)
	} else {
		None
	}
}

fn format_of(kind: ImageKind) -> Option<image::ImageFormat> {
	match kind {
		ImageKind::Jpeg => Some(image::ImageFormat::Jpeg),
		ImageKind::Png => Some(image::ImageFormat::Png),
		ImageKind::Gif => Some(image::ImageFormat::Gif),
		ImageKind::Webp => Some(image::ImageFormat::WebP),
		ImageKind::Avif => None,
	}
}

/// Checks an image: it is of a kind the server decodes, its sides and pixels are within the
/// limits (read from its header, before anything is allocated), and, with `decode`, it decodes
/// under the allocation limit. `bytes` is the whole image for a decode, or its first bytes
pub fn check_image(
	name: &str,
	bytes: &[u8],
	decode: bool,
	limits: &Limits,
) -> Result<(), Reason> {
	let kind = image_kind(bytes).ok_or_else(|| {
		Reason::new(
			code::UNSUPPORTED_IMAGE,
			format!("{} is not a JPEG, PNG, GIF or WebP image", printable(name)),
		)
	})?;
	let size = imagesize::blob_size(bytes).map_err(|_| {
		Reason::new(
			code::IMAGE_UNDECODABLE,
			format!("The size of {} can't be read", printable(name)),
		)
	})?;
	let (width, height) = (size.width as u64, size.height as u64);
	if width == 0 || height == 0 {
		return Err(Reason::new(
			code::IMAGE_UNDECODABLE,
			format!("{} has no size", printable(name)),
		));
	}
	if width > MAX_IMAGE_SIDE
		|| height > MAX_IMAGE_SIDE
		|| width.saturating_mul(height) > limits.max_image_pixels
	{
		return Err(Reason::new(
			code::IMAGE_TOO_LARGE,
			format!(
				"{} is {width}x{height} pixels: more than a page can be",
				printable(name)
			),
		));
	}
	if decode {
		if let Some(format) = format_of(kind) {
			let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
			let mut decode_limits = image::Limits::default();
			decode_limits.max_image_width = Some(MAX_IMAGE_SIDE as u32);
			decode_limits.max_image_height = Some(MAX_IMAGE_SIDE as u32);
			decode_limits.max_alloc = Some(256 * MIB);
			reader.limits(decode_limits);
			reader.decode().map_err(|error| {
				Reason::new(
					code::IMAGE_UNDECODABLE,
					format!("{} can't be decoded: {error}", printable(name)),
				)
			})?;
		}
	}
	Ok(())
}

// ---------------------------------------------------------------------------------- XML

/// A look at an XML text that is read by Stump only (a package, a table of contents): how deep it
/// goes, and whether it declares entities. Whether it is well formed is returned, not required
pub fn xml_check(bytes: &[u8], limits: &Limits, what: &str) -> Result<bool, Reason> {
	let mut reader = Reader::from_reader(bytes);
	let mut depth = 0u32;
	loop {
		match reader.read_event() {
			Ok(Event::Eof) => return Ok(true),
			Ok(Event::Start(_)) => {
				depth += 1;
				if depth > limits.max_xml_depth {
					return Err(Reason::new(
						code::XML_TOO_DEEP,
						format!(
							"{what} nests deeper than {} levels",
							limits.max_xml_depth
						),
					));
				}
			},
			Ok(Event::End(_)) => depth = depth.saturating_sub(1),
			Ok(Event::DocType(text)) => {
				if text
					.windows(8)
					.any(|window| window.eq_ignore_ascii_case(b"<!entity"))
				{
					return Err(Reason::new(
						code::XML_ENTITY,
						format!("{what} declares entities"),
					));
				}
			},
			Ok(_) => {},
			Err(_) => return Ok(false),
		}
	}
}

fn attribute(tag: &quick_xml::events::BytesStart, name: &str) -> Option<String> {
	tag.attributes().flatten().find_map(|attribute| {
		let key = attribute.key.as_ref();
		let local = key.rsplit(|b| *b == b':').next().unwrap_or(key);
		local.eq_ignore_ascii_case(name.as_bytes()).then(|| {
			attribute
				.unescape_value()
				.map(|v| v.into_owned())
				.unwrap_or_else(|_| {
					String::from_utf8_lossy(&attribute.value).into_owned()
				})
		})
	})
}

/// The first `rootfile` of `META-INF/container.xml`
fn container_rootfile(bytes: &[u8], limits: &Limits) -> Result<String, Reason> {
	xml_check(bytes, limits, "container.xml")?;
	let mut reader = Reader::from_reader(bytes);
	loop {
		match reader.read_event() {
			Ok(Event::Start(tag)) | Ok(Event::Empty(tag)) => {
				if tag.local_name().as_ref().eq_ignore_ascii_case(b"rootfile") {
					if let Some(path) = attribute(&tag, "full-path") {
						return Ok(path);
					}
				}
			},
			Ok(Event::Eof) | Err(_) => break,
			Ok(_) => {},
		}
	}
	Err(Reason::new(
		code::NO_PACKAGE,
		"The book names no package file",
	))
}

/// One item of an OPF's manifest
pub struct Item {
	pub href: String,
	pub media_type: String,
	pub properties: String,
}

pub struct Package {
	pub items: Vec<Item>,
}

/// The manifest of an OPF, read strictly, under the limits
fn parse_package(bytes: &[u8], limits: &Limits) -> Result<Package, Reason> {
	let mut reader = Reader::from_reader(bytes);
	let (mut depth, mut spine) = (0u32, 0u64);
	let mut items: Vec<Item> = vec![];
	loop {
		match reader.read_event() {
			Ok(Event::Eof) => break,
			Ok(event @ (Event::Start(_) | Event::Empty(_))) => {
				let (tag, opens) = match &event {
					Event::Start(tag) => (tag, true),
					Event::Empty(tag) => (tag, false),
					_ => unreachable!("only tags are matched here"),
				};
				if opens {
					depth += 1;
					if depth > limits.max_xml_depth {
						return Err(Reason::new(
							code::XML_TOO_DEEP,
							"The package file nests too deep",
						));
					}
				}
				let local = tag.local_name();
				if local.as_ref().eq_ignore_ascii_case(b"item") {
					if items.len() as u64 >= limits.max_entries {
						return Err(Reason::new(
							code::TOO_MANY_ENTRIES,
							"The package lists more items than a book has",
						));
					}
					items.push(Item {
						href: attribute(tag, "href").unwrap_or_default(),
						media_type: attribute(tag, "media-type").unwrap_or_default(),
						properties: attribute(tag, "properties").unwrap_or_default(),
					});
				} else if local.as_ref().eq_ignore_ascii_case(b"itemref") {
					spine += 1;
					if spine > limits.max_spine_items {
						return Err(Reason::new(
							code::TOO_MANY_SPINE_ITEMS,
							format!(
								"The book has more than {} pages in its spine",
								limits.max_spine_items
							),
						));
					}
				}
			},
			Ok(Event::End(_)) => depth = depth.saturating_sub(1),
			Ok(Event::DocType(text)) => {
				if text
					.windows(8)
					.any(|window| window.eq_ignore_ascii_case(b"<!entity"))
				{
					return Err(Reason::new(
						code::XML_ENTITY,
						"The package file declares entities",
					));
				}
			},
			Ok(_) => {},
			Err(error) => {
				return Err(Reason::new(
					code::UNREADABLE,
					format!("The package file is not well formed: {error}"),
				))
			},
		}
	}
	Ok(Package { items })
}

/// An address inside an EPUB resolved against the folder of its package: the name of the entry it
/// is, or `None` when it points out of the book or at another site
fn resolve(base_dir: &str, href: &str) -> Option<String> {
	let href = href.split(['#', '?']).next().unwrap_or(href);
	if href.is_empty()
		|| href.contains("://")
		|| href.starts_with("//")
		|| href.starts_with("data:")
	{
		return None;
	}
	let decoded = urlencoding::decode(href).ok()?.into_owned();
	let mut parts: Vec<&str> = if decoded.starts_with('/') {
		vec![]
	} else {
		base_dir.split('/').filter(|p| !p.is_empty()).collect()
	};
	for part in decoded.split('/') {
		match part {
			"" | "." => {},
			".." => {
				parts.pop()?;
			},
			other => parts.push(other),
		}
	}
	Some(parts.join("/"))
}

// ---------------------------------------------------------------------------------- EPUB

#[derive(Debug)]
pub struct EpubFacts {
	pub active: ActiveSummary,
	pub warnings: Vec<Reason>,
}

fn read_entry(
	archive: &mut ZipArchive<File>,
	index: usize,
	name: &str,
	cap: u64,
) -> Result<Vec<u8>, Reason> {
	let entry = archive.by_index(index).map_err(|error| {
		Reason::new(
			code::UNREADABLE,
			format!("{} can't be opened: {error}", printable(name)),
		)
	})?;
	bounded::read_capped(entry, cap).map_err(|error| {
		if bounded::is_too_large(&error) {
			Reason::new(
				code::ENTRY_TOO_LARGE,
				format!(
					"{} is larger than the {} MB an entry can be",
					printable(name),
					cap / MIB
				),
			)
		} else {
			Reason::new(
				code::UNREADABLE,
				format!("{} is damaged: {error}", printable(name)),
			)
		}
	})
}

const FONT_OBFUSCATION: &[&str] = &[
	"http://www.idpf.org/2008/embedding",
	"http://ns.adobe.com/pdf/enc#rc",
];

/// What of an EPUB the server's readers will meet, checked under the limits
///
/// With `replacements`, the pages that carry active content are also collected as they would be
/// without it (the entry's name, and its bytes), to be put in the book's place by the file job.
pub fn check_epub(
	path: &Path,
	report: &ZipReport,
	limits: &Limits,
	policy: ActiveContent,
	approver: bool,
	mut replacements: Option<&mut Vec<(String, Vec<u8>)>>,
) -> Result<EpubFacts, Reason> {
	let mut archive = ZipArchive::new(
		File::open(path).map_err(|e| Reason::new(code::UNREADABLE, e.to_string()))?,
	)
	.map_err(|e| Reason::new(code::UNREADABLE, e.to_string()))?;
	let index_of: HashMap<&str, usize> = report
		.entries
		.iter()
		.enumerate()
		.filter(|(_, e)| !e.is_dir)
		.map(|(i, e)| (e.name.as_str(), i))
		.collect();
	let mut warnings: Vec<Reason> = vec![];

	// the first entry is `mimetype`, stored, with the right text: many real books break this
	let mimetype_ok = report.entries.first().is_some_and(|first| {
		first.name == "mimetype" && first.method == 0 && first.size == 20
	});
	if !mimetype_ok {
		warnings.push(Reason::new(
			code::MIMETYPE_ORDER,
			"The book's mimetype is not its first entry, stored",
		));
	}

	// the container names the package
	let container_index = *index_of.get("META-INF/container.xml").ok_or_else(|| {
		Reason::new(code::NO_PACKAGE, "The book has no META-INF/container.xml")
	})?;
	let container = read_entry(
		&mut archive,
		container_index,
		"META-INF/container.xml",
		MIB.min(limits.max_xml_bytes),
	)?;
	let opf_name = container_rootfile(&container, limits)?;
	if !is_safe_relative(&opf_name) || !index_of.contains_key(opf_name.as_str()) {
		return Err(Reason::new(
			code::NO_PACKAGE,
			"The package file the book names isn't in it",
		));
	}
	let opf_index = index_of[opf_name.as_str()];
	let opf = read_entry(&mut archive, opf_index, &opf_name, limits.max_xml_bytes)?;
	let package = parse_package(&opf, limits)?;
	let opf_dir = opf_name.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");

	// the media type each entry is declared with
	let mut media_types: HashMap<String, String> = HashMap::new();
	let mut remote_refs = 0u64;
	for item in &package.items {
		match resolve(opf_dir, &item.href) {
			Some(name) => {
				media_types.insert(name, item.media_type.to_ascii_lowercase());
			},
			None => {
				if item.href.contains("://") || item.href.starts_with("//") {
					remote_refs += 1;
				}
			},
		}
	}

	// tables of contents are read by the `epub` crate into a tree: how deep
	for item in &package.items {
		let is_toc = item
			.media_type
			.eq_ignore_ascii_case("application/x-dtbncx+xml")
			|| item.properties.split_whitespace().any(|p| p == "nav");
		if !is_toc {
			continue;
		}
		if let Some(name) = resolve(opf_dir, &item.href) {
			if let Some(&index) = index_of.get(name.as_str()) {
				let bytes = read_entry(&mut archive, index, &name, limits.max_xml_bytes)?;
				xml_check(&bytes, limits, "A table of contents")?;
			}
		}
	}

	// encryption: only the obfuscation of fonts
	if let Some(&index) = index_of.get("META-INF/encryption.xml") {
		let bytes = read_entry(
			&mut archive,
			index,
			"META-INF/encryption.xml",
			MIB.min(limits.max_xml_bytes),
		)?;
		let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
		let mut rest = text.as_str();
		while let Some(at) = rest.find("algorithm=") {
			rest = &rest[at + "algorithm=".len()..];
			let quote = rest.chars().next().unwrap_or('"');
			let value = rest
				.get(1..)
				.unwrap_or("")
				.split(quote)
				.next()
				.unwrap_or("");
			if !FONT_OBFUSCATION.contains(&value) {
				return Err(Reason::new(
					code::DRM,
					"The book is protected by a scheme Stump can't read",
				));
			}
		}
	}

	// every entry a browser could draw, and every image
	let mut summary = ActiveSummary::default();
	let (mut malformed, mut unstrippable) = (0u64, None::<String>);
	for (index, entry) in report.entries.iter().enumerate() {
		if entry.is_dir {
			continue;
		}
		let name = entry.name.as_str();
		if name == opf_name || name.starts_with("META-INF/") {
			continue;
		}
		let media_type = media_types.get(name).map(String::as_str);
		if active::is_document(name, media_type) {
			let bytes = read_entry(&mut archive, index, name, limits.max_entry_bytes)?;
			match active::scan_document(&bytes, active::is_declared_html(media_type)) {
				Outcome::Clean { remote_refs: n } => remote_refs += n,
				Outcome::Found {
					stripped,
					findings,
					remote_refs: n,
				} => {
					remote_refs += n;
					if let Some(collected) = replacements.as_deref_mut() {
						collected.push((entry.name.clone(), stripped));
					}
					for (kind, count) in findings {
						summary.found += count;
						if summary.items.len() < MAX_ACTIVE_ITEMS {
							summary.items.push(ActiveItem {
								kind,
								entry: printable(name),
								count,
							});
						}
					}
				},
				Outcome::Unstrippable(why) => {
					unstrippable.get_or_insert(format!("{}: {why}", printable(name)));
				},
				Outcome::Inert => malformed += 1,
			}
		} else if is_image_name(name) && entry.size > 0 {
			let entry = archive.by_index(index).map_err(|e| {
				Reason::new(
					code::UNREADABLE,
					format!("{} can't be opened: {e}", printable(name)),
				)
			})?;
			let head =
				bounded::read_capped(entry.take(IMAGE_HEAD_BYTES), IMAGE_HEAD_BYTES)
					.map_err(|e| {
						Reason::new(
							code::UNREADABLE,
							format!("{} is damaged: {e}", printable(name)),
						)
					})?;
			// a name that says image and bytes that say another thing: the reader would not
			// show it, and nothing else will read it: only the size is looked at when it is one
			if image_kind(&head).is_some() {
				check_image(name, &head, false, limits)?;
			}
		}
	}
	summary.remote_refs = remote_refs;
	if remote_refs > 0 {
		warnings.push(Reason::new(
			code::REMOTE_RESOURCES,
			format!("The book refers to {remote_refs} resources on other sites (they are not loaded)"),
		));
	}
	if malformed > 0 {
		warnings.push(Reason::new(
			code::MALFORMED_XHTML,
			format!("{malformed} pages are not well formed and will not be shown"),
		));
	}
	if let Some(why) = unstrippable {
		let reason = Reason::new(code::ACTIVE_CONTENT_UNSTRIPPABLE, why);
		if approver {
			warnings.push(reason);
		} else {
			return Err(reason);
		}
	}
	if summary.found > 0 {
		let first = summary
			.items
			.first()
			.map(|i| format!(" (a {} in {})", i.kind, i.entry))
			.unwrap_or_default();
		match policy {
			ActiveContent::Block => {
				return Err(Reason::new(
					code::ACTIVE_CONTENT,
					format!(
						"The book carries {} scripts or other active content{first}",
						summary.found
					),
				))
			},
			ActiveContent::Strip => {
				summary.stripped = true;
				warnings.push(Reason::new(
					code::ACTIVE_CONTENT,
					format!(
						"{} scripts or other active content will be removed{first}",
						summary.found
					),
				));
			},
			ActiveContent::Warn => warnings.push(Reason::new(
				code::ACTIVE_CONTENT,
				format!(
					"The book carries {} scripts or other active content{first}",
					summary.found
				),
			)),
		}
	}
	Ok(EpubFacts {
		active: summary,
		warnings,
	})
}

// ---------------------------------------------------------------------------------- comics

#[derive(Debug)]
pub struct ComicFacts {
	pub pages: u64,
	pub warnings: Vec<Reason>,
}

/// The pages of a CBZ: how many, and each one decodes under the limits
pub fn check_cbz(
	path: &Path,
	report: &ZipReport,
	limits: &Limits,
) -> Result<ComicFacts, Reason> {
	let mut archive = ZipArchive::new(
		File::open(path).map_err(|e| Reason::new(code::UNREADABLE, e.to_string()))?,
	)
	.map_err(|e| Reason::new(code::UNREADABLE, e.to_string()))?;
	let mut pages = 0u64;
	for (index, entry) in report.entries.iter().enumerate() {
		if entry.is_dir || is_hidden(&entry.name) {
			continue;
		}
		let name = entry.name.as_str();
		if is_image_name(name) {
			pages += 1;
			if pages > limits.max_pages {
				return Err(Reason::new(
					code::TOO_MANY_PAGES,
					format!("The comic has more than {} pages", limits.max_pages),
				));
			}
			let bytes = read_entry(&mut archive, index, name, limits.max_entry_bytes)?;
			check_image(name, &bytes, true, limits)?;
		} else if name
			.rsplit('/')
			.next()
			.is_some_and(|n| n.eq_ignore_ascii_case("ComicInfo.xml"))
		{
			let bytes = read_entry(&mut archive, index, name, limits.max_xml_bytes)?;
			xml_check(&bytes, limits, "ComicInfo.xml")?;
		}
	}
	if pages == 0 {
		return Err(Reason::new(code::UNREADABLE, "The comic has no pages"));
	}
	Ok(ComicFacts {
		pages,
		warnings: vec![],
	})
}

/// A comic in a RAR archive: the listing under the limits, then every page unpacked once, read
/// and decoded under them
pub fn check_cbr(path: &Path, limits: &Limits) -> Result<(ComicFacts, u64, u64), Reason> {
	let unreadable = |error: unrar::error::UnrarError| {
		Reason::new(
			code::UNREADABLE,
			format!("The RAR file can't be read: {error}"),
		)
	};
	let (mut entries, mut declared_total, mut max_entry) = (0u64, 0u64, 0u64);
	let listing = unrar::Archive::new(path)
		.open_for_listing()
		.map_err(unreadable)?;
	for entry in listing {
		let entry = entry.map_err(unreadable)?;
		if entry.is_directory() {
			continue;
		}
		entries += 1;
		if entries > limits.max_entries {
			return Err(Reason::new(
				code::TOO_MANY_ENTRIES,
				format!("The archive holds more than {} entries", limits.max_entries),
			));
		}
		let name = entry.filename.to_string_lossy().replace('\\', "/");
		if entry.is_encrypted() {
			return Err(Reason::new(
				code::ENCRYPTED,
				format!("{} is encrypted", printable(&name)),
			));
		}
		if entry.is_split() {
			return Err(Reason::new(
				code::MULTIVOLUME,
				"The archive is in several parts",
			));
		}
		if let Some(problem) = name_problem(name.as_bytes(), &name) {
			return Err(Reason::new(
				code::BAD_PATH,
				format!("An entry's name {problem}: {}", printable(&name)),
			));
		}
		if entry.unpacked_size > limits.max_entry_bytes {
			return Err(Reason::new(
				code::ENTRY_TOO_LARGE,
				format!(
					"{} is larger than the {} MB an entry can be",
					printable(&name),
					limits.max_entry_bytes / MIB
				),
			));
		}
		declared_total = declared_total.saturating_add(entry.unpacked_size);
		if declared_total > limits.max_unpacked_bytes {
			return Err(Reason::new(
				code::UNPACKED_TOO_LARGE,
				format!(
					"The archive unpacks to more than {} MB",
					limits.max_unpacked_bytes / MIB
				),
			));
		}
		max_entry = max_entry.max(entry.unpacked_size);
	}

	let mut archive = unrar::Archive::new(path)
		.open_for_processing()
		.map_err(unreadable)?;
	let mut pages = 0u64;
	loop {
		let Some(header) = archive.read_header().map_err(unreadable)? else {
			break;
		};
		let entry = header.entry();
		let name = entry.filename.to_string_lossy().replace('\\', "/");
		if entry.is_directory() || is_hidden(&name) || !is_image_name(&name) {
			archive = header.skip().map_err(unreadable)?;
			continue;
		}
		pages += 1;
		if pages > limits.max_pages {
			return Err(Reason::new(
				code::TOO_MANY_PAGES,
				format!("The comic has more than {} pages", limits.max_pages),
			));
		}
		let declared = entry.unpacked_size;
		let (data, next) = header.read().map_err(unreadable)?;
		if data.len() as u64 > declared || data.len() as u64 > limits.max_entry_bytes {
			return Err(Reason::new(
				code::SIZE_MISMATCH,
				format!("{} holds more than it says", printable(&name)),
			));
		}
		check_image(&name, &data, true, limits)?;
		archive = next;
	}
	if pages == 0 {
		return Err(Reason::new(code::UNREADABLE, "The comic has no pages"));
	}
	Ok((
		ComicFacts {
			pages,
			warnings: vec![],
		},
		declared_total,
		max_entry,
	))
}

/// How many pages one of the formats has, to tell if a book is too long
pub fn within_pages(pages: i64, limits: &Limits) -> Result<(), Reason> {
	if pages <= 0 {
		return Err(Reason::new(code::PDF_UNREADABLE, "The PDF has no pages"));
	}
	if pages as u64 > limits.max_pages {
		return Err(Reason::new(
			code::TOO_MANY_PAGES,
			format!("The PDF has more than {} pages", limits.max_pages),
		));
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use std::io::Write;

	use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

	use super::*;
	use crate::intake::probe::{contract::Limits, zipscan};

	fn png(width: u32, height: u32) -> Vec<u8> {
		let image = image::RgbImage::new(width, height);
		let mut out = Cursor::new(Vec::new());
		image
			.write_to(&mut out, image::ImageFormat::Png)
			.expect("png");
		out.into_inner()
	}

	fn zip_file(entries: &[(&str, Vec<u8>)]) -> (tempfile::TempDir, std::path::PathBuf) {
		let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
		for (name, data) in entries {
			let method = if *name == "mimetype" {
				CompressionMethod::Stored
			} else {
				CompressionMethod::Deflated
			};
			writer
				.start_file(
					*name,
					SimpleFileOptions::default().compression_method(method),
				)
				.unwrap();
			writer.write_all(data).unwrap();
		}
		let bytes = writer.finish().unwrap().into_inner();
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("book.zip");
		std::fs::write(&path, bytes).unwrap();
		(dir, path)
	}

	const CONTAINER: &str = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;

	fn opf(extra_items: &str, spine: &str) -> String {
		format!(
			r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata><manifest><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/>{extra_items}</manifest><spine>{spine}</spine></package>"#
		)
	}

	fn epub(
		chapter: &str,
		extra: &[(&str, Vec<u8>)],
		opf_text: &str,
	) -> (tempfile::TempDir, std::path::PathBuf) {
		let mut entries: Vec<(&str, Vec<u8>)> = vec![
			("mimetype", b"application/epub+zip".to_vec()),
			("META-INF/container.xml", CONTAINER.as_bytes().to_vec()),
			("OEBPS/content.opf", opf_text.as_bytes().to_vec()),
			("OEBPS/c1.xhtml", chapter.as_bytes().to_vec()),
		];
		entries.extend(extra.iter().map(|(n, d)| (*n, d.clone())));
		zip_file(&entries)
	}

	const CHAPTER: &str = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>c</title></head><body><p>hello</p></body></html>"#;

	fn check(
		path: &std::path::Path,
		policy: ActiveContent,
		approver: bool,
	) -> Result<EpubFacts, Reason> {
		let limits = Limits::default();
		let report = zipscan::scan(path, &limits)?;
		check_epub(path, &report, &limits, policy, approver, None)
	}

	#[test]
	fn images_are_what_their_bytes_say_and_within_the_limits() {
		let limits = Limits::default();
		assert!(check_image("a.png", &png(20, 20), true, &limits).is_ok());
		// what a name says is no matter
		assert!(check_image("a.jpg", &png(20, 20), true, &limits).is_ok());
		assert_eq!(
			check_image("a.jpg", b"II*\x00....", false, &limits)
				.unwrap_err()
				.code,
			code::UNSUPPORTED_IMAGE
		);
		// the header of a big one is read before anything is allocated
		let tight = Limits {
			max_image_pixels: 1_000,
			..Limits::default()
		};
		assert_eq!(
			check_image("a.png", &png(40, 40), true, &tight)
				.unwrap_err()
				.code,
			code::IMAGE_TOO_LARGE
		);
		// a PNG header that says 30,001 pixels wide, with nothing behind it
		let mut huge = png(2, 2);
		huge[16..20].copy_from_slice(&30_001u32.to_be_bytes());
		assert_eq!(
			check_image("a.png", &huge, true, &limits).unwrap_err().code,
			code::IMAGE_TOO_LARGE
		);
		// and one that says it is fine and isn't
		let mut broken = png(20, 20);
		broken.truncate(60);
		assert_eq!(
			check_image("a.png", &broken, true, &limits)
				.unwrap_err()
				.code,
			code::IMAGE_UNDECODABLE
		);
	}

	#[test]
	fn a_plain_epub_passes_and_a_book_with_no_package_does_not() {
		let (_dir, path) = epub(CHAPTER, &[], &opf("", r#"<itemref idref="c1"/>"#));
		let facts = check(&path, ActiveContent::Strip, false).expect("passes");
		assert_eq!(facts.active.found, 0);
		assert!(facts.warnings.is_empty(), "{:?}", facts.warnings);
		let (_dir, path) = zip_file(&[
			("mimetype", b"application/epub+zip".to_vec()),
			("a.txt", b"x".to_vec()),
		]);
		assert_eq!(
			check(&path, ActiveContent::Strip, false).unwrap_err().code,
			code::NO_PACKAGE
		);
		// the container names a package that isn't there, or one outside the book
		for full_path in ["OEBPS/none.opf", "../x.opf", "/etc/x", ""] {
			let container = CONTAINER.replace("OEBPS/content.opf", full_path);
			let (_dir, path) = zip_file(&[
				("mimetype", b"application/epub+zip".to_vec()),
				("META-INF/container.xml", container.into_bytes()),
				("OEBPS/content.opf", opf("", "").into_bytes()),
			]);
			assert_eq!(
				check(&path, ActiveContent::Strip, false).unwrap_err().code,
				code::NO_PACKAGE,
				"{full_path:?}"
			);
		}
	}

	#[test]
	fn packages_that_are_too_deep_declare_entities_or_list_too_much_are_refused() {
		let deep = format!("{}{}", "<a>".repeat(300), "</a>".repeat(300));
		let bad_opf = opf(&deep, "");
		let (_dir, path) = epub(CHAPTER, &[], &bad_opf);
		assert_eq!(
			check(&path, ActiveContent::Strip, false).unwrap_err().code,
			code::XML_TOO_DEEP
		);
		let entity = format!(
			"<?xml version=\"1.0\"?><!DOCTYPE package [ <!ENTITY a \"aaaa\"> ]>{}",
			opf("", "").split_once("?>").unwrap().1
		);
		let (_dir, path) = epub(CHAPTER, &[], &entity);
		assert_eq!(
			check(&path, ActiveContent::Strip, false).unwrap_err().code,
			code::XML_ENTITY
		);
		let spine: String = (0..30).map(|_| r#"<itemref idref="c1"/>"#).collect();
		let (_dir, path) = epub(CHAPTER, &[], &opf("", &spine));
		let limits = Limits {
			max_spine_items: 10,
			..Limits::default()
		};
		let report = zipscan::scan(&path, &limits).unwrap();
		assert_eq!(
			check_epub(&path, &report, &limits, ActiveContent::Strip, false, None)
				.err()
				.unwrap()
				.code,
			code::TOO_MANY_SPINE_ITEMS
		);
		// a table of contents that nests a thousand deep
		let ncx = format!(
			"<ncx>{}{}</ncx>",
			"<navPoint>".repeat(400),
			"</navPoint>".repeat(400)
		);
		let items =
			r#"<item id="n" href="toc.ncx" media-type="application/x-dtbncx+xml"/>"#;
		let (_dir, path) = epub(
			CHAPTER,
			&[("OEBPS/toc.ncx", ncx.into_bytes())],
			&opf(items, ""),
		);
		assert_eq!(
			check(&path, ActiveContent::Strip, false).unwrap_err().code,
			code::XML_TOO_DEEP
		);
	}

	#[test]
	fn encryption_other_than_font_obfuscation_is_drm() {
		let drm = r#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><EncryptedData xmlns="http://www.w3.org/2001/04/xmlenc#"><EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#aes128-cbc"/></EncryptedData></encryption>"#;
		let (_dir, path) = epub(
			CHAPTER,
			&[("META-INF/encryption.xml", drm.as_bytes().to_vec())],
			&opf("", ""),
		);
		assert_eq!(
			check(&path, ActiveContent::Strip, false).unwrap_err().code,
			code::DRM
		);
		let fonts = drm.replace(
			"http://www.w3.org/2001/04/xmlenc#aes128-cbc",
			"http://www.idpf.org/2008/embedding",
		);
		let (_dir, path) = epub(
			CHAPTER,
			&[("META-INF/encryption.xml", fonts.into_bytes())],
			&opf("", ""),
		);
		assert!(check(&path, ActiveContent::Strip, false).is_ok());
	}

	#[test]
	fn active_content_is_stripped_blocked_or_told_by_policy_and_role() {
		let chapter = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body><p>hi</p><script>alert(1)</script><img src="a.png" onerror="x()"/></body></html>"#;
		let (_dir, path) = epub(chapter, &[], &opf("", ""));
		let facts = check(&path, ActiveContent::Strip, false).expect("will be stripped");
		assert_eq!(facts.active.found, 2);
		assert!(facts.active.stripped);
		assert!(facts
			.warnings
			.iter()
			.any(|w| w.code == code::ACTIVE_CONTENT));
		assert_eq!(
			check(&path, ActiveContent::Block, false).unwrap_err().code,
			code::ACTIVE_CONTENT
		);
		let told = check(&path, ActiveContent::Warn, true).expect("told only");
		assert!(!told.active.stripped && told.active.found == 2);
		// a page that can't be cleaned is refused for somebody who needs approval, told otherwise
		let sneaky = r#"<html><body><p>one<img src=x onerror=alert(1)></body>"#;
		let (_dir, path) = epub(sneaky, &[], &opf("", ""));
		assert_eq!(
			check(&path, ActiveContent::Strip, false).unwrap_err().code,
			code::ACTIVE_CONTENT_UNSTRIPPABLE
		);
		assert!(check(&path, ActiveContent::Strip, true).is_ok());
	}

	#[test]
	fn remote_resources_are_told_and_pages_that_are_not_well_formed_are_inert() {
		let chapter = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body><img src="https://example.com/a.png"/></body></html>"#;
		let (_dir, path) = epub(
			chapter,
			&[(
				"OEBPS/c2.xhtml",
				b"<html><body><p>one<p>two</body>".to_vec(),
			)],
			&opf(
				r#"<item id="c2" href="c2.xhtml" media-type="application/xhtml+xml"/>"#,
				"",
			),
		);
		let facts = check(&path, ActiveContent::Strip, false).expect("passes");
		assert_eq!(facts.active.remote_refs, 1);
		assert!(facts
			.warnings
			.iter()
			.any(|w| w.code == code::REMOTE_RESOURCES));
		assert!(facts
			.warnings
			.iter()
			.any(|w| w.code == code::MALFORMED_XHTML));
	}

	#[test]
	fn a_comic_has_pages_that_decode() {
		let limits = Limits::default();
		let (_dir, path) = zip_file(&[
			("001.png", png(30, 40)),
			("002.png", png(30, 40)),
			(
				"ComicInfo.xml",
				b"<ComicInfo><Series>S</Series></ComicInfo>".to_vec(),
			),
			(".hidden.png", b"junk".to_vec()),
		]);
		let report = zipscan::scan(&path, &limits).unwrap();
		let facts = check_cbz(&path, &report, &limits).expect("passes");
		assert_eq!(facts.pages, 2);
		// a page that is not an image the server decodes
		let (_dir, path) =
			zip_file(&[("001.jpg", b"II*\x00 a tiff in disguise".to_vec())]);
		let report = zipscan::scan(&path, &limits).unwrap();
		assert_eq!(
			check_cbz(&path, &report, &limits).unwrap_err().code,
			code::UNSUPPORTED_IMAGE
		);
		// too many pages, and none
		let tight = Limits {
			max_pages: 1,
			..Limits::default()
		};
		let (_dir, path) = zip_file(&[("1.png", png(5, 5)), ("2.png", png(5, 5))]);
		let report = zipscan::scan(&path, &tight).unwrap();
		assert_eq!(
			check_cbz(&path, &report, &tight).unwrap_err().code,
			code::TOO_MANY_PAGES
		);
		let (_dir, path) = zip_file(&[("readme.txt", b"hello".to_vec())]);
		let report = zipscan::scan(&path, &limits).unwrap();
		assert_eq!(
			check_cbz(&path, &report, &limits).unwrap_err().code,
			code::UNREADABLE
		);
	}

	#[test]
	fn hrefs_are_resolved_inside_the_book() {
		assert_eq!(
			resolve("OEBPS", "c1.xhtml"),
			Some("OEBPS/c1.xhtml".to_string())
		);
		assert_eq!(
			resolve("OEBPS", "../img/a%20b.png#x"),
			Some("img/a b.png".to_string())
		);
		assert_eq!(
			resolve("OEBPS/text", "../c.xhtml"),
			Some("OEBPS/c.xhtml".to_string())
		);
		assert_eq!(resolve("", "../x"), None);
		assert_eq!(resolve("OEBPS", "https://example.com/a"), None);
		assert_eq!(resolve("OEBPS", "//cdn.example/a"), None);
		assert_eq!(resolve("OEBPS", "data:text/html,x"), None);
	}
}
