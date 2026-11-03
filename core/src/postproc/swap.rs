//! Replacing a library file with its processed version. The original is first copied into
//! Stump's trash folder (see [`crate::trash::held`]), where *Deleted books* lists it for the
//! retention and it can be put back; then the processed file takes its place.

use std::{
	fs::{self, File},
	io,
	path::{Path, PathBuf},
};

use crate::{scan::own_writes, trash::files};

/// Where an original went
#[derive(Debug, Clone)]
pub struct SwapOutcome {
	/// The held copy of the original (in the trash folder)
	pub held_path: PathBuf,
	pub bytes_before: u64,
	pub bytes_after: u64,
}

fn copy_synced(from: &Path, to: &Path) -> io::Result<u64> {
	let part = files::hidden_sibling(to, "part");
	let bytes = fs::copy(from, &part)?;
	File::open(&part)?.sync_all()?;
	fs::rename(&part, to)?;
	Ok(bytes)
}

/// Replace `original` with `new_file` (left in place: the caller removes its own work file).
///
/// 1. the original is copied to `held` (a free path in the trash folder: see
///    [`crate::trash::held::destination`]), synced and checked for size;
/// 2. the new file is copied next to the original under a hidden name (scans skip hidden
///    files), given the original's permissions, and renamed over it (atomic on one filesystem);
///    the library watcher knows the change is Stump's own.
///
/// If anything fails before step 2's rename, the original is untouched and the held copy is
/// removed
pub fn swap_into_place(
	original: &Path,
	new_file: &Path,
	held: &Path,
) -> io::Result<SwapOutcome> {
	let meta = fs::metadata(original)?;
	let bytes_before = meta.len();
	let bytes_after = fs::metadata(new_file)?.len();

	let copied = files::copy_new(original, held)?;
	if copied != bytes_before {
		let _ = fs::remove_file(held);
		return Err(io::Error::other(format!(
			"the held copy has {copied} bytes, the original {bytes_before}"
		)));
	}

	let staged = files::hidden_sibling(original, "stump-new");
	let result = (|| {
		copy_synced(new_file, &staged)?;
		fs::set_permissions(&staged, meta.permissions())?;
		own_writes::expect(original);
		fs::rename(&staged, original)
	})();
	if let Err(error) = result {
		own_writes::forget(original);
		let _ = fs::remove_file(&staged);
		let _ = fs::remove_file(held);
		return Err(error);
	}
	own_writes::wrote(original);
	if let Some(dir) = original.parent() {
		let _ = File::open(dir).and_then(|d| d.sync_all());
	}

	Ok(SwapOutcome {
		held_path: held.to_path_buf(),
		bytes_before,
		bytes_after,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn swap_holds_the_original_and_replaces_it() {
		let dir = tempfile::tempdir().unwrap();
		let lib = dir.path().join("lib/biblioteca/_Manga/S");
		fs::create_dir_all(&lib).unwrap();
		let original = lib.join("S, v01.cbz");
		fs::write(&original, b"original bytes").unwrap();
		let new_file = dir.path().join("work.cbz");
		fs::write(&new_file, b"new").unwrap();
		let held = dir
			.path()
			.join("config/trash/2026-11-05/_Manga/S/S, v01.cbz");

		let out = swap_into_place(&original, &new_file, &held).unwrap();
		assert_eq!(fs::read(&original).unwrap(), b"new");
		assert_eq!(fs::read(&out.held_path).unwrap(), b"original bytes");
		assert_eq!((out.bytes_before, out.bytes_after), (14, 3));
		// no hidden leftovers next to the book
		assert_eq!(fs::read_dir(&lib).unwrap().count(), 1);
		// the watcher sees Stump's own change
		assert!(own_writes::is_own(
			&dir.path().join("lib/biblioteca"),
			&original
		));

		// the held copy is never overwritten
		fs::write(&new_file, b"newer").unwrap();
		assert!(swap_into_place(&original, &new_file, &held).is_err());
		assert_eq!(fs::read(&original).unwrap(), b"new");
		assert_eq!(fs::read(&held).unwrap(), b"original bytes");
	}

	#[test]
	fn failed_swap_leaves_original() {
		let dir = tempfile::tempdir().unwrap();
		let original = dir.path().join("book.pdf");
		fs::write(&original, b"original").unwrap();
		let missing = dir.path().join("missing.pdf");
		let held = dir.path().join("trash/book.pdf");
		let result = swap_into_place(&original, &missing, &held);
		assert!(result.is_err());
		assert_eq!(fs::read(&original).unwrap(), b"original");
		assert!(!held.exists());
	}
}
