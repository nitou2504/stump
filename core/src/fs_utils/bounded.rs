//! Bounded reads of the entries of an archive (zip, rar), or of any other stream that
//! somebody else wrote.
//!
//! The archives Stump reads come from files anybody can hand to it (an upload, a file dropped
//! in a library). A few KiB of deflate can inflate to gigabytes (a "zip bomb"), and the size an
//! entry declares is only what the file says about itself: `ZipFile::size()` is not a limit on
//! what the reader returns. Reading an entry whole (`read_to_end`) allocates whatever the
//! stream produces, and the process is killed (`panic = "abort"`, and the container's memory
//! limit). Every read here stops at a cap and fails instead, so what is held in memory never
//! exceeds the cap, whatever the file says.
//!
//! The caps are `Stump.toml` keys (`archive_max_entry_mb`, `archive_max_xml_mb`,
//! `archive_max_unpacked_mb`, `archive_max_pack_books`, or `STUMP_<KEY>` in the environment),
//! applied once at start-up by [`configure`]. The defaults are far above the biggest entries
//! of a real library (the biggest entry of 8,143 EPUBs and 156 CBZ files is 34 MB, and the
//! biggest XML text 10 MB).

use std::{
	io::{self, Read, Write},
	sync::atomic::{AtomicU64, Ordering},
};

const MIB: u64 = 1024 * 1024;

/// The most bytes read from one archive entry that is read whole: a page, a cover, a book
/// inside a pack
pub const DEFAULT_MAX_ENTRY_BYTES: u64 = 256 * MIB;
/// The most bytes read from one metadata text entry (`ComicInfo.xml`, an OPF)
pub const DEFAULT_MAX_XML_BYTES: u64 = 16 * MIB;
/// The most bytes unpacked from one archive that is unpacked as a whole (a pack of books)
pub const DEFAULT_MAX_UNPACKED_BYTES: u64 = 4096 * MIB;
/// The most books taken out of one pack. Each is a file handle until it is staged, and the
/// container's limit is 1,024
pub const DEFAULT_MAX_PACK_BOOKS: u64 = 200;

/// The most bytes in an entry that holds a few lines (`mimetype`, `META-INF/container.xml`)
pub const MAX_TINY_BYTES: u64 = MIB;
/// Entries of at least this size are checked against [`MAX_RATIO`]
pub const RATIO_CHECK_MIN_BYTES: u64 = MIB;
/// The most an entry may inflate (declared size over compressed size) before it is taken for a
/// bomb. The most any real entry of the library does is 153 (and 81 above 1 MiB), deflate itself
/// stops at about 1,030
pub const MAX_RATIO: u64 = 200;
/// A `Vec` for a read of an unknown length starts no bigger than this, whatever the entry claims
const MAX_PREALLOCATION: u64 = 16 * MIB;

static MAX_ENTRY_BYTES: AtomicU64 = AtomicU64::new(DEFAULT_MAX_ENTRY_BYTES);
static MAX_XML_BYTES: AtomicU64 = AtomicU64::new(DEFAULT_MAX_XML_BYTES);
static MAX_UNPACKED_BYTES: AtomicU64 = AtomicU64::new(DEFAULT_MAX_UNPACKED_BYTES);
static MAX_PACK_BOOKS: AtomicU64 = AtomicU64::new(DEFAULT_MAX_PACK_BOOKS);

/// A configured size in MB as bytes. 0 is no limit anybody means, so it is the default
fn from_mb(mb: u64, default: u64) -> u64 {
	if mb == 0 {
		default
	} else {
		mb.saturating_mul(MIB)
	}
}

/// Sets the caps from the configuration (sizes in MB, 0 = the default). Called once at start-up
pub fn configure(entry_mb: u64, xml_mb: u64, unpacked_mb: u64, pack_books: u64) {
	MAX_ENTRY_BYTES.store(from_mb(entry_mb, DEFAULT_MAX_ENTRY_BYTES), Ordering::Relaxed);
	MAX_XML_BYTES.store(from_mb(xml_mb, DEFAULT_MAX_XML_BYTES), Ordering::Relaxed);
	MAX_UNPACKED_BYTES.store(
		from_mb(unpacked_mb, DEFAULT_MAX_UNPACKED_BYTES),
		Ordering::Relaxed,
	);
	MAX_PACK_BOOKS.store(
		if pack_books == 0 {
			DEFAULT_MAX_PACK_BOOKS
		} else {
			pack_books
		},
		Ordering::Relaxed,
	);
}

/// The cap for an entry read whole
pub fn max_entry_bytes() -> u64 {
	MAX_ENTRY_BYTES.load(Ordering::Relaxed)
}

/// The cap for a metadata text entry
pub fn max_xml_bytes() -> u64 {
	MAX_XML_BYTES.load(Ordering::Relaxed)
}

/// The cap for what one archive unpacks to
pub fn max_unpacked_bytes() -> u64 {
	MAX_UNPACKED_BYTES.load(Ordering::Relaxed)
}

/// The cap for the books taken out of one pack
pub fn max_pack_books() -> usize {
	usize::try_from(MAX_PACK_BOOKS.load(Ordering::Relaxed)).unwrap_or(usize::MAX)
}

/// The error of a read, copy or declared size past its cap (an `InvalidData` I/O error that
/// carries this, see [`is_too_large`])
#[derive(Debug)]
pub struct TooLarge(pub u64);

impl std::fmt::Display for TooLarge {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "an archive entry is larger than the limit of {} bytes", self.0)
	}
}

impl std::error::Error for TooLarge {}

fn too_large(cap: u64) -> io::Error {
	io::Error::new(io::ErrorKind::InvalidData, TooLarge(cap))
}

/// Whether `error` is a read, copy or declared size past its cap, and not a damaged archive
/// (those are `InvalidData` as well: a bad checksum, a bad stream)
pub fn is_too_large(error: &io::Error) -> bool {
	error
		.get_ref()
		.is_some_and(|inner| inner.is::<TooLarge>())
}

/// Fails when an entry declares more than `cap` bytes. Declared sizes are what the file says
/// about itself, so this only turns away the honest bombs early (a RAR entry is never unpacked
/// past its declared size); use [`read_capped`] on what is actually read
pub fn check_declared(declared: u64, cap: u64) -> io::Result<()> {
	if declared > cap {
		return Err(too_large(cap));
	}
	Ok(())
}

/// Fails when an entry of at least [`RATIO_CHECK_MIN_BYTES`] inflates more than [`MAX_RATIO`]
/// to one (declared sizes: what a bomb says about itself, and a bomb that lies is stopped by
/// the caps on what is actually read)
pub fn check_ratio(uncompressed: u64, compressed: u64) -> io::Result<()> {
	if uncompressed >= RATIO_CHECK_MIN_BYTES && uncompressed / compressed.max(1) > MAX_RATIO {
		return Err(io::Error::new(
			io::ErrorKind::InvalidData,
			format!(
				"an archive entry inflates from {compressed} to {uncompressed} bytes, which is \
				 more than {MAX_RATIO} to one"
			),
		));
	}
	Ok(())
}

/// How much to reserve for an entry that declares `declared` bytes: never more than the cap or
/// [`MAX_PREALLOCATION`], so a lie cannot make an allocation of its own size (a declared size
/// near `u64::MAX` overflows `Vec::with_capacity` and aborts the process)
pub fn capacity(declared: u64, cap: u64) -> usize {
	declared.min(cap).min(MAX_PREALLOCATION) as usize
}

/// Reads all of `reader`, up to `cap` bytes. A stream with more than `cap` bytes is an
/// `InvalidData` error, after at most `cap + 1` bytes were read
pub fn read_capped<R: Read>(reader: R, cap: u64) -> io::Result<Vec<u8>> {
	read_capped_with_capacity(reader, cap, 0)
}

/// [`read_capped`] for an entry that declares `declared` bytes (only a hint for the allocation)
pub fn read_capped_with_capacity<R: Read>(
	reader: R,
	cap: u64,
	declared: u64,
) -> io::Result<Vec<u8>> {
	let mut buf = Vec::with_capacity(capacity(declared, cap));
	reader.take(cap.saturating_add(1)).read_to_end(&mut buf)?;
	if buf.len() as u64 > cap {
		return Err(too_large(cap));
	}
	Ok(buf)
}

/// [`read_capped`] as UTF-8 text, like `Read::read_to_string` (invalid UTF-8 is an
/// `InvalidData` error)
pub fn read_to_string_capped<R: Read>(reader: R, cap: u64) -> io::Result<String> {
	String::from_utf8(read_capped(reader, cap)?)
		.map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "stream did not contain valid UTF-8"))
}

/// Copies all of `reader` to `writer`, up to `cap` bytes, and returns how many. A stream with
/// more than `cap` bytes is an `InvalidData` error (and `writer` has had `cap + 1` of them:
/// the caller drops what it wrote)
pub fn copy_capped<R: Read, W: Write>(reader: R, writer: &mut W, cap: u64) -> io::Result<u64> {
	let copied = io::copy(&mut reader.take(cap.saturating_add(1)), writer)?;
	if copied > cap {
		return Err(too_large(cap));
	}
	Ok(copied)
}

/// How many bytes `reader` holds, up to `cap` (counted without keeping them). Fails like
/// [`read_capped`] past the cap
pub fn count_capped<R: Read>(reader: R, cap: u64) -> io::Result<u64> {
	copy_capped(reader, &mut io::sink(), cap)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::io::Cursor;

	/// A reader of an endless stream that remembers how much was taken from it
	struct Endless(u64);

	impl Read for Endless {
		fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
			buf.fill(b'a');
			self.0 += buf.len() as u64;
			Ok(buf.len())
		}
	}

	#[test]
	fn test_read_capped_within_the_cap() {
		assert_eq!(read_capped(Cursor::new(b"abc"), 10).unwrap(), b"abc");
		// exactly at the cap is within it
		assert_eq!(read_capped(Cursor::new(b"abc"), 3).unwrap(), b"abc");
		assert!(read_capped(Cursor::new(b""), 0).unwrap().is_empty());
	}

	#[test]
	fn test_read_capped_past_the_cap_is_an_error() {
		let error = read_capped(Cursor::new(b"abcd"), 3).unwrap_err();
		assert_eq!(error.kind(), io::ErrorKind::InvalidData);
		assert!(error.to_string().contains("limit of 3 bytes"), "{error}");
	}

	#[test]
	fn test_is_too_large_tells_a_cap_from_a_damaged_stream() {
		let past_the_cap = read_capped(Cursor::new(b"abcd"), 3).unwrap_err();
		assert!(is_too_large(&past_the_cap));
		assert!(is_too_large(&check_declared(4, 3).unwrap_err()));
		let damaged = io::Error::new(io::ErrorKind::InvalidData, "Invalid checksum");
		assert!(!is_too_large(&damaged));
		assert!(!is_too_large(&io::Error::from(io::ErrorKind::UnexpectedEof)));
	}

	#[test]
	fn test_read_capped_stops_an_endless_stream() {
		let mut endless = Endless(0);
		let error = read_capped(&mut endless, 1000).unwrap_err();
		assert_eq!(error.kind(), io::ErrorKind::InvalidData);
		// it never took more than the cap and the byte that proves it
		assert_eq!(endless.0, 1001);
	}

	#[test]
	fn test_read_capped_with_a_lying_declared_size_never_reserves_it() {
		// a declared size near `u64::MAX` must not become an allocation
		let data = read_capped_with_capacity(Cursor::new(b"abc"), 10, u64::MAX).unwrap();
		assert_eq!(data, b"abc");
		assert!(capacity(u64::MAX, u64::MAX) as u64 <= MAX_PREALLOCATION);
		assert_eq!(capacity(5, 100), 5);
		assert_eq!(capacity(500, 100), 100);
	}

	#[test]
	fn test_read_to_string_capped() {
		assert_eq!(
			read_to_string_capped(Cursor::new("héllo".as_bytes()), 10).unwrap(),
			"héllo"
		);
		let error = read_to_string_capped(Cursor::new([0xff, 0xfe]), 10).unwrap_err();
		assert_eq!(error.kind(), io::ErrorKind::InvalidData);
		let error = read_to_string_capped(Cursor::new(b"abcdef"), 3).unwrap_err();
		assert!(error.to_string().contains("limit"), "{error}");
	}

	#[test]
	fn test_copy_capped() {
		let mut out = Vec::new();
		assert_eq!(copy_capped(Cursor::new(b"abc"), &mut out, 3).unwrap(), 3);
		assert_eq!(out, b"abc");

		let mut out = Vec::new();
		let error = copy_capped(Cursor::new(b"abcd"), &mut out, 3).unwrap_err();
		assert_eq!(error.kind(), io::ErrorKind::InvalidData);

		let mut endless = Endless(0);
		assert!(copy_capped(&mut endless, &mut io::sink(), 5000).is_err());
		assert_eq!(endless.0, 5001);
	}

	#[test]
	fn test_count_capped() {
		assert_eq!(count_capped(Cursor::new(b"abc"), 10).unwrap(), 3);
		assert!(count_capped(Cursor::new(b"abcd"), 3).is_err());
	}

	#[test]
	fn test_check_declared() {
		assert!(check_declared(10, 10).is_ok());
		assert!(check_declared(11, 10).is_err());
		assert!(check_declared(u64::MAX, DEFAULT_MAX_ENTRY_BYTES).is_err());
	}

	#[test]
	fn test_check_ratio() {
		// small entries are not checked, whatever they inflate to
		assert!(check_ratio(RATIO_CHECK_MIN_BYTES - 1, 1).is_ok());
		// the most any real entry of the library does is far below the limit
		assert!(check_ratio(10 * MIB, 10 * MIB / 81).is_ok());
		assert!(check_ratio(10 * MIB, 10 * MIB / 200).is_ok());
		// a megabyte of zeros is about a kilobyte of deflate
		assert!(check_ratio(100 * MIB, 100 * 1024).is_err());
		// nothing compressed to nothing is no division by zero
		assert!(check_ratio(10 * MIB, 0).is_err());
	}

	#[test]
	fn test_configured_sizes() {
		assert_eq!(from_mb(0, 7), 7);
		assert_eq!(from_mb(2, 7), 2 * MIB);
		// a size too big for bytes saturates instead of wrapping to a small cap
		assert_eq!(from_mb(u64::MAX, 7), u64::MAX);
	}
}
