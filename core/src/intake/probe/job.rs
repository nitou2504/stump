//! What the probe does with one request: look at the file (a *screening*), or make the file that goes
//! into the library (a *file job*) and look at what it made.
//!
//! It is one pure function of a request and the folders it works in. The child process runs it, the
//! tests run it, and the server's `subprocess` transport runs it through the child. Whatever goes
//! wrong inside is a reason in a verdict, never a panic the caller has to survive.

use std::{
	fs::File,
	io::{Read, Write},
	path::{Path, PathBuf},
	time::Instant,
};

use ring::digest;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use super::{
	check::{self, check_cbr, check_cbz, check_epub, is_hidden, is_page},
	contract::{
		code, probe_revision, ActiveContent, ActiveSummary, Book, BookMetadata, Kind,
		Output, Reason, Request, Status, Usage, Verdict, CONTRACT_VERSION,
	},
	rebuild, zipscan,
};
use crate::{
	config::StumpConfig,
	fs_utils::bounded,
	intake::{
		lookup,
		title_page::read_title_page,
		write::{write_cbz, write_epub, write_pdf, FileMetadata, KeptPdfInfo},
	},
	media::processor::{process_file, MediaProcessorOptions},
};

const MIB: u64 = 1024 * 1024;

/// Where the probe works
pub struct Env {
	/// The inbox: what a request's `input` is relative to
	pub intake_root: PathBuf,
	/// Where a file job leaves what it makes (and its scratch files, which it removes)
	pub out_dir: PathBuf,
	/// For the readers it runs (the path of PDFium)
	pub config: StumpConfig,
}

fn fail(code: &str, detail: impl Into<String>) -> Reason {
	Reason::new(code, detail)
}

fn io_reason(error: std::io::Error) -> Reason {
	fail(code::UNREADABLE, error.to_string())
}

pub fn hex(bytes: &[u8]) -> String {
	bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The SHA-256 of a file, and how long it is
pub fn sha256_of(path: &Path) -> std::io::Result<(String, u64)> {
	let mut file = File::open(path)?;
	let mut context = digest::Context::new(&digest::SHA256);
	let mut buf = vec![0u8; MIB as usize];
	let mut total = 0u64;
	loop {
		let n = file.read(&mut buf)?;
		if n == 0 {
			break;
		}
		context.update(&buf[..n]);
		total += n as u64;
	}
	Ok((hex(context.finish().as_ref()), total))
}

/// What a file is, by its first bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container {
	Zip,
	Rar,
	Pdf,
}

fn sniff(path: &Path) -> Result<Container, Reason> {
	let mut head = Vec::with_capacity(1024);
	File::open(path)
		.and_then(|f| f.take(1024).read_to_end(&mut head))
		.map_err(io_reason)?;
	if head.starts_with(b"PK\x03\x04") {
		Ok(Container::Zip)
	} else if head.starts_with(b"Rar!\x1A\x07\x00")
		|| head.starts_with(b"Rar!\x1A\x07\x01\x00")
	{
		Ok(Container::Rar)
	} else if head.windows(5).any(|w| w == b"%PDF-") {
		Ok(Container::Pdf)
	} else {
		Err(fail(
			code::TYPE_MISMATCH,
			"The file is not a zip, a RAR or a PDF file, whatever its name says",
		))
	}
}

fn extension_class(name: &str) -> &'static str {
	match check::extension_of(name).as_str() {
		"epub" | "zip" => "zip",
		"cbz" | "cbr" | "rar" => "rar-or-zip",
		"pdf" => "pdf",
		_ => "",
	}
}

struct Inspected {
	book: Book,
	warnings: Vec<Reason>,
}

/// Looks at a file: its structure, its content under the limits, then what Stump's own readers
/// make of it (its metadata, its hash and its pages)
fn inspect(
	path: &Path,
	request: &Request,
	env: &Env,
	runtime: &tokio::runtime::Runtime,
	replacements: Option<&mut Vec<(String, Vec<u8>)>>,
) -> Result<Inspected, Reason> {
	let limits = request.limits.clamped();
	let approver = request.approver;
	let container = sniff(path)?;
	// the name says what it says; what the file is, is what decides. A comic called `.cbr` that is
	// a zip is a CBZ, and the other way round; a PDF is never anything else
	match (extension_class(&request.name), container) {
		("pdf", Container::Pdf)
		| ("zip", Container::Zip)
		| ("rar-or-zip", Container::Rar | Container::Zip) => {},
		("", _) => {},
		_ => {
			return Err(fail(
				code::TYPE_MISMATCH,
				"The file is not the kind of file its name says",
			))
		},
	}
	let mut book = Book {
		sha256: sha256_of(path).map_err(io_reason)?.0,
		size: std::fs::metadata(path).map_err(io_reason)?.len(),
		..Book::default()
	};
	let mut warnings: Vec<Reason> = vec![];
	let format = match container {
		Container::Zip => {
			let report = zipscan::scan(path, &limits)?;
			warnings.extend(report.warnings.iter().cloned());
			book.entries = report.files;
			book.unpacked_bytes = report.unpacked_bytes;
			book.max_entry_bytes = report.max_entry_bytes;
			if report
				.entries
				.iter()
				.any(|e| e.name == "META-INF/container.xml")
			{
				let facts = check_epub(
					path,
					&report,
					&limits,
					request.active_content,
					approver,
					replacements,
				)?;
				warnings.extend(facts.warnings);
				book.active_content = facts.active;
				"epub"
			} else if request.name.to_ascii_lowercase().ends_with(".epub") {
				return Err(fail(
					code::NO_PACKAGE,
					"The book has no META-INF/container.xml",
				));
			} else {
				let facts = check_cbz(path, &report, &limits)?;
				warnings.extend(facts.warnings);
				book.pages = i32::try_from(facts.pages).unwrap_or(i32::MAX);
				"cbz"
			}
		},
		Container::Rar => {
			let (facts, unpacked, max_entry) = check_cbr(path, &limits)?;
			warnings.extend(facts.warnings);
			book.pages = i32::try_from(facts.pages).unwrap_or(i32::MAX);
			book.unpacked_bytes = unpacked;
			book.max_entry_bytes = max_entry;
			"cbr"
		},
		Container::Pdf => "pdf",
	};
	book.format = format.to_string();

	// what Stump's readers make of it: through a link with the extension of what it is
	let work = env.out_dir.join("work");
	std::fs::create_dir_all(&work).map_err(io_reason)?;
	let linked = work.join(format!("input.{format}"));
	let _ = std::fs::remove_file(&linked);
	std::os::unix::fs::symlink(path, &linked).map_err(io_reason)?;
	let options = MediaProcessorOptions {
		generate_file_hashes: true,
		process_metadata: true,
		organization_mode: models::shared::enums::OrganizationMode::Metadata,
		cache_directory: work.clone(),
		..Default::default()
	};
	let processed = runtime.block_on(process_file(&linked, options, &env.config));
	let processed = match processed {
		Ok(processed) => processed,
		Err(error) => {
			let code = if format == "pdf" {
				code::PDF_UNREADABLE
			} else {
				code::UNREADABLE
			};
			return Err(fail(code, format!("Stump can't read the book: {error}")));
		},
	};
	if format == "pdf" {
		check::within_pages(i64::from(processed.pages), &limits)?;
		book.pages = processed.pages;
		// the sizes of its pages as the reader reads them, and its first page drawn and read
		check::check_pdf(&linked, &env.config)?;
	} else if format == "epub" {
		book.pages = processed.pages;
		// The cover is what the server decodes in its own process (the first page, `get_page(1)`,
		// for the thumbnail): the same one is taken, and decoded here first, under limits, as
		// whatever format its bytes say it is (a name is not looked at). A book with no cover, or
		// one that can't be read out, is only left without a thumbnail
		if let Ok((_, cover)) = crate::media::processor::get_cover(&linked) {
			check::check_cover(&cover, &limits)?;
		}
		// the routes the reader draws the book by are made here once, as the server makes them
		// for every request: a book they can't be made of, or that takes long, is not filed
		check::check_reader_routes(&linked)?;
		book.original_title = lookup::original_title_of(&linked);
		if let Some(page) = read_title_page(&linked) {
			book.title_page_title = page.title;
			book.title_page_authors = page.authors;
		}
	} else if book.pages != processed.pages {
		// the check counted the pages one way and Stump's reader another: a page that Stump
		// decodes and the check never looked at is how a file gets past it, so they must agree
		return Err(fail(
			code::PAGE_COUNT,
			format!(
				"The check counted {} pages and Stump's reader {}",
				book.pages, processed.pages
			),
		));
	}
	book.stump_hash = processed.hash;
	book.metadata = processed
		.metadata
		.as_ref()
		.map(BookMetadata::from_processed);
	let _ = std::fs::remove_dir_all(&work);
	Ok(Inspected { book, warnings })
}

/// Takes the active content out of an EPUB, and rewrites its archive, always, the way Stump writes
/// one: `mimetype` first and stored, every entry raw-copied, no padding, one directory
fn rebuild_epub(path: &Path, replacements: Vec<(String, Vec<u8>)>) -> Result<(), Reason> {
	let tmp = path.with_extension("rebuilding");
	let names: std::collections::HashSet<&str> =
		replacements.iter().map(|(name, _)| name.as_str()).collect();
	crate::intake::write::rewrite_zip(
		path,
		&tmp,
		&|name| names.contains(name),
		&mut |writer| {
			for (name, bytes) in &replacements {
				writer.start_file(
					name.as_str(),
					SimpleFileOptions::default()
						.compression_method(CompressionMethod::Deflated),
				)?;
				writer.write_all(bytes)?;
			}
			Ok(())
		},
	)
	.map_err(|e| {
		fail(
			code::OUTPUT_CHECK_FAILED,
			format!("The book could not be rebuilt: {e}"),
		)
	})?;
	std::fs::rename(&tmp, path).map_err(io_reason)
}

/// A comic in a RAR archive as a ZIP one: its pages stored, the rest deflated, in the order and
/// under the names it had. The pages were all unpacked once already, in the check
fn cbr_to_cbz(input: &Path, output: &Path, request: &Request) -> Result<(), Reason> {
	let limits = request.limits.clamped();
	let unreadable = |error: unrar::error::UnrarError| {
		fail(
			code::UNREADABLE,
			format!("The RAR file can't be read: {error}"),
		)
	};
	let mut writer = ZipWriter::new(File::create(output).map_err(io_reason)?);
	let mut archive = unrar::Archive::new(input)
		.open_for_processing()
		.map_err(unreadable)?;
	let mut written = 0u64;
	loop {
		let Some(header) = archive.read_header().map_err(unreadable)? else {
			break;
		};
		let entry = header.entry();
		let name = entry.filename.to_string_lossy().replace('\\', "/");
		if entry.is_directory() || is_hidden(&name) {
			archive = header.skip().map_err(unreadable)?;
			continue;
		}
		// A friend's comic keeps its pages and nothing else. What else an archive holds (a package,
		// pages of another kind, scripts) is not a comic's, and a zip with an EPUB's files in it is
		// one to a reader; the metadata that goes in is written anew by `write_cbz`
		if !request.approver && !is_page(&name) {
			archive = header.skip().map_err(unreadable)?;
			continue;
		}
		let declared = entry.unpacked_size;
		if declared > limits.max_entry_bytes {
			return Err(fail(code::ENTRY_TOO_LARGE, "An entry is too large"));
		}
		let (data, next) = header.read().map_err(unreadable)?;
		archive = next;
		if data.len() as u64 > declared {
			return Err(fail(
				code::SIZE_MISMATCH,
				"An entry holds more than it says",
			));
		}
		written = written.saturating_add(data.len() as u64);
		if written > limits.max_unpacked_bytes {
			return Err(fail(
				code::UNPACKED_TOO_LARGE,
				"The archive unpacks to too much",
			));
		}
		let method = if is_page(&name) {
			CompressionMethod::Stored
		} else {
			CompressionMethod::Deflated
		};
		writer
			.start_file(
				name.as_str(),
				SimpleFileOptions::default().compression_method(method),
			)
			.map_err(|e| fail(code::OUTPUT_CHECK_FAILED, e.to_string()))?;
		writer.write_all(&data).map_err(io_reason)?;
	}
	writer
		.finish()
		.map_err(|e| fail(code::OUTPUT_CHECK_FAILED, e.to_string()))?;
	Ok(())
}

/// A name a page may have in a comic: inside the archive, with no way out of it
fn plain_entry_name(name: &str) -> bool {
	!name.is_empty()
		&& !name.starts_with('/')
		&& !name.contains('\0')
		&& name.split('/').all(|part| part != "..")
}

/// A comic in a ZIP archive written anew with its pages alone, in the order and under the names it
/// had. What else is in the archive (pages of another kind, scripts, a second copy of a metadata
/// file) is not a comic's, and the metadata that goes in is written anew by `write_cbz`. A
/// friend's comic is filed like this, not copied
fn cbz_pages_only(input: &Path, output: &Path, request: &Request) -> Result<(), Reason> {
	let limits = request.limits.clamped();
	let unreadable = |error: zip::result::ZipError| {
		fail(
			code::UNREADABLE,
			format!("The ZIP file can't be read: {error}"),
		)
	};
	let mut archive =
		zip::ZipArchive::new(File::open(input).map_err(io_reason)?).map_err(unreadable)?;
	let mut writer = ZipWriter::new(File::create(output).map_err(io_reason)?);
	let mut written = 0u64;
	for index in 0..archive.len() {
		let mut entry = archive.by_index(index).map_err(unreadable)?;
		let name = entry.name().replace('\\', "/");
		if entry.is_dir() || !is_page(&name) || !plain_entry_name(&name) {
			continue;
		}
		let declared = entry.size();
		if declared > limits.max_entry_bytes {
			return Err(fail(code::ENTRY_TOO_LARGE, "An entry is too large"));
		}
		writer
			.start_file(
				name.as_str(),
				SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
			)
			.map_err(|e| fail(code::OUTPUT_CHECK_FAILED, e.to_string()))?;
		// never more than the entry says it holds (and than an entry may hold)
		let copied = std::io::copy(
			&mut (&mut entry).take(declared.min(limits.max_entry_bytes) + 1),
			&mut writer,
		)
		.map_err(io_reason)?;
		if copied > declared {
			return Err(fail(
				code::SIZE_MISMATCH,
				"An entry holds more than it says",
			));
		}
		written = written.saturating_add(copied);
		if written > limits.max_unpacked_bytes {
			return Err(fail(
				code::UNPACKED_TOO_LARGE,
				"The archive unpacks to too much",
			));
		}
	}
	writer
		.finish()
		.map_err(|e| fail(code::OUTPUT_CHECK_FAILED, e.to_string()))?;
	Ok(())
}

/// Makes the file that goes into the library, and checks what it made
fn file_job(
	input: &Path,
	request: &Request,
	env: &Env,
	runtime: &tokio::runtime::Runtime,
) -> Result<Inspected, Reason> {
	let job = request
		.file
		.as_ref()
		.ok_or_else(|| fail(code::UNREADABLE, "A file job without what to write"))?;
	// the input is looked at again, whatever the screening said: this is the file that is used
	let mut replacements: Vec<(String, Vec<u8>)> = vec![];
	let first = inspect(input, request, env, runtime, Some(&mut replacements))?;
	let format = first.book.format.clone();
	let convert = format == "cbr" && request.cbr_to_cbz;
	let out_format = if convert { "cbz" } else { format.as_str() };
	std::fs::create_dir_all(&env.out_dir).map_err(io_reason)?;
	let out_path = env.out_dir.join(format!("000.{out_format}"));
	let _ = std::fs::remove_file(&out_path);
	let meta: &FileMetadata = &job.metadata;
	let limits = request.limits.clamped();
	// what a rebuild of a friend's book made of it, told to the approver
	let mut rebuilt: Option<rebuild::Rebuilt> = None;
	let wrote = |error: crate::intake::write::WriteError| {
		fail(
			code::OUTPUT_CHECK_FAILED,
			format!("The metadata could not be written: {error}"),
		)
	};
	match format.as_str() {
		"epub" if request.active_content != ActiveContent::Warn && !request.approver => {
			// The book of somebody who needs approval is not copied and cleaned: it is written
			// anew, out of what Stump's reader makes of it, by lists of what may be in a book
			// (see `rebuild`). Nothing of its pages, package or archive is kept as it was sent
			let report = zipscan::scan(input, &limits)?;
			let made = rebuild::rebuild(input, &out_path, &report, &limits).inspect_err(|_| {
				let _ = std::fs::remove_file(&out_path);
			})?;
			if request.active_content == ActiveContent::Block && made.active.found > 0 {
				return Err(fail(
					code::ACTIVE_CONTENT,
					format!(
						"The book carries {} scripts or other active content",
						made.active.found
					),
				));
			}
			rebuilt = Some(made);
			write_epub(&out_path, meta).map_err(wrote)?;
		},
		"epub" => {
			std::fs::copy(input, &out_path).map_err(io_reason)?;
			write_epub(&out_path, meta).map_err(wrote)?;
			// what was found is taken out; the archive is rewritten either way
			let strip = if request.active_content == ActiveContent::Strip {
				// the writer has rewritten the package; the pages are the same bytes as before
				std::mem::take(&mut replacements)
			} else {
				vec![]
			};
			rebuild_epub(&out_path, strip)?;
		},
		"cbz" if !request.approver => {
			// a friend's comic is written anew with its pages alone, not copied
			cbz_pages_only(input, &out_path, request).inspect_err(|_| {
				let _ = std::fs::remove_file(&out_path);
			})?;
			write_cbz(&out_path, meta).map_err(wrote)?;
		},
		"cbz" => {
			std::fs::copy(input, &out_path).map_err(io_reason)?;
			write_cbz(&out_path, meta).map_err(wrote)?;
		},
		"cbr" if convert => {
			cbr_to_cbz(input, &out_path, request)?;
			write_cbz(&out_path, meta).map_err(wrote)?;
		},
		"cbr" => {
			// a RAR is not written: it keeps what it has (and is filed only if that is plain text)
			std::fs::copy(input, &out_path).map_err(io_reason)?;
		},
		"pdf" => {
			std::fs::copy(input, &out_path).map_err(io_reason)?;
			write_pdf(&out_path, meta, &KeptPdfInfo::default()).map_err(wrote)?;
		},
		_ => return Err(fail(code::TYPE_MISMATCH, "Not a book")),
	}

	// what came out is checked as a file of its own: the same checks, and nothing left to take out
	let mut as_request = request.clone();
	as_request.name = format!("book.{out_format}");
	let mut checked =
		inspect(&out_path, &as_request, env, runtime, None).map_err(|reason| {
			fail(
				code::OUTPUT_CHECK_FAILED,
				format!("{}: {}", reason.code, reason.detail),
			)
		})?;
	// What came out must be the kind of file that was meant. A zip with the files of an EPUB in it
	// reads as one whatever it is called (a RAR comic converted with the package of a book in it
	// came out as a "cbz" that was an EPUB), and that is not a file to put in a comic's place.
	if checked.book.format != out_format {
		return Err(fail(
			code::OUTPUT_CHECK_FAILED,
			format!(
				"The file came out as {}, not as {out_format}",
				checked.book.format
			),
		));
	}
	if format == "epub"
		&& request.active_content == ActiveContent::Strip
		&& checked.book.active_content.found > 0
	{
		return Err(fail(
			code::OUTPUT_CHECK_FAILED,
			"The book still carries active content after it was taken out",
		));
	}
	if meta.replace_description {
		// what is read from the file is the plain description it was given, or none
		let rewritten = matches!(out_format, "epub" | "cbz" | "pdf");
		runtime
			.block_on(crate::intake::only_the_plain_description(
				&env.config,
				&out_path,
				&request.name,
				meta.description.as_deref(),
				rewritten,
			))
			.map_err(|e| fail(code::DESCRIPTION, e.to_string()))?;
	}
	let (sha256, size) = sha256_of(&out_path).map_err(io_reason)?;
	checked.book.output = Some(Output {
		path: format!("{}/000.{out_format}", request.id),
		format: out_format.to_string(),
		sha256,
		size,
		converted_from: convert.then(|| "cbr".to_string()),
		pages: checked.book.pages,
	});
	// what was found in the input is what the approver is told (for a book that was rebuilt, what
	// the rebuild took out: it is the one that read every page)
	checked.book.active_content = match &rebuilt {
		Some(made) => made.active.clone(),
		None => ActiveSummary {
			stripped: request.active_content == ActiveContent::Strip
				&& first.book.active_content.found > 0,
			..first.book.active_content.clone()
		},
	};
	checked.warnings = first.warnings;
	if let Some(made) = rebuilt {
		checked.warnings.extend(made.warnings);
	}
	if convert {
		checked.warnings.push(fail(
			code::CONVERTED_CBR,
			"The comic was converted from RAR to ZIP",
		));
	}
	Ok(checked)
}

fn valid_request(request: &Request) -> Result<(), Reason> {
	if request.v != CONTRACT_VERSION {
		return Err(fail(code::UNREADABLE, "A request of another version"));
	}
	if !super::contract::is_valid_ticket(&request.id) {
		return Err(fail(
			code::UNREADABLE,
			"A request with a ticket that is not one",
		));
	}
	if !super::contract::is_safe_relative(&request.input) {
		return Err(fail(
			code::BAD_PATH,
			"A request with a path that leaves the inbox",
		));
	}
	Ok(())
}

/// Runs one request: the whole of what the child does
pub fn run(request: &Request, env: &Env) -> Verdict {
	let started = Instant::now();
	let limits = request.limits.clamped();
	let outcome = (|| -> Result<Inspected, Reason> {
		valid_request(request)?;
		let path = env.intake_root.join(&request.input);
		let (sha256, size) = sha256_of(&path).map_err(io_reason)?;
		if size != request.size || sha256 != request.sha256 {
			return Err(fail(
				code::INPUT_CHANGED,
				"The file is not the one that was uploaded",
			));
		}
		if size > limits.max_file_bytes {
			return Err(fail(
				code::ENTRY_TOO_LARGE,
				format!(
					"The file is larger than the {} MB a book can be",
					limits.max_file_bytes / MIB
				),
			));
		}
		let runtime = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.map_err(io_reason)?;
		match request.kind {
			Kind::Screen => inspect(&path, request, env, &runtime, None),
			Kind::File => file_job(&path, request, env, &runtime),
		}
	})();
	let mut verdict = match outcome {
		Ok(inspected) => Verdict {
			v: CONTRACT_VERSION,
			id: request.id.clone(),
			kind: request.kind,
			probe: probe_revision().to_string(),
			status: Status::Ok,
			reasons: vec![],
			warnings: inspected.warnings,
			book: Some(inspected.book),
			usage: Usage::default(),
		},
		Err(reason) => Verdict::rejected(request, vec![reason]),
	};
	verdict.usage.wall_ms = started.elapsed().as_millis() as u64;
	verdict
}

#[cfg(test)]
pub(crate) mod tests {
	use std::io::Cursor;

	use super::*;
	use crate::intake::probe::contract::{FileJob, Limits};

	pub(crate) struct World {
		pub root: tempfile::TempDir,
		pub env: Env,
	}

	pub(crate) fn world() -> World {
		let root = tempfile::tempdir().expect("dir");
		std::fs::create_dir_all(root.path().join("inbox/u/i")).unwrap();
		let env = Env {
			intake_root: root.path().join("inbox"),
			out_dir: root.path().join("out/t1.partial"),
			config: StumpConfig {
				config_dir: root.path().join("config").to_string_lossy().into_owned(),
				..StumpConfig::default()
			},
		};
		World { root, env }
	}

	impl World {
		pub(crate) fn stage(&self, name: &str, bytes: &[u8]) -> Request {
			std::fs::write(self.root.path().join("inbox/u/i").join(name), bytes).unwrap();
			let (sha256, size) =
				sha256_of(&self.root.path().join("inbox/u/i").join(name)).unwrap();
			Request {
				v: CONTRACT_VERSION,
				id: "0123abcd-0000-4000-8000-000000000001".to_string(),
				kind: Kind::Screen,
				input: format!("u/i/{name}"),
				name: name.to_string(),
				size,
				sha256,
				limits: Limits::default(),
				active_content: ActiveContent::Strip,
				cbr_to_cbz: true,
				approver: false,
				file: None,
			}
		}
	}

	fn png(width: u32, height: u32) -> Vec<u8> {
		let image = image::RgbImage::new(width, height);
		let mut out = Cursor::new(Vec::new());
		image.write_to(&mut out, image::ImageFormat::Png).unwrap();
		out.into_inner()
	}

	fn zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
		let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
		for (name, data) in entries {
			let method = if *name == "mimetype" {
				CompressionMethod::Stored
			} else {
				CompressionMethod::Deflated
			};
			writer
				.start_file(
					*name,
					SimpleFileOptions::default().compression_method(method),
				)
				.unwrap();
			writer.write_all(data).unwrap();
		}
		writer.finish().unwrap().into_inner()
	}

	pub(crate) fn epub_with(chapter: &str) -> Vec<u8> {
		let opf = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:1</dc:identifier><dc:title>A Book</dc:title><dc:creator>Ana Writer</dc:creator><dc:language>en</dc:language><dc:description>It has &lt;b&gt;markup&lt;/b&gt; in it</dc:description></metadata><manifest><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/></spine></package>"#;
		epub_with_opf(chapter, opf)
	}

	pub(crate) fn epub_with_opf(chapter: &str, opf: &str) -> Vec<u8> {
		let container = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
		zip(&[
			("mimetype", b"application/epub+zip".to_vec()),
			("META-INF/container.xml", container.as_bytes().to_vec()),
			("OEBPS/content.opf", opf.as_bytes().to_vec()),
			("OEBPS/c1.xhtml", chapter.as_bytes().to_vec()),
		])
	}

	pub(crate) const CLEAN: &str = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>c</title></head><body><p>hello</p></body></html>"#;
	pub(crate) const SCRIPTED: &str = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>c</title></head><body><p>hello</p><script>alert(1)</script><img src="a.png" onerror="x()"/></body></html>"#;

	#[test]
	fn a_screening_reports_what_stump_reads_of_the_book() {
		let world = world();
		let request = world.stage("book.epub", &epub_with(CLEAN));
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		let book = verdict.book.clone().expect("a book");
		assert_eq!(book.format, "epub");
		assert_eq!(book.sha256, request.sha256);
		assert!(book.stump_hash.is_some());
		let metadata = book.metadata.expect("metadata");
		assert_eq!(metadata.title.as_deref(), Some("A Book"));
		assert_eq!(book.active_content.found, 0);
		assert_eq!(verdict.probe, probe_revision());
		// the verdict is a small JSON file the server can read back
		let json = serde_json::to_string(&verdict).unwrap();
		assert!(json.len() < 100_000);
		serde_json::from_str::<Verdict>(&json).expect("a verdict reads back");
	}

	#[test]
	fn a_file_that_is_not_the_one_that_was_counted_is_refused() {
		let world = world();
		let mut request = world.stage("book.epub", &epub_with(CLEAN));
		request.sha256 = "0".repeat(64);
		assert_eq!(
			run(&request, &world.env).reasons[0].code,
			code::INPUT_CHANGED
		);
		let mut request = world.stage("book2.epub", &epub_with(CLEAN));
		request.size += 1;
		assert_eq!(
			run(&request, &world.env).reasons[0].code,
			code::INPUT_CHANGED
		);
		// and requests that leave the inbox
		let mut request = world.stage("book3.epub", &epub_with(CLEAN));
		request.input = "../outside".to_string();
		assert_eq!(run(&request, &world.env).reasons[0].code, code::BAD_PATH);
	}

	#[test]
	fn a_name_that_lies_about_the_file_is_refused() {
		let world = world();
		let request = world.stage("book.pdf", &epub_with(CLEAN));
		assert_eq!(
			run(&request, &world.env).reasons[0].code,
			code::TYPE_MISMATCH
		);
		let request =
			world.stage("book.epub", b"hello, this is not a book at all, really");
		assert_eq!(
			run(&request, &world.env).reasons[0].code,
			code::TYPE_MISMATCH
		);
	}

	#[test]
	fn a_comic_is_screened_and_its_pages_counted_the_same_way_stump_does() {
		let world = world();
		let bytes = zip(&[
			("001.png", png(30, 40)),
			("002.png", png(30, 40)),
			(
				"ComicInfo.xml",
				b"<ComicInfo><Series>S</Series><Number>2</Number></ComicInfo>".to_vec(),
			),
		]);
		// a cbr that is a zip is a cbz
		let request = world.stage("Series 02.cbr", &bytes);
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		let book = verdict.book.unwrap();
		assert_eq!(book.format, "cbz");
		assert_eq!(book.pages, 2);
		assert_eq!(book.metadata.and_then(|m| m.series).as_deref(), Some("S"));
	}

	#[test]
	fn a_zip_bomb_is_refused_before_it_is_unpacked_and_leaves_nothing_behind() {
		let world = world();
		let bomb = zip(&[("001.png", vec![0u8; 24 * 1024 * 1024])]);
		let request = world.stage("bomb.cbz", &bomb);
		let started = Instant::now();
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Reject);
		assert_eq!(verdict.reasons[0].code, code::RATIO);
		assert!(started.elapsed().as_secs() < 10);
		assert!(!world.env.out_dir.join("000.cbz").exists());
	}

	fn file_request(
		world: &World,
		name: &str,
		bytes: &[u8],
		metadata: FileMetadata,
	) -> Request {
		let mut request = world.stage(name, bytes);
		request.kind = Kind::File;
		request.file = Some(FileJob { metadata });
		request
	}

	fn out_file(world: &World, verdict: &Verdict) -> PathBuf {
		let output = verdict
			.book
			.as_ref()
			.unwrap()
			.output
			.as_ref()
			.expect("an output");
		assert!(output
			.path
			.starts_with("0123abcd-0000-4000-8000-000000000001/000."));
		world
			.env
			.out_dir
			.join(output.path.rsplit('/').next().unwrap())
	}

	#[test]
	fn a_file_job_writes_the_metadata_takes_the_scripts_out_and_checks_what_it_made() {
		let world = world();
		let meta = FileMetadata {
			title: Some("Filed Title".to_string()),
			authors: vec!["Ana Writer".to_string()],
			description: Some("A plain description".to_string()),
			replace_description: true,
			..FileMetadata::default()
		};
		let request = file_request(&world, "book.epub", &epub_with(SCRIPTED), meta);
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		let book = verdict.book.clone().unwrap();
		assert!(book.active_content.stripped && book.active_content.found == 2);
		let out = out_file(&world, &verdict);
		let bytes = std::fs::read(&out).unwrap();
		assert_eq!(
			sha256_of(&out).unwrap().0,
			book.output.as_ref().unwrap().sha256
		);
		// the file that came out carries the new title, the plain description, and no script
		let (title, text) = {
			let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
			let opf = bounded::read_to_string_capped(
				archive.by_name("content.opf").unwrap(),
				MIB,
			)
			.unwrap();
			let page = bounded::read_to_string_capped(
				archive.by_name("OEBPS/c1.xhtml").unwrap(),
				MIB,
			)
			.unwrap();
			(opf, page)
		};
		assert!(title.contains("Filed Title") && title.contains("A plain description"));
		assert!(!title.contains("markup"));
		assert!(
			!text.to_lowercase().contains("script")
				&& !text.contains("onerror")
				&& text.contains("<p>hello</p>")
		);
		// and it passes the checks as a file of its own, with the structure of one Stump wrote
		let again = world.stage("again.epub", &bytes);
		let screened = run(&again, &world.env);
		assert_eq!(screened.status, Status::Ok, "{:?}", screened.reasons);
		assert_eq!(screened.book.unwrap().active_content.found, 0);
	}

	#[test]
	fn a_page_the_old_stripper_could_not_clean_is_rebuilt_and_nothing_of_it_runs() {
		let world = world();
		let sneaky = r#"<html><body><p>one<img src=x onerror=alert(1)></body>"#;
		let request = file_request(
			&world,
			"book.epub",
			&epub_with(sneaky),
			FileMetadata::default(),
		);
		let verdict = run(&request, &world.env);
		// the page was never cleaned: it is written anew from what a browser makes of it
		assert!(assert_nothing_runs(&world, &verdict), "{:?}", verdict.reasons);
		let page = entry_text(&out_file(&world, &verdict), "OEBPS/c1.xhtml");
		assert!(page.contains("one"), "{page}");
	}

	// ---- the hostile corpus of the D3 review: what the checker must not let through ----

	fn container_for(opf_path: &str) -> String {
		format!(
			r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="{opf_path}" media-type="application/oebps-package+xml"/></rootfiles></container>"#
		)
	}

	/// The package file of a book with these manifest items and spine, and this in its metadata
	fn opf_of(in_metadata: &str, items: &str, spine: &str) -> String {
		format!(
			r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:1</dc:identifier><dc:title>A Book</dc:title><dc:language>en</dc:language>{in_metadata}</metadata><manifest>{items}</manifest><spine>{spine}</spine></package>"#
		)
	}

	/// An EPUB whose package file is at `opf_path`, with these entries as well
	fn epub_at(opf_path: &str, opf: &str, extra: &[(&str, Vec<u8>)]) -> Vec<u8> {
		let mut entries: Vec<(&str, Vec<u8>)> = vec![
			("mimetype", b"application/epub+zip".to_vec()),
			(
				"META-INF/container.xml",
				container_for(opf_path).into_bytes(),
			),
			(opf_path, opf.as_bytes().to_vec()),
		];
		entries.extend(extra.iter().map(|(name, bytes)| (*name, bytes.clone())));
		zip(&entries)
	}

	fn entry_text(path: &Path, name: &str) -> String {
		let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
		bounded::read_to_string_capped(archive.by_name(name).unwrap(), MIB).unwrap()
	}

	fn assert_clean_of_scripts(page: &str) {
		assert!(
			!page.to_lowercase().contains("script")
				&& !page.contains("onerror")
				&& page.contains("<p>hello</p>"),
			"{page}"
		);
	}

	#[test]
	fn a_page_in_meta_inf_that_the_spine_lists_is_cleaned_like_any_other() {
		let world = world();
		// the package file is at the root of the zip, and the page the reader will draw is under
		// `META-INF/`, where nothing used to be looked at
		let opf = opf_of(
			"",
			r#"<item id="p" href="META-INF/p.xhtml" media-type="application/xhtml+xml"/>"#,
			r#"<itemref idref="p"/>"#,
		);
		let bytes = epub_at(
			"content.opf",
			&opf,
			&[("META-INF/p.xhtml", SCRIPTED.as_bytes().to_vec())],
		);
		let request = file_request(&world, "book.epub", &bytes, FileMetadata::default());
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		assert_eq!(verdict.book.as_ref().unwrap().active_content.found, 2);
		assert_clean_of_scripts(&entry_text(
			&out_file(&world, &verdict),
			"META-INF/p.xhtml",
		));
	}

	#[test]
	fn a_package_file_with_a_script_in_it_is_told_and_never_copied() {
		let world = world();
		let opf = opf_of(
			r#"<script xmlns="http://www.w3.org/1999/xhtml">alert(1)</script>"#,
			r#"<item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/>"#,
			r#"<itemref idref="c1"/>"#,
		);
		let bytes = epub_at(
			"OEBPS/content.opf",
			&opf,
			&[("OEBPS/c1.xhtml", CLEAN.as_bytes().to_vec())],
		);
		// the look at the book tells the approver and does not refuse: the package of the book that
		// is filed is written from what the reader reads of it, and a script is not in that
		let request = world.stage("book.epub", &bytes);
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		assert!(verdict
			.warnings
			.iter()
			.any(|w| w.code == code::ACTIVE_CONTENT_UNSTRIPPABLE));
		let request = file_request(&world, "filed.epub", &bytes, FileMetadata::default());
		let verdict = run(&request, &world.env);
		assert!(assert_nothing_runs(&world, &verdict), "{:?}", verdict.reasons);
		// somebody who files their own books is told, and the book is not refused
		let mut request = world.stage("mine.epub", &bytes);
		request.approver = true;
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		assert!(verdict
			.warnings
			.iter()
			.any(|w| w.code == code::ACTIVE_CONTENT_UNSTRIPPABLE));
	}

	#[test]
	fn a_page_with_an_odd_name_and_a_type_that_ends_in_a_semicolon_is_cleaned() {
		let world = world();
		// the reader drops the `;` and parses `p.dat` as XHTML
		let opf = opf_of(
			"",
			r#"<item id="p" href="p.dat" media-type="application/xhtml+xml;"/>"#,
			r#"<itemref idref="p"/>"#,
		);
		let bytes = epub_at(
			"OEBPS/content.opf",
			&opf,
			&[("OEBPS/p.dat", SCRIPTED.as_bytes().to_vec())],
		);
		let request = file_request(&world, "book.epub", &bytes, FileMetadata::default());
		let verdict = run(&request, &world.env);
		// a page with a name that isn't a page's is not filed at all, and a book with no other
		// page is not made
		assert_eq!(verdict.status, Status::Reject);
		assert_eq!(verdict.reasons[0].code, code::NO_READABLE_PAGES);
		assert!(!world.env.out_dir.join("000.epub").exists());
	}

	#[test]
	fn a_page_the_package_calls_html_with_a_parameter_is_written_as_what_it_is() {
		let world = world();
		// `text/html;x` is `text/html` to the reader: the CDATA is text to XML and a comment
		// that the first `>` ends to HTML, and the `<img>` after it is an element
		let page = format!(
			"{}",
			r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body><![CDATA[><img src=x onerror=alert(1)>]]></body></html>"#
		);
		let opf = opf_of(
			"",
			r#"<item id="p" href="c1.xhtml" media-type="text/html;x"/>"#,
			r#"<itemref idref="p"/>"#,
		);
		let bytes = epub_at(
			"OEBPS/content.opf",
			&opf,
			&[("OEBPS/c1.xhtml", page.into_bytes())],
		);
		// the page is written anew as XHTML, with that text escaped, and the new package says
		// `application/xhtml+xml`: there is no `<img>` element in what is filed, in either reading
		let request = file_request(&world, "book.epub", &bytes, FileMetadata::default());
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		let out = out_file(&world, &verdict);
		let page = entry_text(&out, "OEBPS/c1.xhtml");
		assert!(!page.contains("<img") && !page.contains("<script"), "{page}");
		let package = entry_text(&out, "content.opf");
		assert!(!package.contains("text/html"), "{package}");
	}

	#[test]
	fn a_file_listed_twice_with_another_type_each_is_read_as_the_dangerous_one() {
		let world = world();
		let opf = opf_of(
			"",
			r#"<item id="a" href="p.dat" media-type="image/png"/><item id="p" href="p.dat" media-type="application/xhtml+xml;"/>"#,
			r#"<itemref idref="p"/>"#,
		);
		let bytes = epub_at(
			"OEBPS/content.opf",
			&opf,
			&[("OEBPS/p.dat", SCRIPTED.as_bytes().to_vec())],
		);
		let request = file_request(&world, "book.epub", &bytes, FileMetadata::default());
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Reject);
		assert_eq!(verdict.reasons[0].code, code::NO_READABLE_PAGES);
		assert!(!world.env.out_dir.join("000.epub").exists());
	}

	#[test]
	fn a_folder_entry_that_holds_data_is_refused_before_a_package_can_point_at_it() {
		let world = world();
		// the package names `x/` as its cover: epub-rs would read all of it, in the server
		let opf = opf_of(
			r#"<meta name="cover" content="cover"/>"#,
			r#"<item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/><item id="cover" href="x/" media-type="image/jpeg"/>"#,
			r#"<itemref idref="c1"/>"#,
		);
		let bytes = epub_at(
			"OEBPS/content.opf",
			&opf,
			&[
				("OEBPS/c1.xhtml", CLEAN.as_bytes().to_vec()),
				("OEBPS/x/", vec![0u8; 3 * 1024 * 1024]),
			],
		);
		let request = world.stage("book.epub", &bytes);
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Reject);
		assert_eq!(verdict.reasons[0].code, code::SIZE_MISMATCH);
	}

	#[test]
	fn a_page_with_an_empty_dtd_subset_passes() {
		// Oxford's EPUBs have `<!DOCTYPE html []>` in every page: 2 of the 400 books that the
		// calibration on a real library checked were refused for it
		let world = world();
		let page = r#"<?xml version="1.0"?><!DOCTYPE html []><html xmlns="http://www.w3.org/1999/xhtml"><head><title>c</title></head><body><p>hello</p></body></html>"#;
		let verdict = run(&world.stage("oup.epub", &epub_with(page)), &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		assert_eq!(verdict.book.unwrap().active_content.found, 0);
	}

	fn tiff() -> Vec<u8> {
		let mut bytes = b"II*\0\x08\0\0\0".to_vec();
		bytes.extend_from_slice(&[0u8; 64]);
		bytes
	}

	/// A PNG whose header says it is 30,000 by 30,000
	fn huge_png_header() -> Vec<u8> {
		let mut bytes = png(1, 1);
		bytes[16..20].copy_from_slice(&30_000u32.to_be_bytes());
		bytes[20..24].copy_from_slice(&30_000u32.to_be_bytes());
		bytes
	}

	fn epub_with_cover(
		cover_name: &str,
		cover: Vec<u8>,
		other: &[(&str, Vec<u8>)],
	) -> Vec<u8> {
		let opf = opf_of(
			r#"<meta name="cover" content="cover"/>"#,
			&format!(
				r#"<item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/><item id="cover" href="{cover_name}" media-type="image/jpeg" properties="cover-image"/>"#
			),
			r#"<itemref idref="c1"/>"#,
		);
		let mut extra = vec![("OEBPS/c1.xhtml", CLEAN.as_bytes().to_vec())];
		let path = format!("OEBPS/{cover_name}");
		extra.push((path.as_str(), cover));
		extra.extend(other.iter().map(|(n, b)| (*n, b.clone())));
		epub_at("OEBPS/content.opf", &opf, &extra)
	}

	#[test]
	fn the_cover_is_held_to_the_kinds_the_server_decodes_and_found_under_any_name() {
		let world = world();
		// a TIFF called a JPEG: the server's thumbnail would guess its format and reach the TIFF decoder
		let request =
			world.stage("tiff.epub", &epub_with_cover("cover.dat", tiff(), &[]));
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Reject, "{:?}", verdict.warnings);
		assert_eq!(verdict.reasons[0].code, code::UNSUPPORTED_IMAGE);
		// a PNG that says it is 900 megapixels: refused from its header, never decoded
		let request = world.stage(
			"huge.epub",
			&epub_with_cover("cover.dat", huge_png_header(), &[]),
		);
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Reject);
		assert_eq!(verdict.reasons[0].code, code::IMAGE_TOO_LARGE);
		// the same, not as the cover and under a name that is no image's
		let request = world.stage(
			"huge2.epub",
			&epub_with_cover(
				"cover.png",
				png(8, 8),
				&[("OEBPS/blob.dat", huge_png_header())],
			),
		);
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Reject);
		assert_eq!(verdict.reasons[0].code, code::IMAGE_TOO_LARGE);
		// and a plain one passes
		let request =
			world.stage("fine.epub", &epub_with_cover("cover.png", png(8, 8), &[]));
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
	}

	/// A one-page PDF written by hand, with this `MediaBox`
	fn tiny_pdf(media_box: &str) -> Vec<u8> {
		let mut out = b"%PDF-1.4\n".to_vec();
		let mut offsets = vec![];
		for body in [
			"<</Type/Catalog/Pages 2 0 R>>".to_string(),
			"<</Type/Pages/Kids[3 0 R]/Count 1>>".to_string(),
			format!("<</Type/Page/Parent 2 0 R/MediaBox[{media_box}]>>"),
		] {
			offsets.push(out.len());
			let n = offsets.len();
			out.extend_from_slice(format!("{n} 0 obj\n{body}\nendobj\n").as_bytes());
		}
		let xref = out.len();
		out.extend_from_slice(b"xref\n0 4\n0000000000 65535 f \n");
		for offset in &offsets {
			out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
		}
		out.extend_from_slice(
			format!("trailer\n<</Root 1 0 R/Size 4>>\nstartxref\n{xref}\n%%EOF\n")
				.as_bytes(),
		);
		out
	}

	#[test]
	fn a_pdf_has_its_page_sizes_read_and_its_first_page_drawn() {
		// needs the PDFium library, which a build container doesn't have: skipped there
		if !std::env::var("PDFIUM_PATH").is_ok_and(|path| Path::new(&path).exists()) {
			return;
		}
		let world = world();
		let verdict = run(&world.stage("a.pdf", &tiny_pdf("0 0 200 300")), &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		assert_eq!(verdict.book.unwrap().pages, 1);
		// a page 20,000 points wide is not a page the reader can draw
		let verdict = run(
			&world.stage("b.pdf", &tiny_pdf("0 0 20000 300")),
			&world.env,
		);
		assert_eq!(verdict.status, Status::Reject);
		assert_eq!(verdict.reasons[0].code, code::PAGE_SIZE);
	}

	#[test]
	fn a_comic_is_counted_as_stump_counts_it() {
		let world = world();
		// a page in a folder whose name starts with a dot: Stump counts it (only the file's own
		// name, or `__MACOSX`, hides one), so the check looks at it and counts it too
		let bytes = zip(&[("a/.b/001.png", png(30, 40)), ("002.png", png(30, 40))]);
		let verdict = run(&world.stage("c.cbz", &bytes), &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		assert_eq!(verdict.book.unwrap().pages, 2);
		// a `.heif` is a page to Stump, and not one the server can decode: refused
		let bytes = zip(&[
			("001.png", png(30, 40)),
			("002.heif", b"\0\0\0\x18ftypheic\0\0\0\0heicmif1".to_vec()),
		]);
		let verdict = run(&world.stage("d.cbz", &bytes), &world.env);
		assert_eq!(verdict.status, Status::Reject);
		assert_eq!(verdict.reasons[0].code, code::UNSUPPORTED_IMAGE);
		// a hidden file, a `__MACOSX` entry and a `.bmp` (which Stump doesn't count) are no pages
		let bytes = zip(&[
			("001.png", png(30, 40)),
			(".hidden.png", b"not an image".to_vec()),
			("__MACOSX/._001.png", b"fork".to_vec()),
			("scan.bmp", b"BM".to_vec()),
		]);
		let verdict = run(&world.stage("e.cbz", &bytes), &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		assert_eq!(verdict.book.unwrap().pages, 1);
	}

	#[test]
	fn a_comic_file_job_writes_comic_info_and_a_description_of_its_own_goes() {
		let world = world();
		let bytes = zip(&[
			("001.png", png(30, 40)),
			("ComicInfo.xml", b"<ComicInfo><Summary>&lt;script&gt;alert(1)&lt;/script&gt;</Summary></ComicInfo>".to_vec()),
		]);
		let meta = FileMetadata {
			title: Some("Vol 1".to_string()),
			series: Some("Series".to_string()),
			series_index: Some(1.0),
			description: None,
			replace_description: true,
			..FileMetadata::default()
		};
		let request = file_request(&world, "vol1.cbz", &bytes, meta);
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		let out = out_file(&world, &verdict);
		let mut archive = zip::ZipArchive::new(File::open(out).unwrap()).unwrap();
		let info = bounded::read_to_string_capped(
			archive.by_name("ComicInfo.xml").unwrap(),
			MIB,
		)
		.unwrap();
		assert!(info.contains("<Series>Series</Series>") && !info.contains("script"));
	}

	#[test]
	fn a_book_with_odd_dates_a_huge_id_and_many_creators_is_made_without_aborting() {
		let world = world();
		// dates in scripts the date readers slice by bytes, three hundred creators that each
		// refine something, and an id of a hundred thousand characters (a pattern built from it
		// would not compile): none of it may stop the checker
		let mut creators = String::new();
		for i in 0..300 {
			creators.push_str(&format!(
				r##"<dc:creator id="c{i}">Author {i}</dc:creator><meta refines="#c{i}" property="role">aut</meta>"##
			));
		}
		creators.push_str(&format!(
			r#"<dc:creator id="{}">Big</dc:creator>"#,
			"x".repeat(100_000)
		));
		let opf = format!(
			r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">1</dc:identifier><dc:title>T</dc:title><dc:date>日本語</dc:date><dc:date>20201é</dc:date><dc:language>ja</dc:language>{creators}</metadata><manifest><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/></spine></package>"#
		);
		let bytes = epub_with_opf(CLEAN, &opf);
		let started = Instant::now();
		let screened = run(&world.stage("odd.epub", &bytes), &world.env);
		assert_eq!(screened.status, Status::Ok, "{:?}", screened.reasons);
		let meta = FileMetadata {
			title: Some("Filed".to_string()),
			authors: vec!["Somebody".to_string()],
			..FileMetadata::default()
		};
		let made = run(&file_request(&world, "odd2.epub", &bytes, meta), &world.env);
		assert_eq!(made.status, Status::Ok, "{:?}", made.reasons);
		assert!(started.elapsed().as_secs() < 60);
	}

	// --- the ways round a checker that the re-checks found: each book below must come out with
	// nothing that runs in any of its files, however the reader and the program that looked would
	// have read it differently. The book is written anew from what the reader reads, so none of
	// them depends on the checker agreeing with a browser ---

	/// What must not be found in any file that was made: a script, a handler, a script address or a
	/// frame (the text of a script that was left out is not a marker: it is only text)
	const HOSTILE_MARKERS: &[&str] = &["<script", "onerror", "javascript:", "srcdoc", "<iframe"];

	/// Every entry of the book that was made, by name
	fn everything_in(path: &Path) -> Vec<(String, Vec<u8>)> {
		let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
		(0..archive.len())
			.map(|i| {
				let entry = archive.by_index(i).unwrap();
				let name = entry.name().to_string();
				(
					name,
					bounded::read_capped(entry, 8 * MIB).unwrap_or_default(),
				)
			})
			.collect()
	}

	/// The book was not made, or it was and nothing that runs is in any file of it
	fn assert_nothing_runs(world: &World, verdict: &Verdict) -> bool {
		if verdict.status != Status::Ok {
			assert_eq!(
				verdict.status,
				Status::Reject,
				"{:?}",
				verdict.reasons
			);
			assert!(!world.env.out_dir.join("000.epub").exists());
			return false;
		}
		for (name, bytes) in everything_in(&out_file(world, verdict)) {
			let text = String::from_utf8_lossy(&bytes).to_lowercase();
			for marker in HOSTILE_MARKERS {
				assert!(!text.contains(marker), "{marker} in {name}: {text}");
			}
			assert!(
				!name.contains('?') && !name.contains('#') && !name.contains('%'),
				"{name}"
			);
		}
		true
	}

	fn friend_book(bytes: &[u8]) -> (World, Verdict) {
		let world = world();
		let request = file_request(&world, "book.epub", bytes, FileMetadata::default());
		let verdict = run(&request, &world.env);
		(world, verdict)
	}

	const GOOD_PAGE: &str = CLEAN;

	#[test]
	fn a_doctype_that_a_browser_ends_somewhere_else_hides_no_script() {
		// quick-xml ends the DOCTYPE at the `>` after the `-->`; a browser ends it after the quoted
		// `<`. A detector that read the page like quick-xml saw one DOCTYPE and nothing else
		let page = concat!(
			"<?xml version=\"1.0\"?><!DOCTYPE html SYSTEM \"<\">",
			"<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>t</title></head>",
			"<body><p>hello</p><script>alert(1)</script></body></html><!-- > -->"
		);
		let opf = opf_of(
			"",
			r#"<item id="p" href="p.xhtml" media-type="application/xhtml+xml"/><item id="c" href="c.xhtml" media-type="application/xhtml+xml"/>"#,
			r#"<itemref idref="p"/><itemref idref="c"/>"#,
		);
		let bytes = epub_at(
			"OEBPS/content.opf",
			&opf,
			&[
				("OEBPS/p.xhtml", page.as_bytes().to_vec()),
				("OEBPS/c.xhtml", GOOD_PAGE.as_bytes().to_vec()),
			],
		);
		let (world, verdict) = friend_book(&bytes);
		assert!(assert_nothing_runs(&world, &verdict), "{:?}", verdict.reasons);
	}

	#[test]
	fn a_manifest_child_of_any_name_is_a_resource_for_the_reader_and_a_page_for_nobody() {
		// epub-rs makes a resource of every child of `<manifest>`, whatever it is called, and a
		// spine item of every child of `<spine>`; the reader then serves `p.dat` as `text/html`
		let opf = opf_of(
			"",
			r#"<x id="p" href="p.dat" media-type="text/html"/><item id="c" href="c.xhtml" media-type="application/xhtml+xml"/>"#,
			r#"<itemref idref="p"/><itemref idref="c"/>"#,
		);
		let bytes = epub_at(
			"OEBPS/content.opf",
			&opf,
			&[
				("OEBPS/p.dat", b"x<img src=x onerror=alert(1)>".to_vec()),
				("OEBPS/c.xhtml", GOOD_PAGE.as_bytes().to_vec()),
			],
		);
		let (world, verdict) = friend_book(&bytes);
		assert!(assert_nothing_runs(&world, &verdict), "{:?}", verdict.reasons);
		// and with no other page there is no book
		let only = epub_at(
			"OEBPS/content.opf",
			&opf_of(
				"",
				r#"<x id="p" href="p.dat" media-type="text/html"/>"#,
				r#"<itemref idref="p"/>"#,
			),
			&[("OEBPS/p.dat", b"x<img src=x onerror=alert(1)>".to_vec())],
		);
		let (world, verdict) = friend_book(&only);
		assert!(!assert_nothing_runs(&world, &verdict));
		assert_eq!(verdict.reasons[0].code, code::NO_READABLE_PAGES);
	}

	#[test]
	fn a_type_in_another_case_or_with_a_space_in_it_is_no_way_round() {
		// attribute names are matched exactly by epub-rs: `Media-Type` is not `media-type`, so the
		// type the reader reads is `text/html`; and `&nbsp;` is read as a space
		for (items, readable) in [
			(
				r#"<item id="p" href="p.dat" Media-Type="image/png" media-type="text/html"/>"#,
				true,
			),
			(
				r#"<item id="p" href="p.dat" media-type="text/html&nbsp;"/>"#,
				false,
			),
			(
				r#"<item id="p" href="p.dat" media-type="TEXT/HTML"/>"#,
				true,
			),
		] {
			let opf = opf_of(
				"",
				&format!(
					"{items}<item id=\"c\" href=\"c.xhtml\" media-type=\"application/xhtml+xml\"/>"
				),
				r#"<itemref idref="p"/><itemref idref="c"/>"#,
			);
			let bytes = epub_at(
				"OEBPS/content.opf",
				&opf,
				&[
					("OEBPS/p.dat", b"x<img src=x onerror=alert(1)>".to_vec()),
					("OEBPS/c.xhtml", GOOD_PAGE.as_bytes().to_vec()),
				],
			);
			let (world, verdict) = friend_book(&bytes);
			// the book is made without the page, or refused (Stump's own metadata reader can't read
			// a package with `&nbsp;` in it): in neither is there anything that runs
			let made = assert_nothing_runs(&world, &verdict);
			assert!(made || !readable, "{items}: {:?}", verdict.reasons);
		}
	}

	#[test]
	fn a_decoy_rootfile_before_the_real_one_decides_nothing() {
		// the first `rootfile` that epub-rs matches exactly is the package; a look-alike in another
		// case in front of it is nobody's
		let container = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><ROOTFILE full-path="decoy.opf" media-type="application/oebps-package+xml"/><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
		let opf = opf_of(
			"",
			r#"<item id="p" href="p.xhtml" media-type="application/xhtml+xml"/><item id="c" href="c.xhtml" media-type="application/xhtml+xml"/>"#,
			r#"<itemref idref="p"/><itemref idref="c"/>"#,
		);
		let bytes = zip(&[
			("mimetype", b"application/epub+zip".to_vec()),
			("META-INF/container.xml", container.as_bytes().to_vec()),
			(
				"decoy.opf",
				opf_of("", "", "").as_bytes().to_vec(),
			),
			("OEBPS/content.opf", opf.as_bytes().to_vec()),
			("OEBPS/p.xhtml", SCRIPTED.as_bytes().to_vec()),
			("OEBPS/c.xhtml", GOOD_PAGE.as_bytes().to_vec()),
		]);
		let (world, verdict) = friend_book(&bytes);
		assert!(assert_nothing_runs(&world, &verdict), "{:?}", verdict.reasons);
	}

	#[test]
	fn a_name_with_a_question_mark_is_not_filed_with_its_scripts() {
		// epub-rs keeps the `?` in the path; the reader's address for it encodes it, and the route
		// decodes it again and serves `OEBPS/a?b.xhtml`
		let opf = opf_of(
			"",
			r#"<item id="p" href="a?b.xhtml" media-type="application/xhtml+xml"/><item id="h" href="a%23b.xhtml" media-type="application/xhtml+xml"/><item id="c" href="c.xhtml" media-type="application/xhtml+xml"/>"#,
			r#"<itemref idref="p"/><itemref idref="h"/><itemref idref="c"/>"#,
		);
		let bytes = epub_at(
			"OEBPS/content.opf",
			&opf,
			&[
				("OEBPS/a?b.xhtml", SCRIPTED.as_bytes().to_vec()),
				("OEBPS/a#b.xhtml", SCRIPTED.as_bytes().to_vec()),
				("OEBPS/c.xhtml", GOOD_PAGE.as_bytes().to_vec()),
			],
		);
		let (world, verdict) = friend_book(&bytes);
		assert!(assert_nothing_runs(&world, &verdict), "{:?}", verdict.reasons);
	}

	#[test]
	fn a_page_by_a_prefix_or_a_case_the_xml_reader_does_not_know_is_rebuilt_all_the_same() {
		// scripts by a prefix, in another case, in a namespace of their own, in a CDATA section, behind
		// a comment that a browser ends sooner than an XML reader does
		let pages = [
			r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:h="http://www.w3.org/1999/xhtml"><body><p>hello</p><h:script>alert(1)</h:script></body></html>"#,
			r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>hello</p><SCRIPT>alert(1)</SCRIPT></body></html>"#,
			r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>hello</p><svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg></body></html>"#,
			r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>hello</p><![CDATA[<script>alert(1)</script>]]><a href="javascript:alert(1)">x</a></body></html>"#,
			r#"<html><body><p>hello</p><!--><script>alert(1)</script>--></body></html>"#,
			r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>hello</p><img src="a.png" ONERROR="alert(1)"/><iframe srcdoc="&lt;script&gt;alert(1)&lt;/script&gt;"></iframe></body></html>"#,
		];
		for page in pages {
			let opf = opf_of(
				"",
				r#"<item id="p" href="p.xhtml" media-type="application/xhtml+xml"/><item id="c" href="c.xhtml" media-type="application/xhtml+xml"/>"#,
				r#"<itemref idref="p"/><itemref idref="c"/>"#,
			);
			let bytes = epub_at(
				"OEBPS/content.opf",
				&opf,
				&[
					("OEBPS/p.xhtml", page.as_bytes().to_vec()),
					("OEBPS/c.xhtml", GOOD_PAGE.as_bytes().to_vec()),
				],
			);
			let (world, verdict) = friend_book(&bytes);
			assert!(
				assert_nothing_runs(&world, &verdict),
				"{page}: {:?}",
				verdict.reasons
			);
		}
	}

	/// Not run by default: what the checks make of a real library (`STUMP_PROBE_CALIBRATE_DIR`, a
	/// folder of EPUBs and comics; `STUMP_PROBE_CALIBRATE_MAX` files, evenly spread). A book of
	/// the owner's that is refused for no good reason is a check that is too strict.
	#[test]
	#[ignore]
	fn calibration_on_a_real_library() {
		let Ok(dir) = std::env::var("STUMP_PROBE_CALIBRATE_DIR") else {
			return;
		};
		let max: usize = std::env::var("STUMP_PROBE_CALIBRATE_MAX")
			.ok()
			.and_then(|v| v.parse().ok())
			.unwrap_or(100);
		let mut found: Vec<PathBuf> = walkdir::WalkDir::new(&dir)
			.into_iter()
			.flatten()
			.filter(|e| e.file_type().is_file())
			.map(|e| e.into_path())
			.filter(|p| {
				p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
					matches!(e.to_ascii_lowercase().as_str(), "epub" | "cbz")
				})
			})
			.collect();
		found.sort();
		let step = (found.len() / max.max(1)).max(1);
		let sample: Vec<&PathBuf> = found.iter().step_by(step).take(max).collect();
		let out = tempfile::tempdir().unwrap();
		let env = Env {
			intake_root: PathBuf::from(&dir),
			out_dir: out.path().join("out"),
			config: StumpConfig {
				config_dir: out.path().join("config").to_string_lossy().into_owned(),
				..StumpConfig::default()
			},
		};
		let (mut passed, mut refused) = (0usize, 0usize);
		let mut by_code: std::collections::BTreeMap<String, usize> = Default::default();
		let mut slowest = (0u64, String::new());
		for path in &sample {
			let relative = path
				.strip_prefix(&dir)
				.unwrap()
				.to_string_lossy()
				.into_owned();
			let (sha256, size) = sha256_of(path).unwrap();
			let request = Request {
				v: CONTRACT_VERSION,
				id: "0123abcd-0000-4000-8000-000000000001".to_string(),
				kind: Kind::Screen,
				input: relative.clone(),
				name: path.file_name().unwrap().to_string_lossy().into_owned(),
				size,
				sha256,
				limits: Limits::default(),
				approver: false,
				active_content: ActiveContent::Strip,
				cbr_to_cbz: true,
				file: None,
			};
			let verdict = run(&request, &env);
			if verdict.usage.wall_ms > slowest.0 {
				slowest = (verdict.usage.wall_ms, relative.clone());
			}
			if verdict.status == Status::Ok {
				passed += 1;
			} else {
				refused += 1;
				let reason = verdict
					.reasons
					.first()
					.cloned()
					.unwrap_or_else(|| Reason::new("?", "?"));
				*by_code.entry(reason.code.clone()).or_default() += 1;
				println!("REFUSED {relative}: {} {}", reason.code, reason.detail);
			}
		}
		println!(
			"calibration: {} of {} files checked, {passed} pass, {refused} refused {by_code:?}; slowest {} ms ({})",
			sample.len(),
			found.len(),
			slowest.0,
			slowest.1
		);
	}

	/// The names of what is in a zip file
	fn names_in(path: &Path) -> Vec<String> {
		let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
		(0..archive.len())
			.map(|i| archive.by_index(i).unwrap().name().to_string())
			.collect()
	}

	#[test]
	fn a_friends_comic_is_filed_with_its_pages_alone() {
		let world = world();
		let meta = FileMetadata {
			title: Some("Filed Title".to_string()),
			..FileMetadata::default()
		};
		let bytes = zip(&[
			("001.png", png(30, 40)),
			("sub/002.png", png(30, 40)),
			("OEBPS/page.xhtml", b"<html><body><script>alert(1)</script></body></html>".to_vec()),
			("scripts/x.js", b"alert(1)".to_vec()),
			("readme.txt", b"hello".to_vec()),
			("ComicInfo.xml", b"<ComicInfo><Title>Hostile</Title></ComicInfo>".to_vec()),
			(".hidden.png", png(30, 40)),
		]);
		let request = file_request(&world, "friend.cbz", &bytes, meta.clone());
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		let made = out_file(&world, &verdict);
		let mut names = names_in(&made);
		names.sort();
		// the pages, and the metadata file that was written anew
		assert_eq!(names, vec!["001.png", "ComicInfo.xml", "sub/002.png"], "{names:?}");
		let mut archive = zip::ZipArchive::new(File::open(&made).unwrap()).unwrap();
		let mut info = String::new();
		archive.by_name("ComicInfo.xml").unwrap().read_to_string(&mut info).unwrap();
		assert!(info.contains("Filed Title") && !info.contains("Hostile"), "{info}");
		assert_eq!(verdict.book.unwrap().pages, 2);

		// the comic of somebody who files their own books is copied as it is (nothing else changes)
		let mut request = file_request(&world, "mine.cbz", &bytes, meta);
		request.approver = true;
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		let names = names_in(&out_file(&world, &verdict));
		assert!(names.iter().any(|n| n == "OEBPS/page.xhtml") && names.iter().any(|n| n == "readme.txt"), "{names:?}");
	}

	#[test]
	fn a_friends_comic_with_no_page_is_refused_and_one_with_odd_names_loses_them() {
		let world = world();
		let bytes = zip(&[("OEBPS/page.xhtml", b"<html/>".to_vec()), ("a.txt", b"x".to_vec())]);
		let request = file_request(&world, "none.cbz", &bytes, FileMetadata::default());
		assert_ne!(run(&request, &world.env).status, Status::Ok);
		assert!(!plain_entry_name("../x.png") && !plain_entry_name("/x.png") && !plain_entry_name("a/../x.png"));
		assert!(plain_entry_name("a/b/001.png") && !plain_entry_name(""));
	}

	#[test]
	fn a_rar_comic_is_converted_with_its_pages_alone_for_a_friend() {
		// the fixture holds files that are not pages (a favicon and the junk of a Mac's archive)
		let data = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("integration-tests/data");
		let world = world();
		let mut request = world.stage("x.cbr", b"unused");
		request.cbr_to_cbz = true;
		for fixture in ["book.rar", "book-complex-tree.rar"] {
			let input = data.join(fixture);
			let friend = world.root.path().join(format!("{fixture}.friend.cbz"));
			request.approver = false;
			cbr_to_cbz(&input, &friend, &request).unwrap();
			let own = world.root.path().join(format!("{fixture}.own.cbz"));
			request.approver = true;
			cbr_to_cbz(&input, &own, &request).unwrap();
			let friends = names_in(&friend);
			let owns = names_in(&own);
			assert!(!owns.is_empty(), "{fixture}: the control has entries");
			assert!(friends.iter().all(|n| is_page(n)), "{fixture}: {friends:?}");
			let pages: Vec<String> = owns.iter().filter(|n| is_page(n)).cloned().collect();
			assert_eq!(friends, pages, "{fixture}");
		}
	}
}
