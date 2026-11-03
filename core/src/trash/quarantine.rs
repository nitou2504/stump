//! Moving a deleted book's file into the quarantine folder, and back. Book files are never
//! destroyed: a permanent deletion moves the file to `<quarantine>/deleted/<YYYY-MM-DD>/<its
//! path in the library>` and appends a row to that day's `restore_log.csv`.

use std::{
	fs::{self, File, OpenOptions},
	io::{self, Write},
	path::{Component, Path, PathBuf},
};

use chrono::{DateTime, Utc};

/// The columns of `restore_log.csv`
pub const RESTORE_LOG_HEADER: &str =
	"original_path,quarantine_path,bytes,deleted_by,deleted_at,title,authors";

/// `<root>/deleted/<YYYY-MM-DD>`: where the books deleted permanently that day (UTC) go
pub fn day_dir(root: &Path, at: DateTime<Utc>) -> PathBuf {
	root.join("deleted").join(at.format("%Y-%m-%d").to_string())
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
fn exists(path: &Path) -> bool {
	fs::symlink_metadata(path).is_ok()
}

/// Moves a file, never over another one. A rename when both paths are on one file system;
/// otherwise the file is copied under a hidden name next to its destination, synced, checked
/// for size, given the original's modification time and renamed into place, and only then is
/// the original removed. On any error the original is left where it was. Returns the size.
pub fn move_file(from: &Path, to: &Path) -> io::Result<u64> {
	let meta = fs::metadata(from)?;
	if !meta.is_file() {
		return Err(io::Error::other(format!(
			"{} is not a file",
			from.display()
		)));
	}
	let bytes = meta.len();
	if exists(to) {
		return Err(io::Error::new(
			io::ErrorKind::AlreadyExists,
			format!("{} already exists", to.display()),
		));
	}
	if let Some(parent) = to.parent() {
		fs::create_dir_all(parent)?;
	}
	match fs::rename(from, to) {
		Ok(()) => return Ok(bytes),
		// different mounts (the library and the quarantine folder usually are)
		Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {},
		Err(error) => return Err(error),
	}

	let part = to.with_file_name(format!(
		".{}.part",
		to.file_name().and_then(|n| n.to_str()).unwrap_or("book")
	));
	let copied = (|| {
		let copied = fs::copy(from, &part)?;
		let file = File::options().write(true).open(&part)?;
		if let Ok(modified) = meta.modified() {
			let _ = file.set_modified(modified);
		}
		file.sync_all()?;
		if copied != bytes {
			return Err(io::Error::other(format!(
				"the copy has {copied} bytes, the original {bytes}"
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
	if let Err(error) = copied {
		let _ = fs::remove_file(&part);
		return Err(error);
	}
	if let Err(error) = fs::remove_file(from) {
		// keep the original, drop the copy
		let _ = fs::remove_file(to);
		return Err(error);
	}
	Ok(bytes)
}

/// One row of `restore_log.csv`
#[derive(Debug, Clone)]
pub struct RestoreLogRow<'a> {
	pub original_path: &'a str,
	pub quarantine_path: &'a str,
	pub bytes: u64,
	pub deleted_by: &'a str,
	pub deleted_at: &'a str,
	pub title: &'a str,
	pub authors: &'a str,
}

/// Appends a row to `dir/restore_log.csv`, writing the header first in a new file
pub fn append_restore_log(dir: &Path, row: &RestoreLogRow<'_>) -> io::Result<()> {
	fs::create_dir_all(dir)?;
	let log = dir.join("restore_log.csv");
	let new_log = !log.exists();
	let mut line = String::new();
	if new_log {
		line.push_str(RESTORE_LOG_HEADER);
		line.push('\n');
	}
	let bytes = row.bytes.to_string();
	let fields = [
		row.original_path,
		row.quarantine_path,
		bytes.as_str(),
		row.deleted_by,
		row.deleted_at,
		row.title,
		row.authors,
	];
	line.push_str(
		&fields
			.iter()
			.map(|field| csv_field(field))
			.collect::<Vec<_>>()
			.join(","),
	);
	line.push('\n');
	// one write, so a row is never split by another writer
	let mut fh = OpenOptions::new().create(true).append(true).open(&log)?;
	fh.write_all(line.as_bytes())?;
	fh.sync_all()
}

fn csv_field(value: &str) -> String {
	if value.contains([',', '"', '\n', '\r']) {
		format!("\"{}\"", value.replace('"', "\"\""))
	} else {
		value.to_string()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_day_dir_and_library_relative_path() {
		let at = "2026-11-04T03:59:00Z".parse().unwrap();
		assert_eq!(
			day_dir(Path::new("/quarantine"), at),
			PathBuf::from("/quarantine/deleted/2026-11-04")
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
	fn test_move_file_keeps_the_bytes_and_never_overwrites() {
		let dir = tempfile::tempdir().unwrap();
		let from = dir.path().join("lib/Author/Book.epub");
		fs::create_dir_all(from.parent().unwrap()).unwrap();
		fs::write(&from, b"book bytes").unwrap();
		let to = dir.path().join("q/deleted/2026-11-04/Author/Book.epub");

		assert_eq!(move_file(&from, &to).unwrap(), 10);
		assert!(!from.exists());
		assert_eq!(fs::read(&to).unwrap(), b"book bytes");

		// back, but something new took the original's place: nothing moves
		fs::write(&from, b"another book").unwrap();
		let error = move_file(&to, &from).unwrap_err();
		assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
		assert_eq!(fs::read(&from).unwrap(), b"another book");
		assert_eq!(fs::read(&to).unwrap(), b"book bytes");

		// a missing file is an error, not a no-op
		let missing = dir.path().join("lib/missing.epub");
		assert_eq!(
			move_file(&missing, &dir.path().join("q/x.epub"))
				.unwrap_err()
				.kind(),
			io::ErrorKind::NotFound
		);
	}

	#[test]
	fn test_restore_log_has_a_header_and_quotes_fields() {
		let dir = tempfile::tempdir().unwrap();
		let row = RestoreLogRow {
			original_path: "/lib/Eco, Umberto/El nombre.epub",
			quarantine_path: "/q/deleted/2026-11-04/Eco, Umberto/El nombre.epub",
			bytes: 1234,
			deleted_by: "g3",
			deleted_at: "2026-10-21T16:06:19+00:00",
			title: "El nombre de la \"rosa\"",
			authors: "Umberto Eco",
		};
		append_restore_log(dir.path(), &row).unwrap();
		append_restore_log(dir.path(), &row).unwrap();
		let log = fs::read_to_string(dir.path().join("restore_log.csv")).unwrap();
		let lines: Vec<_> = log.lines().collect();
		assert_eq!(lines.len(), 3);
		assert_eq!(lines[0], RESTORE_LOG_HEADER);
		assert_eq!(
			lines[1],
			"\"/lib/Eco, Umberto/El nombre.epub\",\"/q/deleted/2026-11-04/Eco, Umberto/El nombre.epub\",1234,g3,2026-10-21T16:06:19+00:00,\"El nombre de la \"\"rosa\"\"\",Umberto Eco"
		);
	}
}
