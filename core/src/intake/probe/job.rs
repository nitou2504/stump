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
	check::{self, check_cbr, check_cbz, check_epub, is_hidden, is_image_name},
	contract::{
		code, probe_revision, ActiveContent, ActiveSummary, Book, BookMetadata, Kind, Output, Reason,
		Request, Status, Usage, Verdict, CONTRACT_VERSION,
	},
	zipscan,
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

fn hex(bytes: &[u8]) -> String {
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
	} else if head.starts_with(b"Rar!\x1A\x07\x00") || head.starts_with(b"Rar!\x1A\x07\x01\x00") {
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
		("pdf", Container::Pdf) | ("zip", Container::Zip) | ("rar-or-zip", Container::Rar | Container::Zip) => {},
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
			if report.entries.iter().any(|e| e.name == "META-INF/container.xml") {
				let facts = check_epub(path, &report, &limits, request.active_content, approver, replacements)?;
				warnings.extend(facts.warnings);
				book.active_content = facts.active;
				"epub"
			} else if request.name.to_ascii_lowercase().ends_with(".epub") {
				return Err(fail(code::NO_PACKAGE, "The book has no META-INF/container.xml"));
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
			let code = if format == "pdf" { code::PDF_UNREADABLE } else { code::UNREADABLE };
			return Err(fail(code, format!("Stump can't read the book: {error}")));
		},
	};
	if format == "pdf" {
		check::within_pages(i64::from(processed.pages), &limits)?;
		book.pages = processed.pages;
	} else if format == "epub" {
		book.pages = processed.pages;
		book.original_title = lookup::original_title_of(&linked);
		if let Some(page) = read_title_page(&linked) {
			book.title_page_title = page.title;
			book.title_page_authors = page.authors;
		}
	} else if book.pages != processed.pages {
		// the check counted the pages one way and Stump's reader another: they must agree
		warnings.push(fail(
			code::PAGE_COUNT,
			format!("The check counted {} pages and Stump's reader {}", book.pages, processed.pages),
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
					SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
				)?;
				writer.write_all(bytes)?;
			}
			Ok(())
		},
	)
	.map_err(|e| fail(code::OUTPUT_CHECK_FAILED, format!("The book could not be rebuilt: {e}")))?;
	std::fs::rename(&tmp, path).map_err(io_reason)
}

/// A comic in a RAR archive as a ZIP one: its pages stored, the rest deflated, in the order and
/// under the names it had. The pages were all unpacked once already, in the check
fn cbr_to_cbz(input: &Path, output: &Path, request: &Request) -> Result<(), Reason> {
	let limits = request.limits.clamped();
	let unreadable = |error: unrar::error::UnrarError| {
		fail(code::UNREADABLE, format!("The RAR file can't be read: {error}"))
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
		let declared = entry.unpacked_size;
		if declared > limits.max_entry_bytes {
			return Err(fail(code::ENTRY_TOO_LARGE, "An entry is too large"));
		}
		let (data, next) = header.read().map_err(unreadable)?;
		archive = next;
		if data.len() as u64 > declared {
			return Err(fail(code::SIZE_MISMATCH, "An entry holds more than it says"));
		}
		written = written.saturating_add(data.len() as u64);
		if written > limits.max_unpacked_bytes {
			return Err(fail(code::UNPACKED_TOO_LARGE, "The archive unpacks to too much"));
		}
		let method = if is_image_name(&name) {
			CompressionMethod::Stored
		} else {
			CompressionMethod::Deflated
		};
		writer
			.start_file(name.as_str(), SimpleFileOptions::default().compression_method(method))
			.map_err(|e| fail(code::OUTPUT_CHECK_FAILED, e.to_string()))?;
		writer.write_all(&data).map_err(io_reason)?;
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
	let wrote = |error: crate::intake::write::WriteError| {
		fail(code::OUTPUT_CHECK_FAILED, format!("The metadata could not be written: {error}"))
	};
	match format.as_str() {
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
	let mut checked = inspect(&out_path, &as_request, env, runtime, None)
		.map_err(|reason| fail(code::OUTPUT_CHECK_FAILED, format!("{}: {}", reason.code, reason.detail)))?;
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
	// what was found in the input is what the approver is told
	checked.book.active_content = ActiveSummary {
		stripped: request.active_content == ActiveContent::Strip && first.book.active_content.found > 0,
		..first.book.active_content.clone()
	};
	checked.warnings = first.warnings;
	if convert {
		checked
			.warnings
			.push(fail(code::CONVERTED_CBR, "The comic was converted from RAR to ZIP"));
	}
	Ok(checked)
}

fn valid_request(request: &Request) -> Result<(), Reason> {
	if request.v != CONTRACT_VERSION {
		return Err(fail(code::UNREADABLE, "A request of another version"));
	}
	if request.id.is_empty()
		|| request.id.len() > 64
		|| !request.id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
	{
		return Err(fail(code::UNREADABLE, "A request with a ticket that is not one"));
	}
	if !super::contract::is_safe_relative(&request.input) {
		return Err(fail(code::BAD_PATH, "A request with a path that leaves the inbox"));
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
				format!("The file is larger than the {} MB a book can be", limits.max_file_bytes / MIB),
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
			let (sha256, size) = sha256_of(&self.root.path().join("inbox/u/i").join(name)).unwrap();
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
				.start_file(*name, SimpleFileOptions::default().compression_method(method))
				.unwrap();
			writer.write_all(data).unwrap();
		}
		writer.finish().unwrap().into_inner()
	}

	pub(crate) fn epub_with(chapter: &str) -> Vec<u8> {
		let container = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
		let opf = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:1</dc:identifier><dc:title>A Book</dc:title><dc:creator>Ana Writer</dc:creator><dc:language>en</dc:language><dc:description>It has &lt;b&gt;markup&lt;/b&gt; in it</dc:description></metadata><manifest><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/></spine></package>"#;
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
		assert_eq!(run(&request, &world.env).reasons[0].code, code::INPUT_CHANGED);
		let mut request = world.stage("book2.epub", &epub_with(CLEAN));
		request.size += 1;
		assert_eq!(run(&request, &world.env).reasons[0].code, code::INPUT_CHANGED);
		// and requests that leave the inbox
		let mut request = world.stage("book3.epub", &epub_with(CLEAN));
		request.input = "../outside".to_string();
		assert_eq!(run(&request, &world.env).reasons[0].code, code::BAD_PATH);
	}

	#[test]
	fn a_name_that_lies_about_the_file_is_refused() {
		let world = world();
		let request = world.stage("book.pdf", &epub_with(CLEAN));
		assert_eq!(run(&request, &world.env).reasons[0].code, code::TYPE_MISMATCH);
		let request = world.stage("book.epub", b"hello, this is not a book at all, really");
		assert_eq!(run(&request, &world.env).reasons[0].code, code::TYPE_MISMATCH);
	}

	#[test]
	fn a_comic_is_screened_and_its_pages_counted_the_same_way_stump_does() {
		let world = world();
		let bytes = zip(&[("001.png", png(30, 40)), ("002.png", png(30, 40)), ("ComicInfo.xml", b"<ComicInfo><Series>S</Series><Number>2</Number></ComicInfo>".to_vec())]);
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

	fn file_request(world: &World, name: &str, bytes: &[u8], metadata: FileMetadata) -> Request {
		let mut request = world.stage(name, bytes);
		request.kind = Kind::File;
		request.file = Some(FileJob { metadata });
		request
	}

	fn out_file(world: &World, verdict: &Verdict) -> PathBuf {
		let output = verdict.book.as_ref().unwrap().output.as_ref().expect("an output");
		assert!(output.path.starts_with("0123abcd-0000-4000-8000-000000000001/000."));
		world.env.out_dir.join(output.path.rsplit('/').next().unwrap())
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
		assert_eq!(sha256_of(&out).unwrap().0, book.output.as_ref().unwrap().sha256);
		// the file that came out carries the new title, the plain description, and no script
		let (title, text) = {
			let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
			let opf = bounded::read_to_string_capped(archive.by_name("OEBPS/content.opf").unwrap(), MIB).unwrap();
			let page = bounded::read_to_string_capped(archive.by_name("OEBPS/c1.xhtml").unwrap(), MIB).unwrap();
			(opf, page)
		};
		assert!(title.contains("Filed Title") && title.contains("A plain description"));
		assert!(!title.contains("markup"));
		assert!(!text.to_lowercase().contains("script") && !text.contains("onerror") && text.contains("<p>hello</p>"));
		// and it passes the checks as a file of its own, with the structure of one Stump wrote
		let again = world.stage("again.epub", &bytes);
		let screened = run(&again, &world.env);
		assert_eq!(screened.status, Status::Ok, "{:?}", screened.reasons);
		assert_eq!(screened.book.unwrap().active_content.found, 0);
	}

	#[test]
	fn a_book_with_a_script_it_cannot_clean_is_not_made() {
		let world = world();
		let sneaky = r#"<html><body><p>one<img src=x onerror=alert(1)></body>"#;
		let request = file_request(&world, "book.epub", &epub_with(sneaky), FileMetadata::default());
		let verdict = run(&request, &world.env);
		assert_eq!(verdict.status, Status::Reject);
		assert_eq!(verdict.reasons[0].code, code::ACTIVE_CONTENT_UNSTRIPPABLE);
		assert!(!world.env.out_dir.join("000.epub").exists());
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
		let info = bounded::read_to_string_capped(archive.by_name("ComicInfo.xml").unwrap(), MIB).unwrap();
		assert!(info.contains("<Series>Series</Series>") && !info.contains("script"));
	}
}
