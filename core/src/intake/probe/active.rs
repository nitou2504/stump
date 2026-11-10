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
//! A DOCTYPE that declares an entity is refused (it is how a small text becomes a huge one), and a
//! document that is not well formed is not shown by the reader at all, so it is left as it is, unless
//! its bytes look like they carry anything of the above (then the book is refused, since what the
//! reader would do with it can't be told).
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

/// Whether an entry of an EPUB is a document a browser could render
pub fn is_document(name: &str, media_type: Option<&str>) -> bool {
	let extension = name
		.rsplit_once('.')
		.map(|(_, ext)| ext.to_ascii_lowercase())
		.unwrap_or_default();
	DOCUMENT_EXTENSIONS.contains(&extension.as_str())
		|| media_type.is_some_and(|t| {
			let t = t.trim().to_ascii_lowercase();
			DOCUMENT_TYPES.contains(&t.as_str())
		})
}

/// Whether an item the package declares as `text/html` is one (the reader would read it as HTML,
/// which has more ways to hide an attribute than XML has)
pub fn is_declared_html(media_type: Option<&str>) -> bool {
	media_type.is_some_and(|t| t.trim().eq_ignore_ascii_case("text/html"))
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
}

fn attributes_of(tag: &BytesStart) -> Option<Vec<Attribute>> {
	let mut out = vec![];
	for attribute in tag.attributes() {
		let attribute = attribute.ok()?;
		let text = attribute
			.unescape_value()
			.map(|v| v.into_owned())
			.unwrap_or_else(|_| String::from_utf8_lossy(&attribute.value).into_owned());
		out.push(Attribute {
			key: attribute.key.as_ref().to_vec(),
			local: lowercase_local(attribute.key.as_ref()),
			value: attribute.value.to_vec(),
			text,
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
			if name == "href" || name == "xlink:href" {
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
	if url.starts_with("javascript:") || url.starts_with("vbscript:") {
		return Some("script-address");
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
				if contains_ignore_case(&text, b"<!entity") {
					return Outcome::Unstrippable(
						"the document declares entities".to_string(),
					);
				}
			},
			Ok(Event::PI(pi)) => {
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

/// The bytes of a document as an address in it would be read: lower case, the entities that
/// stand for a tab, a line break or a colon decoded, the tabs and line breaks gone
fn flattened(lower: &[u8]) -> Vec<u8> {
	let mut out = Vec::with_capacity(lower.len());
	let mut i = 0usize;
	while i < lower.len() {
		match lower[i] {
			b'\t' | b'\n' | b'\r' => i += 1,
			b'&' => {
				let reference = lower[i + 1..(i + 12).min(lower.len())]
					.iter()
					.position(|b| *b == b';')
					.map(|end| &lower[i + 1..i + 1 + end]);
				match reference {
					Some(
						b"tab" | b"newline" | b"#9" | b"#x9" | b"#10" | b"#xa" | b"#13"
						| b"#xd",
					) => {
						i += lower[i + 1..].iter().position(|b| *b == b';').unwrap_or(0)
							+ 2;
					},
					Some(b"colon" | b"#58" | b"#x3a") => {
						out.push(b':');
						i += lower[i + 1..].iter().position(|b| *b == b';').unwrap_or(0)
							+ 2;
					},
					_ => {
						out.push(b'&');
						i += 1;
					},
				}
			},
			byte => {
				out.push(byte);
				i += 1;
			},
		}
	}
	out
}

/// A look at the bytes of a document the reader would not parse as XML, or would parse as HTML:
/// does anything in it look like an element, an attribute or an address that runs?
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
	// a meta that refreshes or sets a cookie
	let mut from = 0usize;
	while let Some(at) = find(&lower[from..], b"http-equiv") {
		let at = from + at;
		let window = &lower[at..(at + 120).min(lower.len())];
		if find(window, b"refresh").is_some() || find(window, b"set-cookie").is_some() {
			return true;
		}
		from = at + 10;
	}
	// an event handler: ` onload=`, `/onerror =`, `"onclick=`
	let mut i = 0usize;
	while i + 4 < lower.len() {
		if lower[i] == b'o'
			&& lower[i + 1] == b'n'
			&& i > 0 && matches!(
			lower[i - 1],
			b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'"' | b'\''
		) {
			let mut j = i + 2;
			while j < lower.len() && lower[j].is_ascii_alphabetic() {
				j += 1;
			}
			if j - (i + 2) >= 2 {
				let mut k = j;
				while k < lower.len() && matches!(lower[k], b' ' | b'\t' | b'\n' | b'\r')
				{
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
		let sneaky = page("<a href=\"java&Tab;script&colon;alert(1)\">x</a>");
		assert!(matches!(
			scan_document(&sneaky, true),
			Outcome::Unstrippable(_)
		));
		let fine = page("<p>hello</p>");
		assert!(matches!(scan_document(&fine, true), Outcome::Clean { .. }));
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
		assert!(is_document("OEBPS/ch1.xhtml", None));
		assert!(is_document("img/a.SVG", None));
		assert!(is_document("weird.dat", Some("application/xhtml+xml")));
		assert!(!is_document("img/a.png", Some("image/png")));
		assert!(!is_document("fonts/a.otf", None));
	}
}
