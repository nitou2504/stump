use xml::{writer::XmlEvent, EventWriter};

use crate::error::CoreResult;

pub trait OpdsEnumStr {
	fn as_str(&self) -> &'static str;
}

pub fn tag_id_from_url(tag_authority: &str, url: &str) -> String {
	format!(
		"tag:{}:{}",
		tag_authority,
		url.trim_start_matches('/').replace('/', ":")
	)
}

pub fn write_xml_element(
	name: &str,
	value: &str,
	writer: &mut EventWriter<Vec<u8>>,
) -> CoreResult<()> {
	writer.write(XmlEvent::start_element(name))?;
	writer.write(XmlEvent::characters(value))?;
	writer.write(XmlEvent::end_element())?;
	Ok(())
}

pub fn write_xml_content(
	value: &str,
	writer: &mut EventWriter<Vec<u8>>,
) -> CoreResult<()> {
	writer.write(XmlEvent::start_element("content").attr("type", "html"))?;
	writer.write(XmlEvent::characters(value))?;
	writer.write(XmlEvent::end_element())?;
	Ok(())
}

/// Escapes plain text for an element whose type is `html`, like an entry's `content`.
///
/// The feed's XML writer escapes `&` and `<` for the XML itself, and a client then reads the text
/// *as HTML*. A description somebody else wrote has to arrive as the text it is, so a `<b>`, an
/// `<img onerror=…>` or a link in it must not be markup in the owner's reader: it is escaped
/// here, once, before it is put in the content.
pub fn escape_html(value: &str) -> String {
	let mut escaped = String::with_capacity(value.len());
	for c in value.chars() {
		match c {
			'&' => escaped.push_str("&amp;"),
			'<' => escaped.push_str("&lt;"),
			'>' => escaped.push_str("&gt;"),
			'"' => escaped.push_str("&quot;"),
			'\'' => escaped.push_str("&#39;"),
			c => escaped.push(c),
		}
	}
	escaped
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn escape_html_turns_markup_into_text() {
		assert_eq!(escape_html("<b>&"), "&lt;b&gt;&amp;");
		assert_eq!(
			escape_html(r#"<img src=x onerror="alert('1')">"#),
			"&lt;img src=x onerror=&quot;alert(&#39;1&#39;)&quot;&gt;"
		);
		// the ampersand goes first, so what it makes is not escaped a second time
		assert_eq!(escape_html("&lt;"), "&amp;lt;");
	}

	#[test]
	fn escape_html_leaves_plain_text_alone() {
		for text in [
			"",
			"Fantasy for the summer",
			"Ñandú · 日本語 — ok",
			"1 + 1 = 2",
		] {
			assert_eq!(escape_html(text), text);
		}
	}

	/// What a client sees after reading the feed's XML: the XML is decoded once (the writer
	/// escaped the text for it), and the result is HTML in which nothing is a tag
	#[test]
	fn an_escaped_description_comes_out_of_the_feed_as_the_same_text() {
		let description = "Tom & Jerry <script>alert(1)</script>";
		let mut writer = EventWriter::new(Vec::new());
		write_xml_content(&escape_html(description), &mut writer)
			.expect("should write the content");
		let xml = String::from_utf8(writer.into_inner()).expect("utf-8");
		// the XML holds no tag but its own
		assert!(!xml.contains("<script"), "{xml}");
		assert!(xml.contains("&amp;lt;script&amp;gt;"), "{xml}");
	}
}
