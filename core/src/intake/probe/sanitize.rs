//! The pages of a book that somebody who needs approval sent are never "cleaned": they are
//! **rebuilt**.
//!
//! The reader draws every page of an EPUB as a document of the app's own origin that may run scripts
//! (it needs its own). So a script in a book that somebody sent runs as whoever reads it, the owner
//! included. Looking for scripts in the bytes of a page is a race against every way a browser reads
//! a page differently from the program that looks (a DOCTYPE that ends somewhere else, a name in
//! another case or another namespace, a media type that is not the one the package says...), and
//! that race is lost one parser difference at a time.
//!
//! So nothing of the page that was sent is kept. The page is read into a tree, the way a browser
//! reads it (an XML parser for a page that is well formed XML, which is what the reader needs, and
//! `html5ever`, the HTML parser of Servo, for any other), and a **new** page is written out of what
//! that tree holds, element by element and attribute by attribute, from lists of what may be there:
//!
//! - structure and text: the elements and the attributes of the lists below, with every text and
//!   every value written out escaped, in a skeleton of our own (`<html><head>…</head><body>…</body>`);
//! - images, links and style sheets: only addresses inside the book (one that climbs out of it, or
//!   begins with `/`, or names another scheme, is dropped), `http(s)` and `mailto` links, and the
//!   raster images a page may carry in a `data:` address;
//! - SVG and MathML: a small list of drawing and formula elements, with no scripts, no
//!   `foreignObject`, no animation of a link, and no `url()` that leaves the page;
//! - style sheets: read by the tokenizer of Servo's CSS parser, written out token by token, and a
//!   rule that holds an `@import`, a `url()` that leaves the book, a function that loads or runs
//!   something, or anything the tokenizer can't read, is left out.
//!
//! What the browser would do with the page that was sent doesn't matter any more: the page that is
//! filed is the one written here, and only the lists decide what is in it. A page that can't be read
//! as a page, that nests too deep or that is too big, is refused.

use std::collections::BTreeMap;

use cssparser::{Parser, ParserInput, ToCss, Token};
use html5ever::{
	parse_document, tendril::TendrilSink, tree_builder::TreeBuilderOpts, ParseOpts,
};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use quick_xml::{events::Event, name::ResolveResult, NsReader};

use super::contract::Limits;

pub const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";
pub const SVG_NS: &str = "http://www.w3.org/2000/svg";
pub const MATHML_NS: &str = "http://www.w3.org/1998/Math/MathML";

/// What was taken out, by kind (`script`, `iframe`, `onclick`, `javascript-url`...), and how often
pub type Findings = BTreeMap<String, u64>;

/// Why a page is not rebuilt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
	/// More bytes, more nodes or more output than a page may be
	TooBig,
	/// Nested deeper than a page may be
	TooDeep,
	/// Not a page (no `html` element, no `svg` element where one was needed)
	NotADocument,
}

/// The ceilings one page is held to
#[derive(Debug, Clone, Copy)]
pub struct Settings {
	/// Bytes of the page as it was sent
	pub max_input: usize,
	/// Levels of elements
	pub max_depth: u32,
}

impl Settings {
	pub fn from_limits(limits: &Limits) -> Self {
		Self {
			max_input: usize::try_from(limits.max_xml_bytes.saturating_mul(2))
				.unwrap_or(usize::MAX)
				.clamp(1 << 20, 16 << 20),
			max_depth: limits.max_xml_depth.clamp(16, 256),
		}
	}
}

/// How many nodes a page may have: the tree of a page is some hundred bytes a node
const MAX_NODES: usize = 1_500_000;
/// How long the text of one attribute may be
const MAX_ATTRIBUTE: usize = 128 * 1024;
/// How long an address may be (a `data:` image has its own, larger, ceiling)
const MAX_URL: usize = 4096;
const MAX_DATA_URL: usize = 2 * 1024 * 1024;
/// How many levels of blocks a style sheet may nest
const MAX_CSS_DEPTH: u32 = 24;

/// A page as it was rebuilt
#[derive(Debug, Clone, Default)]
pub struct Page {
	pub bytes: Vec<u8>,
	pub findings: Findings,
	/// Addresses of other sites that the page asked for (images, style sheets): none is kept
	pub remote_refs: u64,
}

// ---------------------------------------------------------------------------------------------
// what may be in a page
// ---------------------------------------------------------------------------------------------

const HTML_ALLOWED: &[&str] = &[
	"a", "abbr", "address", "article", "aside", "b", "bdi", "bdo", "big", "blockquote", "br",
	"caption", "center", "cite", "code", "col", "colgroup", "dd", "del", "details", "dfn", "div",
	"dl", "dt", "em", "figcaption", "figure", "font", "footer", "h1", "h2", "h3", "h4", "h5",
	"h6", "header", "hgroup", "hr", "i", "img", "ins", "kbd", "li", "main", "mark", "nav", "ol",
	"p", "pre", "q", "rb", "rp", "rt", "rtc", "ruby", "s", "samp", "section", "small", "span",
	"strike", "strong", "sub", "summary", "sup", "table", "tbody", "td", "tfoot", "th", "thead",
	"time", "tr", "tt", "u", "ul", "var", "wbr",
];
const HTML_VOID: &[&str] = &["br", "hr", "img", "col", "wbr"];
/// Taken out with what is inside them
const HTML_DROPPED: &[&str] = &[
	"script", "iframe", "frame", "frameset", "object", "embed", "applet", "portal", "noscript",
	"template", "audio", "video", "source", "track", "canvas", "map", "area", "base", "meta",
	"title", "input", "button", "select", "textarea", "option", "optgroup", "datalist", "keygen",
	"param", "isindex", "noembed", "noframes", "xmp", "plaintext", "listing", "head", "slot",
];
/// Of those, what is counted as active content
const ACTIVE_ELEMENTS: &[&str] = &[
	"script", "iframe", "frame", "frameset", "object", "embed", "applet", "portal", "base",
];
const HTML_ATTRIBUTES: &[&str] = &[
	"id", "class", "lang", "xml:lang", "dir", "title", "role", "epub:type", "aria-label",
	"aria-hidden", "aria-labelledby", "aria-describedby", "align", "valign", "width", "height",
	"border", "cellpadding", "cellspacing", "bgcolor", "color", "size", "face", "nowrap",
	"colspan", "rowspan", "headers", "scope", "abbr", "start", "type", "value", "reversed",
	"span", "summary", "datetime", "open", "alt",
];

const SVG_ALLOWED: &[&str] = &[
	"svg", "g", "defs", "image", "title", "desc", "path", "rect", "circle", "ellipse", "line",
	"polyline", "polygon", "text", "tspan", "linearGradient", "radialGradient", "stop",
	"clipPath", "use",
];
const SVG_ACTIVE: &[&str] = &[
	"script",
	"foreignObject",
	"set",
	"animate",
	"animateMotion",
	"animateTransform",
	"handler",
	"listener",
];
const SVG_ATTRIBUTES: &[&str] = &[
	"id", "class", "lang", "xml:lang", "viewBox", "preserveAspectRatio", "width", "height", "x",
	"y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "rx", "ry", "d", "points", "transform", "fill",
	"fill-opacity", "fill-rule", "stroke", "stroke-width", "stroke-linecap", "stroke-linejoin",
	"stroke-miterlimit", "stroke-dasharray", "stroke-dashoffset", "stroke-opacity", "opacity",
	"font-family", "font-size", "font-weight", "font-style", "text-anchor", "dx", "dy",
	"offset", "stop-color", "stop-opacity", "gradientUnits", "gradientTransform",
	"clipPathUnits", "clip-path", "clip-rule", "visibility", "display", "version", "role",
	"aria-label", "aria-hidden", "xml:space", "letter-spacing", "word-spacing",
	"text-decoration", "dominant-baseline", "baseline-shift", "spreadMethod", "fx", "fy",
];

const MATH_ALLOWED: &[&str] = &[
	"math", "mrow", "mi", "mn", "mo", "msup", "msub", "msubsup", "mfrac", "msqrt", "mroot",
	"mtext", "mspace", "mover", "munder", "munderover", "mtable", "mtr", "mtd", "mstyle",
	"mpadded", "mphantom", "mmultiscripts", "mprescripts", "none",
];
const MATH_ATTRIBUTES: &[&str] = &[
	"id", "class", "lang", "dir", "display", "mathvariant", "mathsize", "mathcolor",
	"mathbackground", "fence", "stretchy", "separator", "accent", "accentunder", "largeop",
	"movablelimits", "symmetric", "lspace", "rspace", "linethickness", "columnalign",
	"rowalign", "columnspan", "rowspan", "displaystyle", "scriptlevel", "minsize", "maxsize",
	"width", "height", "depth", "voffset",
];

/// At-rules a style sheet may have
const CSS_AT_RULES: &[&str] = &[
	"media", "supports", "font-face", "page", "keyframes", "-webkit-keyframes", "-moz-keyframes",
	"counter-style", "font-feature-values", "layer",
];
/// Functions of a style sheet that load or run something that isn't a plain `url()`
const CSS_FUNCTIONS: &[&str] = &[
	"expression", "image-set", "-webkit-image-set", "image", "cross-fade", "-webkit-cross-fade",
	"element", "-moz-element", "paint", "src", "-moz-binding",
];
const CSS_IDENTS: &[&str] = &["behavior", "-ms-behavior", "-moz-binding", "binding"];

// ---------------------------------------------------------------------------------------------
// the tree a page is read into
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ns {
	Html,
	Svg,
	Math,
	Other,
}

fn ns_of(uri: &str) -> Ns {
	match uri {
		XHTML_NS => Ns::Html,
		SVG_NS => Ns::Svg,
		MATHML_NS => Ns::Math,
		_ => Ns::Other,
	}
}

#[derive(Debug)]
enum Node {
	Text(String),
	Element(El),
}

#[derive(Debug)]
struct El {
	ns: Ns,
	name: String,
	/// `prefix:local` as written
	attrs: Vec<(String, String)>,
	children: Vec<Node>,
}

/// A page's bytes as text: UTF-8 (a BOM is dropped), or UTF-16 with a BOM; what is not valid is
/// replaced, never trusted
fn decode(bytes: &[u8]) -> String {
	if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
		return String::from_utf8_lossy(rest).into_owned();
	}
	if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
		let little = bytes[0] == 0xFF;
		let units = bytes[2..].chunks_exact(2).map(|pair| {
			if little {
				u16::from_le_bytes([pair[0], pair[1]])
			} else {
				u16::from_be_bytes([pair[0], pair[1]])
			}
		});
		return char::decode_utf16(units)
			.map(|c| c.unwrap_or('\u{FFFD}'))
			.collect();
	}
	String::from_utf8_lossy(bytes).into_owned()
}

/// What an HTML entity (`nbsp`, `mdash`...) stands for. XHTML pages of the EPUB 2 days use them
/// with an XHTML DOCTYPE, and a browser knows them for that DOCTYPE
fn named_entity(name: &str) -> Option<String> {
	let key = format!("{name};");
	let &(first, second) = html5ever::data::NAMED_ENTITIES.get(key.as_str())?;
	let mut out = String::new();
	out.push(char::from_u32(first)?);
	if second != 0 {
		out.push(char::from_u32(second)?);
	}
	Some(out)
}

fn push_text(stack: &mut [El], text: &str) -> bool {
	match stack.last_mut() {
		Some(top) => {
			match top.children.last_mut() {
				Some(Node::Text(existing)) => existing.push_str(text),
				_ => top.children.push(Node::Text(text.to_string())),
			}
			true
		},
		// outside the root element nothing but white space may be
		None => text.trim().is_empty(),
	}
}

/// The page as XML, if it is well formed XML (`Ok(None)` if it is not). A DOCTYPE that holds markup
/// of its own is not read as XML: whatever is made of it, the page that is written does not hold it
fn parse_xml(text: &str, set: Settings) -> Result<Option<El>, Refusal> {
	let mut reader = NsReader::from_str(text);
	reader.config_mut().expand_empty_elements = true;
	let mut stack: Vec<El> = Vec::new();
	let mut root: Option<El> = None;
	let mut nodes = 0usize;
	loop {
		let (resolved, event) = match reader.read_resolved_event() {
			Ok(pair) => pair,
			Err(_) => return Ok(None),
		};
		match event {
			Event::Start(start) => {
				if root.is_some() {
					return Ok(None);
				}
				nodes += 1;
				if nodes > MAX_NODES {
					return Err(Refusal::TooBig);
				}
				if stack.len() as u32 >= set.max_depth {
					return Err(Refusal::TooDeep);
				}
				let ns = match &resolved {
					ResolveResult::Bound(uri) => {
						ns_of(std::str::from_utf8(uri.as_ref()).unwrap_or(""))
					},
					// a page with no namespace is read as the HTML it means to be
					ResolveResult::Unbound => Ns::Html,
					ResolveResult::Unknown(_) => return Ok(None),
				};
				let name = match std::str::from_utf8(start.local_name().as_ref()) {
					Ok(name) => name.to_string(),
					Err(_) => return Ok(None),
				};
				let mut attrs = Vec::new();
				for attribute in start.attributes() {
					let Ok(attribute) = attribute else {
						return Ok(None);
					};
					let Ok(key) = std::str::from_utf8(attribute.key.as_ref()) else {
						return Ok(None);
					};
					let Ok(value) = attribute.unescape_value() else {
						return Ok(None);
					};
					attrs.push((key.to_string(), value.into_owned()));
				}
				stack.push(El {
					ns,
					name,
					attrs,
					children: vec![],
				});
			},
			Event::End(_) => {
				let Some(done) = stack.pop() else {
					return Ok(None);
				};
				match stack.last_mut() {
					Some(parent) => parent.children.push(Node::Element(done)),
					None => root = Some(done),
				}
			},
			Event::Text(text) => {
				let Ok(text) = text.decode() else {
					return Ok(None);
				};
				if !push_text(&mut stack, &text) {
					return Ok(None);
				}
			},
			Event::CData(data) => {
				let Ok(text) = data.decode() else {
					return Ok(None);
				};
				if !push_text(&mut stack, &text) {
					return Ok(None);
				}
			},
			Event::GeneralRef(reference) => {
				let Ok(name) = reference.decode() else {
					return Ok(None);
				};
				let text = match reference.resolve_char_ref() {
					Ok(Some(c)) => c.to_string(),
					Ok(None) => match &*name {
						"lt" => "<".to_string(),
						"gt" => ">".to_string(),
						"amp" => "&".to_string(),
						"quot" => "\"".to_string(),
						"apos" => "'".to_string(),
						other => match named_entity(other) {
							Some(text) => text,
							None => return Ok(None),
						},
					},
					Err(_) => return Ok(None),
				};
				if !push_text(&mut stack, &text) {
					return Ok(None);
				}
			},
			Event::DocType(doctype) => {
				let Ok(text) = doctype.decode() else {
					return Ok(None);
				};
				if text.contains(['[', '<', '>']) || text.to_ascii_uppercase().contains("ENTITY") {
					return Ok(None);
				}
			},
			Event::Eof => break,
			// (empty elements are expanded into a start and an end)
			Event::Empty(_) => return Ok(None),
			Event::Decl(_) | Event::PI(_) | Event::Comment(_) => {},
		}
	}
	if !stack.is_empty() {
		return Ok(None);
	}
	Ok(root)
}

fn key_of(name: &html5ever::QualName) -> String {
	match &name.prefix {
		Some(prefix) if !prefix.is_empty() => format!("{}:{}", &**prefix, &*name.local),
		_ => name.local.to_string(),
	}
}

fn convert(
	handle: &Handle,
	depth: u32,
	set: Settings,
	nodes: &mut usize,
) -> Result<Option<Node>, Refusal> {
	match &handle.data {
		NodeData::Text { contents } => {
			*nodes += 1;
			if *nodes > MAX_NODES {
				return Err(Refusal::TooBig);
			}
			Ok(Some(Node::Text(String::from(&*contents.borrow()))))
		},
		NodeData::Element { name, attrs, .. } => {
			if depth >= set.max_depth {
				return Err(Refusal::TooDeep);
			}
			*nodes += 1;
			if *nodes > MAX_NODES {
				return Err(Refusal::TooBig);
			}
			let ns = ns_of(&name.ns);
			let attrs = attrs
				.borrow()
				.iter()
				.map(|a| (key_of(&a.name), String::from(&a.value)))
				.collect();
			let mut children = Vec::new();
			for child in handle.children.borrow().iter() {
				if let Some(node) = convert(child, depth + 1, set, nodes)? {
					children.push(node);
				}
			}
			Ok(Some(Node::Element(El {
				ns,
				name: name.local.to_string(),
				attrs,
				children,
			})))
		},
		_ => Ok(None),
	}
}

/// The page as an HTML parser reads it: the tree a browser would make of it
fn parse_html(text: String, set: Settings) -> Result<El, Refusal> {
	let options = ParseOpts {
		tree_builder: TreeBuilderOpts {
			scripting_enabled: false,
			drop_doctype: true,
			..Default::default()
		},
		..Default::default()
	};
	let dom = parse_document(RcDom::default(), options).one(text);
	let mut nodes = 0usize;
	let children: Vec<Handle> = dom.document.children.borrow().clone();
	for child in &children {
		if matches!(child.data, NodeData::Element { .. }) {
			if let Some(Node::Element(root)) = convert(child, 0, set, &mut nodes)? {
				return Ok(root);
			}
		}
	}
	Err(Refusal::NotADocument)
}

fn parse_markup(bytes: &[u8], set: Settings) -> Result<El, Refusal> {
	if bytes.len() > set.max_input {
		return Err(Refusal::TooBig);
	}
	let text = decode(bytes);
	match parse_xml(&text, set)? {
		Some(root) => Ok(root),
		None => parse_html(text, set),
	}
}

// ---------------------------------------------------------------------------------------------
// addresses
// ---------------------------------------------------------------------------------------------

/// An address as a browser reads it: tabs and line breaks gone, the white space around it gone
fn clean_url(value: &str) -> Option<String> {
	let url: String = value
		.chars()
		.filter(|c| !matches!(c, '\t' | '\n' | '\r'))
		.collect();
	let url = url.trim_matches(|c: char| c <= ' ').to_string();
	if url.is_empty()
		|| url.len() > MAX_DATA_URL
		|| url.contains('\\')
		|| url.chars().any(|c| c < ' ' || ('\u{7f}'..='\u{9f}').contains(&c))
		// a path that begins with `/` is the app's own, not the book's
		|| url.starts_with('/')
	{
		return None;
	}
	Some(url)
}

/// The scheme of an address, if it has one (`javascript` in `javascript:alert(1)`)
fn scheme_of(url: &str) -> Option<&str> {
	let end = url.find([':', '/', '?', '#'])?;
	if url.as_bytes()[end] != b':' || end == 0 {
		return None;
	}
	let scheme = &url[..end];
	let mut chars = scheme.chars();
	let first = chars.next()?;
	(first.is_ascii_alphabetic()
		&& chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')))
	.then_some(scheme)
}

/// Whether a path that doesn't begin with `/` stays inside the book from the folder of the file it
/// is in (`base`): a path that climbs out of it names something of the app's, not of the book's
fn stays_in_book(url: &str, base: &str) -> bool {
	let path = url.split(['#', '?']).next().unwrap_or("");
	// a browser reads `%2e` as a dot in a dot segment
	let path = path.replace("%2e", ".").replace("%2E", ".");
	let mut depth = base.split('/').filter(|s| !s.is_empty()).count() as i64;
	for segment in path.split('/') {
		match segment {
			"" | "." => {},
			".." => {
				depth -= 1;
				if depth < 0 {
					return false;
				}
			},
			_ => depth += 1,
		}
	}
	true
}

/// A raster image in a `data:` address
fn raster_data_url(url: &str) -> bool {
	let head: String = url.chars().take(48).collect::<String>().to_ascii_lowercase();
	["data:image/png", "data:image/jpeg", "data:image/gif", "data:image/webp"]
		.iter()
		.any(|prefix| head.starts_with(prefix))
		&& head.contains(',')
}

fn is_handler(key: &str) -> bool {
	// bytes, not a string slice: an attribute name such as `a\u{e9}` or `\u{30a2}x` is a valid XML
	// name, and slicing it at 2 would panic in the middle of a character (and the checker aborts)
	key.len() > 2
		&& key.as_bytes()[..2].eq_ignore_ascii_case(b"on")
		&& key.chars().all(|c| c.is_ascii_alphabetic())
}

// ---------------------------------------------------------------------------------------------
// writing the new page
// ---------------------------------------------------------------------------------------------

struct Cx<'a> {
	base: &'a str,
	out: String,
	cap: usize,
	findings: Findings,
	remote_refs: u64,
	in_svg: bool,
	in_math: bool,
}

impl<'a> Cx<'a> {
	fn new(base: &'a str, set: Settings) -> Self {
		Self {
			base,
			out: String::new(),
			cap: set.max_input.saturating_mul(2).max(64 * 1024),
			findings: Findings::new(),
			remote_refs: 0,
			in_svg: false,
			in_math: false,
		}
	}

	fn found(&mut self, kind: &str) {
		let kind: String = kind.chars().take(24).collect();
		*self.findings.entry(kind).or_default() += 1;
	}

	fn room(&self) -> Result<(), Refusal> {
		if self.out.len() > self.cap {
			Err(Refusal::TooBig)
		} else {
			Ok(())
		}
	}

	fn finish(self) -> Page {
		Page {
			bytes: self.out.into_bytes(),
			findings: self.findings,
			remote_refs: self.remote_refs,
		}
	}
}

fn xml_char(c: char) -> bool {
	matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

fn escape_text(text: &str, out: &mut String) {
	for c in text.chars() {
		match c {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			c if xml_char(c) => out.push(c),
			_ => {},
		}
	}
}

fn escape_attribute(text: &str, out: &mut String) {
	for c in text.chars() {
		match c {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			'"' => out.push_str("&quot;"),
			'\n' => out.push_str("&#10;"),
			'\r' => out.push_str("&#13;"),
			'\t' => out.push_str("&#9;"),
			c if xml_char(c) => out.push(c),
			_ => {},
		}
	}
}

fn clean_value(value: &str) -> String {
	value.chars().filter(|c| xml_char(*c)).collect()
}

fn put_attribute(cx: &mut Cx, key: &str, value: &str) {
	cx.out.push(' ');
	cx.out.push_str(key);
	cx.out.push_str("=\"");
	escape_attribute(value, &mut cx.out);
	cx.out.push('"');
}

/// A link: inside the book, or an `http(s)`/`mailto` one
fn link_url(value: &str, cx: &mut Cx) -> Option<String> {
	let url = clean_url(value)?;
	match scheme_of(&url) {
		None => (url.len() <= MAX_URL && !url.starts_with("//") && stays_in_book(&url, cx.base))
			.then_some(url),
		Some(scheme) => match scheme.to_ascii_lowercase().as_str() {
			"http" | "https" | "mailto" => (url.len() <= MAX_URL).then_some(url),
			"javascript" | "vbscript" | "data" => {
				cx.found("script-url");
				None
			},
			_ => None,
		},
	}
}

/// What loads by itself (an image, a style sheet): inside the book, or a raster image in the page
fn load_url(value: &str, cx: &mut Cx, data_allowed: bool) -> Option<String> {
	let url = clean_url(value)?;
	match scheme_of(&url) {
		None => (url.len() <= MAX_URL && !url.starts_with("//") && stays_in_book(&url, cx.base))
			.then_some(url),
		Some(scheme) => match scheme.to_ascii_lowercase().as_str() {
			"data" if data_allowed && raster_data_url(&url) => Some(url),
			"http" | "https" => {
				cx.remote_refs += 1;
				None
			},
			"javascript" | "vbscript" => {
				cx.found("script-url");
				None
			},
			_ => None,
		},
	}
}

/// An SVG paint or reference that is a plain `url(#id)`, or has no `url(` at all
fn paint_ok(value: &str) -> bool {
	let lower = value.to_ascii_lowercase();
	if !lower.contains("url(") {
		return true;
	}
	let t = lower.trim();
	t.starts_with("url(#")
		&& t.ends_with(')')
		&& t.matches('(').count() == 1
		&& t.chars().all(|c| c.is_ascii_alphanumeric() || "()#_.:-".contains(c))
}

// ----- style sheets ----------------------------------------------------------------------------

struct Rule {
	buf: String,
	bad: bool,
	remote: u64,
}

impl Rule {
	fn new() -> Self {
		Self {
			buf: String::new(),
			bad: false,
			remote: 0,
		}
	}
}

fn css_url_ok(url: &str, base: &str, rule: &mut Rule) -> bool {
	let Some(url) = clean_url(url) else {
		return false;
	};
	match scheme_of(&url) {
		None => url.len() <= MAX_URL && !url.starts_with("//") && stays_in_book(&url, base),
		Some(scheme) => match scheme.to_ascii_lowercase().as_str() {
			"data" => raster_data_url(&url),
			"http" | "https" => {
				rule.remote += 1;
				false
			},
			_ => false,
		},
	}
}

fn css_nested<'i, 't>(
	parser: &mut Parser<'i, 't>,
	rule: &mut Rule,
	base: &str,
	depth: u32,
	in_url: bool,
) {
	if depth >= MAX_CSS_DEPTH {
		rule.bad = true;
		return;
	}
	let _ = parser.parse_nested_block::<_, (), ()>(|inner| {
		while let Ok(token) = inner.next_including_whitespace_and_comments() {
			let token = token.clone();
			css_token(&token, inner, rule, base, depth + 1, in_url);
		}
		Ok(())
	});
}

/// One token of a style sheet, written out if it may be; anything that may not be marks the rule
fn css_token<'i, 't>(
	token: &Token<'i>,
	parser: &mut Parser<'i, 't>,
	rule: &mut Rule,
	base: &str,
	depth: u32,
	in_url: bool,
) {
	match token {
		Token::WhiteSpace(_) | Token::Comment(_) => rule.buf.push(' '),
		Token::CDO | Token::CDC => {
			// at the top of a style sheet these two are dropped, which is all that writing nothing for
			// them is right for. Anywhere else they are tokens of their own (a browser makes the rule
			// invalid), and writing nothing glued the pieces on either side together: `url<!--(...)`
			// came out as `url(...)` and `@<!--import "..."` as `@import "..."`. A rule that holds one
			// is left out, as the browser would drop it.
			if !rule.buf.trim().is_empty() {
				rule.bad = true;
			}
		},
		Token::BadUrl(_) | Token::BadString(_) => rule.bad = true,
		Token::CloseParenthesis | Token::CloseSquareBracket | Token::CloseCurlyBracket => {
			rule.bad = true;
		},
		Token::AtKeyword(name) => {
			if !CSS_AT_RULES.contains(&name.to_ascii_lowercase().as_str()) {
				rule.bad = true;
			}
			rule.buf.push_str(&token.to_css_string());
		},
		Token::Ident(name) => {
			if CSS_IDENTS.contains(&name.to_ascii_lowercase().as_str()) {
				rule.bad = true;
			}
			rule.buf.push_str(&token.to_css_string());
		},
		Token::UnquotedUrl(url) => {
			if !css_url_ok(url, base, rule) {
				rule.bad = true;
			}
			rule.buf.push_str(&token.to_css_string());
		},
		Token::QuotedString(text) => {
			if in_url && !css_url_ok(text, base, rule) {
				rule.bad = true;
			}
			rule.buf.push_str(&token.to_css_string());
		},
		Token::Function(name) => {
			let lower = name.to_ascii_lowercase();
			if CSS_FUNCTIONS.contains(&lower.as_str()) {
				rule.bad = true;
			}
			rule.buf.push_str(&token.to_css_string());
			css_nested(parser, rule, base, depth, lower == "url");
			rule.buf.push(')');
		},
		Token::ParenthesisBlock => {
			rule.buf.push('(');
			css_nested(parser, rule, base, depth, false);
			rule.buf.push(')');
		},
		Token::SquareBracketBlock => {
			rule.buf.push('[');
			css_nested(parser, rule, base, depth, false);
			rule.buf.push(']');
		},
		Token::CurlyBracketBlock => {
			rule.buf.push('{');
			css_nested(parser, rule, base, depth, false);
			rule.buf.push('}');
		},
		other => rule.buf.push_str(&other.to_css_string()),
	}
}

/// A style sheet, rule by rule: what is kept is what its tokens say, written out again
pub fn clean_stylesheet(css: &str, base: &str) -> (String, u64) {
	let mut input = ParserInput::new(css);
	let mut parser = Parser::new(&mut input);
	let mut out = String::new();
	let mut rule = Rule::new();
	let mut remote = 0u64;
	let mut finish = |rule: &mut Rule, out: &mut String| {
		let done = std::mem::replace(rule, Rule::new());
		remote += done.remote;
		let text = done.buf.trim();
		if !done.bad && !text.is_empty() {
			out.push_str(text);
			out.push('\n');
		}
	};
	while let Ok(token) = parser.next_including_whitespace_and_comments() {
		let token = token.clone();
		match &token {
			Token::Semicolon => {
				rule.buf.push(';');
				finish(&mut rule, &mut out);
			},
			Token::CurlyBracketBlock => {
				rule.buf.push('{');
				css_nested(&mut parser, &mut rule, base, 0, false);
				rule.buf.push('}');
				finish(&mut rule, &mut out);
			},
			_ => css_token(&token, &mut parser, &mut rule, base, 0, false),
		}
	}
	finish(&mut rule, &mut out);
	(out, remote)
}

/// The declarations of a `style` attribute, or `None` if one of them may not be
fn clean_declarations(value: &str, base: &str, cx: &mut Cx) -> Option<String> {
	let mut input = ParserInput::new(value);
	let mut parser = Parser::new(&mut input);
	let mut rule = Rule::new();
	while let Ok(token) = parser.next_including_whitespace_and_comments() {
		let token = token.clone();
		css_token(&token, &mut parser, &mut rule, base, 0, false);
	}
	cx.remote_refs += rule.remote;
	(!rule.bad).then(|| rule.buf.trim().to_string())
}

// ----- elements ----------------------------------------------------------------------------------

fn write_children(el: &El, cx: &mut Cx) -> Result<(), Refusal> {
	for child in &el.children {
		match child {
			Node::Text(text) => escape_text(text, &mut cx.out),
			Node::Element(child) => write_el(child, cx)?,
		}
		cx.room()?;
	}
	Ok(())
}

fn write_el(el: &El, cx: &mut Cx) -> Result<(), Refusal> {
	match el.ns {
		Ns::Html => write_html(el, cx),
		Ns::Svg => write_svg(el, cx),
		Ns::Math => write_math(el, cx),
		Ns::Other => Ok(()),
	}
}

fn text_of(el: &El) -> String {
	el.children
		.iter()
		.filter_map(|child| match child {
			Node::Text(text) => Some(text.as_str()),
			Node::Element(_) => None,
		})
		.collect()
}

fn write_style(el: &El, cx: &mut Cx) -> Result<(), Refusal> {
	let (css, remote) = clean_stylesheet(&text_of(el), cx.base);
	cx.remote_refs += remote;
	if css.trim().is_empty() {
		return Ok(());
	}
	cx.out.push_str("<style type=\"text/css\"");
	if let Some((_, media)) = el.attrs.iter().find(|(k, _)| k == "media") {
		if media.len() < 200
			&& media
				.chars()
				.all(|c| c.is_ascii_alphanumeric() || " ,:()._-".contains(c))
		{
			put_attribute(cx, "media", media);
		}
	}
	cx.out.push('>');
	escape_text(&css, &mut cx.out);
	cx.out.push_str("</style>");
	Ok(())
}

fn write_link(el: &El, cx: &mut Cx) -> Result<(), Refusal> {
	let attribute = |key: &str| {
		el.attrs
			.iter()
			.find(|(k, _)| k == key)
			.map(|(_, v)| v.as_str())
	};
	let rel = attribute("rel").unwrap_or("").to_ascii_lowercase();
	let mut words = rel.split_ascii_whitespace();
	if !words.clone().any(|w| w == "stylesheet") || words.any(|w| w == "alternate") {
		return Ok(());
	}
	let Some(href) = attribute("href").and_then(|href| load_url(href, cx, false)) else {
		return Ok(());
	};
	cx.out.push_str("<link rel=\"stylesheet\" type=\"text/css\"");
	put_attribute(cx, "href", &href);
	cx.out.push_str("/>");
	Ok(())
}

fn write_html_attributes(el: &El, cx: &mut Cx) {
	let name = el.name.as_str();
	for (key, value) in &el.attrs {
		let key = key.as_str();
		if key.starts_with("xmlns") {
			continue;
		}
		if is_handler(key) {
			cx.found(&key.to_ascii_lowercase());
			continue;
		}
		let value = clean_value(value);
		if value.len() > MAX_ATTRIBUTE {
			continue;
		}
		let base = cx.base;
		let keep = match key {
			"style" => clean_declarations(&value, base, cx).filter(|s| !s.is_empty()),
			"href" if name == "a" => link_url(&value, cx),
			"src" if name == "img" => load_url(&value, cx, true),
			"name" if name == "a" => Some(value),
			_ => HTML_ATTRIBUTES.contains(&key).then_some(value),
		};
		if let Some(value) = keep {
			let key = match key {
				"href" | "src" | "name" | "style" => key,
				_ => HTML_ATTRIBUTES.iter().find(|k| **k == key).copied().unwrap_or(key),
			};
			put_attribute(cx, key, &value);
		}
	}
}

fn write_html(el: &El, cx: &mut Cx) -> Result<(), Refusal> {
	if cx.in_svg || cx.in_math {
		return Ok(());
	}
	let name = el.name.as_str();
	match name {
		"style" => return write_style(el, cx),
		"link" => return write_link(el, cx),
		_ => {},
	}
	if HTML_DROPPED.contains(&name) {
		if ACTIVE_ELEMENTS.contains(&name) {
			cx.found(name);
		} else if name == "meta"
			&& el.attrs.iter().any(|(k, v)| {
				k == "http-equiv"
					&& (v.eq_ignore_ascii_case("refresh") || v.eq_ignore_ascii_case("set-cookie"))
			}) {
			cx.found("meta-refresh");
		}
		return Ok(());
	}
	if !HTML_ALLOWED.contains(&name) {
		// an element that isn't known is not shown, but what is in it is
		return write_children(el, cx);
	}
	cx.out.push('<');
	cx.out.push_str(name);
	write_html_attributes(el, cx);
	if HTML_VOID.contains(&name) {
		cx.out.push_str("/>");
		return Ok(());
	}
	cx.out.push('>');
	write_children(el, cx)?;
	cx.out.push_str("</");
	cx.out.push_str(name);
	cx.out.push('>');
	Ok(())
}

fn write_svg(el: &El, cx: &mut Cx) -> Result<(), Refusal> {
	if cx.in_math {
		return Ok(());
	}
	let name = el.name.as_str();
	if SVG_ACTIVE.contains(&name) {
		cx.found(name);
		return Ok(());
	}
	match name {
		"style" => return write_style(el, cx),
		"a" | "switch" | "symbol" | "marker" => return write_children(el, cx),
		_ => {},
	}
	if !SVG_ALLOWED.contains(&name) {
		return Ok(());
	}
	let fragment_only = |value: &str| {
		clean_url(value).is_some_and(|u| {
			u.starts_with('#') && u.len() < 256 && u.chars().all(|c| !c.is_whitespace())
		})
	};
	// a `use` that points outside the page is not drawn
	if name == "use" {
		let target = el
			.attrs
			.iter()
			.find(|(k, _)| k == "href" || k == "xlink:href")
			.map(|(_, v)| v.as_str());
		if !target.is_some_and(fragment_only) {
			return Ok(());
		}
	}
	cx.out.push('<');
	cx.out.push_str(name);
	if !cx.in_svg {
		cx.out.push_str(" xmlns=\"");
		cx.out.push_str(SVG_NS);
		cx.out.push_str("\" xmlns:xlink=\"http://www.w3.org/1999/xlink\"");
	}
	let mut href_written = false;
	for (key, value) in &el.attrs {
		let key = key.as_str();
		if key.starts_with("xmlns") {
			continue;
		}
		if is_handler(key) {
			cx.found(&key.to_ascii_lowercase());
			continue;
		}
		let value = clean_value(value);
		if value.len() > MAX_ATTRIBUTE {
			continue;
		}
		let base = cx.base;
		match key {
			"style" => {
				if let Some(style) = clean_declarations(&value, base, cx).filter(|s| !s.is_empty()) {
					put_attribute(cx, "style", &style);
				}
			},
			"href" | "xlink:href" => {
				// `href` and `xlink:href` on one element would both be written as `xlink:href`, and a
				// page with a repeated attribute is not well formed XML (the reader draws nothing):
				// the first one that is valid is kept
				if href_written {
					continue;
				}
				let kept = match name {
					"image" => load_url(&value, cx, true),
					"use" => fragment_only(&value).then(|| value.trim().to_string()),
					_ => None,
				};
				if let Some(kept) = kept {
					put_attribute(cx, "xlink:href", &kept);
					href_written = true;
				}
			},
			_ => {
				if let Some(known) = SVG_ATTRIBUTES.iter().find(|k| **k == key) {
					if paint_ok(&value) {
						put_attribute(cx, known, &value);
					}
				}
			},
		}
	}
	cx.out.push('>');
	let was = cx.in_svg;
	cx.in_svg = true;
	let children = write_children(el, cx);
	cx.in_svg = was;
	children?;
	cx.out.push_str("</");
	cx.out.push_str(name);
	cx.out.push('>');
	Ok(())
}

fn write_math(el: &El, cx: &mut Cx) -> Result<(), Refusal> {
	if cx.in_svg {
		return Ok(());
	}
	let name = el.name.as_str();
	match name {
		"annotation" | "annotation-xml" | "maction" => return Ok(()),
		"semantics" => return write_children(el, cx),
		_ => {},
	}
	if !MATH_ALLOWED.contains(&name) {
		return write_children(el, cx);
	}
	cx.out.push('<');
	cx.out.push_str(name);
	if !cx.in_math {
		cx.out.push_str(" xmlns=\"");
		cx.out.push_str(MATHML_NS);
		cx.out.push('"');
	}
	for (key, value) in &el.attrs {
		if let Some(known) = MATH_ATTRIBUTES.iter().find(|k| **k == key.as_str()) {
			let value = clean_value(value);
			if value.len() <= 256 {
				put_attribute(cx, known, &value);
			}
		}
	}
	cx.out.push('>');
	let was = cx.in_math;
	cx.in_math = true;
	let children = write_children(el, cx);
	cx.in_math = was;
	children?;
	cx.out.push_str("</");
	cx.out.push_str(name);
	cx.out.push('>');
	Ok(())
}

// ---------------------------------------------------------------------------------------------
// pages
// ---------------------------------------------------------------------------------------------

fn child_named<'a>(el: &'a El, name: &str) -> Option<&'a El> {
	el.children.iter().find_map(|child| match child {
		Node::Element(e) if e.ns == Ns::Html && e.name == name => Some(e),
		_ => None,
	})
}

fn write_page(root: &El, cx: &mut Cx) -> Result<(), Refusal> {
	if root.ns != Ns::Html || root.name != "html" {
		return Err(Refusal::NotADocument);
	}
	cx.out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n");
	cx.out.push_str(
		"<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"",
	);
	for (key, value) in &root.attrs {
		if matches!(key.as_str(), "lang" | "xml:lang" | "dir") {
			let value = clean_value(value);
			if value.len() < 64 {
				put_attribute(cx, key, &value);
			}
		}
	}
	cx.out.push_str("><head>");
	let head = child_named(root, "head");
	let title = head
		.and_then(|head| child_named(head, "title"))
		.map(|t| clean_value(text_of(t).trim()))
		.filter(|t| !t.is_empty())
		.unwrap_or_else(|| "Page".to_string());
	cx.out.push_str("<title>");
	escape_text(&title.chars().take(300).collect::<String>(), &mut cx.out);
	cx.out.push_str("</title>");
	if let Some(head) = head {
		for child in &head.children {
			if let Node::Element(child) = child {
				if child.ns == Ns::Html && child.name != "title" {
					write_html(child, cx)?;
				}
			}
		}
	}
	cx.out.push_str("</head><body");
	let body = child_named(root, "body");
	if let Some(body) = body {
		write_html_attributes(body, cx);
	}
	cx.out.push('>');
	match body {
		Some(body) => write_children(body, cx)?,
		None => {
			// no `body` (a page that wasn't made with one): everything but the head is the body
			for child in &root.children {
				match child {
					Node::Text(text) => escape_text(text, &mut cx.out),
					Node::Element(e) if e.ns == Ns::Html && e.name == "head" => {},
					Node::Element(e) => write_el(e, cx)?,
				}
			}
		},
	}
	cx.out.push_str("</body></html>\n");
	cx.room()
}

/// A page of the book, rebuilt. `base` is the folder (in the book) the page is in
pub fn clean_page(bytes: &[u8], base: &str, set: Settings) -> Result<Page, Refusal> {
	let root = parse_markup(bytes, set)?;
	let mut cx = Cx::new(base, set);
	write_page(&root, &mut cx)?;
	Ok(cx.finish())
}

/// An SVG file of the book, rebuilt (only as XML: it is not a page)
pub fn clean_svg(bytes: &[u8], base: &str, set: Settings) -> Result<Page, Refusal> {
	if bytes.len() > set.max_input {
		return Err(Refusal::TooBig);
	}
	let text = decode(bytes);
	let Some(root) = parse_xml(&text, set)? else {
		return Err(Refusal::NotADocument);
	};
	if root.ns != Ns::Svg || root.name != "svg" {
		return Err(Refusal::NotADocument);
	}
	let mut cx = Cx::new(base, set);
	cx.out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
	write_svg(&root, &mut cx)?;
	cx.out.push('\n');
	Ok(cx.finish())
}

/// A style sheet of the book, rebuilt
pub fn clean_css(bytes: &[u8], base: &str, set: Settings) -> Result<Page, Refusal> {
	if bytes.len() > set.max_input {
		return Err(Refusal::TooBig);
	}
	let (css, remote) = clean_stylesheet(&decode(bytes), base);
	Ok(Page {
		bytes: css.into_bytes(),
		findings: Findings::new(),
		remote_refs: remote,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	fn set() -> Settings {
		Settings::from_limits(&Limits::default())
	}

	fn page(source: &str) -> String {
		let page = clean_page(source.as_bytes(), "OEBPS/text", set()).expect("a page");
		String::from_utf8(page.bytes).expect("utf-8")
	}

	/// Whether `bytes` are well formed XML (the reader refuses a page that isn't)
	fn well_formed(text: &str) -> bool {
		let mut reader = quick_xml::Reader::from_str(text);
		loop {
			match reader.read_event() {
				Ok(Event::Eof) => return true,
				Ok(Event::Start(e) | Event::Empty(e)) => {
					if e.attributes().any(|a| a.is_err()) {
						return false;
					}
				},
				Ok(Event::Text(t)) => {
					if t.decode().is_err() {
						return false;
					}
				},
				Ok(Event::GeneralRef(r)) => {
					let known = matches!(r.decode().as_deref(), Ok("lt" | "gt" | "amp" | "quot" | "apos"))
						|| matches!(r.resolve_char_ref(), Ok(Some(_)));
					if !known {
						return false;
					}
				},
				Ok(_) => {},
				Err(_) => return false,
			}
		}
	}

	const FORBIDDEN: &[&str] = &[
		"script", "iframe", "object", "embed", "onerror", "onclick", "onload", "javascript:",
		"alert(", "foreignobject", "<base", "http-equiv",
	];

	fn assert_clean(output: &str) {
		assert!(well_formed(output), "not well formed: {output}");
		let lower = output.to_ascii_lowercase();
		for word in FORBIDDEN {
			assert!(!lower.contains(word), "{word} is in {output}");
		}
	}

	#[test]
	fn a_doctype_that_hides_the_page_from_an_xml_reader_hides_nothing() {
		let hidden = "<?xml version=\"1.0\"?><!DOCTYPE html SYSTEM \"<\"><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>t</title></head><body><script>alert(1)</script><p>hello</p></body></html><!-- > -->";
		let output = page(hidden);
		assert!(output.contains("hello"));
		assert_clean(&output);
		// the variant that ends the hidden part inside the body, with a prefixed script
		let prefixed = "<!DOCTYPE html SYSTEM \"<\"><html xmlns=\"http://www.w3.org/1999/xhtml\"><body><x:script xmlns:x=\"http://www.w3.org/1999/xhtml\">alert(1)</x:script><p>hello</p><!-- > </body></html>";
		// read as HTML, `x:script` is no script, and what it holds is text; the page that is written
		// has no element of that name, whatever it is called
		let output = page(prefixed);
		assert!(well_formed(&output), "{output}");
		let lower = output.to_ascii_lowercase();
		assert!(!lower.contains("<script") && !lower.contains("script>") && !lower.contains(":script"), "{output}");
		assert!(output.contains("hello"));
	}

	#[test]
	fn what_a_browser_runs_in_any_spelling_is_not_written() {
		let hostile = [
			"<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><img src=\"a.png\" onerror=\"alert(1)\"/></body></html>",
			"<html><body><IMG SRC=a.png ONERROR=alert(1)><SCRIPT>alert(1)</SCRIPT></body></html>",
			"<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><a href=\"javascript:alert(1)\">x</a><a href=\" jav&#x09;ascript:alert(1)\">y</a><a href=\"&#106;avascript:alert(1)\">z</a></body></html>",
			"<html><body><noscript><p title=\"</noscript><img src=x onerror=alert(1)>\"></noscript></body></html>",
			"<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><iframe src=\"a.html\"></iframe><object data=\"a.swf\"></object><embed src=\"a\"/><form action=\"x\"><input name=\"a\"/></form></body></html>",
			"<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><meta http-equiv=\"refresh\" content=\"0;url=http://x\"/><base href=\"http://x/\"/></head><body><p>x</p></body></html>",
			"<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script><foreignObject><p>x</p></foreignObject><a href=\"javascript:alert(1)\"><rect/></a><set attributeName=\"href\" to=\"javascript:alert(1)\"/></svg></body></html>",
			"<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p onclick=alert(1) style=\"background:url(javascript:alert(1))\">x</p></body></html>",
			"<html><body><math><maction actiontype=\"statusline\">x<mtext>s</mtext></maction><mi xlink:href=\"javascript:alert(1)\">a</mi></math></body></html>",
			"<html><body><a href=\"data:text/html,<script>alert(1)</script>\">x</a><img src=\"data:text/html,x\"></body></html>",
		];
		for source in hostile {
			assert_clean(&page(source));
		}
	}

	#[test]
	fn an_element_of_another_name_space_or_case_is_no_way_round() {
		let output = page("<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:h=\"http://www.w3.org/1999/xhtml\"><body><h:script>alert(1)</h:script><h:p h:onclick=\"alert(1)\">kept</h:p><s:script xmlns:s=\"http://www.w3.org/2000/svg\">alert(1)</s:script></body></html>");
		assert!(output.contains("kept"));
		assert_clean(&output);
		// in an XML page `SCRIPT` is not a script, and it is not written as an element either
		let upper = page("<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><SCRIPT>text</SCRIPT></body></html>");
		assert!(!upper.to_ascii_lowercase().contains("<script"));
		assert!(well_formed(&upper));
	}

	#[test]
	fn a_page_is_written_as_well_formed_xhtml_whatever_it_was() {
		for source in [
			"<p>a<b>b<i>c</b>d</i>e",
			"<html><body><table><tr><td>x<td>y</table><br><img src=a.png>",
			"<html><body>a &nbsp; b &mdash; c &unknown; &amp; \u{0}\u{1}d</body></html>",
			"<<<>>>&&&",
		] {
			let output = page(source);
			assert!(well_formed(&output), "{source} gave {output}");
		}
	}

	#[test]
	fn addresses_that_leave_the_book_are_dropped() {
		let output = page("<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><img src=\"../../../../api/v2/x\"/><img src=\"/api/v2/x\"/><img src=\"//evil.example/x.png\"/><img src=\"http://evil.example/x.png\"/><img src=\"%2e%2e/%2e%2e/%2e%2e/x\"/><img src=\"..\\..\\x\"/><img src=\"../images/ok.png\"/><a href=\"ch2.xhtml#s1\">ok</a><a href=\"https://example.com/\">out</a><a href=\"/logout\">x</a></body></html>");
		assert!(output.contains("../images/ok.png"));
		assert!(output.contains("ch2.xhtml#s1"));
		assert!(output.contains("https://example.com/"));
		for gone in ["api/v2", "evil.example", "logout", "..\\"] {
			assert!(!output.contains(gone), "{gone} is in {output}");
		}
	}

	#[test]
	fn the_html_comment_tokens_of_css_do_not_glue_what_is_around_them_into_a_url_or_an_import() {
		// `<!--` and `-->` are tokens of their own to a CSS parser; writing nothing for them joined
		// `url` to the `(` after it (a url() that leaves the book) and `@` to `import`
		let (css, remote) = clean_stylesheet(
			"p { background: url<!--(http://evil.example/p.png) }\n\
			 em { color: blue }\n\
			 @<!--import \"http://evil.example/x.css\";\n\
			 u { color: green }\n\
			 a { background: url-->(http://evil.example/q.png) }\n\
			 b { background: url <!-- (http://evil.example/r.png) }\n\
			 i { color: red --> }",
			"OEBPS",
		);
		assert!(!css.contains("evil.example"), "{css}");
		assert!(!css.to_ascii_lowercase().contains("@import"), "{css}");
		assert!(!css.contains("url(") && !css.contains("url ("), "{css}");
		// the rules without one are kept as they are
		assert!(css.contains("color:blue") || css.contains("color: blue"), "{css}");
		assert!(css.contains("color:green") || css.contains("color: green"), "{css}");
		assert_eq!(remote, 0);
		// the same in a style attribute and in a <style> element of a page
		let output = page(
			"<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><style>p { background: url<!--(http://evil.example/s.png) }</style></head>\
			 <body><p style=\"background:url<!--(http://evil.example/t.png)\">hello</p></body></html>",
		);
		assert!(output.contains("hello"));
		assert!(!output.contains("evil.example") && !output.contains("url("), "{output}");
	}

	#[test]
	fn a_style_sheet_wrapped_in_the_old_html_comment_still_reads() {
		// the old way of hiding a style sheet from browsers that don't know <style>: the two
		// tokens sit at the top of the sheet, where a CSS parser drops them
		let (css, _) = clean_stylesheet("<!--\np { color: red }\n-->\n<!-- q { color: blue } -->", "OEBPS");
		assert!(css.contains("color:red") || css.contains("color: red"), "{css}");
		assert!(css.contains("color:blue") || css.contains("color: blue"), "{css}");
		assert!(!css.contains("<!--") && !css.contains("-->"), "{css}");
	}

	#[test]
	fn a_data_image_is_kept_and_another_data_address_is_not() {
		let png = "data:image/png;base64,iVBORw0KGgo=";
		let output = page(&format!("<html><body><img src=\"{png}\"/><img src=\"data:image/svg+xml,<svg/>\"/></body></html>"));
		assert!(output.contains(png));
		assert!(!output.contains("svg+xml"));
	}

	#[test]
	fn style_sheets_are_written_out_token_by_token() {
		let (css, remote) = clean_stylesheet(
			"@import url(http://evil.example/a.css);\n/* c */ p { color: red; background: url(http://evil.example/x.png) }\n\
			 q { background: url(\"../img/a.png\"); behavior: url(x.htc) }\n\
			 h1 { width: expression(alert(1)) }\n\
			 .a { b\\61 ckground: \\75rl(http://evil.example/y.png) }\n\
			 @font-face { font-family: F; src: url(fonts/f.woff2) format(\"woff2\") }\n\
			 em { font-style: italic }\n\
			 @media print { i { color: blue } u { background: image-set(\"http://evil.example/z.png\" 1x) } }\n\
			 a { background-image: url(/api/v2/x) }",
			"OEBPS",
		);
		assert!(css.contains("font-style:italic") || css.contains("font-style: italic"), "{css}");
		assert!(css.contains("fonts/f.woff2"), "{css}");
		for gone in ["evil.example", "expression", "behavior", "image-set", "/api/", "@import", "q {", "p {"] {
			assert!(!css.contains(gone), "{gone} is in {css}");
		}
		assert!(remote >= 3, "{remote} remote references");
	}

	#[test]
	fn a_style_attribute_with_anything_that_may_not_be_is_dropped_whole() {
		let output = page("<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p style=\"color:red\">a</p><p style=\"color:red;background:url(http://evil.example/x)\">b</p><p style=\"width:expression(alert(1))\">c</p></body></html>");
		assert!(output.contains("style=\"color:red\""));
		assert!(!output.contains("evil.example") && !output.contains("expression"));
	}

	#[test]
	fn an_svg_cover_and_a_formula_are_kept_and_nothing_that_runs_in_them_is() {
		let output = page("<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><body><svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" viewBox=\"0 0 10 20\" width=\"100%\"><image width=\"10\" height=\"20\" xlink:href=\"../images/cover.jpg\"/><path d=\"M0 0L1 1\" fill=\"url(#g)\" stroke=\"url(http://evil.example/p)\"/></svg><math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mfrac><mi>a</mi><mn>2</mn></mfrac></math></body></html>");
		assert!(output.contains("viewBox=\"0 0 10 20\""));
		assert!(output.contains("../images/cover.jpg"));
		assert!(output.contains("<mfrac>") && output.contains("<mn>2</mn>"), "{output}");
		assert!(output.contains("fill=\"url(#g)\""));
		assert!(!output.contains("evil.example"));
		assert_clean(&output);
	}

	#[test]
	fn an_svg_image_with_both_href_spellings_is_written_with_one() {
		// `href` and `xlink:href` on one element were both written as `xlink:href`: a repeated
		// attribute is not well formed XML, and the reader draws nothing for such a page
		let source = "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" viewBox=\"0 0 10 20\">\
			<image width=\"10\" height=\"20\" href=\"../images/a.jpg\" xlink:href=\"../images/b.jpg\"/>\
			<image width=\"10\" height=\"20\" xlink:href=\"../images/c.jpg\" href=\"../images/d.jpg\"/>\
			<image width=\"10\" height=\"20\" href=\"http://evil.example/e.jpg\" xlink:href=\"../images/f.jpg\"/>\
			<use href=\"#g\" xlink:href=\"#h\"/></svg></body></html>";
		let output = page(source);
		assert!(well_formed(&output), "not well formed: {output}");
		assert_eq!(output.matches("xlink:href=").count(), 4, "{output}");
		assert!(output.contains("../images/a.jpg") && !output.contains("../images/b.jpg"), "{output}");
		assert!(output.contains("../images/c.jpg") && !output.contains("../images/d.jpg"), "{output}");
		// the first one that is valid: the address that leaves the book is dropped, the next one stays
		assert!(output.contains("../images/f.jpg") && !output.contains("evil.example"), "{output}");
		assert!(output.contains("#g") && !output.contains("#h"), "{output}");
	}

	#[test]
	fn a_book_that_reads_like_the_ones_people_make_comes_out_as_it_went_in() {
		let source = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd\">\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"es\" lang=\"es\"><head><title>Capítulo 1</title><link rel=\"stylesheet\" type=\"text/css\" href=\"../styles/main.css\"/><style>p.x{margin:0}</style></head><body class=\"c1\"><h1 id=\"t\">Título</h1><p class=\"x\">Uno&nbsp;dos<a id=\"p1\"/> tres <a epub:type=\"noteref\" href=\"#n1\">1</a></p><aside epub:type=\"footnote\" id=\"n1\"><p>Nota</p></aside><table><tr><th colspan=\"2\">H</th></tr><tr><td>a</td><td>b</td></tr></table><ruby>漢<rt>kan</rt></ruby><pre>  keep\n  this</pre></body></html>";
		let output = page(source);
		assert!(output.contains("<title>Capítulo 1</title>"));
		assert!(output.contains("href=\"../styles/main.css\""));
		assert!(output.contains("p.x"));
		assert!(output.contains("<h1 id=\"t\">Título</h1>"));
		// an empty anchor stays an empty anchor, and does not swallow what follows
		assert!(output.contains("<a id=\"p1\"></a> tres"), "{output}");
		assert!(output.contains("epub:type=\"noteref\"") && output.contains("href=\"#n1\""));
		assert!(output.contains("<aside epub:type=\"footnote\" id=\"n1\">") || output.contains("<aside id=\"n1\" epub:type=\"footnote\">"));
		assert!(output.contains("colspan=\"2\""));
		assert!(output.contains("<ruby>漢<rt>kan</rt></ruby>"));
		assert!(output.contains("<pre>  keep\n  this</pre>"));
		assert!(output.contains("Uno\u{a0}dos"));
		assert!(output.contains("xml:lang=\"es\""));
		assert!(well_formed(&output));
	}

	#[test]
	fn a_page_that_is_too_deep_or_too_big_is_refused() {
		let deep = format!("<html xmlns=\"http://www.w3.org/1999/xhtml\"><body>{}x{}</body></html>", "<div>".repeat(5_000), "</div>".repeat(5_000));
		assert_eq!(clean_page(deep.as_bytes(), "", set()).unwrap_err(), Refusal::TooDeep);
		let deep_html = format!("<body>{}x", "<div>".repeat(5_000));
		assert_eq!(clean_page(deep_html.as_bytes(), "", set()).unwrap_err(), Refusal::TooDeep);
		let tight = Settings { max_input: 1024, max_depth: 64 };
		let big = vec![b'a'; 2048];
		assert_eq!(clean_page(&big, "", tight).unwrap_err(), Refusal::TooBig);
		assert_eq!(clean_css(&big, "", tight).unwrap_err(), Refusal::TooBig);
	}

	#[test]
	fn what_is_not_a_page_is_not_one() {
		assert_eq!(clean_svg(b"<html xmlns=\"http://www.w3.org/1999/xhtml\"/>", "", set()).unwrap_err(), Refusal::NotADocument);
		let svg = clean_svg(b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"><script>alert(1)</script><rect width=\"1\" height=\"1\" onclick=\"x\"/></svg>", "", set()).expect("svg");
		let text = String::from_utf8(svg.bytes).unwrap();
		assert!(text.contains("<rect") && !text.contains("script") && !text.contains("onclick"));
		assert_eq!(svg.findings.get("script"), Some(&1));
		assert_eq!(svg.findings.get("onclick"), Some(&1));
	}

	#[test]
	fn what_was_taken_out_is_counted() {
		let result = clean_page(b"<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><script>1</script><script>2</script><p onclick=\"x\">a</p><a href=\"javascript:1\">b</a><img src=\"http://e.example/x.png\"/></body></html>", "", set()).unwrap();
		assert_eq!(result.findings.get("script"), Some(&2));
		assert_eq!(result.findings.get("onclick"), Some(&1));
		assert_eq!(result.findings.get("script-url"), Some(&1));
		assert_eq!(result.remote_refs, 1);
	}

	#[test]
	fn text_in_utf_16_is_read() {
		let text = "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>héllo</p></body></html>";
		let mut bytes = vec![0xFF, 0xFE];
		for unit in text.encode_utf16() {
			bytes.extend_from_slice(&unit.to_le_bytes());
		}
		let output = String::from_utf8(clean_page(&bytes, "", set()).unwrap().bytes).unwrap();
		assert!(output.contains("héllo"));
	}

	#[test]
	fn an_attribute_name_with_a_multibyte_character_does_not_abort_the_checker() {
		// valid in XML and in HTML; slicing such a name at byte 2 split a character and panicked
		// (the checking process aborts on a panic, and the book was refused as a crash)
		for name in ["a\u{e9}", "\u{30a2}x", "\u{e9}\u{e9}k", "o\u{e9}", "on\u{e9}", "\u{e9}n"] {
			let source = format!(
				"<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p {name}=\"v\" onclick=\"alert(1)\">hello</p>\
				 <svg xmlns=\"http://www.w3.org/2000/svg\"><rect {name}=\"v\" onload=\"alert(1)\" width=\"1\" height=\"1\"/></svg></body></html>"
			);
			let output = page(&source);
			assert!(output.contains("hello"), "{name}: {output}");
			assert_clean(&output);
		}
		assert!(is_handler("onclick") && is_handler("ONLOAD") && !is_handler("on") && !is_handler("o\u{e9}"));
	}
}
