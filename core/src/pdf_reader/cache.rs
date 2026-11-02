//! The disk cache of rendered PDF pages (`<config>/cache/pdf_pages`), with a size cap.
//!
//! Before this, the cache never shrank (the live server held 245 MB in 1011 files after a few
//! days). Writes add to an approximate total; when it passes the cap, the oldest files (by
//! modification time, refreshed on hits) are removed down to 90% of the cap. The total is
//! measured once at startup by walking the directory.

use std::{
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, AtomicU64, Ordering},
		Arc,
	},
	time::{Duration, SystemTime},
};

/// A hit older than this has its modification time refreshed, so eviction keeps pages in use
const TOUCH_AFTER: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone)]
pub struct PageCache {
	inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
	directory: PathBuf,
	max_bytes: u64,
	total_bytes: AtomicU64,
	evicting: AtomicBool,
}

impl PageCache {
	/// `max_bytes = 0` disables the cap
	pub fn new(directory: PathBuf, max_bytes: u64) -> Self {
		Self {
			inner: Arc::new(Inner {
				directory,
				max_bytes,
				total_bytes: AtomicU64::new(0),
				evicting: AtomicBool::new(false),
			}),
		}
	}

	/// Measures the directory and evicts past the cap, on a background thread (once at startup)
	pub fn start_background_scan(&self) {
		let measuring = self.clone();
		std::thread::Builder::new()
			.name("pdf-cache-scan".to_string())
			.spawn(move || {
				let total = measuring.measure();
				measuring.inner.total_bytes.store(total, Ordering::Relaxed);
				measuring.evict_if_needed();
			})
			.ok();
	}

	pub fn directory(&self) -> &Path {
		&self.inner.directory
	}

	pub fn path_for(&self, file_name: &str) -> PathBuf {
		self.inner.directory.join(file_name)
	}

	/// Reads a cached file, refreshing its modification time when it's old
	pub fn read(&self, file_name: &str) -> Option<Vec<u8>> {
		let path = self.path_for(file_name);
		let bytes = std::fs::read(&path).ok()?;
		if bytes.is_empty() {
			let _ = std::fs::remove_file(&path);
			return None;
		}
		touch_if_old(&path);
		Some(bytes)
	}

	/// Writes a file atomically (temp file + rename) and evicts old files past the cap
	pub fn write(&self, file_name: &str, bytes: &[u8]) {
		if bytes.is_empty() {
			return;
		}
		let directory = &self.inner.directory;
		if !directory.exists() && std::fs::create_dir_all(directory).is_err() {
			return;
		}
		let path = self.path_for(file_name);
		let temp = self.path_for(&format!("{file_name}.tmp"));
		if std::fs::write(&temp, bytes).is_err() {
			let _ = std::fs::remove_file(&temp);
			return;
		}
		if std::fs::rename(&temp, &path).is_err() {
			let _ = std::fs::remove_file(&temp);
			return;
		}
		self.add(bytes.len() as u64);
	}

	/// Records bytes written by another writer into the same directory (the image reader's
	/// legacy page cache)
	pub fn add(&self, bytes: u64) {
		let total = self.inner.total_bytes.fetch_add(bytes, Ordering::Relaxed) + bytes;
		if self.inner.max_bytes > 0 && total > self.inner.max_bytes {
			self.evict_in_background();
		}
	}

	pub fn total_bytes(&self) -> u64 {
		self.inner.total_bytes.load(Ordering::Relaxed)
	}

	fn evict_in_background(&self) {
		if self.inner.evicting.swap(true, Ordering::AcqRel) {
			return;
		}
		let cache = self.clone();
		let spawned = std::thread::Builder::new()
			.name("pdf-cache-evict".to_string())
			.spawn(move || {
				cache.evict_if_needed();
				cache.inner.evicting.store(false, Ordering::Release);
			});
		if spawned.is_err() {
			self.inner.evicting.store(false, Ordering::Release);
		}
	}

	fn measure(&self) -> u64 {
		list_files(&self.inner.directory)
			.iter()
			.map(|(_, size, _)| size)
			.sum()
	}

	/// Removes the oldest files until the cache is at 90% of its cap. Returns the bytes removed.
	pub fn evict_if_needed(&self) -> u64 {
		let max = self.inner.max_bytes;
		if max == 0 {
			return 0;
		}
		let mut files = list_files(&self.inner.directory);
		let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
		if total <= max {
			self.inner.total_bytes.store(total, Ordering::Relaxed);
			return 0;
		}
		let target = max / 10 * 9;
		files.sort_by_key(|(_, _, modified)| *modified);
		let mut removed = 0;
		for (path, size, _) in files {
			if total <= target {
				break;
			}
			if std::fs::remove_file(&path).is_ok() {
				total = total.saturating_sub(size);
				removed += size;
			}
		}
		self.inner.total_bytes.store(total, Ordering::Relaxed);
		tracing::debug!(removed, total, max, "Evicted old PDF page cache files");
		removed
	}
}

fn list_files(directory: &Path) -> Vec<(PathBuf, u64, SystemTime)> {
	let Ok(entries) = std::fs::read_dir(directory) else {
		return Vec::new();
	};
	entries
		.filter_map(Result::ok)
		.filter_map(|entry| {
			let metadata = entry.metadata().ok()?;
			metadata.is_file().then(|| {
				(
					entry.path(),
					metadata.len(),
					metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
				)
			})
		})
		.collect()
}

fn touch_if_old(path: &Path) {
	let Ok(metadata) = std::fs::metadata(path) else {
		return;
	};
	let old = metadata
		.modified()
		.ok()
		.and_then(|modified| modified.elapsed().ok())
		.is_some_and(|age| age > TOUCH_AFTER);
	if old {
		if let Ok(file) = std::fs::File::options().write(true).open(path) {
			let _ = file.set_modified(SystemTime::now());
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn age(path: &Path, seconds: u64) {
		let file = std::fs::File::options().write(true).open(path).unwrap();
		file.set_modified(SystemTime::now() - Duration::from_secs(seconds))
			.unwrap();
	}

	#[test]
	fn evicts_the_oldest_files_past_the_cap() {
		let dir = tempfile::tempdir().unwrap();
		let cache = PageCache::new(dir.path().to_path_buf(), 1000);
		for (name, seconds) in [("old", 300), ("middle", 200), ("new", 100)] {
			std::fs::write(dir.path().join(name), vec![1u8; 400]).unwrap();
			age(&dir.path().join(name), seconds);
		}
		let removed = cache.evict_if_needed();
		assert_eq!(removed, 400);
		assert!(!dir.path().join("old").exists());
		assert!(dir.path().join("middle").exists());
		assert!(dir.path().join("new").exists());
		assert_eq!(cache.total_bytes(), 800);
	}

	#[test]
	fn writes_and_reads_back_and_refreshes_old_hits() {
		let dir = tempfile::tempdir().unwrap();
		let cache = PageCache::new(dir.path().join("pages"), 0);
		cache.write("a.webp", b"abc");
		assert_eq!(cache.read("a.webp").as_deref(), Some(&b"abc"[..]));
		assert!(cache.read("missing.webp").is_none());
		let path = cache.path_for("a.webp");
		age(&path, 7200);
		cache.read("a.webp");
		let fresh = std::fs::metadata(&path)
			.unwrap()
			.modified()
			.unwrap()
			.elapsed()
			.unwrap();
		assert!(fresh < Duration::from_secs(60));
		assert_eq!(cache.evict_if_needed(), 0, "no cap");
	}
}
