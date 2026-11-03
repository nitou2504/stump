//! The file side of *Deleted books*: where a held file goes in Stump's trash folder, copying a
//! file there (and back) safely, and deleting a book file for good.

use std::{
	fs::{self, File},
	io,
	path::{Component, Path, PathBuf},
};

use chrono::{DateTime, Utc};

use crate::scan::own_writes;

/// `<trash>/<YYYY-MM-DD>`: where the files held that day (UTC) go
pub fn day_dir(root: &Path, at: DateTime<Utc>) -> PathBuf {
	root.join(at.format("%Y-%m-%d").to_string())
}

/// The book's path inside its library (`Author/Title.epub`), or its file name when it isn't
/// below the library's folder. Never absolute and never climbing out with `..`
pub fn library_relative(library_path: &Path, book: &Path) -> PathBuf {
	let file_name = || {
		book.file_name()
			.map(PathBuf::from)
			.unwrap_or_else(|| PathBuf::from("book"))
	};
	match book.strip_prefix(library_path) {
		Ok(rel)
			if !rel.as_os_str().is_empty()
				&& rel.components().all(|c| matches!(c, Component::Normal(_))) =>
		{
			rel.to_path_buf()
		},
		_ => file_name(),
	}
}

/// `dir/rel` if nothing is there yet, else `<stem>.prev1.<ext>`, `.prev2`, ... next to it
pub fn free_path(dir: &Path, rel: &Path) -> PathBuf {
	let first = dir.join(rel);
	if !exists(&first) {
		return first;
	}
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
	(1..)
		.map(|n| first.with_file_name(format!("{stem}.prev{n}{ext}")))
		.find(|candidate| !exists(candidate))
		.expect("an unbounded range always finds a free name")
}

/// Whether anything (even a dangling symlink) is at `path`
pub fn exists(path: &Path) -> bool {
	fs::symlink_metadata(path).is_ok()
}

/// A hidden name next to `path` (scans and the library watcher skip hidden files)
pub fn hidden_sibling(path: &Path, tag: &str) -> PathBuf {
	path.with_file_name(format!(
		".{}.{tag}",
		path.file_name().and_then(|n| n.to_str()).unwrap_or("book")
	))
}

/// Copies `from` to `to`, which must not exist yet: the copy is written under a hidden name
/// next to `to`, keeps the modification time, is synced, checked for size and then renamed
/// into place. Returns the size. On any error nothing is left at `to`
pub fn copy_new(from: &Path, to: &Path) -> io::Result<u64> {
	let meta = fs::metadata(from)?;
	if !meta.is_file() {
		return Err(io::Error::other(format!(
			"{} is not a file",
			from.display()
		)));
	}
	if exists(to) {
		return Err(io::Error::new(
			io::ErrorKind::AlreadyExists,
			format!("{} already exists", to.display()),
		));
	}
	if let Some(parent) = to.parent() {
		fs::create_dir_all(parent)?;
	}
	let part = hidden_sibling(to, "part");
	let copied = (|| {
		let copied = fs::copy(from, &part)?;
		let file = File::options().write(true).open(&part)?;
		if let Ok(modified) = meta.modified() {
			let _ = file.set_modified(modified);
		}
		file.sync_all()?;
		if copied != meta.len() {
			return Err(io::Error::other(format!(
				"the copy has {copied} bytes, the original {}",
				meta.len()
			)));
		}
		if exists(to) {
			return Err(io::Error::new(
				io::ErrorKind::AlreadyExists,
				format!("{} already exists", to.display()),
			));
		}
		fs::rename(&part, to)
	})();
	match copied {
		Ok(()) => Ok(meta.len()),
		Err(error) => {
			let _ = fs::remove_file(&part);
			Err(error)
		},
	}
}

/// Puts `from` in place of the library file `target` (which may exist): a synced copy under a
/// hidden name next to it, with `target`'s permissions when it has some, renamed over it. The
/// library watcher knows the change is Stump's own. `from` is left where it is
pub fn replace_in_library(from: &Path, target: &Path) -> io::Result<u64> {
	let staged = hidden_sibling(target, "stump-put-back");
	let permissions = fs::metadata(target).ok().map(|m| m.permissions());
	let _ = fs::remove_file(&staged);
	let bytes = copy_new(from, &staged)?;
	let placed = (|| {
		if let Some(permissions) = permissions {
			fs::set_permissions(&staged, permissions)?;
		}
		own_writes::expect(target);
		fs::rename(&staged, target)
	})();
	match placed {
		Ok(()) => {
			own_writes::wrote(target);
			if let Some(dir) = target.parent() {
				let _ = File::open(dir).and_then(|d| d.sync_all());
			}
			Ok(bytes)
		},
		Err(error) => {
			own_writes::forget(target);
			let _ = fs::remove_file(&staged);
			Err(error)
		},
	}
}

/// A book file on its way out of the library: renamed to a hidden name next to itself (scans
/// skip it from then on), so the book's data can be deleted before the file is, and the file
/// can go back if that fails
pub struct Doomed {
	original: PathBuf,
	hidden: PathBuf,
	pub bytes: u64,
}

impl Doomed {
	pub fn hide(path: &Path) -> io::Result<Self> {
		let bytes = fs::metadata(path)?.len();
		let hidden = hidden_sibling(path, "stump-deleting");
		if exists(&hidden) {
			fs::remove_file(&hidden)?;
		}
		own_writes::expect(path);
		if let Err(error) = fs::rename(path, &hidden) {
			own_writes::forget(path);
			return Err(error);
		}
		own_writes::wrote(path);
		Ok(Self {
			original: path.to_path_buf(),
			hidden,
			bytes,
		})
	}

	/// Puts the file back where it was (its book stays)
	pub fn restore(self) -> io::Result<()> {
		own_writes::expect(&self.original);
		let result = fs::rename(&self.hidden, &self.original);
		own_writes::wrote(&self.original);
		result
	}

	/// Deletes the file for good
	pub fn delete(self) -> io::Result<()> {
		fs::remove_file(&self.hidden)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_day_dir_and_library_relative_path() {
		let at = "2026-11-04T03:59:00Z".parse().unwrap();
		assert_eq!(
			day_dir(Path::new("/config/trash"), at),
			PathBuf::from("/config/trash/2026-11-04")
		);
		assert_eq!(
			library_relative(
				Path::new("/biblioteca/biblioteca"),
				Path::new("/biblioteca/biblioteca/Umberto_Eco/El_nombre.epub")
			),
			PathBuf::from("Umberto_Eco/El_nombre.epub")
		);
		// outside the library, or climbing out of it: only the file name
		assert_eq!(
			library_relative(Path::new("/lib"), Path::new("/elsewhere/Book.pdf")),
			PathBuf::from("Book.pdf")
		);
		assert_eq!(
			library_relative(Path::new("/lib"), Path::new("/lib/../etc/Book.pdf")),
			PathBuf::from("Book.pdf")
		);
	}

	#[test]
	fn test_free_path_never_reuses_a_name() {
		let dir = tempfile::tempdir().unwrap();
		let rel = Path::new("A/Book.epub");
		let first = free_path(dir.path(), rel);
		assert_eq!(first, dir.path().join("A/Book.epub"));
		fs::create_dir_all(first.parent().unwrap()).unwrap();
		fs::write(&first, b"1").unwrap();
		let second = free_path(dir.path(), rel);
		assert_eq!(second, dir.path().join("A/Book.prev1.epub"));
		fs::write(&second, b"2").unwrap();
		assert_eq!(
			free_path(dir.path(), rel),
			dir.path().join("A/Book.prev2.epub")
		);
	}

	#[test]
	fn test_copy_new_keeps_the_bytes_and_never_overwrites() {
		let dir = tempfile::tempdir().unwrap();
		let from = dir.path().join("lib/Author/Book.epub");
		fs::create_dir_all(from.parent().unwrap()).unwrap();
		fs::write(&from, b"book bytes").unwrap();
		let to = dir.path().join("trash/2026-11-04/Author/Book.epub");

		assert_eq!(copy_new(&from, &to).unwrap(), 10);
		assert_eq!(fs::read(&from).unwrap(), b"book bytes");
		assert_eq!(fs::read(&to).unwrap(), b"book bytes");
		assert_eq!(
			fs::metadata(&to).unwrap().modified().unwrap(),
			fs::metadata(&from).unwrap().modified().unwrap()
		);

		// never over another file
		let error = copy_new(&from, &to).unwrap_err();
		assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
		// no hidden leftovers
		assert_eq!(fs::read_dir(to.parent().unwrap()).unwrap().count(), 1);

		let missing = dir.path().join("lib/missing.epub");
		assert_eq!(
			copy_new(&missing, &dir.path().join("trash/x.epub"))
				.unwrap_err()
				.kind(),
			io::ErrorKind::NotFound
		);
	}

	#[test]
	fn test_replace_in_library_and_doomed_files() {
		let dir = tempfile::tempdir().unwrap();
		let book = dir.path().join("lib/Author/Book.cbz");
		fs::create_dir_all(book.parent().unwrap()).unwrap();
		fs::write(&book, b"processed").unwrap();
		let held = dir.path().join("trash/Book.cbz");
		fs::create_dir_all(held.parent().unwrap()).unwrap();
		fs::write(&held, b"the original").unwrap();

		assert_eq!(replace_in_library(&held, &book).unwrap(), 12);
		assert_eq!(fs::read(&book).unwrap(), b"the original");
		assert!(held.exists(), "the held copy stays until its row goes");
		assert_eq!(fs::read_dir(book.parent().unwrap()).unwrap().count(), 1);

		// hidden, then back
		let doomed = Doomed::hide(&book).unwrap();
		assert!(!book.exists());
		assert_eq!(doomed.bytes, 12);
		doomed.restore().unwrap();
		assert_eq!(fs::read(&book).unwrap(), b"the original");

		// hidden, then deleted for good
		Doomed::hide(&book).unwrap().delete().unwrap();
		assert!(!book.exists());
		assert_eq!(fs::read_dir(book.parent().unwrap()).unwrap().count(), 0);
	}
}
