//! The checks of each format, run by the probe's child on a file it has not read before.
//!
//! Every limit comes from the request. What is checked here is what Stump's own readers would later
//! do with the file in the server: so the checks read it the way they do, under the same caps, and a
//! file that gets through is one that those readers finished on.

use std::{
	collections::{HashMap, HashSet},
	fs::File,
	io::{Cursor, Read},
	path::Path,
};

use epub::doc::NavPoint;
use quick_xml::{events::Event, Reader};
use zip::ZipArchive;

use super::{
	active::{self, Outcome},
	contract::{
		code, is_safe_relative, ActiveContent, ActiveItem, ActiveSummary, Limits, Reason,
	},
	zipscan::{name_problem, printable, ZipReport},
};
use crate::fs_utils::{bounded, PathUtils};

const MIB: u64 = 1024 * 1024;
/// The most bytes of an image that are looked at first for its size
const IMAGE_HEAD_BYTES: u64 = 256 * 1024;
/// What is looked through for the size of an image whose size isn't in its first bytes: a JPEG's
/// comes after its profile, which can be hundreds of KB (3 of 1,500 books of a real library have
/// one that is over 256 KB) and is never more than a few MB. Beyond this there is no header
const BIG_IMAGE_HEAD_BYTES: u64 = 8 * 1024 * 1024;
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

/// Whether a name is one Stump skips (a file whose name starts with a dot, or anything in a
/// `__MACOSX` folder): Stump's own rule, not another that looks like it
pub fn is_hidden(name: &str) -> bool {
	Path::new(name).is_hidden_file()
}

/// Whether Stump counts an entry of a comic as one of its pages: not hidden, and an image by what
/// Stump's own readers take as one. The comics are counted by this, and by nothing that only looks
/// like it, since the number of pages and which one is the first are what Stump decides
pub fn is_page(name: &str) -> bool {
	let path = Path::new(name);
	!path.is_hidden_file() && path.is_image()
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

/// Whether a text is in an encoding other than UTF-8 that a reader could still read: a byte order
/// mark of UTF-16, or a NUL among the first bytes (UTF-16 and UTF-32 without one). epub-rs reads a
/// package in UTF-16, and no check here reads one, so a package or a table of contents that is
/// written in it is refused
pub fn is_not_utf8(bytes: &[u8]) -> bool {
	bytes.starts_with(&[0xFF, 0xFE])
		|| bytes.starts_with(&[0xFE, 0xFF])
		|| bytes.iter().take(4).any(|b| *b == 0)
}

/// A look at an XML text that is read by Stump only (a package, a table of contents): how deep it
/// goes, and whether it declares entities or has a DOCTYPE that is read in two ways. Whether it
/// is well formed is returned, not required
pub fn xml_check(bytes: &[u8], limits: &Limits, what: &str) -> Result<bool, Reason> {
	let too_deep = || {
		Reason::new(
			code::XML_TOO_DEEP,
			format!("{what} nests deeper than {} levels", limits.max_xml_depth),
		)
	};
	let mut reader = Reader::from_reader(bytes);
	let mut depth = 0u32;
	loop {
		match reader.read_event() {
			Ok(Event::Eof) => return Ok(true),
			Ok(Event::Start(_)) => {
				depth += 1;
				if depth > limits.max_xml_depth {
					return Err(too_deep());
				}
			},
			Ok(Event::End(_)) => depth = depth.saturating_sub(1),
			Ok(Event::DocType(text)) => {
				if let Some(problem) = active::doctype_problem(&text) {
					return Err(Reason::new(
						code::XML_ENTITY,
						format!("{what} {problem}"),
					));
				}
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
			Err(_) => {
				// not well formed to this reader. A reader that is lenient about it (epub-rs keeps
				// what it read before the error) may read on, so how deep it can be is told
				// from the tags alone, in a way that never reads it as shorter than it is
				if active::rough_depth(bytes) > limits.max_xml_depth {
					return Err(too_deep());
				}
				return Ok(false);
			},
		}
	}
}

/// [`xml_check`] for a file that the server's EPUB reader parses (the container, the package, a
/// table of contents), which is held to being UTF-8 too: that reader also takes UTF-16
pub fn package_xml_check(
	bytes: &[u8],
	limits: &Limits,
	what: &str,
) -> Result<bool, Reason> {
	if is_not_utf8(bytes) {
		return Err(Reason::new(
			code::UNREADABLE,
			format!("{what} is not written in UTF-8"),
		));
	}
	if tree_weight(bytes) > MAX_TREE_WEIGHT {
		return Err(Reason::new(
			code::TOO_MANY_ENTRIES,
			format!("{what} has more elements, or declares more namespaces, than a book's has"),
		));
	}
	xml_check(bytes, limits, what)
}

/// The value of the first attribute of a tag whose name, without a prefix, is exactly `name`:
/// how epub-rs finds one (`get_attr`: the case matters, and the prefix doesn't). `Err`: it has an
/// entity that this reader doesn't know and epub-rs does (it reads `&nbsp;` as a space), so the
/// two would read a different value
fn exact_attribute(
	tag: &quick_xml::events::BytesStart,
	name: &str,
) -> Result<Option<String>, Reason> {
	for attribute in tag.attributes().flatten() {
		let key = attribute.key.as_ref();
		let local = key.rsplit(|b| *b == b':').next().unwrap_or(key);
		if local != name.as_bytes() {
			continue;
		}
		return attribute
			.unescape_value()
			.map(|value| Some(value.into_owned()))
			.map_err(|_| {
				Reason::new(
					code::UNREADABLE,
					format!("The `{name}` of an element of the package has an entity that Stump's readers read differently"),
				)
			});
	}
	Ok(None)
}

/// Whether a tag has an attribute with this name, without a prefix, exactly
fn has_attribute(tag: &quick_xml::events::BytesStart, name: &str) -> bool {
	tag.attributes().flatten().any(|attribute| {
		let key = attribute.key.as_ref();
		key.rsplit(|b| *b == b':').next().unwrap_or(key) == name.as_bytes()
	})
}

/// The path of the package that `META-INF/container.xml` names, as epub-rs finds it (it is what
/// the server opens a book by): the first element called exactly `rootfile` below the root,
/// whatever its prefix, and its `full-path`. The first one only: one in another case is no
/// package, and one with no `full-path` leaves the book without one
pub fn container_rootfile(bytes: &[u8], limits: &Limits) -> Result<String, Reason> {
	package_xml_check(bytes, limits, "container.xml")?;
	let mut reader = Reader::from_reader(bytes);
	let mut seen_root = false;
	loop {
		match reader.read_event() {
			Ok(Event::Start(tag)) | Ok(Event::Empty(tag)) => {
				if !seen_root {
					// the root element is not searched, only what is below it
					seen_root = true;
				} else if tag.local_name().as_ref() == b"rootfile" {
					return exact_attribute(&tag, "full-path")?.ok_or_else(|| {
						Reason::new(code::NO_PACKAGE, "The book names no package file")
					});
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

/// One resource of an OPF's manifest, as epub-rs reads it
#[derive(Debug, Clone)]
pub struct Item {
	pub id: String,
	pub href: String,
	pub media_type: String,
	pub properties: String,
}

/// What of a package the check reads before the server's reader does (so that what that reader
/// parses is held to the limits first), the way that reader finds it: the first element called
/// `manifest` and the first called `spine` anywhere below the root, and the children of each,
/// whatever they are called
#[derive(Debug, Default)]
pub struct Package {
	/// The children of the manifest that have an `id`, an `href` and a `media-type`, in order
	pub items: Vec<Item>,
	/// Every child of the manifest, whatever it is called and has
	pub manifest_children: u64,
	/// Every child of the spine, whatever it is called and has
	pub spine_children: u64,
	/// The children of the spine that have an `idref`
	pub spine_pages: u64,
	/// The resource the spine names as the table of contents
	pub toc_id: Option<String>,
}

/// The most namespaces a package or a table of contents may have in scope at an element, and the
/// most the nodes of its tree may weigh (each node counts one, and one for every namespace in
/// scope at it). epub-rs's XML reader keeps the whole table of namespaces in scope in every node
/// it reads, and the server builds the tree for every request that opens the book, in several at
/// once: what a book costs grows with both. Of 8,067 real books, the most namespaces in scope in
/// a package is 47, and the heaviest tree is 116,000 (a package) and 43,000 (a table of
/// contents); a tree of 600,000 takes some 70 MB
const MAX_NAMESPACES_IN_SCOPE: u64 = 128;
const MAX_TREE_WEIGHT: u64 = 600_000;

/// How many namespaces a tag declares
fn declared_namespaces(tag: &quick_xml::events::BytesStart) -> u64 {
	tag.attributes()
		.flatten()
		.filter(|attribute| {
			let key = attribute.key.as_ref();
			key == b"xmlns" || key.starts_with(b"xmlns:")
		})
		.count() as u64
}

/// What the tree epub-rs builds of an XML text weighs (see [`MAX_TREE_WEIGHT`]), counted until it
/// is more than the most it may, or the text ends or stops being well formed
fn tree_weight(bytes: &[u8]) -> u64 {
	let mut reader = Reader::from_reader(bytes);
	let (mut scope, mut in_scope, mut weight) = (Vec::<u64>::new(), 0u64, 0u64);
	loop {
		match reader.read_event() {
			Ok(Event::Start(tag)) => {
				let declared = declared_namespaces(&tag);
				in_scope += declared;
				scope.push(declared);
				weight = weight.saturating_add(1 + in_scope);
			},
			Ok(Event::Empty(tag)) => {
				weight = weight.saturating_add(1 + in_scope + declared_namespaces(&tag));
			},
			Ok(Event::End(_)) => in_scope -= scope.pop().unwrap_or(0).min(in_scope),
			Ok(Event::Eof) | Err(_) => return weight,
			Ok(_) => {},
		}
		if weight > MAX_TREE_WEIGHT {
			return weight;
		}
	}
}

/// The manifest and the spine of an OPF, read strictly, under the limits
fn parse_package(bytes: &[u8], limits: &Limits) -> Result<Package, Reason> {
	if is_not_utf8(bytes) {
		return Err(Reason::new(
			code::UNREADABLE,
			"The package file is not written in UTF-8",
		));
	}
	let mut reader = Reader::from_reader(bytes);
	let mut package = Package::default();
	// the names of the elements that are open, outermost first, and the namespaces each declares
	let mut open: Vec<(Vec<u8>, u64)> = vec![];
	// the level (1 is the root) of the manifest and of the spine, once each has been found
	let (mut manifest_level, mut spine_level) = (None::<usize>, None::<usize>);
	let (mut manifest_done, mut spine_done) = (false, false);
	// the namespaces in scope, and what the tree of it weighs so far
	let (mut in_scope, mut weight) = (0u64, 0u64);
	loop {
		match reader.read_event() {
			Ok(Event::Eof) => break,
			Ok(event @ (Event::Start(_) | Event::Empty(_))) => {
				let (tag, opens) = match &event {
					Event::Start(tag) => (tag, true),
					Event::Empty(tag) => (tag, false),
					_ => unreachable!("only tags are matched here"),
				};
				let level = open.len() + 1;
				if opens && level as u32 > limits.max_xml_depth {
					return Err(Reason::new(
						code::XML_TOO_DEEP,
						"The package file nests too deep",
					));
				}
				let declared = declared_namespaces(tag);
				if in_scope + declared > MAX_NAMESPACES_IN_SCOPE {
					return Err(Reason::new(
						code::UNREADABLE,
						"The package file has more namespaces in scope than a book has",
					));
				}
				weight = weight.saturating_add(1 + in_scope + declared);
				if weight > MAX_TREE_WEIGHT {
					return Err(Reason::new(
						code::TOO_MANY_ENTRIES,
						"The package file has more elements than a book has",
					));
				}
				let local = tag.local_name().as_ref().to_vec();
				// the root is not searched, only what is below it
				if level > 1 {
					if local == b"manifest" && manifest_level.is_none() && !manifest_done
					{
						manifest_level = Some(level);
					} else if local == b"spine" && spine_level.is_none() && !spine_done {
						spine_level = Some(level);
						package.toc_id = exact_attribute(tag, "toc")?;
					}
				}
				// a child of the manifest, or of the spine (the manifest or the spine itself
				// is one level up)
				if manifest_level.is_some_and(|at| level == at + 1) {
					package.manifest_children += 1;
					if package.manifest_children > limits.max_entries {
						return Err(Reason::new(
							code::TOO_MANY_ENTRIES,
							"The package lists more items than a book has",
						));
					}
					if has_attribute(tag, "media-type") {
						if let (Some(id), Some(href)) =
							(exact_attribute(tag, "id")?, exact_attribute(tag, "href")?)
						{
							package.items.push(Item {
								id,
								href,
								media_type: tag
									.attributes()
									.flatten()
									.find(|a| {
										let key = a.key.as_ref();
										key.rsplit(|b| *b == b':').next().unwrap_or(key)
											== b"media-type"
									})
									.map(|a| {
										a.unescape_value()
											.map(|v| v.into_owned())
											.unwrap_or_else(|_| {
												String::from_utf8_lossy(&a.value)
													.into_owned()
											})
									})
									.unwrap_or_default(),
								properties: tag
									.attributes()
									.flatten()
									.find(|a| {
										let key = a.key.as_ref();
										key.rsplit(|b| *b == b':').next().unwrap_or(key)
											== b"properties"
									})
									.map(|a| {
										String::from_utf8_lossy(&a.value).into_owned()
									})
									.unwrap_or_default(),
							});
						}
					}
				}
				if spine_level.is_some_and(|at| level == at + 1) {
					package.spine_children += 1;
					if package.spine_children > limits.max_spine_items {
						return Err(Reason::new(
							code::TOO_MANY_SPINE_ITEMS,
							format!(
								"The book has more than {} pages in its spine",
								limits.max_spine_items
							),
						));
					}
					if has_attribute(tag, "idref") {
						package.spine_pages += 1;
					}
				}
				if opens {
					in_scope += declared;
					open.push((local, declared));
				} else {
					// an element with nothing in it that is the manifest or the spine is over
					if manifest_level == Some(level) {
						manifest_level = None;
						manifest_done = true;
					}
					if spine_level == Some(level) {
						spine_level = None;
						spine_done = true;
					}
				}
			},
			Ok(Event::End(_)) => {
				let level = open.len();
				if manifest_level == Some(level) {
					manifest_level = None;
					manifest_done = true;
				}
				if spine_level == Some(level) {
					spine_level = None;
					spine_done = true;
				}
				if let Some((_, declared)) = open.pop() {
					in_scope -= declared.min(in_scope);
				}
			},
			Ok(Event::DocType(text)) => {
				if let Some(problem) = active::doctype_problem(&text) {
					return Err(Reason::new(
						code::XML_ENTITY,
						format!("The package file {problem}"),
					));
				}
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
	Ok(package)
}

/// A path of a book made lexical: `.` and empty parts dropped, `..` taking a part off. `None`
/// when it climbs out of the book
fn lexical(path: &str) -> Option<String> {
	let mut parts: Vec<&str> = vec![];
	for part in path.split('/') {
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

/// The entries of the book that the server serves for the path of a resource, every way it could:
/// the reader asks for the path cut at `#` and percent-encoded, the route decodes it, makes it
/// lexical, and epub-rs takes the entry called exactly that, or called what that decodes to. All
/// the ways are taken, so that a resource is never missed by how one of them reads its address
fn served_entries(path: &str, index_of: &HashMap<&str, usize>) -> Vec<usize> {
	let path = path.split('#').next().unwrap_or(path);
	let mut names: Vec<String> = vec![path.to_string()];
	if let Some(lexical) = lexical(path) {
		names.push(lexical);
	}
	for name in names.clone() {
		if let Ok(decoded) = urlencoding::decode(&name) {
			names.push(decoded.into_owned());
		}
	}
	let mut found: Vec<usize> = vec![];
	for name in names {
		if let Some(&index) = index_of.get(name.as_str()) {
			if !found.contains(&index) {
				found.push(index);
			}
		}
	}
	found
}

/// The path of a resource as the server's reader makes it: the folder of the package, then the
/// `href` as it is written, with its `.` and `..` resolved (what `open_epub` does)
fn package_path(opf_name: &str, href: &str) -> String {
	let base = Path::new(opf_name)
		.parent()
		.unwrap_or_else(|| Path::new(""));
	let joined = base.join(href);
	crate::media::processor::epub::resolve_dot_segments(&joined)
		.unwrap_or(joined)
		.to_string_lossy()
		.into_owned()
}

/// How many entries a table of contents has, and how deep it nests, counted without recursion
fn toc_shape(toc: &[NavPoint]) -> (u64, u32) {
	let (mut count, mut deepest) = (0u64, 0u32);
	let mut pending: Vec<(&NavPoint, u32)> = toc.iter().map(|nav| (nav, 1)).collect();
	while let Some((nav, depth)) = pending.pop() {
		count += 1;
		deepest = deepest.max(depth);
		pending.extend(nav.children.iter().map(|child| (child, depth + 1)));
	}
	(count, deepest)
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

/// The first bytes of an entry (what an image's size, or whether a text starts with `<`, is told by)
fn read_head(
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
	bounded::read_capped(entry.take(cap), cap).map_err(|error| {
		Reason::new(
			code::UNREADABLE,
			format!("{} is damaged: {error}", printable(name)),
		)
	})
}

/// Whether the name or the package calls an entry an image
fn named_as_image(name: &str, types: &[String]) -> bool {
	is_image_name(name)
		|| types.iter().any(|t| {
			active::readium_media_type(t).is_some_and(|(kind, _, _)| kind == "image")
		})
}

/// The cover of an EPUB, as Stump's thumbnails take it (`get_cover`, the first page): the server
/// decodes it in its own process, so it is looked at the way the server will: its kind from its
/// bytes (JPEG, PNG, GIF or WebP, or AVIF by its header; not what a guess of the `image` crate
/// would reach, TIFF, EXR or DDS), its sides and pixels from the header, then a decode of the whole
/// of it under an allocation limit
pub fn check_cover(bytes: &[u8], limits: &Limits) -> Result<(), Reason> {
	if image_kind(bytes).is_some() {
		return check_image("The cover", bytes, true, limits);
	}
	match image::guess_format(bytes) {
		// not an image at all: nothing decodes it, and the thumbnail is just not made
		Err(_) => Ok(()),
		Ok(format) if !format.reading_enabled() => Ok(()),
		Ok(format) => Err(Reason::new(
			code::UNSUPPORTED_IMAGE,
			format!("The cover is a {format:?} image: not a kind Stump takes (JPEG, PNG, GIF or WebP)"),
		)),
	}
}

/// The most pages times top-level table of contents entries a book may have: the work the server
/// does to give each page its title (a search of the entries for each page, in the manifest and
/// again in the positions). The biggest of 8,058 real books is 347,498 (The Jataka: 586 pages
/// and 593 entries), and the median is 476; this is some 14 times the biggest, which costs the
/// server a fraction of a second where 20,000 pages and 100,000 entries would cost minutes
const MAX_TITLE_MATCHES: u64 = 5_000_000;

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

	// the container names the package, the way the server's reader finds it
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

	// the files the server's reader parses into trees (a deep one overflows its stack, and the
	// reader is open in several requests at once): each is held to the limits before it is opened.
	// They are the container and the package (above), and the file the spine's `toc` names,
	// whatever it is called or declared to be. The files declared to be tables of contents are
	// looked at too, which costs nothing
	let mut toc_paths: Vec<String> = vec![];
	if let Some(toc_id) = &package.toc_id {
		// the last of the items with that id is the one the reader keeps
		if let Some(item) = package.items.iter().rev().find(|item| &item.id == toc_id) {
			toc_paths.push(package_path(&opf_name, &item.href));
		}
	}
	for item in &package.items {
		let declared = item
			.media_type
			.eq_ignore_ascii_case("application/x-dtbncx+xml")
			|| item.properties.split_whitespace().any(|p| p == "nav");
		if declared {
			toc_paths.push(package_path(&opf_name, &item.href));
		}
	}
	for toc_path in &toc_paths {
		for index in served_entries(toc_path, &index_of) {
			let name = report.entries[index].name.as_str();
			let bytes = read_entry(&mut archive, index, name, limits.max_xml_bytes)?;
			package_xml_check(&bytes, limits, "A table of contents")?;
		}
	}

	// the book as the server reads it: the same reader opens it here that opens it there, and its
	// resources, their types, its spine and its table of contents are what everything below goes
	// by (a package is read in more than one way: every child of the manifest is a resource to
	// it, whatever it is called)
	let book = crate::media::processor::epub::open_epub(path).map_err(|error| {
		Reason::new(
			code::UNREADABLE,
			format!("Stump can't open the book's package: {error}"),
		)
	})?;
	// what was read above, to hold the files to the limits before they were opened, must be what
	// was opened: if the two readers read the package differently, the limits may not have been
	// applied to what the server reads, and the book is not filed
	let distinct_ids: HashSet<&str> =
		package.items.iter().map(|i| i.id.as_str()).collect();
	let mut agreed = book.root_file.to_str() == Some(opf_name.as_str())
		&& book.spine.len() as u64 == package.spine_pages
		&& book.resources.len() == distinct_ids.len();
	if agreed {
		// the last item of an id is the one that is kept
		let mut last: HashMap<&str, &Item> = HashMap::new();
		for item in &package.items {
			last.insert(item.id.as_str(), item);
		}
		agreed = last.iter().all(|(id, item)| {
			book.resources.get(*id).is_some_and(|resource| {
				resource.path.to_str()
					== Some(package_path(&opf_name, &item.href).as_str())
			})
		});
	}
	if !agreed {
		return Err(Reason::new(
			code::UNREADABLE,
			"The package is read in one way by the check and in another by Stump's readers",
		));
	}
	if book.resources.len() as u64 > limits.max_entries {
		return Err(Reason::new(
			code::TOO_MANY_ENTRIES,
			"The package lists more items than a book has",
		));
	}
	if book.spine.len() as u64 > limits.max_spine_items {
		return Err(Reason::new(
			code::TOO_MANY_SPINE_ITEMS,
			format!(
				"The book has more than {} pages in its spine",
				limits.max_spine_items
			),
		));
	}
	// the table of contents the reader built: the server walks it recursively and matches each
	// page against every entry of it
	let (toc_entries, toc_depth) = toc_shape(&book.toc);
	if toc_depth > limits.max_xml_depth {
		return Err(Reason::new(
			code::XML_TOO_DEEP,
			format!(
				"The table of contents nests deeper than {} levels",
				limits.max_xml_depth
			),
		));
	}
	if toc_entries > limits.max_toc_entries {
		return Err(Reason::new(
			code::TOO_MANY_ENTRIES,
			format!(
				"The table of contents has more than {} entries",
				limits.max_toc_entries
			),
		));
	}
	// the server finds each page's title by looking through the table of contents for the first
	// entry whose address contains the page's, for the manifest and again for the positions: the
	// work grows with the pages times the entries, so that is held to a few times what any real
	// book has
	let title_matches = (book.spine.len() as u64).saturating_mul(book.toc.len() as u64);
	if title_matches > MAX_TITLE_MATCHES {
		return Err(Reason::new(
			code::TOO_DEMANDING,
			"The book has too many pages for the size of its table of contents",
		));
	}

	// the types each entry is declared with: all of them, since a package can list one file twice
	// with another type each, and the reader takes the one of the item the spine names. Keyed by
	// the lower case name, so the case of an address doesn't decide what is looked at. The
	// pages are the entries the spine lists, by any of the ways their address is read
	let mut media_types: HashMap<String, Vec<String>> = HashMap::new();
	let mut pages: HashSet<String> = HashSet::new();
	for resource in book.resources.values() {
		let path = resource.path.to_string_lossy();
		for index in served_entries(&path, &index_of) {
			media_types
				.entry(report.entries[index].name.to_lowercase())
				.or_default()
				.push(resource.mime.clone());
		}
	}
	for item in &book.spine {
		if let Some(resource) = book.resources.get(&item.idref) {
			let path = resource.path.to_string_lossy();
			for index in served_entries(&path, &index_of) {
				pages.insert(report.entries[index].name.to_lowercase());
			}
		}
	}
	let mut remote_refs = 0u64;
	for item in &package.items {
		if item.href.contains("://") || item.href.starts_with("//") {
			remote_refs += 1;
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

	// every entry, by what it is and never by where it is or what it is called: the reader draws
	// any file the reading order lists (a page in `META-INF/`, a package file listed as a page, a
	// `.dat` declared `application/xhtml+xml;`), so every file that starts with `<` or that the
	// package or its name calls a document is read as one, and every file that is an image by its
	// bytes is looked at as one
	let mut summary = ActiveSummary::default();
	let (mut malformed, mut unstrippable, mut odd_images) = (0u64, None::<String>, 0u64);
	for (index, entry) in report.entries.iter().enumerate() {
		if entry.is_dir {
			continue;
		}
		let name = entry.name.as_str();
		let types: &[String] = media_types
			.get(&name.to_lowercase())
			.map(Vec::as_slice)
			.unwrap_or(&[]);
		let is_package = name == opf_name || name == "META-INF/container.xml";
		let mut head = if entry.size == 0 {
			vec![]
		} else {
			read_head(&mut archive, index, name, IMAGE_HEAD_BYTES)?
		};
		let kind = image_kind(&head);
		let html = active::is_html_page(name, types);
		// a page of the spine is a document unless its bytes are an image (the reader draws those
		// as an image, whatever the spine says, and a document declared to be HTML is `html`)
		let is_document = active::is_document(name, types)
			|| (kind.is_none() && pages.contains(&name.to_lowercase()));
		let scan = html
			|| is_document
			|| match active::starts_with_markup(&head) {
				Some(starts) => starts,
				// nothing but white space so far: it may go on
				None => entry.size > head.len() as u64,
			};
		if scan {
			let bytes = if entry.size <= head.len() as u64 {
				head.clone()
			} else {
				read_entry(&mut archive, index, name, limits.max_entry_bytes)?
			};
			match active::scan_document(&bytes, html) {
				Outcome::Clean { remote_refs: n } => remote_refs += n,
				Outcome::Found {
					stripped,
					findings,
					remote_refs: n,
				} => {
					remote_refs += n;
					if is_package {
						// the package file is rewritten by the file job, and is never taken
						// apart here: a book that lists it as a page is not filed
						unstrippable.get_or_insert(format!(
							"{}: the package file carries active content",
							printable(name)
						));
					} else if let Some(collected) = replacements.as_deref_mut() {
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
				Outcome::Inert => {
					if is_document && !name.starts_with("META-INF/") {
						malformed += 1;
					}
				},
			}
		}
		if kind.is_some() {
			// what the bytes say it is: its size from the header, whatever the name. A header that
			// isn't in the first bytes (a JPEG with a long profile) is looked for in more of them
			if imagesize::blob_size(&head).is_err() && entry.size > head.len() as u64 {
				head = read_head(&mut archive, index, name, BIG_IMAGE_HEAD_BYTES)?;
			}
			check_image(name, &head, false, limits)?;
		} else if !scan && entry.size > 0 && named_as_image(name, types) {
			// called an image, and not one Stump decodes: told, and the cover is held to more below
			if image::guess_format(&head).is_ok_and(|f| f.reading_enabled()) {
				odd_images += 1;
			}
		}
	}
	if odd_images > 0 {
		warnings.push(Reason::new(
			code::UNSUPPORTED_IMAGE,
			format!(
				"{odd_images} images are of a kind Stump does not show (they are kept)"
			),
		));
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

/// How long the server's reader may take to make the manifest and the positions of a book. A real
/// book takes milliseconds; the server makes them for every request that asks, so a book that
/// takes seconds here is one that anybody who opens it can keep the server busy with
///
/// (A debug build, which only the tests run, is some ten times slower.)
const READER_ROUTE_BUDGET: std::time::Duration =
	std::time::Duration::from_secs(if cfg!(debug_assertions) { 30 } else { 3 });

/// What the routes that the reader draws a book by do with it (its manifest, and its positions),
/// done once here, under the limits of the checking process: the server makes both on every
/// request, in the middle of other work, so a book that can't be made one, or that takes too long
/// to, is not filed
pub fn check_reader_routes(path: &Path) -> Result<(), Reason> {
	let started = std::time::Instant::now();
	let generator = crate::readium::ReadiumManifestGenerator::new(
		path.to_string_lossy().into_owned(),
		"http://stump.invalid/api/v2/epub/book",
	);
	generator.generate_manifest().map_err(|error| {
		Reason::new(
			code::UNREADABLE,
			format!("Stump's reader can't make the book's manifest: {error}"),
		)
	})?;
	generator.generate_positions().map_err(|error| {
		Reason::new(
			code::UNREADABLE,
			format!("Stump's reader can't make the book's positions: {error}"),
		)
	})?;
	if started.elapsed() > READER_ROUTE_BUDGET {
		return Err(Reason::new(
			code::TOO_DEMANDING,
			"Stump's reader takes too long to open the book",
		));
	}
	Ok(())
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
		if is_page(name) {
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
		if entry.is_directory() || !is_page(&name) {
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

/// The longest side a page of a PDF may have, in points (3 metres)
const MAX_PAGE_POINTS: f32 = 14_400.0;

/// Whether every page of a PDF has a size a reader can draw: a number, and between 1 and 14,400
/// points a side
pub fn pages_within_size(sizes: &[[f32; 2]]) -> Result<(), Reason> {
	let fits = |side: f32| side.is_finite() && (1.0..=MAX_PAGE_POINTS).contains(&side);
	for (index, [width, height]) in sizes.iter().enumerate() {
		if !fits(*width) || !fits(*height) {
			return Err(Reason::new(
				code::PAGE_SIZE,
				format!(
					"Page {} is {width} by {height} points: not a size a page can be",
					index + 1
				),
			));
		}
	}
	Ok(())
}

/// What the reader does first with a PDF, done here under the checker's limits: the layout (every
/// page's size, the outline, whether there is text), then the first page drawn at the smallest
/// width and its text layer read. These are the reader's own code paths, so a PDF that gets through
/// is one PDFium finished on. Drawing every page (the deep pass) is not done here
pub fn check_pdf(path: &Path, config: &crate::config::StumpConfig) -> Result<(), Reason> {
	let unreadable = |what: &str, error: &dyn std::fmt::Display| {
		Reason::new(code::PDF_UNREADABLE, format!("{what}: {error}"))
	};
	let pdfium = crate::media::processor::pdf::renderer(&config.pdfium_path)
		.map_err(|e| unreadable("PDFium can't be used", &e))?;
	let document = pdfium
		.load_pdf_from_file(path.to_str().unwrap_or_default(), None)
		.map_err(|e| unreadable("The PDF can't be opened", &e))?;
	let layout = crate::pdf_reader::read_layout(&document, "check", config.pdf_max_tier)
		.map_err(|e| unreadable("The layout of the PDF can't be read", &e))?;
	pages_within_size(&layout.pages)?;
	if layout.page_count >= 1 {
		crate::pdf_reader::render_tier(&document, 1, 720)
			.map_err(|e| unreadable("The first page can't be drawn", &e))?;
		crate::pdf_reader::extract_text(&document, 1)
			.map_err(|e| unreadable("The text of the first page can't be read", &e))?;
	}
	Ok(())
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

	#[test]
	fn the_sizes_of_a_pdfs_pages_are_held_to_what_a_page_can_be() {
		assert!(pages_within_size(&[[612.0, 792.0], [14_400.0, 1.0]]).is_ok());
		for bad in [
			[0.0, 792.0],
			[612.0, 0.5],
			[14_401.0, 792.0],
			[f32::NAN, 100.0],
			[f32::INFINITY, 100.0],
			[-5.0, 100.0],
		] {
			let error = pages_within_size(&[[612.0, 792.0], bad]).unwrap_err();
			assert_eq!(error.code, code::PAGE_SIZE);
			assert!(error.detail.starts_with("Page 2 "), "{}", error.detail);
		}
	}

	#[test]
	fn a_comic_page_is_what_stump_counts_as_one() {
		for page in [
			"001.png",
			"a/b/c.JPG",
			"x/.dir/001.webp",
			"z.avif",
			"p.heif",
			"q.jxl",
		] {
			assert!(is_page(page), "{page}");
		}
		// hidden by Stump's rule: a file whose name starts with a dot, or anything in `__MACOSX`
		for hidden in [
			".001.png",
			"a/.001.png",
			"__MACOSX/a/001.png",
			"__MACOSX/001.png",
		] {
			assert!(!is_page(hidden), "{hidden}");
			assert!(is_hidden(hidden), "{hidden}");
		}
		// not images to Stump, whatever other programs think
		for other in [
			"scan.bmp",
			"scan.tif",
			"scan.heic",
			"page.svg",
			"notes.txt",
			"001",
		] {
			assert!(!is_page(other), "{other}");
		}
	}

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

	/// Not run by default. With `STUMP_SURVEY_DIR` (a folder of EPUBs, read only) and
	/// `STUMP_SURVEY_STEP` (every n-th book), runs the check a friend's book is run through over
	/// each book and tallies what came of it, to `/app/.survey-out.txt`: what the checker does to
	/// books that are known to be fine
	#[test]
	#[ignore = "reads a real library"]
	fn real_library_survey() {
		use std::collections::BTreeMap;
		let Ok(dir) = std::env::var("STUMP_SURVEY_DIR") else {
			return;
		};
		let step: usize = std::env::var("STUMP_SURVEY_STEP")
			.ok()
			.and_then(|s| s.parse().ok())
			.unwrap_or(1)
			.max(1);
		let mut files: Vec<std::path::PathBuf> = vec![];
		let mut folders = vec![std::path::PathBuf::from(dir)];
		while let Some(folder) = folders.pop() {
			for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
				let path = entry.path();
				if path.is_dir() {
					folders.push(path);
				} else if path
					.extension()
					.is_some_and(|e| e.eq_ignore_ascii_case("epub"))
					&& !entry.file_name().to_string_lossy().starts_with('.')
				{
					files.push(path);
				}
			}
		}
		files.sort();
		let limits = Limits::default();
		let mut tally: BTreeMap<String, u64> = BTreeMap::new();
		let mut examples: BTreeMap<String, Vec<String>> = BTreeMap::new();
		let mut slowest = (std::time::Duration::ZERO, String::new());
		for file in files.iter().step_by(step) {
			let started = std::time::Instant::now();
			let (outcome, detail) = match zipscan::scan(file, &limits) {
				Err(reason) => (format!("scan:{}", reason.code), reason.detail),
				Ok(report) => {
					match check_epub(
						file,
						&report,
						&limits,
						ActiveContent::Strip,
						false,
						None,
					) {
						// the routes the reader draws the book by, as the probe makes them
						Ok(_) => match check_reader_routes(file) {
							Ok(()) => ("ok".to_string(), String::new()),
							Err(reason) => {
								(format!("routes:{}", reason.code), reason.detail)
							},
						},
						Err(reason) => (reason.code.to_string(), reason.detail),
					}
				},
			};
			let took = started.elapsed();
			if took > slowest.0 {
				slowest = (took, file.display().to_string());
			}
			*tally.entry(outcome.clone()).or_default() += 1;
			let seen = examples.entry(outcome).or_default();
			if seen.len() < 12 {
				seen.push(format!(
					"{} :: {}",
					file.file_name().unwrap_or_default().to_string_lossy(),
					detail
				));
			}
		}
		let mut report = format!("books checked: {}\n", tally.values().sum::<u64>());
		for (outcome, count) in &tally {
			report.push_str(&format!("{outcome}: {count}\n"));
		}
		report.push_str(&format!("slowest: {:?} {}\n", slowest.0, slowest.1));
		for (outcome, seen) in &examples {
			if outcome != "ok" {
				for line in seen {
					report.push_str(&format!("  [{outcome}] {line}\n"));
				}
			}
		}
		std::fs::write("/app/.survey-out.txt", report).expect("report");
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

	// ------------------------------------------------------------------------------------------
	// What the check reads of a book must be what the server and the reader read of it. The
	// server opens an EPUB with epub-rs, which reads a package its own way (every child of the
	// manifest is a resource and every child of the spine a page, whatever it is called; names
	// and attributes are matched exactly; `&nbsp;` is a space; the first exact `rootfile` wins),
	// and the reader draws a page by the type the server says it has.
	// ------------------------------------------------------------------------------------------

	const SCRIPT_IN_A_DAT: &str = "x<img src=x onerror=alert(1)>";
	const SWALLOWED_PAGE: &str = "<?xml version=\"1.0\"?><!DOCTYPE html SYSTEM \"<\"><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>t</title></head><body><script>alert(1)</script></body></html><!-- > -->";
	const SWALLOWED_IN_BODY_PAGE: &str = "<?xml version=\"1.0\"?><!DOCTYPE html SYSTEM \"<\"><html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:x=\"http://www.w3.org/1999/xhtml\"><body><p title=\">\"></p><x:script>alert(1)</x:script></body></html>";

	fn is_refused_for_active_content(path: &std::path::Path, what: &str) {
		match check(path, ActiveContent::Strip, false) {
			Err(reason) => assert_eq!(
				reason.code,
				code::ACTIVE_CONTENT_UNSTRIPPABLE,
				"{what}: {reason:?}"
			),
			Ok(facts) => panic!("{what}: the book was let through: {facts:?}"),
		}
	}

	#[test]
	fn a_page_that_hides_in_a_doctype_is_refused() {
		for page in [SWALLOWED_PAGE, SWALLOWED_IN_BODY_PAGE] {
			let (_dir, path) = epub(page, &[], &opf("", r#"<itemref idref="c1"/>"#));
			is_refused_for_active_content(&path, page);
		}
	}

	#[test]
	fn a_page_the_reader_draws_by_a_type_the_check_did_not_read_is_held_to_what_its_bytes_look_like(
	) {
		// a manifest child that isn't called `item` is a resource to epub-rs
		let (_dir, path) = epub(
			CHAPTER,
			&[("OEBPS/p.dat", SCRIPT_IN_A_DAT.as_bytes().to_vec())],
			&opf(
				r#"<x id="p" href="p.dat" media-type="text/html"/>"#,
				r#"<itemref idref="c1"/><itemref idref="p"/>"#,
			),
		);
		is_refused_for_active_content(&path, "a manifest child called x");
		// attribute names are matched exactly: the second is the one epub-rs takes
		let (_dir, path) = epub(
			CHAPTER,
			&[("OEBPS/p.dat", SCRIPT_IN_A_DAT.as_bytes().to_vec())],
			&opf(
				r#"<item id="p" href="p.dat" Media-Type="image/png" media-type="text/html"/>"#,
				r#"<itemref idref="c1"/><itemref idref="p"/>"#,
			),
		);
		is_refused_for_active_content(&path, "Media-Type before media-type");
		// `&nbsp;` is a space to the reader the server uses, and the reader drops white space
		let (_dir, path) = epub(
			CHAPTER,
			&[("OEBPS/p.dat", SCRIPT_IN_A_DAT.as_bytes().to_vec())],
			&opf(
				r#"<item id="p" href="p.dat" media-type="text/html&nbsp;"/>"#,
				r#"<itemref idref="c1"/><itemref idref="p"/>"#,
			),
		);
		is_refused_for_active_content(&path, "text/html&nbsp;");
	}

	#[test]
	fn the_package_is_the_one_epub_rs_takes_and_not_a_decoy_before_it() {
		// the first `rootfile` of exactly that name is the package; one in another case is not
		let container = CONTAINER.replace(
			r#"<rootfile full-path"#,
			r#"<ROOTFILE full-path="decoy.opf" media-type="application/oebps-package+xml"/><rootfile full-path"#,
		);
		let real = opf(
			r#"<item id="p" href="p.dat" media-type="text/html"/>"#,
			r#"<itemref idref="c1"/><itemref idref="p"/>"#,
		);
		let (_dir, path) = zip_file(&[
			("mimetype", b"application/epub+zip".to_vec()),
			("META-INF/container.xml", container.into_bytes()),
			("decoy.opf", opf("", "").into_bytes()),
			("OEBPS/content.opf", real.into_bytes()),
			("OEBPS/c1.xhtml", CHAPTER.as_bytes().to_vec()),
			("OEBPS/p.dat", SCRIPT_IN_A_DAT.as_bytes().to_vec()),
		]);
		is_refused_for_active_content(&path, "a decoy ROOTFILE before the real one");
	}

	#[test]
	fn the_table_of_contents_is_the_file_the_spine_names_whatever_it_is_called() {
		// epub-rs reads the resource the spine's `toc` names as a table of contents, deep or not
		let ncx = format!(
			"<ncx>{}{}</ncx>",
			"<navPoint>".repeat(400),
			"</navPoint>".repeat(400)
		);
		let opf_text = format!(
			r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="2.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata><manifest><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/><item id="n" href="toc.dat" media-type="application/octet-stream"/></manifest><spine toc="n"><itemref idref="c1"/></spine></package>"#
		);
		let (_dir, path) =
			epub(CHAPTER, &[("OEBPS/toc.dat", ncx.into_bytes())], &opf_text);
		assert_eq!(
			check(&path, ActiveContent::Strip, false).unwrap_err().code,
			code::XML_TOO_DEEP
		);
	}

	#[test]
	fn the_limits_count_what_epub_rs_counts_whatever_the_elements_are_called() {
		// every child of the spine is a page to epub-rs, and every child of the manifest a resource
		let spine: String = (0..30).map(|_| r#"<x idref="c1"/>"#).collect();
		let (_dir, path) = epub(CHAPTER, &[], &opf("", &spine));
		let limits = Limits {
			max_spine_items: 10,
			..Limits::default()
		};
		let report = zipscan::scan(&path, &limits).unwrap();
		assert_eq!(
			check_epub(&path, &report, &limits, ActiveContent::Strip, false, None)
				.err()
				.expect("refused")
				.code,
			code::TOO_MANY_SPINE_ITEMS
		);
		let many: String = (0..30)
			.map(|i| format!(r#"<x id="i{i}" href="c1.xhtml" media-type="text/css"/>"#))
			.collect();
		let (_dir, path) = epub(CHAPTER, &[], &opf(&many, ""));
		let tight = Limits {
			max_entries: 20,
			..Limits::default()
		};
		let report = zipscan::scan(&path, &tight).unwrap();
		assert_eq!(
			check_epub(&path, &report, &tight, ActiveContent::Strip, false, None)
				.err()
				.expect("refused")
				.code,
			code::TOO_MANY_ENTRIES
		);
	}

	#[test]
	fn a_page_named_with_a_question_mark_is_still_a_page() {
		// the reader asks for `a%3Fb.xhtml` and gets `OEBPS/a?b.xhtml`: it is read like the others
		let page = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body><p>hi</p><script>alert(1)</script></body></html>"#;
		let (_dir, path) = epub(
			CHAPTER,
			&[("OEBPS/a?b.xhtml", page.as_bytes().to_vec())],
			&opf(
				r#"<item id="q" href="a?b.xhtml" media-type="application/xhtml+xml"/>"#,
				r#"<itemref idref="c1"/><itemref idref="q"/>"#,
			),
		);
		let mut replaced: Vec<(String, Vec<u8>)> = vec![];
		let limits = Limits::default();
		let report = zipscan::scan(&path, &limits).unwrap();
		let facts = check_epub(
			&path,
			&report,
			&limits,
			ActiveContent::Strip,
			false,
			Some(&mut replaced),
		)
		.expect("stripped");
		assert!(facts.active.stripped);
		assert!(replaced
			.iter()
			.any(|(name, bytes)| name == "OEBPS/a?b.xhtml"
				&& !String::from_utf8_lossy(bytes).contains("script")));
	}

	fn jpeg_with_a_long_profile(profile_bytes: usize) -> Vec<u8> {
		let image = image::RgbImage::new(20, 20);
		let mut out = Cursor::new(Vec::new());
		image
			.write_to(&mut out, image::ImageFormat::Jpeg)
			.expect("jpeg");
		let jpeg = out.into_inner();
		// APP2 segments (an ICC profile is a run of them) right after the start of image marker
		let mut with = vec![0xFF, 0xD8];
		let (mut left, mut seed) = (profile_bytes, 0x2545_F491u32);
		while left > 0 {
			let chunk = left.min(60_000);
			with.extend_from_slice(&[0xFF, 0xE2]);
			with.extend_from_slice(&((chunk + 2) as u16).to_be_bytes());
			// bytes that don't compress, or the archive's ratio limit would stop the book first
			with.extend((0..chunk).map(|_| {
				seed ^= seed << 13;
				seed ^= seed >> 17;
				seed ^= seed << 5;
				seed as u8
			}));
			left -= chunk;
		}
		with.extend_from_slice(&jpeg[2..]);
		with
	}

	#[test]
	fn an_image_with_a_long_profile_before_its_size_is_looked_at_further_but_not_for_ever(
	) {
		// 3 of 1,500 real books have images like this: the size comes after 300 KB of profile
		let jpeg = jpeg_with_a_long_profile(300_000);
		assert!(jpeg.len() > 256 * 1024);
		let (_dir, path) = epub(
			CHAPTER,
			&[("OEBPS/p.jpg", jpeg)],
			&opf(
				r#"<item id="p" href="p.jpg" media-type="image/jpeg"/>"#,
				r#"<itemref idref="c1"/>"#,
			),
		);
		check(&path, ActiveContent::Strip, false).expect("passes");
		// more than the most that is looked through is not an image with a size
		let (_dir, path) = epub(
			CHAPTER,
			&[("OEBPS/p.jpg", jpeg_with_a_long_profile(9 * 1024 * 1024))],
			&opf(
				r#"<item id="p" href="p.jpg" media-type="image/jpeg"/>"#,
				r#"<itemref idref="c1"/>"#,
			),
		);
		assert_eq!(
			check(&path, ActiveContent::Strip, false).unwrap_err().code,
			code::IMAGE_UNDECODABLE
		);
	}

	#[test]
	fn the_entries_the_server_serves_for_an_address_are_found_every_way_it_could_read_it()
	{
		let names = [
			"OEBPS/c1.xhtml",
			"OEBPS/a b.xhtml",
			"OEBPS/a?b.xhtml",
			"img/x.png",
		];
		let index_of: HashMap<&str, usize> = names
			.iter()
			.copied()
			.enumerate()
			.map(|(index, name)| (name, index))
			.collect();
		assert_eq!(served_entries("OEBPS/c1.xhtml", &index_of), vec![0]);
		// the fragment is what the reader keeps to itself
		assert_eq!(served_entries("OEBPS/c1.xhtml#a", &index_of), vec![0]);
		// the route decodes the address, and epub-rs tries the entry decoded
		assert_eq!(served_entries("OEBPS/a%20b.xhtml", &index_of), vec![1]);
		// a question mark is a part of the name, not the start of a query
		assert_eq!(served_entries("OEBPS/a?b.xhtml", &index_of), vec![2]);
		// the route makes the address lexical
		assert_eq!(served_entries("OEBPS/../img/./x.png", &index_of), vec![3]);
		assert!(served_entries("OEBPS/none.xhtml", &index_of).is_empty());
		assert!(served_entries("../x.png", &index_of).is_empty());
	}

	#[test]
	fn a_resource_is_where_the_servers_reader_puts_it() {
		assert_eq!(
			package_path("OEBPS/content.opf", "c1.xhtml"),
			"OEBPS/c1.xhtml"
		);
		assert_eq!(
			package_path("OEBPS/content.opf", "../img/a%20b.png#x"),
			"img/a%20b.png#x"
		);
		assert_eq!(package_path("content.opf", "c1.xhtml"), "c1.xhtml");
		assert_eq!(
			package_path("OEBPS/text/c.opf", "../c.xhtml"),
			"OEBPS/c.xhtml"
		);
	}

	#[test]
	fn the_reader_routes_are_made_once_here_and_a_plain_book_passes() {
		let (_dir, path) = epub(CHAPTER, &[], &opf("", r#"<itemref idref="c1"/>"#));
		assert!(check_reader_routes(&path).is_ok());
		// a file that is no book at all has no manifest to make
		let (_dir, junk) = zip_file(&[("a.txt", b"x".to_vec())]);
		assert!(check_reader_routes(&junk).is_err());
	}

	#[test]
	fn a_deep_text_that_is_not_well_formed_is_still_held_to_how_deep_it_goes() {
		let limits = Limits::default();
		// the reader the server uses keeps what it read before an error, so a text that stops
		// being well formed at once is as deep as its tags say
		let broken = format!("<ncx></wrong>{}", "<a>".repeat(400));
		assert_eq!(
			xml_check(broken.as_bytes(), &limits, "A table of contents")
				.unwrap_err()
				.code,
			code::XML_TOO_DEEP
		);
		// and one that is shallow and broken is only told to be
		let shallow = "<ncx><a>x</wrong></ncx>";
		assert!(!xml_check(shallow.as_bytes(), &limits, "A table of contents").unwrap());
	}

	#[test]
	fn a_package_whose_tree_costs_the_server_too_much_is_refused() {
		// epub-rs keeps the namespaces in scope in every node of its tree: with 40 in scope, 15,000
		// children weigh more than any real package does
		let package = |namespaces: usize| {
			let declared: String = (0..namespaces)
				.map(|i| format!(" xmlns:n{i}=\"urn:n{i}\""))
				.collect();
			let children: String = (0..15_000).map(|_| "<x/>").collect();
			format!(
				r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"{declared}><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata><manifest><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/>{children}</manifest><spine><itemref idref="c1"/></spine></package>"#
			)
		};
		let (_dir, path) = epub(CHAPTER, &[], &package(40));
		assert_eq!(
			check(&path, ActiveContent::Strip, false)
				.err()
				.expect("refused")
				.code,
			code::TOO_MANY_ENTRIES
		);
		// the same children with the namespaces a package has are no trouble
		let (_dir, path) = epub(CHAPTER, &[], &package(5));
		check(&path, ActiveContent::Strip, false).expect("passes");
		// and more namespaces in scope than any book has are refused whatever the rest
		let (_dir, path) = epub(CHAPTER, &[], &package(200));
		assert!(check(&path, ActiveContent::Strip, false).is_err());
	}

	#[test]
	fn a_book_that_costs_the_server_too_much_to_open_is_refused() {
		// the server looks through the table of contents for each page's title, so the work is the
		// pages times the entries: 2,300 of each is more than any real book comes near
		let entries: String = (0..2_300)
			.map(|i| format!("<navPoint id=\"p{i}\" playOrder=\"{i}\"><navLabel><text>L</text></navLabel><content src=\"c1.xhtml#{i}\"/></navPoint>"))
			.collect();
		let ncx = format!(
			"<?xml version=\"1.0\"?><ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\"><docTitle><text>T</text></docTitle><navMap>{entries}</navMap></ncx>"
		);
		let spine: String = (0..2_300).map(|_| r#"<itemref idref="c1"/>"#).collect();
		let opf_text = format!(
			r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="2.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata><manifest><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/><item id="n" href="toc.ncx" media-type="application/x-dtbncx+xml"/></manifest><spine toc="n">{spine}</spine></package>"#
		);
		let (_dir, path) =
			epub(CHAPTER, &[("OEBPS/toc.ncx", ncx.into_bytes())], &opf_text);
		assert_eq!(
			check(&path, ActiveContent::Strip, false)
				.err()
				.expect("refused")
				.code,
			code::TOO_DEMANDING
		);
	}

	#[test]
	fn a_table_of_contents_with_more_entries_than_a_book_has_is_refused() {
		let entries: String = (0..30)
			.map(|i| format!("<navPoint id=\"p{i}\" playOrder=\"{i}\"><navLabel><text>L{i}</text></navLabel><content src=\"c1.xhtml#{i}\"/></navPoint>"))
			.collect();
		let ncx = format!(
			"<?xml version=\"1.0\"?><ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\"><docTitle><text>T</text></docTitle><navMap>{entries}</navMap></ncx>"
		);
		let opf_text = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="2.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata><manifest><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/><item id="n" href="toc.ncx" media-type="application/x-dtbncx+xml"/></manifest><spine toc="n"><itemref idref="c1"/></spine></package>"#;
		let (_dir, path) =
			epub(CHAPTER, &[("OEBPS/toc.ncx", ncx.into_bytes())], opf_text);
		let limits = Limits {
			max_toc_entries: 10,
			..Limits::default()
		};
		let report = zipscan::scan(&path, &limits).unwrap();
		assert_eq!(
			check_epub(&path, &report, &limits, ActiveContent::Strip, false, None)
				.err()
				.expect("refused")
				.code,
			code::TOO_MANY_ENTRIES
		);
		// and the same book with room for them passes
		let roomy = Limits::default();
		let report = zipscan::scan(&path, &roomy).unwrap();
		assert!(
			check_epub(&path, &report, &roomy, ActiveContent::Strip, false, None).is_ok()
		);
	}
}
