//! Active content in the documents of an EPUB: scripts, frames, event handlers and the like.
//!
//! The reader draws every page of an EPUB as a document of the app's own origin that is allowed to
//! run scripts (it needs the book's own, inline or not, for the few books that carry one). A script
//! in a book somebody sent therefore runs as whoever reads it, the owner included, and can call the
//! server with their session. So the books of people who need approval are filed without any: what
//! is found is taken out, and **only** that: every other byte of the document stays as it was, so a
//! book is never made worse than it has to be.
//!
//! A document is read as XML (that is how the reader parses an EPUB's pages): by local name, so
//! `<h:script>` is a script too, whatever the prefix and the case. What it removes:
//!
//! - the elements `script`, `iframe`, `frame`, `frameset`, `object`, `embed`, `applet`, `portal`
//!   and `base`, with what is inside them; a `meta` that refreshes the page or sets a cookie;
//!   an SVG animation that sets a link (`attributeName="href"`);
//! - the attributes `on*`, `srcdoc`, and any whose value is a `javascript:` or `vbscript:` address
//!   (read the way a browser reads one: entities decoded, tabs and line breaks ignored), and links
//!   to a `data:` document (HTML, XHTML, XML, SVG) except on an image;
//! - the `<?xml-stylesheet?>` instruction.
//!
//! A DOCTYPE with an internal subset is refused (`<!ENTITY` is how a small text becomes a huge one,
//! and `<!ATTLIST` is how an attribute such as `onerror` is given to every `<img>` without being
//! written in the page), and a document that is not well formed is not shown by the reader at all,
//! so it is left as it is, unless its bytes look like they carry anything of the above (then the
//! book is refused, since what the reader would do with it can't be told).
//!
//! What was taken out is checked: the result must be well formed and must have nothing left to take
//! out.

use std::collections::BTreeMap;

use quick_xml::{
	events::{BytesStart, Event},
	Reader,
};

/// What was found of a kind, and in how many places
pub type Findings = BTreeMap<String, u64>;

/// What a document turned out to be
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
	/// Nothing to take out
	Clean { remote_refs: u64 },
	/// It carries some, and this is the document without it
	Found {
		stripped: Vec<u8>,
		findings: Findings,
		remote_refs: u64,
	},
	/// It can't be cleaned, and shouldn't be filed
	Unstrippable(String),
	/// It is not well formed: the reader doesn't show it. `remote_refs` is not counted.
	Inert,
}

const REMOVED_ELEMENTS: &[&str] = &[
	"script", "iframe", "frame", "frameset", "object", "embed", "applet", "portal",
	"base",
];
const URL_ATTRIBUTES: &[&str] = &["href", "src", "data", "action", "formaction"];
const DOCUMENT_SCHEMES: &[&str] = &[
	"data:text/html",
	"data:application/xhtml+xml",
	"data:text/xml",
	"data:application/xml",
	"data:image/svg+xml",
];
const DOCUMENT_EXTENSIONS: &[&str] =
	&["xhtml", "xht", "html", "htm", "svg", "xsl", "xslt", "mml"];
const DOCUMENT_TYPES: &[&str] = &[
	"application/xhtml+xml",
	"text/html",
	"image/svg+xml",
	"application/xml",
	"text/xml",
];

/// A media type as Readium reads it (`MediaType.parse`): every white space inside it is gone, the
/// type and the subtype are lower case, and a parameter that is not `name=value` is dropped. `None`
/// when it isn't `type/subtype` at all (the reader then can't show the page)
pub fn readium_media_type(raw: &str) -> Option<(String, String, Vec<(String, String)>)> {
	// what a script's `\s` is, and the other white space Rust knows
	let cleaned: String = raw
		.chars()
		.filter(|c| !(c.is_whitespace() || *c == '\u{feff}'))
		.collect();
	let mut parts = cleaned.split(';');
	let head = parts.next().unwrap_or_default();
	let (kind, subtype) = head.split_once('/')?;
	if kind.is_empty() || subtype.is_empty() || subtype.contains('/') {
		return None;
	}
	let mut parameters: Vec<(String, String)> = parts
		.filter_map(|part| {
			let (name, value) = part.split_once('=')?;
			(!value.contains('=')).then(|| (name.to_lowercase(), value.to_string()))
		})
		.collect();
	parameters.sort();
	Some((kind.to_lowercase(), subtype.to_lowercase(), parameters))
}

/// Whether the reader would parse a page with this declared type as HTML, which has more ways to
/// hide an attribute or an address than XML has (`text/html`, whatever follows a `;`, and the
/// wildcards, which it reads as containing it)
pub fn is_declared_html(media_type: Option<&str>) -> bool {
	media_type
		.and_then(readium_media_type)
		.is_some_and(|(kind, subtype, _)| {
			matches!(kind.as_str(), "text" | "*") && matches!(subtype.as_str(), "html" | "*")
		})
}

/// Whether a declared type is one of a document a browser could render
fn is_document_type(media_type: &str) -> bool {
	readium_media_type(media_type).is_some_and(|(kind, subtype, _)| {
		let full = format!("{kind}/{subtype}");
		DOCUMENT_TYPES.contains(&full.as_str()) || is_declared_html(Some(media_type))
	})
}

/// Whether an entry of an EPUB is a document a browser could render, by its name or by any of the
/// types the package declares for it
pub fn is_document(name: &str, media_types: &[String]) -> bool {
	let extension = name
		.rsplit_once('.')
		.map(|(_, ext)| ext.to_ascii_lowercase())
		.unwrap_or_default();
	DOCUMENT_EXTENSIONS.contains(&extension.as_str())
		|| media_types.iter().any(|t| is_document_type(t))
}

/// Whether a page is to be held to what its bytes look like, because the reader could take it as
/// HTML: any of the types the package declares for it says so, or it declares none and is named
/// like HTML
pub fn is_html_page(name: &str, media_types: &[String]) -> bool {
	if media_types.iter().any(|t| is_declared_html(Some(t))) {
		return true;
	}
	let lower = name.to_ascii_lowercase();
	media_types.iter().all(|t| readium_media_type(t).is_none())
		&& (lower.ends_with(".html") || lower.ends_with(".htm"))
}

/// Whether the first thing in a text, after a byte order mark and white space, is `<`. `None`:
/// there was nothing but white space in it
pub fn starts_with_markup(head: &[u8]) -> Option<bool> {
	let head = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
	head.iter()
		.find(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C))
		.map(|b| *b == b'<')
}

/// An address as a browser reads it: its entities decoded (by the caller), the tabs and line breaks
/// that it ignores gone, the leading control characters and spaces gone, in lower case
fn normalised_url(value: &str) -> String {
	let mut url: String = value
		.chars()
		.filter(|c| !matches!(c, '\t' | '\n' | '\r'))
		.collect();
	let start = url
		.char_indices()
		.find(|(_, c)| *c > ' ')
		.map_or(url.len(), |(i, _)| i);
	url.drain(..start);
	url.make_ascii_lowercase();
	url
}

fn lowercase_local(name: &[u8]) -> String {
	let local = name.rsplit(|b| *b == b':').next().unwrap_or(name);
	String::from_utf8_lossy(local).to_ascii_lowercase()
}

/// What to do with one element
enum Element {
	Keep,
	Remove(String),
}

struct Attribute {
	key: Vec<u8>,
	local: String,
	value: Vec<u8>,
	text: String,
	/// Its value has an entity XML doesn't know (`&colon;`, `&Tab;`): a page with an XHTML DOCTYPE
	/// has them read by a browser as HTML's, so what the value says can't be told from here
	undecodable: bool,
}

fn attributes_of(tag: &BytesStart) -> Option<Vec<Attribute>> {
	let mut out = vec![];
	for attribute in tag.attributes() {
		let attribute = attribute.ok()?;
		let (text, undecodable) = match attribute.unescape_value() {
			Ok(value) => (value.into_owned(), false),
			Err(_) => (String::from_utf8_lossy(&attribute.value).into_owned(), true),
		};
		out.push(Attribute {
			key: attribute.key.as_ref().to_vec(),
			local: lowercase_local(attribute.key.as_ref()),
			value: attribute.value.to_vec(),
			text,
			undecodable,
		});
	}
	Some(out)
}

fn classify(local: &str, attributes: &[Attribute]) -> Element {
	if REMOVED_ELEMENTS.contains(&local) {
		return Element::Remove(local.to_string());
	}
	let value_of = |key: &str| {
		attributes
			.iter()
			.find(|a| a.local == key)
			.map(|a| a.text.trim().to_ascii_lowercase())
	};
	if local == "meta" {
		if let Some(equiv) = value_of("http-equiv") {
			if equiv == "refresh" || equiv == "set-cookie" {
				return Element::Remove("meta-refresh".to_string());
			}
		}
	}
	if matches!(
		local,
		"animate" | "set" | "animatemotion" | "animatetransform"
	) {
		if let Some(name) = value_of("attributename") {
			// by the part after the prefix, whatever the prefix is (`x:href` is `href` too), and
			// an event handler can't be set by an animation either
			let local = name.rsplit(':').next().unwrap_or(&name);
			if local == "href"
				|| (local.len() > 2
					&& local.starts_with("on")
					&& local.bytes().all(|b| b.is_ascii_alphabetic()))
			{
				return Element::Remove("svg-animation".to_string());
			}
		}
	}
	Element::Keep
}

/// Which of an element's attributes go, and why
fn dropped(local: &str, attribute: &Attribute) -> Option<&'static str> {
	let key = attribute.local.as_str();
	if key.len() > 2
		&& key.starts_with("on")
		&& key.bytes().all(|b| b.is_ascii_alphabetic())
	{
		return Some("event-handler");
	}
	if key == "srcdoc" {
		return Some("srcdoc");
	}
	let url = normalised_url(&attribute.text);
	// anywhere in the value: an animation's `values="a;javascript:..."` is a list of addresses
	if url.contains("javascript:") || url.contains("vbscript:") {
		return Some("script-address");
	}
	// an address with an entity that can't be read from here: a browser may read it as one of HTML's
	// (`javascript&colon;...`), so it is not kept
	if attribute.undecodable && carries_address(key) {
		return Some("undecodable-address");
	}
	if URL_ATTRIBUTES.contains(&key)
		&& !matches!(local, "img" | "image")
		&& DOCUMENT_SCHEMES
			.iter()
			.any(|scheme| url.starts_with(scheme))
	{
		return Some("document-address");
	}
	None
}

/// The attributes whose value is an address (or a list of them) that a browser may follow or set
fn carries_address(key: &str) -> bool {
	URL_ATTRIBUTES.contains(&key)
		|| matches!(
			key,
			"poster" | "background" | "from" | "to" | "values" | "by" | "cite" | "longdesc" | "ping"
		)
}

fn is_remote(local: &str, attribute: &Attribute) -> bool {
	let embeds = matches!(attribute.local.as_str(), "src" | "poster" | "data")
		|| (attribute.local == "href" && matches!(local, "link" | "image" | "use"));
	if !embeds {
		return false;
	}
	let url = normalised_url(&attribute.text);
	url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//")
}

/// A start tag without the attributes that go, written from what it had
fn rebuilt(tag: &BytesStart, kept: &[&Attribute], empty: bool) -> Vec<u8> {
	let mut out = Vec::with_capacity(64);
	out.push(b'<');
	out.extend_from_slice(tag.name().as_ref());
	for attribute in kept {
		out.push(b' ');
		out.extend_from_slice(&attribute.key);
		out.push(b'=');
		// a raw value has the quote it was written with in it never, and the other one at most
		let quote = if attribute.value.contains(&b'"') {
			b'\''
		} else {
			b'"'
		};
		out.push(quote);
		out.extend_from_slice(&attribute.value);
		out.push(quote);
	}
	if empty {
		out.push(b'/');
	}
	out.push(b'>');
	out
}

fn add(findings: &mut Findings, kind: &str) {
	*findings.entry(kind.to_string()).or_default() += 1;
}

/// Reads a document and takes out what a reader must not run. `declared_html`: the package calls
/// it `text/html`, which a browser reads with HTML's rules
pub fn scan_document(bytes: &[u8], declared_html: bool) -> Outcome {
	// a document in UTF-16 or UTF-32 can't be read the way the rest are: an EPUB's pages are UTF-8
	if bytes.starts_with(&[0xFF, 0xFE])
		|| bytes.starts_with(&[0xFE, 0xFF])
		|| bytes.iter().take(4).any(|b| *b == 0)
	{
		return Outcome::Unstrippable("the document is not written in UTF-8".to_string());
	}
	let outcome = scan_once(bytes);
	let outcome = match outcome {
		Outcome::Found {
			stripped,
			findings,
			remote_refs,
		} => {
			// what is left must have nothing to take out
			match scan_once(&stripped) {
				Outcome::Clean { .. } => Outcome::Found {
					stripped,
					findings,
					remote_refs,
				},
				_ => Outcome::Unstrippable(
					"what is left of the document still carries active content"
						.to_string(),
				),
			}
		},
		other => other,
	};
	if declared_html {
		// HTML has more ways to write an attribute or an address than XML (`&colon;`, `&Tab;`,
		// case): a book that says its pages are HTML is held to what its bytes look like
		let left: &[u8] = match &outcome {
			Outcome::Found { stripped, .. } => stripped,
			_ => bytes,
		};
		if looks_active(left) {
			return Outcome::Unstrippable(
				"a page declared as HTML carries something that looks like active content"
					.to_string(),
			);
		}
	}
	if let Outcome::Inert = outcome {
		if looks_active(bytes) {
			return Outcome::Unstrippable(
				"a page that is not well formed carries something that looks like active content"
					.to_string(),
			);
		}
	}
	outcome
}

/// What a read of one document has made so far
struct Stripper<'a> {
	bytes: &'a [u8],
	out: Vec<u8>,
	copied_to: usize,
	skipping: usize,
	findings: Findings,
	remote_refs: u64,
	changed: bool,
}

impl Stripper<'_> {
	/// Drops `start..end` of the input (what was kept before it goes out first)
	fn drop_span(&mut self, start: usize, end: usize) {
		self.out
			.extend_from_slice(&self.bytes[self.copied_to..start]);
		self.copied_to = end;
		self.changed = true;
	}

	/// One start tag (or empty one). `None`: it can't be read, so the document is not well formed
	fn tag(
		&mut self,
		tag: &BytesStart,
		empty: bool,
		start: usize,
		end: usize,
	) -> Option<()> {
		let attributes = attributes_of(tag)?;
		let local = lowercase_local(tag.name().as_ref());
		match classify(&local, &attributes) {
			Element::Remove(kind) => {
				self.drop_span(start, end);
				add(&mut self.findings, &kind);
				if !empty {
					self.skipping = 1;
				}
			},
			Element::Keep => {
				let mut kept: Vec<&Attribute> = vec![];
				let mut any_dropped = false;
				for attribute in &attributes {
					if let Some(kind) = dropped(&local, attribute) {
						add(&mut self.findings, kind);
						any_dropped = true;
					} else {
						if is_remote(&local, attribute) {
							self.remote_refs += 1;
						}
						kept.push(attribute);
					}
				}
				if any_dropped {
					self.out
						.extend_from_slice(&self.bytes[self.copied_to..start]);
					self.out.extend_from_slice(&rebuilt(tag, &kept, empty));
					self.copied_to = end;
					self.changed = true;
				}
			},
		}
		Some(())
	}
}

fn scan_once(bytes: &[u8]) -> Outcome {
	let mut reader = Reader::from_reader(bytes);
	let mut stripper = Stripper {
		bytes,
		out: Vec::new(),
		copied_to: 0,
		skipping: 0,
		findings: Findings::new(),
		remote_refs: 0,
		changed: false,
	};
	loop {
		let start = reader.buffer_position() as usize;
		let event = reader.read_event();
		let end = reader.buffer_position() as usize;
		match event {
			Err(_) => return Outcome::Inert,
			Ok(Event::Eof) => break,
			// inside an element that goes: its start tags only deepen it, and what ends it
			// ends it
			Ok(Event::Start(_)) if stripper.skipping > 0 => stripper.skipping += 1,
			Ok(Event::Empty(_)) if stripper.skipping > 0 => {},
			Ok(Event::End(_)) if stripper.skipping > 0 => {
				stripper.skipping -= 1;
				if stripper.skipping == 0 {
					stripper.copied_to = end;
				}
			},
			Ok(Event::DocType(text)) => {
				// a subset can declare an entity (a small text that becomes a huge one) or give
				// every element of a kind an attribute (`<!ATTLIST img onerror CDATA "...">`)
				if text.contains(&b'[') || contains_ignore_case(&text, b"<!entity") {
					return Outcome::Unstrippable(
						"the document has a DTD subset (it can declare entities or give elements attributes)"
							.to_string(),
					);
				}
			},
			// not inside an element that is being removed: dropping a span there would copy what
			// was removed before it back into the output
			Ok(Event::PI(pi)) if stripper.skipping == 0 => {
				if pi.target().eq_ignore_ascii_case(b"xml-stylesheet") {
					stripper.drop_span(start, end);
					add(&mut stripper.findings, "xml-stylesheet");
				}
			},
			Ok(Event::Start(tag)) => {
				if stripper.tag(&tag, false, start, end).is_none() {
					return Outcome::Inert;
				}
			},
			Ok(Event::Empty(tag)) => {
				if stripper.tag(&tag, true, start, end).is_none() {
					return Outcome::Inert;
				}
			},
			Ok(_) => {},
		}
	}
	if stripper.skipping > 0 {
		// a removed element that never ends: not well formed
		return Outcome::Inert;
	}
	if !stripper.changed {
		return Outcome::Clean {
			remote_refs: stripper.remote_refs,
		};
	}
	let Stripper {
		mut out,
		copied_to,
		findings,
		remote_refs,
		..
	} = stripper;
	out.extend_from_slice(&bytes[copied_to..]);
	Outcome::Found {
		stripped: out,
		findings,
		remote_refs,
	}
}

fn contains_ignore_case(haystack: &[u8], needle: &[u8]) -> bool {
	needle.len() <= haystack.len()
		&& haystack
			.windows(needle.len())
			.any(|window| window.eq_ignore_ascii_case(needle))
}

/// A character reference at `at` (an `&`): the character it stands for (`?` for one past ASCII)
/// and where it ends. Numeric ones count with or without their `;`, as a browser reads them; of
/// the named ones only the three that can hide the characters of an address are known
fn reference_at(bytes: &[u8], at: usize) -> Option<(char, usize)> {
	let rest = bytes.get(at + 1..)?;
	if rest.first() == Some(&b'#') {
		let (radix, digits_at) = match rest.get(1) {
			Some(b'x') => (16, 2),
			_ => (10, 1),
		};
		let digits: Vec<u8> = rest[digits_at.min(rest.len())..]
			.iter()
			.take(8)
			.take_while(|b| (**b as char).is_digit(radix))
			.copied()
			.collect();
		if digits.is_empty() {
			return None;
		}
		let value = u32::from_str_radix(std::str::from_utf8(&digits).ok()?, radix).ok()?;
		let mut end = at + 1 + digits_at + digits.len();
		if bytes.get(end) == Some(&b';') {
			end += 1;
		}
		let ch = char::from_u32(value).filter(char::is_ascii).unwrap_or('?');
		return Some((ch, end));
	}
	let name_len = rest
		.iter()
		.take(12)
		.take_while(|b| b.is_ascii_alphanumeric())
		.count();
	if rest.get(name_len) != Some(&b';') {
		return None;
	}
	let ch = match &rest[..name_len] {
		b"tab" => '\t',
		b"newline" => '\n',
		b"colon" => ':',
		_ => return None,
	};
	Some((ch, at + 1 + name_len + 1))
}

/// The bytes of a document as an address in it would be read: lower case, the character references
/// that stand for an ASCII character decoded, the tabs and line breaks gone
fn flattened(lower: &[u8]) -> Vec<u8> {
	let mut out = Vec::with_capacity(lower.len());
	let mut i = 0usize;
	while i < lower.len() {
		match lower[i] {
			b'\t' | b'\n' | b'\r' => i += 1,
			b'&' => match reference_at(lower, i) {
				Some(('\t' | '\n' | '\r', next)) => i = next,
				Some((ch, next)) => {
					out.push(ch as u8);
					i = next;
				},
				None => {
					out.push(b'&');
					i += 1;
				},
			},
			byte => {
				out.push(byte);
				i += 1;
			},
		}
	}
	out
}

/// White space as HTML's tokenizer reads it between the parts of a tag (a form feed too)
fn is_space(byte: u8) -> bool {
	matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

/// The text of each tag of a document, from its `<` to the `>` that closes it, the way an HTML
/// parser reads them: a tag starts at `<` and a letter, `/`, `!` or `?`, and a quote opens a value
/// only right after `=` (a `>` inside a value doesn't close the tag)
fn tags(bytes: &[u8]) -> Vec<&[u8]> {
	let mut out = vec![];
	let mut i = 0usize;
	while i < bytes.len() {
		let starts = bytes[i] == b'<'
			&& bytes
				.get(i + 1)
				.is_some_and(|b| b.is_ascii_alphabetic() || matches!(b, b'/' | b'!' | b'?'));
		if !starts {
			i += 1;
			continue;
		}
		let (mut quote, mut expect_value, mut j) = (0u8, false, i + 1);
		while j < bytes.len() {
			let b = bytes[j];
			if quote != 0 {
				if b == quote {
					quote = 0;
				}
			} else if b == b'>' {
				break;
			} else if b == b'=' {
				expect_value = true;
			} else if is_space(b) {
			} else if expect_value && (b == b'"' || b == b'\'') {
				quote = b;
				expect_value = false;
			} else {
				expect_value = false;
			}
			j += 1;
		}
		out.push(&bytes[i..j.min(bytes.len())]);
		i = j + 1;
	}
	out
}

/// Whether a tag has something in it that a browser would read differently from what is written:
/// a numeric character reference, a named one HTML knows and XML doesn't (`&colon;`, `&Tab;`), or
/// the name of a scripting address with its colon hidden
fn tag_hides_something(tag: &[u8]) -> bool {
	if find(tag, b"javascript").is_some() || find(tag, b"vbscript").is_some() {
		return true;
	}
	let mut i = 0usize;
	while i < tag.len() {
		if tag[i] == b'&' {
			if tag.get(i + 1) == Some(&b'#') {
				return true;
			}
			let name_len = tag[i + 1..]
				.iter()
				.take(32)
				.take_while(|b| b.is_ascii_alphanumeric())
				.count();
			if name_len > 0
				&& tag.get(i + 1 + name_len) == Some(&b';')
				&& !matches!(&tag[i + 1..i + 1 + name_len], b"amp" | b"lt" | b"gt" | b"quot" | b"apos")
			{
				return true;
			}
		}
		i += 1;
	}
	false
}

/// A look at the bytes of a document the reader would not parse as XML, or would parse as HTML:
/// does anything in it look like an element, an attribute or an address that runs?
///
/// It is meant to say yes too often: HTML has more ways to write all of these than XML has
/// (form feeds between attributes, references without a `;`, names with their colon hidden)
pub fn looks_active(bytes: &[u8]) -> bool {
	let lower: Vec<u8> = bytes.iter().map(u8::to_ascii_lowercase).collect();
	for needle in [
		&b"<script"[..],
		b"<iframe",
		b"<object",
		b"<embed",
		b"<frame",
		b"<applet",
		b"<portal",
		b"<base",
		b"srcdoc",
		b"<?xml-stylesheet",
		b"<!attlist",
		b"<!entity",
	] {
		if find(&lower, needle).is_some() {
			return true;
		}
	}
	// addresses that run, however the characters of their names were hidden
	let flat = flattened(&lower);
	for needle in [
		&b"javascript:"[..],
		b"vbscript:",
		b"data:text/html",
		b"data:application/xhtml+xml",
	] {
		if find(&flat, needle).is_some() {
			return true;
		}
	}
	if tags(&lower).into_iter().any(tag_hides_something) {
		return true;
	}
	// a meta that refreshes or sets a cookie: its tag, however far from `http-equiv` the value is
	let mut from = 0usize;
	while let Some(at) = find(&lower[from..], b"http-equiv") {
		let at = from + at;
		let reach = (at + 4096).min(lower.len());
		let end = lower[at..reach]
			.iter()
			.position(|b| *b == b'>')
			.map_or(reach, |p| at + p);
		let window = flattened(&lower[at..end]);
		if find(&window, b"refresh").is_some() || find(&window, b"set-cookie").is_some() {
			return true;
		}
		from = at + 10;
	}
	// an event handler: ` onload=`, `/onerror =`, `"onclick=`, a form feed before or after
	let mut i = 0usize;
	while i + 4 < lower.len() {
		if lower[i] == b'o'
			&& lower[i + 1] == b'n'
			&& i > 0 && (is_space(lower[i - 1]) || matches!(lower[i - 1], b'/' | b'"' | b'\''))
		{
			let mut j = i + 2;
			while j < lower.len() && lower[j].is_ascii_alphabetic() {
				j += 1;
			}
			if j - (i + 2) >= 2 {
				let mut k = j;
				while k < lower.len() && is_space(lower[k]) {
					k += 1;
				}
				if lower.get(k) == Some(&b'=') {
					return true;
				}
			}
			i = j.max(i + 1);
		} else {
			i += 1;
		}
	}
	false
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
	if needle.is_empty() || needle.len() > haystack.len() {
		return None;
	}
	haystack
		.windows(needle.len())
		.position(|window| window == needle)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn page(body: &str) -> Vec<u8> {
		format!(
			"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:h=\"http://www.w3.org/1999/xhtml\"><head><title>T</title></head><body>{body}</body></html>"
		)
		.into_bytes()
	}

	fn stripped(body: &str) -> (String, Findings) {
		match scan_document(&page(body), false) {
			Outcome::Found {
				stripped, findings, ..
			} => (String::from_utf8(stripped).expect("text"), findings),
			other => panic!("expected something to strip, got {other:?}"),
		}
	}

	#[test]
	fn a_clean_page_is_left_alone() {
		let doc = page("<p class='a' id=\"b\">Hello <a href=\"x.xhtml#n\">there</a></p><img src=\"c.png\"/>");
		assert_eq!(
			scan_document(&doc, false),
			Outcome::Clean { remote_refs: 0 }
		);
	}

	#[test]
	fn scripts_go_in_every_spelling_and_everything_else_stays_byte_for_byte() {
		let before = "<p>one</p>";
		let after = "<p>two <b>bold</b> &amp; more</p><!-- note -->";
		for script in [
			"<script>alert(1)</script>",
			"<script src=\"x.js\"/>",
			"<script type=\"text/javascript\">\n  var a = 1 &lt; 2; // <![CDATA[ not here ]]>\n</script>",
			"<h:script>alert(1)</h:script>",
			"<SCRIPT>alert(1)</SCRIPT>",
			"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>",
		] {
			let body = format!("{before}{script}{after}");
			let (result, findings) = stripped(&body);
			assert!(!result.to_lowercase().contains("script"), "{script}");
			assert!(result.contains(before) && result.contains(after), "{script}");
			assert!(findings.values().sum::<u64>() >= 1);
			// and the head and the framing of the document are the original bytes
			assert!(result.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<html xmlns="));
		}
	}

	#[test]
	fn frames_objects_and_refreshes_go() {
		for element in [
			"<iframe src=\"x\"></iframe>",
			"<iframe srcdoc=\"&lt;script&gt;\"/>",
			"<object data=\"x.svg\"><param name=\"a\" value=\"b\"/></object>",
			"<embed src=\"x.swf\"/>",
			"<applet code=\"x\"></applet>",
			"<frameset><frame src=\"a\"/></frameset>",
			"<base href=\"http://evil.example/\"/>",
		] {
			let (result, _) = stripped(&format!("<p>keep</p>{element}"));
			assert!(result.contains("<p>keep</p>"), "{element}");
			assert!(
				["iframe", "object", "embed", "applet", "frame", "base", "param"]
					.iter()
					.all(|tag| !result.contains(&format!("<{tag}"))),
				"{element}: {result}"
			);
		}
		let doc = "<?xml version=\"1.0\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><meta http-equiv=\"refresh\" content=\"0;url=http://x\"/><meta charset=\"utf-8\"/></head><body/></html>";
		match scan_document(doc.as_bytes(), false) {
			Outcome::Found { stripped, .. } => {
				let text = String::from_utf8(stripped).unwrap();
				assert!(!text.contains("refresh") && text.contains("charset"));
			},
			other => panic!("{other:?}"),
		}
	}

	#[test]
	fn event_handlers_and_script_addresses_go_and_the_rest_of_the_tag_stays() {
		let (result, findings) =
			stripped("<img src=\"a.png\" onerror=\"alert(1)\" alt='say \"hi\"'/><body2 onload=\"x()\"/>");
		assert!(
			result.contains("<img src=\"a.png\" alt='say \"hi\"'/>"),
			"{result}"
		);
		assert!(!result.contains("onerror") && !result.contains("onload"));
		assert_eq!(findings.get("event-handler"), Some(&2));
		for url in [
			"javascript:alert(1)",
			"  JavaScript:alert(1)",
			"java\tscript:alert(1)",
			"java&#x0A;script:alert(1)",
			"&#x6A;avascript:alert(1)",
			"\u{1}javascript:alert(1)",
			"vbscript:msgbox(1)",
		] {
			let (result, findings) = stripped(&format!("<a href=\"{url}\">x</a>"));
			assert!(result.contains("<a>x</a>"), "{url}: {result}");
			assert_eq!(findings.get("script-address"), Some(&1), "{url}");
		}
	}

	#[test]
	fn a_data_document_goes_but_an_image_stays() {
		let (result, findings) =
			stripped("<a href=\"data:text/html;base64,PHNjcmlwdD4=\">x</a><img src=\"data:image/svg+xml;utf8,&lt;svg/&gt;\"/>");
		assert!(result.contains("<a>x</a>"));
		assert!(result.contains("data:image/svg+xml"));
		assert_eq!(findings.get("document-address"), Some(&1));
	}

	#[test]
	fn an_svg_that_sets_a_link_and_a_stylesheet_instruction_go() {
		let (result, _) = stripped(
			"<svg xmlns=\"http://www.w3.org/2000/svg\"><a xmlns:xlink=\"http://www.w3.org/1999/xlink\"><animate attributeName=\"xlink:href\" to=\"javascript:alert(1)\"/><text>t</text></a></svg>",
		);
		assert!(!result.contains("animate") && result.contains("<text>t</text>"));
		let doc = "<?xml version=\"1.0\"?><?xml-stylesheet type=\"text/xsl\" href=\"x.xsl\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"/>";
		match scan_document(doc.as_bytes(), false) {
			Outcome::Found { stripped, .. } => {
				assert!(!String::from_utf8(stripped)
					.unwrap()
					.contains("xml-stylesheet"))
			},
			other => panic!("{other:?}"),
		}
	}

	#[test]
	fn declared_entities_are_refused() {
		let doc = "<?xml version=\"1.0\"?><!DOCTYPE html [ <!ENTITY a \"aaaa\"> <!ENTITY b \"&a;&a;&a;\"> ]><html xmlns=\"http://www.w3.org/1999/xhtml\"><body>&b;</body></html>";
		assert!(matches!(
			scan_document(doc.as_bytes(), false),
			Outcome::Unstrippable(_)
		));
		// an ordinary DOCTYPE is not
		let ok = "<?xml version=\"1.0\"?><!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd\"><html xmlns=\"http://www.w3.org/1999/xhtml\"><body/></html>";
		assert!(matches!(
			scan_document(ok.as_bytes(), false),
			Outcome::Clean { .. }
		));
	}

	#[test]
	fn a_page_that_is_not_well_formed_is_inert_unless_it_looks_active() {
		// the reader doesn't show it, and nothing in it looks like active content
		assert_eq!(
			scan_document(b"<html><body><p>one<p>two</body>", false),
			Outcome::Inert
		);
		// it would be inert, but it carries a script and a handler: refused
		for doc in [
			&b"<html><body><p>one<script>alert(1)</script></body>"[..],
			b"<html><body><img src=x onerror=alert(1)>",
			b"<html><head><meta http-equiv=\"refresh\" content=\"0\"></head>",
		] {
			assert!(
				matches!(scan_document(doc, false), Outcome::Unstrippable(_)),
				"{}",
				String::from_utf8_lossy(doc)
			);
		}
		// a charset meta is not a refresh, and the word `on` is not a handler
		assert_eq!(
			scan_document(
				b"<html><head><meta http-equiv=\"Content-Type\" content=\"text/html\"></head><body><p>one<p>on and on = 2</body>",
				false
			),
			Outcome::Inert
		);
	}

	#[test]
	fn a_page_declared_as_html_is_held_to_its_bytes() {
		// well formed, so the reader would parse it either way, but an address whose characters
		// are hidden by entities XML doesn't know is not kept: it is taken out
		let sneaky = page("<a href=\"java&Tab;script&colon;alert(1)\">x</a>");
		for declared_html in [true, false] {
			match scan_document(&sneaky, declared_html) {
				Outcome::Found {
					stripped, findings, ..
				} => {
					let text = String::from_utf8(stripped).unwrap();
					assert!(text.contains("<a>x</a>"), "{text}");
					assert_eq!(findings.get("undecodable-address"), Some(&1));
				},
				other => panic!("{other:?}"),
			}
		}
		let fine = page("<p>hello</p>");
		assert!(matches!(scan_document(&fine, true), Outcome::Clean { .. }));
	}

	#[test]
	fn what_a_page_declared_as_html_hides_in_cdata_is_found_by_its_bytes() {
		// XML reads this as text; HTML reads `<![CDATA[` as a comment that the first `>` ends,
		// and the `<img>` after it is an element
		let page = page("<![CDATA[><img src=x onerror=alert(1)>]]>");
		assert!(matches!(
			scan_document(&page, true),
			Outcome::Unstrippable(_)
		));
		// the same page as XML is what it says: text
		assert!(matches!(scan_document(&page, false), Outcome::Clean { .. }));
	}

	#[test]
	fn html_that_is_not_xml_is_not_let_through_by_a_form_feed_a_reference_or_a_hidden_colon() {
		for doc in [
			// a form feed is a separator in HTML, before and after the name of a handler
			&b"<html><body><img src=x\x0conerror=alert(1)></body>"[..],
			b"<html><body><img src=x onerror\x0c=alert(1)></body>",
			// references to the characters of the address, with and without their `;`
			b"<html><body><a href=\"&#106;avascript:alert(1)\">x</a>",
			b"<html><body><a href=&#106avascript:alert(1)>x</a>",
			b"<html><body><a href=\"java&#x09;script:alert(1)\">x</a>",
			b"<html><body><a href=\"jav&#x61script:alert(1)\">x</a>",
			b"<html><body><a href=\"javascript&colon;alert(1)\">x</a>",
			b"<html><body><a href=\"java&Tab;script&colon;alert(1)\">x</a>",
			// the name of the address without its colon, in a tag
			b"<html><body><a href=\"javascript\">x</a>",
			// a meta whose value is far from its name
			b"<html><head><meta http-equiv                                                                                                                                  =\"refresh\" content=\"0;url=http://x\"></head>",
			// attributes given to every element by the DTD
			b"<!DOCTYPE html [<!ATTLIST img onerror CDATA \"alert(1)\">]><html><body><img src=x></body>",
		] {
			// refused, or cleaned (what the XML read can decode is taken out) to something that
			// no longer looks like anything: never kept as it was
			let handled = match scan_document(doc, true) {
				Outcome::Unstrippable(_) => true,
				Outcome::Found { stripped, .. } => !looks_active(&stripped),
				_ => false,
			};
			assert!(handled, "{}", String::from_utf8_lossy(doc));
		}
		// pages that XML can't read at all are refused by what their bytes look like
		for doc in [
			&b"<html><body><img src=x\x0conerror=alert(1)></body>"[..],
			b"<html><body><a href=&#106avascript:alert(1)>x</a>",
			b"<html><head><meta http-equiv                                                                                                                                  =refresh content=0;url=http://x></head>",
		] {
			assert!(
				matches!(scan_document(doc, true), Outcome::Unstrippable(_)),
				"{}",
				String::from_utf8_lossy(doc)
			);
		}
		// text that only talks about them is a page, not a danger: the references are not in a tag
		let text = b"<html><body><p>The word javascript, an &#169; sign, and a form feed \x0c in text";
		assert!(!looks_active(text), "{}", String::from_utf8_lossy(text));
		// `&amp;` in an address is how a link carries a second parameter
		assert!(!looks_active(
			b"<html><body><a href=\"a.xhtml?x=1&amp;y=2\">x</a>"
		));
	}

	#[test]
	fn a_dtd_subset_is_refused_whatever_it_declares() {
		let attlist = "<?xml version=\"1.0\"?><!DOCTYPE html [ <!ATTLIST img onerror CDATA \"alert(1)\"> ]><html xmlns=\"http://www.w3.org/1999/xhtml\"><body><img src=\"a.png\"/></body></html>";
		assert!(matches!(
			scan_document(attlist.as_bytes(), false),
			Outcome::Unstrippable(_)
		));
		let empty = "<?xml version=\"1.0\"?><!DOCTYPE html [ ]><html xmlns=\"http://www.w3.org/1999/xhtml\"><body/></html>";
		assert!(matches!(
			scan_document(empty.as_bytes(), false),
			Outcome::Unstrippable(_)
		));
		assert!(looks_active(b"<!ATTLIST img onerror CDATA \"x\">"));
	}

	#[test]
	fn an_address_with_an_entity_xml_does_not_know_is_taken_out_and_other_values_are_kept() {
		let doc = "<?xml version=\"1.0\"?><!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd\"><html xmlns=\"http://www.w3.org/1999/xhtml\"><body><a href=\"javascript&colon;alert(1)\">x</a><img alt=\"a&nbsp;b\" src=\"x.png\"/></body></html>";
		match scan_document(doc.as_bytes(), false) {
			Outcome::Found {
				stripped, findings, ..
			} => {
				let text = String::from_utf8(stripped).unwrap();
				assert!(text.contains("<a>x</a>"), "{text}");
				// an alt text with an entity is not an address
				assert!(text.contains("alt=\"a&nbsp;b\""), "{text}");
				assert_eq!(findings.get("undecodable-address"), Some(&1));
			},
			other => panic!("{other:?}"),
		}
	}

	#[test]
	fn svg_animations_go_by_the_name_they_set_whatever_its_prefix_and_lists_of_addresses_go() {
		let (result, _) = stripped(
			"<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:x=\"http://www.w3.org/1999/xlink\"><a x:href=\"a\"><animate attributeName=\"x:href\" values=\"a;b\"/><set attributeName=\"onmouseover\" to=\"alert(1)\"/><text>t</text></a></svg>",
		);
		assert!(!result.contains("animate") && !result.contains("<set"));
		assert!(result.contains("<text>t</text>"));
		// an address anywhere in a value, not only at its start
		let (result, findings) = stripped("<p values=\"a;javascript:alert(1)\" id=\"k\">x</p>");
		assert!(result.contains("<p id=\"k\">x</p>"), "{result}");
		assert_eq!(findings.get("script-address"), Some(&1));
	}

	#[test]
	fn media_types_are_read_the_way_the_reader_reads_them() {
		let parse = |raw| readium_media_type(raw).map(|(k, s, p)| (format!("{k}/{s}"), p));
		assert_eq!(
			parse("application/xhtml+xml;").map(|(t, _)| t).as_deref(),
			Some("application/xhtml+xml")
		);
		assert_eq!(parse("text/html;x").map(|(t, _)| t).as_deref(), Some("text/html"));
		assert_eq!(
			parse(" Text / HTML ; Charset = utf-8 ").map(|(t, p)| (t, p.len())),
			Some(("text/html".to_string(), 1))
		);
		// a parameter that isn't `name=value` is dropped
		assert_eq!(parse("text/html;a=b=c").map(|(_, p)| p.len()), Some(0));
		for invalid in ["texthtml", "a/b/c", "/html", "text/", ""] {
			assert_eq!(parse(invalid), None, "{invalid}");
		}
		for html in ["text/html", "TEXT/HTML ;x", "text/html;charset=utf-8", "*/*", "text/*"] {
			assert!(is_declared_html(Some(html)), "{html}");
		}
		for other in ["application/xhtml+xml", "image/png", "text/css", "application/xml", ""] {
			assert!(!is_declared_html(Some(other)), "{other}");
		}
		assert!(!is_declared_html(None));
	}

	#[test]
	fn a_page_is_a_document_by_any_type_the_package_declares_for_it() {
		let xhtml = |t: &str| vec![t.to_string()];
		assert!(is_document("weird.dat", &xhtml("application/xhtml+xml")));
		assert!(is_document("weird.dat", &xhtml("application/xhtml+xml;")));
		assert!(is_document("weird.dat", &xhtml(" text/html ; x ")));
		// listed twice, with an innocent type the second time
		assert!(is_document(
			"weird.dat",
			&["image/png".to_string(), "application/xhtml+xml;".to_string()]
		));
		assert!(!is_document("a.png", &xhtml("image/png")));
		// HTML is held to its bytes by the type, or by a name that is HTML's when there is no type
		assert!(is_html_page("p.dat", &xhtml("text/html;x")));
		assert!(is_html_page("p.htm", &[]));
		assert!(!is_html_page("p.xhtml", &xhtml("application/xhtml+xml")));
		assert!(!is_html_page("p.dat", &[]));
	}

	#[test]
	fn what_starts_with_markup_is_told_from_its_first_bytes() {
		assert_eq!(starts_with_markup(b"  \n<html/>"), Some(true));
		assert_eq!(starts_with_markup(b"\xef\xbb\xbf\x0c<html/>"), Some(true));
		assert_eq!(starts_with_markup(b"hello <b>"), Some(false));
		assert_eq!(starts_with_markup(b"GIF89a<script>"), Some(false));
		assert_eq!(starts_with_markup(b"   \t "), None);
		assert_eq!(starts_with_markup(b""), None);
	}

	#[test]
	fn remote_resources_are_counted_and_kept() {
		let doc = page("<img src=\"https://example.com/a.png\"/><a href=\"https://example.com\">link</a><link href=\"//cdn.example/x.css\"/>");
		assert_eq!(
			scan_document(&doc, false),
			Outcome::Clean { remote_refs: 2 }
		);
	}

	#[test]
	fn an_instruction_inside_what_is_removed_does_not_bring_it_back() {
		// the stylesheet instruction is inside an `<object>` that goes: what is before it inside
		// the object (a script, a text) must not be copied back into what is kept
		let (result, _) = stripped(
			"<p>a</p><object><b>inner</b><script>alert(1)</script><?xml-stylesheet href=\"x.xsl\"?><i>more</i></object><p>b</p>",
		);
		assert!(result.contains("<p>a</p>") && result.contains("<p>b</p>"), "{result}");
		for gone in ["inner", "script", "more", "xml-stylesheet"] {
			assert!(!result.contains(gone), "{gone}: {result}");
		}
	}

	#[test]
	fn what_is_left_is_clean_whatever_was_nested() {
		// scripts inside scripts' siblings, a deep tree, and a removed element that holds elements
		let mut body = String::new();
		for _ in 0..200 {
			body.push_str("<div>");
		}
		body.push_str("<object><p>inner</p><script>x</script></object>text");
		for _ in 0..200 {
			body.push_str("</div>");
		}
		let (result, _) = stripped(&body);
		assert!(
			result.contains("text")
				&& !result.contains("inner")
				&& !result.contains("script")
		);
		assert!(matches!(
			scan_document(result.as_bytes(), false),
			Outcome::Clean { .. }
		));
	}

	#[test]
	fn which_entries_are_documents() {
		assert!(is_document("OEBPS/ch1.xhtml", &[]));
		assert!(is_document("img/a.SVG", &[]));
		assert!(is_document("weird.dat", &["application/xhtml+xml".to_string()]));
		assert!(!is_document("img/a.png", &["image/png".to_string()]));
		assert!(!is_document("fonts/a.otf", &[]));
	}
}
