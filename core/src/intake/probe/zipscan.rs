//! A zip file, looked at as bytes before any zip library opens it.
//!
//! An EPUB, a CBZ and a pack of books are zip files, and two zip readers open every EPUB in Stump
//! (Stump's own, and the one inside the `epub` crate): a file that looks different to each is one
//! they can be made to disagree about. So the structure is checked first, by Stump's own code, from
//! the bytes: exactly one end-of-central-directory record that ends where the file ends, a central
//! directory that fits in the file and holds no more entries than the limit, local headers that say
//! what the directory says, and entries whose data lie one after another, never over each other (the
//! way a few KiB of deflate is made to inflate to gigabytes, by entries that share their data).
//!
//! What a file *says* is then checked against what it *is*: every entry is unpacked once, through a
//! counter, and its real size must be the size it declared, within the limits and the ratio.

use std::{
	fs::File,
	io::{Read, Seek, SeekFrom},
	path::Path,
};

use zip::ZipArchive;

use super::contract::{code, Limits, Reason};

const MIB: u64 = 1024 * 1024;
/// The central directory is read whole: it is held to this
const MAX_DIRECTORY_BYTES: u64 = 64 * MIB;
/// The longest end-of-central-directory record: 22 bytes and a 65,535-byte comment
const MAX_EOCD_BYTES: u64 = 22 + 65_535;
/// The longest name an entry may have
const MAX_NAME_BYTES: usize = 1024;
/// The room data descriptors can take between entries
const DESCRIPTOR_ROOM: u64 = 24;

/// One entry of the central directory
#[derive(Debug, Clone)]
pub struct Entry {
	pub name: String,
	raw_name: Vec<u8>,
	pub method: u16,
	/// The compressed size and the size it declares
	pub compressed: u64,
	pub size: u64,
	offset: u64,
	pub is_dir: bool,
}

/// What a scan found
#[derive(Debug, Clone, Default)]
pub struct ZipReport {
	pub entries: Vec<Entry>,
	/// The entries that are files
	pub files: u64,
	/// What the files really unpack to, counted as they were unpacked
	pub unpacked_bytes: u64,
	pub max_entry_bytes: u64,
	pub warnings: Vec<Reason>,
}

fn reject(code: &str, detail: impl Into<String>) -> Reason {
	Reason::new(code, detail)
}

fn u16_at(buf: &[u8], at: usize) -> Option<u16> {
	let bytes = buf.get(at..at.checked_add(2)?)?;
	Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(buf: &[u8], at: usize) -> Option<u32> {
	let bytes = buf.get(at..at.checked_add(4)?)?;
	Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn u64_at(buf: &[u8], at: usize) -> Option<u64> {
	let bytes = buf.get(at..at.checked_add(8)?)?;
	let mut array = [0u8; 8];
	array.copy_from_slice(bytes);
	Some(u64::from_le_bytes(array))
}

fn truncated() -> Reason {
	reject(code::TRUNCATED, "The zip file is cut short or damaged")
}

/// Where the central directory is, from the end of the file
struct Directory {
	offset: u64,
	size: u64,
	entries: u64,
	/// Where the records that follow the directory start (its end must not pass it)
	limit: u64,
}

fn read_exact_at(file: &mut File, at: u64, len: usize) -> Result<Vec<u8>, Reason> {
	file.seek(SeekFrom::Start(at)).map_err(|_| truncated())?;
	let mut buf = vec![0u8; len];
	file.read_exact(&mut buf).map_err(|_| truncated())?;
	Ok(buf)
}

fn locate_directory(file: &mut File, len: u64) -> Result<Directory, Reason> {
	if len < 22 {
		return Err(truncated());
	}
	let tail_len = len.min(MAX_EOCD_BYTES);
	let tail_start = len - tail_len;
	let tail = read_exact_at(file, tail_start, tail_len as usize)?;

	// exactly one record that ends where the file ends
	let mut found: Vec<usize> = vec![];
	for at in (0..=tail.len() - 22).rev() {
		if tail.get(at..at + 4) == Some(b"PK\x05\x06") {
			let comment = u16_at(&tail, at + 20).unwrap_or(u16::MAX) as usize;
			if at + 22 + comment == tail.len() {
				found.push(at);
			}
		}
	}
	let at = match found.as_slice() {
		[] => return Err(truncated()),
		[at] => *at,
		_ => {
			return Err(reject(
				code::AMBIGUOUS_ZIP,
				"The zip file has more than one end record",
			))
		},
	};
	let eocd_pos = tail_start + at as u64;
	let (disk, directory_disk) = (u16_at(&tail, at + 4), u16_at(&tail, at + 6));
	if disk != Some(0) || directory_disk != Some(0) {
		return Err(reject(code::UNREADABLE, "A zip file in several parts"));
	}
	let total16 = u16_at(&tail, at + 10).ok_or_else(truncated)?;
	let size32 = u32_at(&tail, at + 12).ok_or_else(truncated)?;
	let offset32 = u32_at(&tail, at + 16).ok_or_else(truncated)?;

	if total16 != u16::MAX && size32 != u32::MAX && offset32 != u32::MAX {
		return Ok(Directory {
			offset: u64::from(offset32),
			size: u64::from(size32),
			entries: u64::from(total16),
			limit: eocd_pos,
		});
	}

	// zip64: the locator sits right before the end record, and points at the zip64 record
	let locator_pos = eocd_pos.checked_sub(20).ok_or_else(truncated)?;
	let locator = read_exact_at(file, locator_pos, 20)?;
	if locator.get(0..4) != Some(b"PK\x06\x07") {
		return Err(truncated());
	}
	let record_pos = u64_at(&locator, 8).ok_or_else(truncated)?;
	if record_pos >= locator_pos {
		return Err(truncated());
	}
	let record = read_exact_at(file, record_pos, 56)?;
	if record.get(0..4) != Some(b"PK\x06\x06") {
		return Err(truncated());
	}
	Ok(Directory {
		entries: u64_at(&record, 32).ok_or_else(truncated)?,
		size: u64_at(&record, 40).ok_or_else(truncated)?,
		offset: u64_at(&record, 48).ok_or_else(truncated)?,
		limit: record_pos,
	})
}

/// The sizes and the offset of an entry whose 32-bit fields are saturated, from its zip64 extra
fn zip64_fields(extra: &[u8], size: &mut u64, compressed: &mut u64, offset: &mut u64) {
	let mut at = 0usize;
	while let (Some(id), Some(len)) = (u16_at(extra, at), u16_at(extra, at + 2)) {
		let body_start = at + 4;
		let Some(body_end) = body_start.checked_add(len as usize) else {
			return;
		};
		let Some(body) = extra.get(body_start..body_end) else {
			return;
		};
		if id == 1 {
			let mut cursor = 0usize;
			let mut next = |wanted: bool, field: &mut u64| {
				if wanted {
					if let Some(value) = u64_at(body, cursor) {
						*field = value;
					}
					cursor += 8;
				}
			};
			let (want_size, want_compressed, want_offset) = (
				*size == u64::from(u32::MAX),
				*compressed == u64::from(u32::MAX),
				*offset == u64::from(u32::MAX),
			);
			next(want_size, size);
			next(want_compressed, compressed);
			next(want_offset, offset);
			return;
		}
		at = body_end;
	}
}

/// Whether the name of an entry is one a book can have: relative, inside the archive, plain
fn name_problem(raw: &[u8], name: &str) -> Option<&'static str> {
	if raw.len() > MAX_NAME_BYTES {
		return Some("is longer than a name can be");
	}
	if raw.contains(&0) {
		return Some("has a NUL in it");
	}
	if raw.contains(&b'\\') {
		return Some("has a backslash in it");
	}
	if raw.first() == Some(&b'/') {
		return Some("is an absolute path");
	}
	if raw.get(1) == Some(&b':') && raw.first().is_some_and(u8::is_ascii_alphabetic) {
		return Some("starts with a drive");
	}
	let trimmed = name.trim_end_matches('/');
	if trimmed
		.split('/')
		.any(|part| part.is_empty() || part == "." || part == "..")
	{
		return Some("leaves its folder");
	}
	None
}

/// The bytes a nested archive starts with
fn is_nested_archive(head: &[u8]) -> bool {
	head.starts_with(b"PK\x03\x04")
		|| head.starts_with(b"PK\x05\x06")
		|| head.starts_with(b"Rar!\x1a\x07")
		|| head.starts_with(b"7z\xbc\xaf\x27\x1c")
		|| head.starts_with(b"\x1f\x8b")
		|| head.starts_with(b"\xfd7zXZ\x00")
		|| head.starts_with(b"%PDF-")
		|| (head.starts_with(b"BZh") && head.get(3).is_some_and(|b| (b'1'..=b'9').contains(b)))
}

/// Looks at the structure of a zip file, then unpacks every entry once through a counter
pub fn scan(path: &Path, limits: &Limits) -> Result<ZipReport, Reason> {
	let limits = limits.clamped();
	let mut file = File::open(path).map_err(|e| reject(code::UNREADABLE, e.to_string()))?;
	let len = file
		.metadata()
		.map_err(|e| reject(code::UNREADABLE, e.to_string()))?
		.len();
	let directory = locate_directory(&mut file, len)?;

	// Z2: how many, and how big, before anything is allocated
	if directory.entries > limits.max_entries {
		return Err(reject(
			code::TOO_MANY_ENTRIES,
			format!(
				"The zip holds {} entries: more than the {} allowed",
				directory.entries, limits.max_entries
			),
		));
	}
	if directory.size > MAX_DIRECTORY_BYTES
		|| directory.offset.checked_add(directory.size).map_or(true, |end| end > directory.limit)
		|| directory.entries.checked_mul(46).map_or(true, |least| least > directory.size)
	{
		return Err(reject(
			code::UNREADABLE,
			"The zip's directory doesn't fit in the file",
		));
	}
	let table = read_exact_at(&mut file, directory.offset, directory.size as usize)?;

	// Z3-Z5: every record
	let mut entries: Vec<Entry> = Vec::with_capacity(directory.entries as usize);
	let mut names = std::collections::HashSet::with_capacity(directory.entries as usize);
	let mut lowered = std::collections::HashSet::new();
	let mut warnings: Vec<Reason> = vec![];
	let (mut declared_total, mut files) = (0u64, 0u64);
	let mut at = 0usize;
	for _ in 0..directory.entries {
		if table.get(at..at + 4) != Some(b"PK\x01\x02") {
			return Err(reject(code::UNREADABLE, "A record of the directory is damaged"));
		}
		let made_by = u16_at(&table, at + 4).ok_or_else(truncated)?;
		let flags = u16_at(&table, at + 8).ok_or_else(truncated)?;
		let method = u16_at(&table, at + 10).ok_or_else(truncated)?;
		let mut compressed = u64::from(u32_at(&table, at + 20).ok_or_else(truncated)?);
		let mut size = u64::from(u32_at(&table, at + 24).ok_or_else(truncated)?);
		let name_len = u16_at(&table, at + 28).ok_or_else(truncated)? as usize;
		let extra_len = u16_at(&table, at + 30).ok_or_else(truncated)? as usize;
		let comment_len = u16_at(&table, at + 32).ok_or_else(truncated)? as usize;
		let external = u32_at(&table, at + 38).ok_or_else(truncated)?;
		let mut offset = u64::from(u32_at(&table, at + 42).ok_or_else(truncated)?);
		let name_at = at + 46;
		let extra_at = name_at + name_len;
		let record_end = extra_at
			.checked_add(extra_len)
			.and_then(|n| n.checked_add(comment_len))
			.ok_or_else(truncated)?;
		let raw_name = table
			.get(name_at..extra_at)
			.ok_or_else(truncated)?
			.to_vec();
		let extra = table
			.get(extra_at..extra_at + extra_len)
			.ok_or_else(truncated)?;
		if record_end > table.len() {
			return Err(truncated());
		}
		zip64_fields(extra, &mut size, &mut compressed, &mut offset);
		at = record_end;

		let name = String::from_utf8_lossy(&raw_name).into_owned();
		if let Some(problem) = name_problem(&raw_name, &name) {
			return Err(reject(
				code::BAD_PATH,
				format!("An entry's name {problem}: {}", printable(&name)),
			));
		}
		// encrypted (bit 0), strongly encrypted (bit 6) or masked (bit 13)
		if flags & (1 | 1 << 6 | 1 << 13) != 0 {
			return Err(reject(
				code::ENCRYPTED,
				format!("{} is encrypted", printable(&name)),
			));
		}
		if method != 0 && method != 8 {
			return Err(reject(
				code::UNSUPPORTED_COMPRESSION,
				format!("{} uses a compression Stump doesn't read", printable(&name)),
			));
		}
		if made_by >> 8 == 3 && (external >> 16) & 0o170000 == 0o120000 {
			return Err(reject(
				code::SYMLINK,
				format!("{} is a link", printable(&name)),
			));
		}
		if !names.insert(raw_name.clone()) {
			return Err(reject(
				code::DUPLICATE_NAME,
				format!("Two entries are called {}", printable(&name)),
			));
		}
		if !lowered.insert(name.to_lowercase()) {
			warnings.push(reject(
				code::CASE_DUPLICATE,
				format!("Two entries differ only by case: {}", printable(&name)),
			));
		}
		let is_dir = name.ends_with('/');
		if !is_dir {
			files += 1;
			if size > limits.max_entry_bytes {
				return Err(reject(
					code::ENTRY_TOO_LARGE,
					format!("{} is larger than the {} MB an entry can be", printable(&name), limits.max_entry_bytes / MIB),
				));
			}
			declared_total = declared_total.saturating_add(size);
			if declared_total > limits.max_unpacked_bytes {
				return Err(reject(
					code::UNPACKED_TOO_LARGE,
					format!("The zip unpacks to more than {} MB", limits.max_unpacked_bytes / MIB),
				));
			}
			if method == 0 && compressed != size {
				return Err(reject(
					code::HEADER_MISMATCH,
					format!("{} is stored, and its sizes differ", printable(&name)),
				));
			}
			if size >= MIB && (compressed == 0 || size / compressed > limits.max_ratio) {
				return Err(reject(
					code::RATIO,
					format!("{} inflates far more than a book does", printable(&name)),
				));
			}
		}
		entries.push(Entry {
			name,
			raw_name,
			method,
			compressed,
			size,
			offset,
			is_dir,
		});
	}
	if at != table.len() {
		return Err(reject(
			code::AMBIGUOUS_ZIP,
			"The zip's directory holds more than its records",
		));
	}

	// Z6: the local headers say what the directory says, and the data lie one after another
	let mut order: Vec<usize> = (0..entries.len()).collect();
	order.sort_by_key(|&i| entries[i].offset);
	if let Some(&first) = order.first() {
		if entries[first].offset != 0 {
			return Err(reject(
				code::AMBIGUOUS_ZIP,
				"There is something in the zip before its first entry",
			));
		}
	}
	let mut previous_end = 0u64;
	for (position, &i) in order.iter().enumerate() {
		let entry = &entries[i];
		if entry.offset < previous_end {
			return Err(reject(
				code::OVERLAPPING_ENTRIES,
				format!("{} lies over another entry", printable(&entry.name)),
			));
		}
		let header = read_exact_at(&mut file, entry.offset, 30)?;
		if header.get(0..4) != Some(b"PK\x03\x04") {
			return Err(reject(
				code::HEADER_MISMATCH,
				format!("{} has no entry where the directory says", printable(&entry.name)),
			));
		}
		let local_method = u16_at(&header, 8).ok_or_else(truncated)?;
		let local_name_len = u16_at(&header, 26).ok_or_else(truncated)? as usize;
		let local_extra_len = u16_at(&header, 28).ok_or_else(truncated)? as u64;
		if local_method != entry.method || local_name_len != entry.raw_name.len() {
			return Err(reject(
				code::HEADER_MISMATCH,
				format!("{} is described differently in two places", printable(&entry.name)),
			));
		}
		let local_name = read_exact_at(&mut file, entry.offset + 30, local_name_len)?;
		if local_name != entry.raw_name {
			return Err(reject(
				code::HEADER_MISMATCH,
				format!("{} is called something else in its own header", printable(&entry.name)),
			));
		}
		let data_start = entry
			.offset
			.checked_add(30 + local_name_len as u64 + local_extra_len)
			.ok_or_else(truncated)?;
		let data_end = data_start
			.checked_add(entry.compressed)
			.ok_or_else(truncated)?;
		if data_end > directory.offset {
			return Err(reject(
				code::OVERLAPPING_ENTRIES,
				format!("{} runs into the directory", printable(&entry.name)),
			));
		}
		if let Some(&next) = order.get(position + 1) {
			if entries[next].offset < data_end {
				return Err(reject(
					code::OVERLAPPING_ENTRIES,
					format!("{} lies over {}", printable(&entries[next].name), printable(&entry.name)),
				));
			}
			if entries[next].offset - data_end > 4096 {
				warnings.push(reject(
					code::AMBIGUOUS_ZIP,
					"There is unused space between entries",
				));
			}
		}
		previous_end = data_end;
	}
	if directory.offset.saturating_sub(previous_end) > 4096 + DESCRIPTOR_ROOM {
		return Err(reject(
			code::AMBIGUOUS_ZIP,
			"There is something in the zip after its last entry",
		));
	}

	// Z7: what is declared is what comes out
	let mut archive = ZipArchive::new(file)
		.map_err(|e| reject(code::UNREADABLE, format!("The zip can't be opened: {e}")))?;
	if archive.len() != entries.len() {
		return Err(reject(
			code::AMBIGUOUS_ZIP,
			"The zip reader sees another number of entries",
		));
	}
	let (mut actual_total, mut max_entry) = (0u64, 0u64);
	let mut buf = vec![0u8; 64 * 1024];
	for (i, entry) in entries.iter().enumerate() {
		if entry.is_dir {
			continue;
		}
		let mut reader = archive
			.by_index(i)
			.map_err(|e| reject(code::UNREADABLE, format!("{} can't be opened: {e}", printable(&entry.name))))?;
		if reader.name_raw() != entry.raw_name.as_slice() {
			return Err(reject(
				code::AMBIGUOUS_ZIP,
				"The zip reader sees an entry under another name",
			));
		}
		let mut counted = 0u64;
		let mut head = [0u8; 8];
		let mut head_len = 0usize;
		let cap = entry.size.saturating_add(1);
		let mut taken = (&mut reader).take(cap);
		loop {
			let n = taken
				.read(&mut buf)
				.map_err(|e| reject(code::UNREADABLE, format!("{} is damaged: {e}", printable(&entry.name))))?;
			if n == 0 {
				break;
			}
			if head_len < head.len() {
				let take = (head.len() - head_len).min(n);
				head[head_len..head_len + take].copy_from_slice(&buf[..take]);
				head_len += take;
			}
			counted += n as u64;
			if counted > entry.size {
				return Err(reject(
					code::SIZE_MISMATCH,
					format!("{} holds more than it says", printable(&entry.name)),
				));
			}
		}
		if counted != entry.size {
			return Err(reject(
				code::SIZE_MISMATCH,
				format!("{} holds less than it says", printable(&entry.name)),
			));
		}
		if is_nested_archive(&head[..head_len]) {
			return Err(reject(
				code::NESTED_ARCHIVE,
				format!("{} is an archive inside the book", printable(&entry.name)),
			));
		}
		actual_total = actual_total.saturating_add(counted);
		if actual_total > limits.max_unpacked_bytes {
			return Err(reject(
				code::UNPACKED_TOO_LARGE,
				format!("The zip unpacks to more than {} MB", limits.max_unpacked_bytes / MIB),
			));
		}
		max_entry = max_entry.max(counted);
	}

	Ok(ZipReport {
		entries,
		files,
		unpacked_bytes: actual_total,
		max_entry_bytes: max_entry,
		warnings,
	})
}

/// A name for a message: short, and with nothing in it that could spoil a line
pub fn printable(name: &str) -> String {
	name.chars()
		.filter(|c| !c.is_control())
		.take(120)
		.collect()
}

#[cfg(test)]
pub(crate) mod tests {
	use std::io::Write;

	use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

	use super::*;

	pub(crate) fn zip_of(entries: &[(&str, &[u8], CompressionMethod)]) -> Vec<u8> {
		let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
		for (name, data, method) in entries {
			writer
				.start_file(*name, SimpleFileOptions::default().compression_method(*method))
				.expect("start");
			writer.write_all(data).expect("write");
		}
		writer.finish().expect("finish").into_inner()
	}

	pub(crate) fn scan_bytes(bytes: &[u8], limits: &Limits) -> Result<ZipReport, Reason> {
		let dir = tempfile::tempdir().expect("dir");
		let path = dir.path().join("t.zip");
		std::fs::write(&path, bytes).expect("write");
		scan(&path, limits)
	}

	fn code_of(result: Result<ZipReport, Reason>) -> String {
		result.expect_err("refused").code
	}

	/// The offset of the n-th central record's field
	fn record_at(bytes: &[u8], n: usize) -> usize {
		let mut found = 0;
		let mut at = 0;
		while at + 4 <= bytes.len() {
			if &bytes[at..at + 4] == b"PK\x01\x02" {
				if found == n {
					return at;
				}
				found += 1;
			}
			at += 1;
		}
		panic!("no record {n}");
	}

	#[test]
	fn a_plain_zip_passes() {
		let bytes = zip_of(&[
			("mimetype", b"application/epub+zip", CompressionMethod::Stored),
			("a/b.txt", &[b'x'; 5000], CompressionMethod::Deflated),
			("c.bin", b"\x00\x01\x02", CompressionMethod::Stored),
		]);
		let report = scan_bytes(&bytes, &Limits::default()).expect("passes");
		assert_eq!(report.files, 3);
		assert_eq!(report.unpacked_bytes, 20 + 5000 + 3);
		assert_eq!(report.max_entry_bytes, 5000);
	}

	#[test]
	fn a_file_that_is_not_a_zip_or_is_cut_short_is_refused() {
		for bytes in [&b""[..], b"hello", b"PK\x03\x04 and nothing else at all, 22 bytes"] {
			assert_eq!(code_of(scan_bytes(bytes, &Limits::default())), code::TRUNCATED);
		}
		let whole = zip_of(&[("a", b"hello", CompressionMethod::Stored)]);
		assert_eq!(
			code_of(scan_bytes(&whole[..whole.len() - 6], &Limits::default())),
			code::TRUNCATED
		);
	}

	#[test]
	fn two_end_records_or_a_prefix_are_ambiguous() {
		let whole = zip_of(&[("a", b"hello", CompressionMethod::Stored)]);
		// a second end record inside the comment, ending exactly at the end of the file
		let mut two = whole.clone();
		let comment_at = two.len() - 2;
		let fake: Vec<u8> = {
			let mut f = b"PK\x05\x06".to_vec();
			f.extend_from_slice(&[0u8; 16]);
			f.extend_from_slice(&0u16.to_le_bytes());
			f
		};
		two[comment_at..comment_at + 2].copy_from_slice(&(fake.len() as u16).to_le_bytes());
		two.extend_from_slice(&fake);
		assert_eq!(code_of(scan_bytes(&two, &Limits::default())), code::AMBIGUOUS_ZIP);
		// something before the first entry (a self-extracting stub)
		let mut prefixed = b"MZ stub".to_vec();
		prefixed.extend_from_slice(&whole);
		assert!(scan_bytes(&prefixed, &Limits::default()).is_err());
	}

	#[test]
	fn too_many_entries_are_refused_before_the_directory_is_read() {
		let entries: Vec<(String, Vec<u8>)> =
			(0..6).map(|i| (format!("{i}.txt"), vec![b'a'; 10])).collect();
		let borrowed: Vec<(&str, &[u8], CompressionMethod)> = entries
			.iter()
			.map(|(n, d)| (n.as_str(), d.as_slice(), CompressionMethod::Stored))
			.collect();
		let bytes = zip_of(&borrowed);
		let limits = Limits {
			max_entries: 5,
			..Limits::default()
		};
		assert_eq!(code_of(scan_bytes(&bytes, &limits)), code::TOO_MANY_ENTRIES);
		// a directory that claims millions of entries in a few bytes is refused the same way
		let mut lying = bytes.clone();
		let eocd = lying.len() - 22;
		lying[eocd + 10..eocd + 12].copy_from_slice(&60_000u16.to_le_bytes());
		let result = scan_bytes(&lying, &Limits::default());
		assert!(result.is_err());
	}

	#[test]
	fn entries_that_inflate_too_much_are_refused() {
		// 24 MB of zeros deflate to about 24 KB: a ratio of a thousand
		let bytes = zip_of(&[(
			"bomb.bin",
			&vec![0u8; 24 * 1024 * 1024],
			CompressionMethod::Deflated,
		)]);
		assert!(bytes.len() < 400 * 1024);
		assert_eq!(code_of(scan_bytes(&bytes, &Limits::default())), code::RATIO);
		// and one that is larger than an entry may be, however well it packs
		let limits = Limits {
			max_ratio: 100_000,
			max_entry_bytes: 8 * MIB,
			..Limits::default()
		};
		assert_eq!(code_of(scan_bytes(&bytes, &limits)), code::ENTRY_TOO_LARGE);
		// the whole may not pass the total either
		let two = zip_of(&[
			("a.bin", &vec![7u8; 3 * 1024 * 1024], CompressionMethod::Stored),
			("b.bin", &vec![9u8; 3 * 1024 * 1024], CompressionMethod::Stored),
		]);
		let limits = Limits {
			max_unpacked_bytes: 5 * MIB,
			..Limits::default()
		};
		assert_eq!(code_of(scan_bytes(&two, &limits)), code::UNPACKED_TOO_LARGE);
	}

	#[test]
	fn what_an_entry_declares_is_what_it_must_be() {
		let bytes = zip_of(&[("a.txt", &[b'z'; 4000], CompressionMethod::Deflated)]);
		// the directory says it unpacks to less than it does
		let mut lying = bytes.clone();
		let record = record_at(&lying, 0);
		lying[record + 24..record + 28].copy_from_slice(&10u32.to_le_bytes());
		let result = scan_bytes(&lying, &Limits::default());
		assert!(
			matches!(code_of(result).as_str(), code::SIZE_MISMATCH | code::UNREADABLE),
			"a lie about the size is refused"
		);
		// and more than it does
		let mut more = bytes.clone();
		let record = record_at(&more, 0);
		more[record + 24..record + 28].copy_from_slice(&9000u32.to_le_bytes());
		let result = scan_bytes(&more, &Limits::default());
		assert!(matches!(
			code_of(result).as_str(),
			code::SIZE_MISMATCH | code::UNREADABLE
		));
	}

	#[test]
	fn entries_that_share_their_data_are_refused() {
		let bytes = zip_of(&[
			("a.txt", &[b'a'; 3000], CompressionMethod::Stored),
			("b.txt", &[b'b'; 3000], CompressionMethod::Stored),
		]);
		// the second record points at the first entry's data
		let mut shared = bytes.clone();
		let second = record_at(&shared, 1);
		shared[second + 42..second + 46].copy_from_slice(&0u32.to_le_bytes());
		let result = scan_bytes(&shared, &Limits::default());
		assert!(
			matches!(
				code_of(result).as_str(),
				code::OVERLAPPING_ENTRIES | code::HEADER_MISMATCH
			),
			"two entries over the same data are refused"
		);
	}

	#[test]
	fn a_local_header_that_says_another_thing_is_refused() {
		let bytes = zip_of(&[("a.txt", &[b'a'; 100], CompressionMethod::Stored)]);
		// another method in the directory than in the local header
		let mut other = bytes.clone();
		let record = record_at(&other, 0);
		other[record + 10..record + 12].copy_from_slice(&8u16.to_le_bytes());
		assert_eq!(code_of(scan_bytes(&other, &Limits::default())), code::HEADER_MISMATCH);
		// another name in the local header (same length)
		let mut renamed = bytes.clone();
		renamed[30..35].copy_from_slice(b"b.txt");
		assert_eq!(code_of(scan_bytes(&renamed, &Limits::default())), code::HEADER_MISMATCH);
	}

	#[test]
	fn names_that_leave_the_folder_are_refused() {
		for name in ["../x.txt", "a/../../x.txt", "/etc/passwd", "C:x.txt", "a\\b.txt", "a//b.txt", "./a.txt"] {
			let bytes = zip_of(&[(name, b"x", CompressionMethod::Stored)]);
			let result = scan_bytes(&bytes, &Limits::default());
			assert_eq!(code_of(result), code::BAD_PATH, "{name}");
		}
		// a NUL is in the name's bytes, in both places
		let mut bytes = zip_of(&[("a.txt", b"x", CompressionMethod::Stored)]);
		let record = record_at(&bytes, 0);
		bytes[record + 46 + 1] = 0;
		assert_eq!(code_of(scan_bytes(&bytes, &Limits::default())), code::BAD_PATH);
	}

	#[test]
	fn two_entries_with_one_name_are_refused_and_a_case_difference_is_told() {
		let twice = zip_of(&[
			("a.txt", b"1", CompressionMethod::Stored),
			("b.txt", b"2", CompressionMethod::Stored),
		]);
		let mut same = twice.clone();
		// rename the second entry to the first's name, in both places
		let local_b = same
			.windows(5)
			.position(|w| w == b"b.txt")
			.expect("local name");
		same[local_b..local_b + 5].copy_from_slice(b"a.txt");
		let record = record_at(&same, 1);
		same[record + 46..record + 51].copy_from_slice(b"a.txt");
		assert_eq!(code_of(scan_bytes(&same, &Limits::default())), code::DUPLICATE_NAME);
		let cases = zip_of(&[
			("a.txt", b"1", CompressionMethod::Stored),
			("A.txt", b"2", CompressionMethod::Stored),
		]);
		let report = scan_bytes(&cases, &Limits::default()).expect("a warning only");
		assert!(report.warnings.iter().any(|w| w.code == code::CASE_DUPLICATE));
	}

	#[test]
	fn encrypted_odd_compressed_and_linked_entries_are_refused() {
		let bytes = zip_of(&[("a.txt", b"hello", CompressionMethod::Stored)]);
		let record = record_at(&bytes, 0);
		let mut encrypted = bytes.clone();
		encrypted[record + 8] |= 1;
		assert_eq!(code_of(scan_bytes(&encrypted, &Limits::default())), code::ENCRYPTED);
		let mut bzip = bytes.clone();
		bzip[record + 10..record + 12].copy_from_slice(&12u16.to_le_bytes());
		assert_eq!(
			code_of(scan_bytes(&bzip, &Limits::default())),
			code::UNSUPPORTED_COMPRESSION
		);
		// a unix symlink: host 3, mode 0o120777 in the external attributes
		let mut link = bytes.clone();
		link[record + 5] = 3;
		link[record + 38..record + 42].copy_from_slice(&(0o120777u32 << 16).to_le_bytes());
		assert_eq!(code_of(scan_bytes(&link, &Limits::default())), code::SYMLINK);
	}

	#[test]
	fn an_archive_inside_the_book_is_refused() {
		let inner = zip_of(&[("x", b"1", CompressionMethod::Stored)]);
		for payload in [&inner[..], b"Rar!\x1a\x07\x00....", b"\x1f\x8b\x08....", b"%PDF-1.4 ...."] {
			let bytes = zip_of(&[("book/page.dat", payload, CompressionMethod::Stored)]);
			assert_eq!(
				code_of(scan_bytes(&bytes, &Limits::default())),
				code::NESTED_ARCHIVE
			);
		}
		// an ordinary binary is not an archive
		let ok = zip_of(&[("font.otf", b"OTTO\x00\x01", CompressionMethod::Stored)]);
		assert!(scan_bytes(&ok, &Limits::default()).is_ok());
	}

	#[test]
	fn a_zip64_directory_is_read() {
		// the same one-entry zip, rewritten with a zip64 end record
		let bytes = zip_of(&[("a.txt", b"hello", CompressionMethod::Stored)]);
		let eocd = bytes.len() - 22;
		let cd_size = u32::from_le_bytes(bytes[eocd + 12..eocd + 16].try_into().unwrap());
		let cd_offset = u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().unwrap());
		let mut out = bytes[..eocd].to_vec();
		let record_pos = out.len() as u64;
		out.extend_from_slice(b"PK\x06\x06");
		out.extend_from_slice(&44u64.to_le_bytes());
		out.extend_from_slice(&[45, 0, 45, 0]);
		out.extend_from_slice(&[0u8; 8]);
		out.extend_from_slice(&1u64.to_le_bytes());
		out.extend_from_slice(&1u64.to_le_bytes());
		out.extend_from_slice(&u64::from(cd_size).to_le_bytes());
		out.extend_from_slice(&u64::from(cd_offset).to_le_bytes());
		out.extend_from_slice(b"PK\x06\x07");
		out.extend_from_slice(&0u32.to_le_bytes());
		out.extend_from_slice(&record_pos.to_le_bytes());
		out.extend_from_slice(&1u32.to_le_bytes());
		out.extend_from_slice(b"PK\x05\x06");
		out.extend_from_slice(&[0u8; 4]);
		out.extend_from_slice(&u16::MAX.to_le_bytes());
		out.extend_from_slice(&u16::MAX.to_le_bytes());
		out.extend_from_slice(&u32::MAX.to_le_bytes());
		out.extend_from_slice(&u32::MAX.to_le_bytes());
		out.extend_from_slice(&0u16.to_le_bytes());
		let report = scan_bytes(&out, &Limits::default()).expect("zip64 is read");
		assert_eq!(report.files, 1);
		// and a directory that claims five million entries there is refused with no allocation
		let mut huge = out.clone();
		let at = huge.windows(4).position(|w| w == b"PK\x06\x06").unwrap() + 32;
		huge[at..at + 8].copy_from_slice(&5_000_000u64.to_le_bytes());
		assert_eq!(code_of(scan_bytes(&huge, &Limits::default())), code::TOO_MANY_ENTRIES);
	}
}
