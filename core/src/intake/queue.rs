//! The server's side of the isolated checker (see [`super::probe`]): asking for a check, waiting for
//! the verdict, and not believing a word of it before it has been looked at.
//!
//! For the books of people who need approval the server reads **none** of the bytes. It counts and
//! hashes what arrived (that is not reading a book), writes a request, and gets a verdict from a
//! probe: either a container of its own that reads the shared spool (`queue`), or a child process of
//! the server (`subprocess`, for a server without the second container). The analysis then works
//! from the verdict (the book's metadata, hash, title page), and filing is a probe job too: the file
//! that goes into the library is one the probe made, from the reviewed metadata.
//!
//! A verdict is data from a process that has just read a hostile file. It is accepted only when it
//! parses (no field it doesn't know), answers this request (ticket, kind, the file's hash and size),
//! comes from the same build as this server, and, for a file job, names a file in the spool that is
//! there, has the size and the hash the verdict says. Its texts are cut again here.

use std::path::{Path, PathBuf};

use models::shared::intake::IntakeFile;
use thiserror::Error;
use uuid::Uuid;

use super::probe::{
	check,
	contract::{
		code, cut_chars, is_safe_relative, probe_revision, ActiveContent, Book, Kind,
		Limits, Reason, Request, Status, Verdict, CONTRACT_VERSION, MAX_VERDICT_BYTES,
	},
	job,
	runner::{self, Spawn, Spool, OFFLINE_AFTER_SECS},
	zipscan,
};
use crate::{config::StumpConfig, intake::write::FileMetadata, CoreError, Ctx};

const MIB: u64 = 1024 * 1024;
/// How long a ticket may wait in the queue, on top of the time its own check may take
const QUEUE_WAIT_SECS: u64 = 30 * 60;
/// How many times a check that the checker couldn't make (it was short of memory) is asked again
const RETRIES: usize = 2;

/// How the books of people who need approval are checked (`intake_probe_mode`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
	Off,
	InProcess,
	Subprocess,
	Queue,
}

pub fn mode(config: &StumpConfig) -> Mode {
	match config.intake_probe_mode.as_str() {
		"inprocess" => Mode::InProcess,
		"subprocess" => Mode::Subprocess,
		"queue" => Mode::Queue,
		_ => Mode::Off,
	}
}

/// Whether the books of people who need approval are read by the checker (and not by the server)
pub fn sandboxed(config: &StumpConfig) -> bool {
	matches!(mode(config), Mode::Subprocess | Mode::Queue)
}

/// Whether the server may read the books of people who need approval itself: only where their
/// uploaders are trusted (`inprocess`). With the checker on (`subprocess`, `queue`) it reads them,
/// and with checking `off` (the way back from it) nothing reads them: such a book is neither
/// analysed nor filed, whatever was uploaded before the setting was changed
pub fn reads_here(config: &StumpConfig) -> bool {
	matches!(mode(config), Mode::InProcess)
}

/// What somebody is told when a book of a person who needs approval would have to be read with
/// checking off
pub const OFF_TEXT: &str = "Checking is turned off on this server, so books from people who need approval aren't read or filed (the owner can turn it on)";

/// The folder the server and the checker share: `queue/`, `running/`, `done/`, `out/`, `worker.json`
pub fn spool(config: &StumpConfig) -> Spool {
	Spool::new(config.config_directory().join("workers").join("intake"))
}

/// The inbox, as the checker sees it
pub fn intake_root(config: &StumpConfig) -> PathBuf {
	config.config_directory().join("intake")
}

/// What the checker is held to: the configuration, under the ceilings the checker has
pub fn limits_of(config: &StumpConfig) -> Limits {
	Limits {
		max_entries: config.intake_probe_max_entries,
		max_entry_bytes: config.intake_probe_max_entry_mb.saturating_mul(MIB),
		max_unpacked_bytes: config.intake_probe_max_unpacked_mb.saturating_mul(MIB),
		max_pages: config.intake_probe_max_pages,
		max_image_pixels: config.intake_probe_max_image_mp.saturating_mul(1_000_000),
		max_file_bytes: config.intake_max_file_mb.saturating_mul(MIB),
		timeout_secs: config.intake_probe_timeout_secs,
		mem_mb: config.intake_probe_mem_mb,
		..Limits::default()
	}
	.clamped()
}

fn active_content_of(config: &StumpConfig) -> ActiveContent {
	if config.intake_active_content.eq_ignore_ascii_case("block") {
		ActiveContent::Block
	} else {
		ActiveContent::Strip
	}
}

/// What a stored verdict was made under: a verdict is used again only if all of it is the same
pub fn fingerprint(config: &StumpConfig) -> String {
	format!(
		"{}|{}|{:?}|{}",
		probe_revision(),
		serde_json::to_string(&limits_of(config)).unwrap_or_default(),
		active_content_of(config),
		config.intake_cbr_to_cbz
	)
}

/// Why a check could not be made (not why a book was refused: that is a verdict)
#[derive(Debug, Error)]
pub enum CheckError {
	/// The checker is not there, or not the same build: try again later
	#[error("{0}")]
	Offline(String),
	#[error("{0}")]
	Failed(String),
}

impl From<CheckError> for CoreError {
	fn from(error: CheckError) -> Self {
		CoreError::BadRequest(error.to_string())
	}
}

fn failed(detail: impl std::fmt::Display) -> CheckError {
	CheckError::Failed(detail.to_string())
}

/// Whether a check can be made now
pub fn ready(config: &StumpConfig) -> Result<(), CheckError> {
	match mode(config) {
		Mode::Queue => match spool(config).heartbeat_age() {
			Some((age, version))
				if age <= OFFLINE_AFTER_SECS && version == probe_revision() =>
			{
				Ok(())
			},
			Some((age, _)) if age <= OFFLINE_AFTER_SECS => Err(CheckError::Offline(
				"Checking is out of date: the checker is another version than the server"
					.to_string(),
			)),
			_ => Err(CheckError::Offline("Checking is offline".to_string())),
		},
		Mode::Subprocess => {
			let exe = exe_of(config);
			if exe.is_file() {
				Ok(())
			} else {
				Err(CheckError::Offline("Checking is not available".to_string()))
			}
		},
		_ => Ok(()),
	}
}

fn exe_of(config: &StumpConfig) -> PathBuf {
	config
		.intake_probe_exe
		.as_ref()
		.map(PathBuf::from)
		.or_else(|| std::env::current_exe().ok())
		.unwrap_or_default()
}

/// The text a verdict that refused a book is shown as
pub fn refusal_text(verdict: &Verdict) -> String {
	let said = verdict
		.reasons
		.first()
		.map(|r| r.detail.clone())
		.filter(|d| !d.is_empty())
		.unwrap_or_else(|| "it could not be checked".to_string());
	format!("The book was checked and not accepted: {said}")
}

fn cut(text: &mut String, chars: usize) {
	*text = text
		.chars()
		.filter(|c| !c.is_control())
		.take(chars)
		.collect();
}

/// What the server keeps of a verdict: its texts cut to what a book has
fn clean(verdict: &mut Verdict) {
	verdict.reasons.truncate(20);
	verdict.warnings.truncate(30);
	for reason in verdict
		.reasons
		.iter_mut()
		.chain(verdict.warnings.iter_mut())
	{
		cut(&mut reason.code, 40);
		cut(&mut reason.detail, 400);
	}
	if let Some(book) = verdict.book.as_mut() {
		if let Some(metadata) = book.metadata.as_mut() {
			metadata.cap();
		}
		for text in [&mut book.original_title, &mut book.title_page_title]
			.into_iter()
			.flatten()
		{
			cut(text, 300);
		}
		book.title_page_authors.truncate(20);
		book.title_page_authors.iter_mut().for_each(|a| cut(a, 200));
		book.active_content.items.truncate(30);
		for item in book.active_content.items.iter_mut() {
			cut(&mut item.kind, 40);
			cut(&mut item.entry, 200);
		}
		cut(&mut book.format, 10);
		// the texts of a verdict that name no file: a hash is 64 characters, whatever a
		// compromised checker made of it
		cut(&mut book.sha256, 64);
		if let Some(hash) = book.stump_hash.as_mut() {
			cut(hash, 64);
		}
		if let Some(output) = book.output.as_mut() {
			cut(&mut output.format, 10);
			cut(&mut output.sha256, 64);
			if let Some(from) = output.converted_from.as_mut() {
				cut(from, 10);
			}
		}
	}
}

/// Whether a verdict is what it must be to be believed
async fn validate(
	spool: &Spool,
	request: &Request,
	mut verdict: Verdict,
) -> Result<Verdict, CheckError> {
	let invalid = |why: &str| {
		tracing::warn!(
			ticket = request.id,
			why,
			"The checker's verdict is not valid"
		);
		Ok(Verdict::rejected(
			request,
			vec![Reason::new(
				code::VERDICT_INVALID,
				"The checker's answer could not be used",
			)],
		))
	};
	if verdict.v != CONTRACT_VERSION
		|| verdict.id != request.id
		|| verdict.kind != request.kind
	{
		return invalid("not an answer to this request");
	}
	if verdict.probe != probe_revision() {
		return Err(CheckError::Offline(
			"Checking is out of date: the checker is another version than the server"
				.to_string(),
		));
	}
	clean(&mut verdict);
	if verdict.status != Status::Ok {
		return Ok(verdict);
	}
	let Some(book) = verdict.book.as_ref() else {
		return invalid("a pass with no book");
	};
	if !matches!(book.format.as_str(), "epub" | "cbz" | "cbr" | "pdf") {
		return invalid("a format that is not one");
	}
	if request.kind == Kind::Screen {
		if book.sha256 != request.sha256 || book.size != request.size {
			return invalid("another file than the one asked about");
		}
		return Ok(verdict);
	}
	// a file job: the file that is to go in the library is where it says, and is what it says
	let Some(output) = book.output.as_ref() else {
		return invalid("a file job with no file");
	};
	let wanted = format!("{}/000.", request.id);
	let ext = output.path.strip_prefix(&wanted).unwrap_or("");
	if !is_safe_relative(&output.path)
		|| !matches!(ext, "epub" | "cbz" | "cbr" | "pdf")
		|| ext != output.format
	{
		return invalid("a file that is not in its folder");
	}
	let path = spool.out().join(&output.path);
	let hashed = {
		let path = path.clone();
		tokio::task::spawn_blocking(move || job::sha256_of(&path))
			.await
			.map_err(failed)?
	};
	let Ok((sha256, size)) = hashed else {
		return invalid("a file that is not there");
	};
	if sha256 != output.sha256 || size != output.size {
		return invalid("a file that is not the one it says");
	}
	Ok(verdict)
}

async fn submit_queued(
	spool: &Spool,
	request: &Request,
	bytes: &[u8],
) -> Result<Verdict, CheckError> {
	let name = format!("{}.json", request.id);
	let waiting = spool.queue().join(&name);
	let done = spool.done().join(&name);
	let (waiting_for_write, to_write) = (waiting.clone(), bytes.to_vec());
	tokio::task::spawn_blocking(move || {
		runner::write_atomic(&waiting_for_write, &to_write)
	})
	.await
	.map_err(failed)?
	.map_err(failed)?;
	let wait = request.limits.deadline_secs(request.size) + QUEUE_WAIT_SECS;
	let started = std::time::Instant::now();
	loop {
		if done.exists() {
			let read = {
				let done = done.clone();
				tokio::task::spawn_blocking(move || {
					use std::io::Read;
					let mut buf = Vec::new();
					std::fs::File::open(&done)
						.and_then(|f| f.take(MAX_VERDICT_BYTES + 1).read_to_end(&mut buf))
						.map(|_| buf)
				})
				.await
				.map_err(failed)?
			};
			let _ = tokio::fs::remove_file(&done).await;
			let buf = read.map_err(failed)?;
			if buf.len() as u64 > MAX_VERDICT_BYTES {
				return Ok(Verdict::rejected(
					request,
					vec![Reason::new(
						code::VERDICT_INVALID,
						"The checker's answer is too large",
					)],
				));
			}
			return Ok(serde_json::from_slice(&buf).unwrap_or_else(|_| {
				Verdict::rejected(
					request,
					vec![Reason::new(
						code::VERDICT_INVALID,
						"The checker's answer is not a verdict",
					)],
				)
			}));
		}
		if started.elapsed().as_secs() > wait {
			let _ = tokio::fs::remove_file(&waiting).await;
			return Err(failed("The checker did not answer in time"));
		}
		tokio::time::sleep(std::time::Duration::from_millis(500)).await;
	}
}

async fn submit_subprocess(
	ctx: &Ctx,
	spool: &Spool,
	request: &Request,
	bytes: &[u8],
) -> Result<Verdict, CheckError> {
	let file = spool.running().join(format!("{}.json", request.id));
	{
		let (file, bytes) = (file.clone(), bytes.to_vec());
		tokio::task::spawn_blocking(move || runner::write_atomic(&file, &bytes))
			.await
			.map_err(failed)?
			.map_err(failed)?;
	}
	let spawn = Spawn {
		exe: exe_of(&ctx.config),
		intake_root: intake_root(&ctx.config),
		// children are started from threads that come and go: the deadline is what stops one
		die_with_parent: false,
	};
	let (spool, request_for_run, file_for_run) =
		(spool.clone(), request.clone(), file.clone());
	let verdict = tokio::task::spawn_blocking(move || {
		runner::run_ticket(&spawn, &spool, &file_for_run, &request_for_run)
	})
	.await
	.map_err(failed)?;
	let _ = tokio::fs::remove_file(&file).await;
	Ok(verdict)
}

/// Asks the checker, and returns a verdict that can be believed
pub async fn submit(ctx: &Ctx, request: Request) -> Result<Verdict, CheckError> {
	ready(&ctx.config)?;
	let spool = spool(&ctx.config);
	{
		let spool = spool.clone();
		tokio::task::spawn_blocking(move || spool.ensure())
			.await
			.map_err(failed)?
			.map_err(failed)?;
	}
	for attempt in 0..=RETRIES {
		// every try is a ticket of its own
		let mut ticket = request.clone();
		ticket.id = Uuid::new_v4().to_string();
		let bytes = serde_json::to_vec(&ticket).map_err(failed)?;
		let verdict = match mode(&ctx.config) {
			Mode::Queue => submit_queued(&spool, &ticket, &bytes).await?,
			Mode::Subprocess => submit_subprocess(ctx, &spool, &ticket, &bytes).await?,
			_ => return Err(CheckError::Offline("Checking is not on".to_string())),
		};
		let verdict = validate(&spool, &ticket, verdict).await?;
		if verdict.status == Status::Retry {
			if attempt < RETRIES {
				continue;
			}
			return Ok(Verdict::rejected(
				&ticket,
				vec![Reason::new(
					code::TOO_DEMANDING,
					"The checker was short of memory too often to check this book now",
				)],
			));
		}
		return Ok(verdict);
	}
	Err(failed("The check could not be made"))
}

/// The path of a staged file relative to the inbox, as the checker is asked about it
fn relative_input(
	ctx: &Ctx,
	staging_dir: &str,
	staged_name: &str,
) -> Result<String, CheckError> {
	let full = Path::new(staging_dir).join(staged_name);
	let relative = full
		.strip_prefix(intake_root(&ctx.config))
		.map_err(|_| failed("a staged file outside the inbox"))?;
	let text = relative.to_string_lossy().replace('\\', "/");
	if is_safe_relative(&text) {
		Ok(text)
	} else {
		Err(failed("a staged file with a path that leaves the inbox"))
	}
}

/// A request about one staged file. What the server does to the file is count and hash it: that is
/// not reading a book
async fn request_for(
	ctx: &Ctx,
	staging_dir: &str,
	file: &IntakeFile,
	kind: Kind,
	metadata: Option<FileMetadata>,
) -> Result<Request, CheckError> {
	let input = relative_input(ctx, staging_dir, &file.staged_name)?;
	let path = intake_root(&ctx.config).join(&input);
	let (sha256, size) = tokio::task::spawn_blocking(move || job::sha256_of(&path))
		.await
		.map_err(failed)?
		.map_err(|_| failed("The staged file can't be read"))?;
	Ok(Request {
		v: CONTRACT_VERSION,
		id: Uuid::new_v4().to_string(),
		kind,
		input,
		name: file.original_name.clone(),
		size,
		sha256,
		limits: limits_of(&ctx.config),
		approver: false,
		active_content: active_content_of(&ctx.config),
		cbr_to_cbz: ctx.config.intake_cbr_to_cbz,
		file: metadata.map(|metadata| super::probe::contract::FileJob { metadata }),
	})
}

/// The most a stored screening takes of an item's `files` column. The column is read whole each
/// time the item is loaded (the inbox reloads every 1.5 s while something moves), and a person can
/// have thousands of files in it, so what is kept of a book is a summary, not what its file said
const STORED_VERDICT_BYTES: usize = 48 * 1024;

/// A verdict as it is kept: what the analysis needs again, in a few KB. The texts are cut shorter
/// than the server uses them once (a description to 8,000 characters), the lists are shortened,
/// and if that is still more than the budget the longest parts go first
pub fn trimmed(verdict: &Verdict) -> Verdict {
	let mut kept = verdict.clone();
	kept.reasons.truncate(5);
	kept.warnings.truncate(12);
	for reason in kept.reasons.iter_mut().chain(kept.warnings.iter_mut()) {
		cut(&mut reason.detail, 300);
	}
	let size =
		|verdict: &Verdict| serde_json::to_vec(verdict).map_or(usize::MAX, |v| v.len());
	if let Some(book) = kept.book.as_mut() {
		book.title_page_authors.truncate(10);
		book.active_content.items.truncate(10);
		if let Some(metadata) = book.metadata.as_mut() {
			if let Some(summary) = metadata.summary.as_mut() {
				cut_chars(summary, 8_000);
			}
			for (list, count) in [
				(&mut metadata.writers, 20),
				(&mut metadata.genres, 40),
				(&mut metadata.tags, 40),
			] {
				if let Some(items) = list {
					items.truncate(count);
				}
			}
		}
	}
	// still over the budget: the parts that can be worked out again, or done without, go
	for step in 0..4 {
		if size(&kept) <= STORED_VERDICT_BYTES {
			break;
		}
		match step {
			0 => {
				if let Some(summary) = kept
					.book
					.as_mut()
					.and_then(|b| b.metadata.as_mut())
					.and_then(|m| m.summary.as_mut())
				{
					cut_chars(summary, 2_000);
				}
			},
			1 => {
				if let Some(metadata) =
					kept.book.as_mut().and_then(|b| b.metadata.as_mut())
				{
					for list in [&mut metadata.genres, &mut metadata.tags] {
						if let Some(items) = list {
							items.truncate(10);
						}
					}
				}
			},
			2 => kept.warnings.truncate(3),
			_ => {
				if let Some(book) = kept.book.as_mut() {
					book.metadata = None;
				}
			},
		}
	}
	kept
}

/// What is stored of a screening in an item's file
fn stored(config: &StumpConfig, verdict: &Verdict) -> serde_json::Value {
	serde_json::json!({ "fingerprint": fingerprint(config), "verdict": trimmed(verdict) })
}

/// The verdict stored for a file, if it was made under the same checker and limits as now
pub fn stored_verdict(config: &StumpConfig, file: &IntakeFile) -> Option<Verdict> {
	let value = file.screening.as_ref()?;
	if value.get("fingerprint")?.as_str()? != fingerprint(config) {
		return None;
	}
	serde_json::from_value(value.get("verdict")?.clone()).ok()
}

/// The screening of a staged file: the stored one when there is one made under the same checker and
/// limits (a file is checked once, however often its item is analysed again), else a new one, which
/// is stored in the file
pub async fn screened(
	ctx: &Ctx,
	staging_dir: &str,
	file: &mut IntakeFile,
) -> Result<Verdict, CheckError> {
	if let Some(verdict) = stored_verdict(&ctx.config, file) {
		return Ok(verdict);
	}
	let request = request_for(ctx, staging_dir, file, Kind::Screen, None).await?;
	let verdict = submit(ctx, request).await?;
	// stored whatever it said: a book that was refused is refused for good (a hostile book that
	// crashed or timed the checker out must not cost another check every time it is analysed
	// again). What could not be asked at all is an error, and is not stored.
	file.screening = Some(stored(&ctx.config, &verdict));
	Ok(verdict)
}

/// What a file job made: the verdict, and the file it names (in the spool, until it is placed)
pub struct Made {
	pub verdict: Verdict,
	pub path: PathBuf,
}

/// The file that goes into the library for a staged file, made by the checker from the reviewed
/// metadata
pub async fn make(
	ctx: &Ctx,
	staging_dir: &str,
	file: &IntakeFile,
	metadata: FileMetadata,
) -> Result<Made, CoreError> {
	let request = request_for(ctx, staging_dir, file, Kind::File, Some(metadata)).await?;
	let verdict = submit(ctx, request).await?;
	if verdict.status != Status::Ok {
		return Err(CoreError::BadRequest(format!(
			"'{}': {}",
			file.original_name,
			refusal_text(&verdict)
		)));
	}
	let output = verdict
		.book
		.as_ref()
		.and_then(|book| book.output.as_ref())
		.ok_or_else(|| CoreError::BadRequest("The checker made no file".to_string()))?;
	let path = spool(&ctx.config).out().join(&output.path);
	Ok(Made { verdict, path })
}

/// What the checker made for the files of one item, in the order of the files. The folders of the
/// tickets are removed when this is dropped: the files are placed in the library by then, or the
/// filing was given up (and tries again with new ones)
#[derive(Default)]
pub struct Spooled(Vec<Made>);

impl Spooled {
	pub fn push(&mut self, made: Made) {
		self.0.push(made);
	}

	/// The hash the checker says the file it made has: the file is hashed again right before it is
	/// copied into the library, and must still be the one the verdict was checked against
	pub fn expected_hash(&self, index: usize) -> Option<String> {
		self.0
			.get(index)?
			.verdict
			.book
			.as_ref()?
			.output
			.as_ref()
			.map(|output| output.sha256.clone())
	}

	pub fn get(&self, index: usize) -> Option<&Made> {
		self.0.get(index)
	}
}

impl Drop for Spooled {
	fn drop(&mut self) {
		for made in &self.0 {
			if let Some(dir) = made.path.parent() {
				// only ever a ticket's own folder: `<spool>/out/<ticket>`
				if dir.parent().is_some_and(|out| out.ends_with("out")) {
					let _ = std::fs::remove_dir_all(dir);
				}
			}
		}
	}
}

/// The warnings of a verdict, as the lines an approver reads
pub fn warning_lines(verdict: &Verdict) -> Vec<String> {
	verdict
		.warnings
		.iter()
		.filter(|w| {
			matches!(
				w.code.as_str(),
				code::ACTIVE_CONTENT
					| code::ACTIVE_CONTENT_UNSTRIPPABLE
					| code::REMOTE_RESOURCES
					| code::MALFORMED_XHTML
					| code::DROPPED_FILES
			)
		})
		.map(|w| w.detail.clone())
		.collect()
}

/// The biggest EPUB looked at for what its owner is told about it
const OWN_BOOK_SCAN_BYTES: u64 = 300 * MIB;

/// What a book that the server reads itself carries (the book of somebody who files their own
/// books): the scripts and other active content in its pages, told to whoever reads its card. It is
/// filed as it is: only the books of people who need approval have theirs taken out. A file that
/// isn't a well formed zip says nothing here (the readers have their own errors for it)
pub async fn own_book_warnings(path: &Path, config: &StumpConfig) -> Vec<String> {
	let (path, limits) = (path.to_path_buf(), limits_of(config));
	tokio::task::spawn_blocking(move || {
		if std::fs::metadata(&path).map_or(true, |m| m.len() > OWN_BOOK_SCAN_BYTES) {
			return vec![];
		}
		let Ok(report) = zipscan::scan(&path, &limits) else {
			return vec![];
		};
		match check::check_epub(&path, &report, &limits, ActiveContent::Warn, true, None)
		{
			Ok(facts) => facts
				.warnings
				.into_iter()
				.filter(|w| {
					matches!(
						w.code.as_str(),
						code::ACTIVE_CONTENT
							| code::ACTIVE_CONTENT_UNSTRIPPABLE
							| code::REMOTE_RESOURCES
							| code::MALFORMED_XHTML
					)
				})
				.map(|w| w.detail)
				.collect(),
			Err(_) => vec![],
		}
	})
	.await
	.unwrap_or_default()
}

/// The book of a passing verdict
pub fn book_of(verdict: &Verdict) -> Option<&Book> {
	(verdict.status == Status::Ok)
		.then_some(verdict.book.as_ref())
		.flatten()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::intake::probe::contract::{Book, BookMetadata, Output, Usage};

	fn request() -> Request {
		Request {
			v: CONTRACT_VERSION,
			id: "0123abcd-0000-4000-8000-0000000000aa".to_string(),
			kind: Kind::Screen,
			input: "u/i/001_book.epub".to_string(),
			name: "book.epub".to_string(),
			size: 100,
			sha256: "ab".repeat(32),
			limits: Limits::default(),
			approver: false,
			active_content: ActiveContent::Strip,
			cbr_to_cbz: true,
			file: None,
		}
	}

	fn pass(request: &Request) -> Verdict {
		Verdict {
			v: CONTRACT_VERSION,
			id: request.id.clone(),
			kind: request.kind,
			probe: probe_revision().to_string(),
			status: Status::Ok,
			reasons: vec![],
			warnings: vec![],
			book: Some(Book {
				format: "epub".to_string(),
				sha256: request.sha256.clone(),
				size: request.size,
				..Book::default()
			}),
			usage: Usage::default(),
		}
	}

	#[tokio::test]
	async fn somebody_who_files_their_own_books_is_told_what_is_in_them() {
		use crate::intake::probe::job::tests::{epub_with, CLEAN, SCRIPTED};
		let dir = tempfile::tempdir().unwrap();
		let config = StumpConfig::default();
		let scripted = dir.path().join("scripted.epub");
		std::fs::write(&scripted, epub_with(SCRIPTED)).unwrap();
		let lines = own_book_warnings(&scripted, &config).await;
		assert!(
			lines
				.iter()
				.any(|l| l.contains("scripts or other active content")),
			"{lines:?}"
		);
		// nothing is taken out: it is only told
		assert_eq!(std::fs::read(&scripted).unwrap(), epub_with(SCRIPTED));
		let clean = dir.path().join("clean.epub");
		std::fs::write(&clean, epub_with(CLEAN)).unwrap();
		assert!(own_book_warnings(&clean, &config).await.is_empty());
		// what isn't a zip, or isn't there, says nothing
		let junk = dir.path().join("junk.epub");
		std::fs::write(&junk, b"not a zip at all, but long enough to be looked at")
			.unwrap();
		assert!(own_book_warnings(&junk, &config).await.is_empty());
		assert!(own_book_warnings(&dir.path().join("none.epub"), &config)
			.await
			.is_empty());
	}

	#[test]
	fn what_is_stored_of_a_verdict_is_a_summary_of_a_few_kb() {
		let request = request();
		let mut verdict = pass(&request);
		{
			let book = verdict.book.as_mut().unwrap();
			book.pages = 12;
			book.stump_hash = Some("f".repeat(64));
			book.metadata = Some(BookMetadata {
				title: Some("A title".to_string()),
				summary: Some(format!("{}\nsecond paragraph", "é".repeat(20_000))),
				writers: Some(vec!["w".repeat(200); 50]),
				genres: Some(vec!["g".repeat(100); 100]),
				tags: Some(vec!["t".repeat(100); 100]),
				..BookMetadata::default()
			});
			book.title_page_authors = vec!["a".repeat(200); 20];
		}
		verdict.warnings = vec![Reason::new(code::NESTED_ARCHIVE, "x".repeat(400)); 30];
		let kept = trimmed(&verdict);
		let size = serde_json::to_vec(&kept).unwrap().len();
		assert!(size <= STORED_VERDICT_BYTES, "{size} bytes");
		// what the analysis uses again is there
		let book = kept.book.as_ref().unwrap();
		assert_eq!((book.format.as_str(), book.pages), ("epub", 12));
		assert_eq!(book.sha256, request.sha256);
		assert!(book.stump_hash.is_some());
		let metadata = book.metadata.as_ref().unwrap();
		assert_eq!(metadata.title.as_deref(), Some("A title"));
		assert!(metadata.summary.as_ref().unwrap().chars().count() <= 8_000);
		assert!(kept.warnings.len() <= 12);
		// and the whole of a book that says little is kept as it is
		let small = pass(&request);
		assert_eq!(
			serde_json::to_value(trimmed(&small)).unwrap(),
			serde_json::to_value(&small).unwrap()
		);
		// a screening is stored trimmed, and read back the same under the same checker
		let config = StumpConfig::default();
		let stored = stored(&config, &verdict);
		let mut file = IntakeFile::default();
		file.screening = Some(stored);
		assert!(stored_verdict(&config, &file).is_some());
	}

	#[tokio::test]
	async fn a_verdict_is_believed_only_when_it_answers_this_request() {
		let dir = tempfile::tempdir().unwrap();
		let spool = Spool::new(dir.path());
		spool.ensure().unwrap();
		let request = request();
		let good = validate(&spool, &request, pass(&request)).await.unwrap();
		assert_eq!(good.status, Status::Ok);
		// another ticket, another kind, another file, another format, no book
		let mut other = pass(&request);
		other.id = "ffff0000-0000-4000-8000-000000000000".to_string();
		let mut file_kind = pass(&request);
		file_kind.kind = Kind::File;
		let mut changed = pass(&request);
		changed.book.as_mut().unwrap().sha256 = "00".repeat(32);
		let mut bad_size = pass(&request);
		bad_size.book.as_mut().unwrap().size = 5;
		let mut format = pass(&request);
		format.book.as_mut().unwrap().format = "exe".to_string();
		let mut empty = pass(&request);
		empty.book = None;
		for bad in [other, file_kind, changed, bad_size, format, empty] {
			let verdict = validate(&spool, &request, bad).await.unwrap();
			assert_eq!(verdict.status, Status::Reject);
			assert_eq!(verdict.reasons[0].code, code::VERDICT_INVALID);
		}
		// a checker of another build is not an answer at all
		let mut old = pass(&request);
		old.probe = "0000000".to_string();
		assert!(matches!(
			validate(&spool, &request, old).await,
			Err(CheckError::Offline(_))
		));
	}

	#[tokio::test]
	async fn the_file_a_file_job_names_must_be_there_and_be_what_it_says() {
		let dir = tempfile::tempdir().unwrap();
		let spool = Spool::new(dir.path());
		spool.ensure().unwrap();
		let mut request = request();
		request.kind = Kind::File;
		let ticket_dir = spool.out().join(&request.id);
		std::fs::create_dir_all(&ticket_dir).unwrap();
		std::fs::write(ticket_dir.join("000.epub"), b"the made file").unwrap();
		let (sha256, size) = job::sha256_of(&ticket_dir.join("000.epub")).unwrap();
		let with_output = |path: &str, format: &str, sha: &str, size: u64| {
			let mut verdict = pass(&request);
			verdict.book.as_mut().unwrap().output = Some(Output {
				path: path.to_string(),
				format: format.to_string(),
				sha256: sha.to_string(),
				size,
				converted_from: None,
				pages: 1,
			});
			verdict
		};
		let own = format!("{}/000.epub", request.id);
		let good = validate(&spool, &request, with_output(&own, "epub", &sha256, size))
			.await
			.unwrap();
		assert_eq!(good.status, Status::Ok);
		for bad in [
			// a path out of the ticket's folder, another ticket's, a wrong format, a wrong hash or size
			with_output("../x/000.epub", "epub", &sha256, size),
			with_output(
				"ffff0000-0000-4000-8000-000000000000/000.epub",
				"epub",
				&sha256,
				size,
			),
			with_output(&own, "pdf", &sha256, size),
			with_output(&own, "epub", &"00".repeat(32), size),
			with_output(&own, "epub", &sha256, size + 1),
			with_output(&format!("{}/000.cbz", request.id), "cbz", &sha256, size),
		] {
			let verdict = validate(&spool, &request, bad).await.unwrap();
			assert_eq!(
				verdict.reasons.first().map(|r| r.code.as_str()),
				Some(code::VERDICT_INVALID)
			);
		}
	}

	#[test]
	fn what_a_verdict_says_is_cut_to_what_a_book_has() {
		let request = request();
		let mut verdict = pass(&request);
		verdict.warnings = (0..100)
			.map(|i| {
				Reason::new(
					code::ACTIVE_CONTENT,
					format!("{i}{}\u{7}", "x".repeat(10_000)),
				)
			})
			.collect();
		let book = verdict.book.as_mut().unwrap();
		book.title_page_title = Some("t".repeat(5000));
		book.title_page_authors = (0..100).map(|_| "a".repeat(1000)).collect();
		clean(&mut verdict);
		assert_eq!(verdict.warnings.len(), 30);
		assert!(verdict
			.warnings
			.iter()
			.all(|w| w.detail.chars().count() <= 400 && !w.detail.contains('\u{7}')));
		let book = verdict.book.unwrap();
		assert_eq!(book.title_page_title.unwrap().chars().count(), 300);
		assert_eq!(book.title_page_authors.len(), 20);
	}

	#[test]
	fn the_mode_decides_who_reads_the_books() {
		let mut config = StumpConfig::default();
		for (text, expected, sandbox) in [
			("off", Mode::Off, false),
			("inprocess", Mode::InProcess, false),
			("subprocess", Mode::Subprocess, true),
			("queue", Mode::Queue, true),
			("anything else", Mode::Off, false),
		] {
			config.intake_probe_mode = text.to_string();
			assert_eq!(mode(&config), expected);
			assert_eq!(sandboxed(&config), sandbox);
			// the server reads a friend's book itself only where uploaders are trusted: never with
			// checking off, and never where the checker reads it
			assert_eq!(reads_here(&config), expected == Mode::InProcess);
		}
		// the limits come from the configuration, under the ceilings
		config.intake_probe_max_entries = 5;
		config.intake_probe_max_unpacked_mb = 99_999_999;
		let limits = limits_of(&config);
		assert_eq!(limits.max_entries, 5);
		assert_eq!(limits.max_unpacked_bytes, Limits::hard().max_unpacked_bytes);
	}

	#[test]
	fn a_queued_checker_that_is_not_beating_is_offline() {
		let dir = tempfile::tempdir().unwrap();
		let mut config = StumpConfig::default();
		config.config_dir = dir.path().to_string_lossy().into_owned();
		config.intake_probe_mode = "queue".to_string();
		assert!(matches!(ready(&config), Err(CheckError::Offline(_))));
		let spool = spool(&config);
		spool.ensure().unwrap();
		std::fs::write(
			spool.heartbeat(),
			serde_json::json!({"version": probe_revision(), "updated": 1, "busy": false})
				.to_string(),
		)
		.unwrap();
		assert!(
			matches!(ready(&config), Err(CheckError::Offline(_))),
			"a heartbeat from long ago"
		);
		let now = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_secs();
		std::fs::write(
			spool.heartbeat(),
			serde_json::json!({"version": probe_revision(), "updated": now, "busy": false}).to_string(),
		)
		.unwrap();
		assert!(ready(&config).is_ok());
		std::fs::write(
			spool.heartbeat(),
			serde_json::json!({"version": "other", "updated": now, "busy": false})
				.to_string(),
		)
		.unwrap();
		assert!(
			matches!(ready(&config), Err(CheckError::Offline(m)) if m.contains("out of date"))
		);
	}
}
