//! Stump's own changes to files in library folders: a metadata edit renamed over a book, a
//! post-processed file swapped in, a book filed by *Add books*, an original put back from
//! *Deleted books*, a book file deleted permanently. Each of them already updates the book in
//! the database (or enqueues the scan it needs), so the library watcher must not answer it with
//! a scan of the whole library.
//!
//! Stump registers a path right before it changes it ([`expect`]) and records what it left
//! there right after ([`wrote`]: the file's size and modification time, a folder, or nothing).
//! When the watcher flushes its batch of changed paths, a path that is still exactly what Stump
//! left there is not a reason to scan; a file changed again since (another program rewrote it)
//! still is. Hidden files and folders in a library (Stump's work copies, `.part` files) never
//! are: scans skip them anyway.

use std::{
	collections::HashMap,
	io,
	path::{Component, Path, PathBuf},
	sync::{LazyLock, Mutex},
	time::{Duration, Instant, SystemTime},
};

/// How long a registered change is recognised: the watcher flushes its batch a few seconds
/// after the last event, so this is generous
const TTL: Duration = Duration::from_secs(120);

/// What Stump left at a path
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expected {
	/// Stump is changing it
	Pending,
	/// A file with this size and modification time
	File {
		len: u64,
		modified: Option<SystemTime>,
	},
	/// A folder (its modification time follows the files added to it)
	Folder,
	/// Nothing: Stump removed it
	Absent,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
	expected: Expected,
	until: Instant,
}

static WRITES: LazyLock<Mutex<HashMap<PathBuf, Entry>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

fn insert(path: &Path, expected: Expected) {
	if let Ok(mut writes) = WRITES.lock() {
		let now = Instant::now();
		writes.retain(|_, entry| entry.until > now);
		writes.insert(
			path.to_path_buf(),
			Entry {
				expected,
				until: now + TTL,
			},
		);
	}
}

/// What is at `path` now
fn observe(path: &Path) -> io::Result<Expected> {
	match std::fs::symlink_metadata(path) {
		Ok(meta) if meta.is_dir() => Ok(Expected::Folder),
		Ok(meta) => Ok(Expected::File {
			len: meta.len(),
			modified: meta.modified().ok(),
		}),
		Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Expected::Absent),
		Err(error) => Err(error),
	}
}

/// Registers a path Stump is about to change (write, replace, create or remove)
pub fn expect(path: &Path) {
	insert(path, Expected::Pending);
}

/// Records what Stump left at `path` once it changed it
pub fn wrote(path: &Path) {
	insert(path, observe(path).unwrap_or(Expected::Pending));
}

/// Forgets a registered path: the change failed and left nothing new there
pub fn forget(path: &Path) {
	if let Ok(mut writes) = WRITES.lock() {
		writes.remove(path);
	}
}

/// Whether `path` is hidden inside `root` (a component of its path below `root` starts with a
/// dot, or is a macOS resource folder): scans skip it
pub fn is_hidden_in(root: &Path, path: &Path) -> bool {
	path.strip_prefix(root).is_ok_and(|rel| {
		rel.components().any(|component| match component {
			Component::Normal(name) => {
				let name = name.to_string_lossy();
				name.starts_with('.') || name == "__MACOSX"
			},
			_ => false,
		})
	})
}

/// Whether a change the watcher saw at `path`, in the library at `root`, was Stump's own and
/// needs no scan: a hidden path, or a path still exactly as Stump left it
pub fn is_own(root: &Path, path: &Path) -> bool {
	if is_hidden_in(root, path) {
		return true;
	}
	let entry = match WRITES.lock() {
		Ok(mut writes) => {
			let now = Instant::now();
			writes.retain(|_, entry| entry.until > now);
			writes.get(path).copied()
		},
		Err(_) => None,
	};
	let Some(entry) = entry else {
		return false;
	};
	match entry.expected {
		Expected::Pending => true,
		Expected::Folder => matches!(observe(path), Ok(Expected::Folder)),
		expected => observe(path).is_ok_and(|now| now == expected),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_hidden_paths_are_never_a_reason_to_scan() {
		let root = Path::new("/lib/biblioteca");
		assert!(is_hidden_in(
			root,
			Path::new("/lib/biblioteca/A/.stump-edit.B.epub")
		));
		assert!(is_hidden_in(
			root,
			Path::new("/lib/biblioteca/.cache/B.epub")
		));
		assert!(is_hidden_in(
			root,
			Path::new("/lib/biblioteca/A/__MACOSX/B.cbz")
		));
		assert!(!is_hidden_in(root, Path::new("/lib/biblioteca/A/B.epub")));
		// a hidden folder above the library is not the library's business
		assert!(!is_hidden_in(
			Path::new("/home/u/.books"),
			Path::new("/home/u/.books/A/B.epub")
		));
		assert!(is_own(root, Path::new("/lib/biblioteca/A/.B.epub.part")));
	}

	#[test]
	fn test_own_writes_until_another_program_changes_the_file() {
		let dir = tempfile::tempdir().unwrap();
		let root = dir.path();
		let book = root.join("Author/Book.epub");
		std::fs::create_dir_all(book.parent().unwrap()).unwrap();
		std::fs::write(&book, b"v1").unwrap();
		assert!(!is_own(root, &book), "nobody registered it");

		expect(&book);
		assert!(is_own(root, &book), "being written by Stump");
		std::fs::write(&book, b"v2 from stump").unwrap();
		wrote(&book);
		assert!(is_own(root, &book), "exactly as Stump left it");

		// another program rewrites it afterwards: that change needs a scan
		std::fs::write(&book, b"v3 from someone else, longer").unwrap();
		assert!(!is_own(root, &book));

		// a removal Stump made, then a file that appears there again
		expect(&book);
		std::fs::remove_file(&book).unwrap();
		wrote(&book);
		assert!(is_own(root, &book));
		std::fs::write(&book, b"back").unwrap();
		assert!(!is_own(root, &book));

		// a folder Stump created stays its own while files are added to it
		let folder = root.join("New_Author");
		expect(&folder);
		std::fs::create_dir_all(&folder).unwrap();
		wrote(&folder);
		std::fs::write(folder.join("x.epub"), b"x").unwrap();
		assert!(is_own(root, &folder));

		// a failed change is forgotten
		let other = root.join("Author/Other.epub");
		expect(&other);
		forget(&other);
		std::fs::write(&other, b"x").unwrap();
		assert!(!is_own(root, &other));
	}
}
