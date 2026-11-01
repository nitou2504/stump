//! Replacing a library file with its processed version: the original is first copied into the
//! quarantine folder (never deleted) and logged in a `restore_log.csv` with the same columns
//! as book-curator's swaps (`original_path,quarantine_path,bytes_before,bytes_after`)

use std::{
	fs::{self, File, OpenOptions},
	io::{self, Write},
	path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};

/// Where an original went
#[derive(Debug, Clone)]
pub struct SwapOutcome {
	pub quarantine_path: PathBuf,
	pub bytes_before: u64,
	pub bytes_after: u64,
}

/// The quarantine folder of one batch, e.g. `<root>/2026-11-02-stump-grey16`
pub fn batch_dir(root: &Path, kind: &str, now: DateTime<Utc>) -> PathBuf {
	root.join(format!("{}-stump-{kind}", now.format("%Y-%m-%d")))
}

/// The path of a book inside a quarantine batch: its path relative to the library's parent
/// folder, so a library at `/biblioteca/biblioteca` keeps the `biblioteca/...` prefix that
/// book-curator's quarantine folders use
pub fn quarantine_rel(library_path: &Path, book: &Path) -> PathBuf {
	let base = library_path.parent().unwrap_or(library_path);
	book.strip_prefix(base)
		.map(Path::to_path_buf)
		.unwrap_or_else(|_| {
			book.file_name()
				.map(PathBuf::from)
				.unwrap_or_else(|| PathBuf::from("book"))
		})
}

fn copy_synced(from: &Path, to: &Path) -> io::Result<u64> {
	let part = to.with_file_name(format!(
		".{}.part",
		to.file_name().and_then(|n| n.to_str()).unwrap_or("copy")
	));
	let bytes = fs::copy(from, &part)?;
	File::open(&part)?.sync_all()?;
	fs::rename(&part, to)?;
	Ok(bytes)
}

/// Replace `original` with `new_file` (left in place: the caller removes its own work file).
///
/// 1. the original is copied to `batch/rel` (`.prevN` added when that name is taken) and the
///    copy's size is checked;
/// 2. the new file is copied next to the original under a hidden name (scans skip hidden
///    files), given the original's permissions, and renamed over it (atomic on one filesystem);
/// 3. a row is appended to `batch/restore_log.csv`.
///
/// If anything fails before step 2's rename, the original is untouched
pub fn swap_into_place(
	original: &Path,
	new_file: &Path,
	batch: &Path,
	rel: &Path,
) -> io::Result<SwapOutcome> {
	let meta = fs::metadata(original)?;
	let bytes_before = meta.len();
	let bytes_after = fs::metadata(new_file)?.len();

	let mut quarantine_path = batch.join(rel);
	let mut n = 1;
	while quarantine_path.exists() {
		let stem = rel
			.file_stem()
			.and_then(|s| s.to_str())
			.unwrap_or("book")
			.to_string();
		let ext = rel
			.extension()
			.and_then(|s| s.to_str())
			.map(|e| format!(".{e}"))
			.unwrap_or_default();
		quarantine_path = batch
			.join(rel)
			.with_file_name(format!("{stem}.prev{n}{ext}"));
		n += 1;
	}
	if let Some(parent) = quarantine_path.parent() {
		fs::create_dir_all(parent)?;
	}
	let copied = copy_synced(original, &quarantine_path)?;
	if copied != bytes_before {
		return Err(io::Error::other(format!(
			"quarantine copy has {copied} bytes, the original {bytes_before}"
		)));
	}

	let staged = original.with_file_name(format!(
		".{}.stump-new",
		original
			.file_name()
			.and_then(|n| n.to_str())
			.unwrap_or("book")
	));
	let result = (|| {
		copy_synced(new_file, &staged)?;
		fs::set_permissions(&staged, meta.permissions())?;
		fs::rename(&staged, original)
	})();
	if let Err(error) = result {
		let _ = fs::remove_file(&staged);
		return Err(error);
	}

	let log = batch.join("restore_log.csv");
	let new_log = !log.exists();
	let mut fh = OpenOptions::new().create(true).append(true).open(&log)?;
	if new_log {
		writeln!(fh, "original_path,quarantine_path,bytes_before,bytes_after")?;
	}
	writeln!(
		fh,
		"{},{},{bytes_before},{bytes_after}",
		csv_field(&original.to_string_lossy()),
		csv_field(&quarantine_path.to_string_lossy()),
	)?;

	Ok(SwapOutcome {
		quarantine_path,
		bytes_before,
		bytes_after,
	})
}

fn csv_field(value: &str) -> String {
	if value.contains([',', '"', '\n']) {
		format!("\"{}\"", value.replace('"', "\"\""))
	} else {
		value.to_string()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn quarantine_keeps_library_folder_prefix() {
		assert_eq!(
			quarantine_rel(
				Path::new("/biblioteca/biblioteca"),
				Path::new("/biblioteca/biblioteca/_Manga/Yotsuba&!/Yotsuba&! v01.cbz")
			),
			PathBuf::from("biblioteca/_Manga/Yotsuba&!/Yotsuba&! v01.cbz")
		);
	}

	#[test]
	fn swap_moves_original_to_quarantine_and_logs_it() {
		let dir = tempfile::tempdir().unwrap();
		let lib = dir.path().join("lib/biblioteca/_Manga/S");
		fs::create_dir_all(&lib).unwrap();
		let original = lib.join("S, v01.cbz");
		fs::write(&original, b"original bytes").unwrap();
		let new_file = dir.path().join("work.cbz");
		fs::write(&new_file, b"new").unwrap();
		let batch = dir.path().join("q/2026-11-02-stump-grey16");
		let rel = quarantine_rel(&dir.path().join("lib/biblioteca"), &original);

		let out = swap_into_place(&original, &new_file, &batch, &rel).unwrap();
		assert_eq!(fs::read(&original).unwrap(), b"new");
		assert_eq!(fs::read(&out.quarantine_path).unwrap(), b"original bytes");
		assert_eq!((out.bytes_before, out.bytes_after), (14, 3));
		assert!(out
			.quarantine_path
			.ends_with("biblioteca/_Manga/S/S, v01.cbz"));
		// no hidden leftovers next to the book
		assert_eq!(fs::read_dir(&lib).unwrap().count(), 1);

		// a second swap of the same book keeps both originals
		fs::write(&new_file, b"newer").unwrap();
		let out2 = swap_into_place(&original, &new_file, &batch, &rel).unwrap();
		assert!(out2
			.quarantine_path
			.to_string_lossy()
			.ends_with("S, v01.prev1.cbz"));
		let log = fs::read_to_string(batch.join("restore_log.csv")).unwrap();
		let lines: Vec<_> = log.lines().collect();
		assert_eq!(
			lines[0],
			"original_path,quarantine_path,bytes_before,bytes_after"
		);
		assert_eq!(lines.len(), 3);
		assert!(lines[1].starts_with('"'), "paths with commas are quoted");
	}

	#[test]
	fn failed_swap_leaves_original() {
		let dir = tempfile::tempdir().unwrap();
		let original = dir.path().join("book.pdf");
		fs::write(&original, b"original").unwrap();
		let missing = dir.path().join("missing.pdf");
		let batch = dir.path().join("q");
		let result = swap_into_place(&original, &missing, &batch, Path::new("book.pdf"));
		assert!(result.is_err());
		assert_eq!(fs::read(&original).unwrap(), b"original");
	}
}
