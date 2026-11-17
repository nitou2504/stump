//! The OPDS 1.2 catalog as the home listener serves it: reading lists and shelves (package B4).
//!
//! The helpers read a feed the way a client does: the XML is decoded once (so what the writer
//! escaped for the XML is text again), and what is left is what the client's HTML reader is given
//! for an entry's `content`.

mod discover;
mod reading_lists;
mod shelves;

use crate::common::TestApp;

/// One entry of a feed
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
	pub id: String,
	pub title: String,
	/// The content as a client reads it (HTML, after the XML is decoded)
	pub content: String,
	pub author: Option<String>,
	pub href: String,
}

/// A feed's text, which fails the test when the request failed
pub(crate) async fn feed(app: &TestApp, token: &str, path: &str) -> String {
	let response = app.get_with_token(path, token).await;
	let status = response.status_code().as_u16();
	let body = String::from_utf8_lossy(response.as_bytes()).to_string();
	assert_eq!(status, 200, "{path}: {body}");
	body
}

/// The status and body of a request, whatever it was
pub(crate) async fn answer(app: &TestApp, token: &str, path: &str) -> (u16, String) {
	let response = app.get_with_token(path, token).await;
	(
		response.status_code().as_u16(),
		String::from_utf8_lossy(response.as_bytes()).to_string(),
	)
}

/// The five entities XML predefines, read once, the way an XML reader does
pub(crate) fn decode(text: &str) -> String {
	text.replace("&lt;", "<")
		.replace("&gt;", ">")
		.replace("&quot;", "\"")
		.replace("&apos;", "'")
		.replace("&amp;", "&")
}

/// What is between `open` and `close` in `text`, from the start of `text`
fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
	let start = text.find(open)? + open.len();
	let end = text[start..].find(close)? + start;
	Some(&text[start..end])
}

/// The entries of a feed, in order
pub(crate) fn entries(feed: &str) -> Vec<Entry> {
	feed.split("<entry>")
		.skip(1)
		.map(|chunk| {
			let chunk = chunk.split("</entry>").next().expect("an entry");
			let content = chunk
				.find("<content")
				.and_then(|at| {
					let rest = &chunk[at..];
					let open_end = rest.find('>')?;
					// an empty content is written as a tag that closes itself
					if rest[..open_end].ends_with('/') {
						return Some(String::new());
					}
					between(&rest[open_end + 1..], "", "</content>").map(decode)
				})
				.unwrap_or_default();
			Entry {
				id: decode(between(chunk, "<id>", "</id>").expect("an id")),
				title: decode(between(chunk, "<title>", "</title>").expect("a title")),
				content,
				author: between(chunk, "<author><name>", "</name>").map(decode),
				href: decode(between(chunk, "href=\"", "\"").unwrap_or_default()),
			}
		})
		.collect()
}

/// The titles of a feed's entries, in order
pub(crate) fn titles(feed: &str) -> Vec<String> {
	entries(feed).into_iter().map(|entry| entry.title).collect()
}

/// Every `thr:count` in a feed: how many books each facet link says is behind it
pub(crate) fn facet_counts(feed: &str) -> Vec<u64> {
	feed.split("thr:count=\"")
		.skip(1)
		.map(|rest| {
			rest.split('"')
				.next()
				.and_then(|count| count.parse().ok())
				.expect("a number")
		})
		.collect()
}
