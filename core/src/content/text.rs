//! The text of sections, read from the book's file.
//!
//! The index keeps the words (positions included) but NOT the text: that would be a second copy of
//! every book. A snippet, a read or a grep reads the section from the EPUB or PDF again, with the same
//! code that read it for the index (`extract`), so positions agree.
//!
//! - A section knows where it came from (`content_sections.source`): an EPUB reads that one item of
//!   its reading order, a PDF that range of pages. The book is opened once for all the sections asked
//!   of it.
//! - A section of a book indexed before that was recorded (no `source`) reads the whole book and takes
//!   the section by its number, kept in a small memo so a grep does it once. The index job gives such a
//!   book a `source` the next time it runs.
//! - Section texts are kept in a process-wide LRU bounded in bytes. A section's id changes when its
//!   book is indexed again, so an entry never outlives its text.

use std::{
	collections::{HashMap, VecDeque},
	path::Path,
	sync::{Arc, Mutex, OnceLock},
};

use tokio::sync::Semaphore;

use super::{
	extract::{self, SourceRef},
	ContentError, ContentResult,
};
use crate::config::StumpConfig;

/// Bytes of section text kept (and, apart, of whole-book texts)
const CACHE_BYTES: usize = 96 * 1024 * 1024;
/// EPUB reads at once (each is a blocking read of a zip)
const CONCURRENT_READS: usize = 4;

/// A section whose text is wanted
#[derive(Debug, Clone)]
pub struct Want {
	/// `content_sections.id`
	pub id: i64,
	/// `content_sections.n`
	pub n: i64,
	/// `content_sections.source`
	pub source: Option<String>,
}

struct ByteLru<K, V> {
	map: HashMap<K, (V, usize)>,
	order: VecDeque<K>,
	bytes: usize,
	max: usize,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> ByteLru<K, V> {
	fn new(max: usize) -> Self {
		Self {
			map: HashMap::new(),
			order: VecDeque::new(),
			bytes: 0,
			max,
		}
	}

	fn get(&mut self, key: &K) -> Option<V> {
		let value = self.map.get(key)?.0.clone();
		self.order.retain(|k| k != key);
		self.order.push_back(key.clone());
		Some(value)
	}

	fn put(&mut self, key: K, value: V, size: usize) {
		if size > self.max / 2 {
			return;
		}
		if let Some((_, old)) = self.map.remove(&key) {
			self.bytes -= old;
			self.order.retain(|k| *k != key);
		}
		while self.bytes + size > self.max {
			let Some(oldest) = self.order.pop_front() else {
				break;
			};
			if let Some((_, old)) = self.map.remove(&oldest) {
				self.bytes -= old;
			}
		}
		self.order.push_back(key.clone());
		self.map.insert(key, (value, size));
		self.bytes += size;
	}
}

/// Kept by the book's path and the section's id: the id alone is not enough across databases (the
/// tests have many), and a section's id changes when its book is indexed again
fn sections() -> &'static Mutex<ByteLru<(String, i64), Arc<String>>> {
	static CACHE: OnceLock<Mutex<ByteLru<(String, i64), Arc<String>>>> = OnceLock::new();
	CACHE.get_or_init(|| Mutex::new(ByteLru::new(CACHE_BYTES)))
}

fn gate() -> &'static Semaphore {
	static GATE: OnceLock<Semaphore> = OnceLock::new();
	GATE.get_or_init(|| Semaphore::new(CONCURRENT_READS))
}

/// The memo of whole books read for sections that have no `source`, by the book's path and the
/// file's size and modification time
fn books() -> &'static Mutex<HashMap<String, Arc<Vec<Arc<String>>>>> {
	static BOOKS: OnceLock<Mutex<HashMap<String, Arc<Vec<Arc<String>>>>>> = OnceLock::new();
	BOOKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whole books the memo keeps (it is for the greps and reads of a few books, not a cache)
const MEMO_BOOKS: usize = 2;

fn stamp(path: &str) -> String {
	let meta = std::fs::metadata(path).ok();
	let size = meta.as_ref().map_or(0, |m| m.len());
	let mtime = meta
		.and_then(|m| m.modified().ok())
		.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
		.map_or(0, |d| d.as_secs());
	format!("{path}\0{size}\0{mtime}")
}

/// The text of the `wanted` sections of one book, by section id. A section that can no longer be
/// read from the file (it changed since it was indexed) is left out; the book's file missing or
/// unreadable is an error.
pub async fn read_sections(
	config: &StumpConfig,
	path: &str,
	kind: &str,
	wanted: &[Want],
) -> ContentResult<HashMap<i64, Arc<String>>> {
	let mut out: HashMap<i64, Arc<String>> = HashMap::with_capacity(wanted.len());
	if !Path::new(path).is_file() {
		return Err(ContentError::Unavailable(
			"The book's file is not available".to_string(),
		));
	}
	let mut missing: Vec<&Want> = Vec::new();
	{
		let mut cache = sections().lock().unwrap_or_else(|e| e.into_inner());
		for want in wanted {
			match cache.get(&(path.to_string(), want.id)) {
				Some(text) => {
					out.insert(want.id, text);
				},
				None => missing.push(want),
			}
		}
	}
	if missing.is_empty() {
		return Ok(out);
	}
	let (known, legacy): (Vec<&Want>, Vec<&Want>) = missing
		.into_iter()
		.partition(|w| w.source.as_deref().and_then(SourceRef::parse).is_some());
	let mut read: Vec<(i64, String)> = Vec::new();

	if !known.is_empty() {
		if kind == "pdf" {
			let ranges: Vec<(i32, i32)> = known
				.iter()
				.filter_map(|w| match w.source.as_deref().and_then(SourceRef::parse) {
					Some(SourceRef::Pages(first, last)) => Some((first, last)),
					_ => None,
				})
				.collect();
			let texts = extract::read_pdf_ranges(config, Path::new(path), &ranges).await?;
			for want in &known {
				if let Some(SourceRef::Pages(first, last)) =
					want.source.as_deref().and_then(SourceRef::parse)
				{
					if let Some(text) = texts.get(&(first, last)).filter(|t| !t.is_empty()) {
						read.push((want.id, text.clone()));
					}
				}
			}
		} else {
			let spines: Vec<usize> = known
				.iter()
				.filter_map(|w| match w.source.as_deref().and_then(SourceRef::parse) {
					Some(SourceRef::Spine(index)) => Some(index),
					_ => None,
				})
				.collect();
			let owned = path.to_string();
			let _permit = gate().acquire().await.ok();
			let mut texts = tokio::task::spawn_blocking(move || {
				std::panic::catch_unwind(|| extract::read_epub_items(&owned, &spines)).unwrap_or_else(
					|_| Err(ContentError::Extract("the reader failed on this file".into())),
				)
			})
			.await
			.map_err(|_| ContentError::Extract("the reading failed".to_string()))??;
			for want in &known {
				if let Some(SourceRef::Spine(index)) =
					want.source.as_deref().and_then(SourceRef::parse)
				{
					if let Some(text) = texts.remove(&index) {
						read.push((want.id, text));
					}
				}
			}
		}
	}

	if !legacy.is_empty() {
		let book = whole_book(config, path, kind).await?;
		for want in &legacy {
			if let Some(text) = usize::try_from(want.n).ok().and_then(|n| book.get(n)) {
				read.push((want.id, text.as_ref().clone()));
			}
		}
	}

	let mut cache = sections().lock().unwrap_or_else(|e| e.into_inner());
	for (id, text) in read {
		let text = Arc::new(text);
		cache.put((path.to_string(), id), text.clone(), text.len());
		out.insert(id, text);
	}
	Ok(out)
}

/// Every section's text of a book, by section number: the way to read a section that has no
/// `source`
async fn whole_book(
	config: &StumpConfig,
	path: &str,
	kind: &str,
) -> ContentResult<Arc<Vec<Arc<String>>>> {
	let key = stamp(path);
	if let Some(found) = books().lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
		return Ok(found.clone());
	}
	let extracted = if kind == "pdf" {
		extract::extract_pdf(config, Path::new(path)).await?
	} else {
		let owned = path.to_string();
		let _permit = gate().acquire().await.ok();
		tokio::task::spawn_blocking(move || {
			std::panic::catch_unwind(|| extract::extract_epub(&owned)).unwrap_or_else(|_| {
				Err(ContentError::Extract("the reader failed on this file".into()))
			})
		})
		.await
		.map_err(|_| ContentError::Extract("the reading failed".to_string()))??
	};
	let texts: Arc<Vec<Arc<String>>> = Arc::new(
		extracted
			.sections
			.into_iter()
			.map(|s| Arc::new(s.text))
			.collect(),
	);
	let mut memo = books().lock().unwrap_or_else(|e| e.into_inner());
	if memo.len() >= MEMO_BOOKS {
		memo.clear();
	}
	memo.insert(key, texts.clone());
	Ok(texts)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_lru_drops_the_oldest_by_bytes_and_skips_what_would_fill_it() {
		let mut lru: ByteLru<i64, u8> = ByteLru::new(10);
		lru.put(1, 1, 4);
		lru.put(2, 2, 4);
		assert_eq!(lru.get(&1), Some(1)); // 1 is now the newest
		lru.put(3, 3, 4); // over 10 bytes: 2, the oldest, goes
		assert_eq!(lru.get(&2), None);
		assert_eq!(lru.get(&1), Some(1));
		assert_eq!(lru.get(&3), Some(3));
		lru.put(4, 4, 9); // more than half the cache: not kept
		assert_eq!(lru.get(&4), None);
		assert_eq!(lru.get(&3), Some(3));
	}

	#[test]
	fn sources_round_trip() {
		for source in [SourceRef::Spine(7), SourceRef::Pages(3, 19)] {
			assert_eq!(SourceRef::parse(&source.encode()), Some(source));
		}
		assert_eq!(SourceRef::parse("x:1"), None);
		assert_eq!(SourceRef::parse("p:1"), None);
	}
}
