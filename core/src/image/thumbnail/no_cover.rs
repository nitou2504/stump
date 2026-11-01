//! Books without a cover image (an EPUB may legitimately have none) are remembered with a
//! small marker file, so thumbnail and placeholder jobs don't reopen them and report the same
//! miss after every scan. A marker holds the book file's size and modification time: once the
//! file changes, or a regeneration is forced, the book is tried again.

use std::{
	path::{Path, PathBuf},
	time::UNIX_EPOCH,
};

use tokio::fs;

use crate::config::StumpConfig;

/// The folder, inside the thumbnails directory, holding one marker per coverless book. It
/// never matches an entity ID, so thumbnail lookups and removals by ID skip it
const NO_COVER_DIR: &str = ".no-cover";

fn marker_path(config: &StumpConfig, book_id: &str) -> PathBuf {
	config
		.thumbnails_directory()
		.join(NO_COVER_DIR)
		.join(book_id)
}

/// Identifies a version of a book file by its size and modification time
async fn file_signature(book_path: &Path) -> Option<String> {
	let metadata = fs::metadata(book_path).await.ok()?;
	let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
	Some(format!("{} {}", metadata.len(), modified.as_nanos()))
}

/// Whether this version of the book file is already known to have no cover image
pub(crate) async fn is_known_coverless(
	config: &StumpConfig,
	book_id: &str,
	book_path: &Path,
) -> bool {
	// The marker is read first: most books have none, and their file isn't touched
	let Ok(saved) = fs::read_to_string(marker_path(config, book_id)).await else {
		return false;
	};
	file_signature(book_path)
		.await
		.is_some_and(|signature| signature == saved)
}

/// Remembers that this version of the book file has no cover image. This is logged once,
/// since the book is not opened again until its file changes or a regeneration is forced
pub(crate) async fn remember_coverless(
	config: &StumpConfig,
	book_id: &str,
	book_path: &Path,
) {
	tracing::info!(
		book_id = %book_id,
		path = %book_path.display(),
		"The book has no cover image, it keeps the default cover until its file changes"
	);
	let Some(signature) = file_signature(book_path).await else {
		return;
	};
	let path = marker_path(config, book_id);
	if let Err(error) = write_marker(&path, signature).await {
		// Not fatal: the book is just tried again on the next thumbnail job
		tracing::warn!(?error, ?path, "Failed to remember a book without a cover");
	}
}

async fn write_marker(path: &Path, signature: String) -> std::io::Result<()> {
	if let Some(dir) = path.parent() {
		fs::create_dir_all(dir).await?;
	}
	fs::write(path, signature).await
}

#[cfg(test)]
mod tests {
	use super::*;

	fn config_in(dir: &Path) -> StumpConfig {
		StumpConfig {
			config_dir: dir.to_string_lossy().to_string(),
			..StumpConfig::debug()
		}
	}

	#[tokio::test]
	async fn test_coverless_marker_follows_the_book_file() {
		let temp = tempfile::tempdir().expect("tempdir");
		let config = config_in(temp.path());
		let book = temp.path().join("book.epub");
		std::fs::write(&book, b"first version").expect("write book");

		assert!(!is_known_coverless(&config, "book-id", &book).await);

		remember_coverless(&config, "book-id", &book).await;
		assert!(is_known_coverless(&config, "book-id", &book).await);
		// Only that book is known
		assert!(!is_known_coverless(&config, "other-id", &book).await);

		// A changed file is tried again
		std::fs::write(&book, b"second, longer version").expect("rewrite book");
		assert!(!is_known_coverless(&config, "book-id", &book).await);
	}

	#[tokio::test]
	async fn test_coverless_marker_is_not_a_thumbnail() {
		let temp = tempfile::tempdir().expect("tempdir");
		let config = config_in(temp.path());
		std::fs::create_dir_all(config.thumbnails_directory()).expect("thumbnails dir");
		let book = temp.path().join("book.epub");
		std::fs::write(&book, b"book").expect("write book");

		remember_coverless(&config, "book-id", &book).await;

		// Lookups by ID ignore the marker folder
		let thumbnail =
			crate::image::get_thumbnail(config.thumbnails_directory(), "book-id", None)
				.await
				.expect("read thumbnails dir");
		assert!(thumbnail.is_none());
	}

	#[tokio::test]
	async fn test_missing_book_is_never_known_coverless() {
		let temp = tempfile::tempdir().expect("tempdir");
		let config = config_in(temp.path());
		let book = temp.path().join("missing.epub");

		remember_coverless(&config, "book-id", &book).await;
		assert!(!is_known_coverless(&config, "book-id", &book).await);
	}
}
