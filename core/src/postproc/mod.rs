//! Post-processing of library files: manga CBZ pages reduced to 16 grey levels (in Stump, see
//! [`grey16`]) and scanned PDFs sent to the PDF worker container for OCR and shrinking (see
//! [`pdf_queue`], `docker/Dockerfile.pdf-worker` and `scripts/fork/workers/`). Before a processed
//! file replaces a book's file, the original is copied into Stump's trash folder, where
//! *Deleted books* lists it for the retention period and it can be put back (see [`swap`] and
//! [`crate::trash::held`]).
//!
//! Post-processing jobs run on their own worker (one at a time), so a long 16-grey pass never
//! holds up scans, and every conversion thread runs at idle CPU and I/O priority.

pub mod grey16;
pub mod pdf_queue;
pub mod priority;
pub mod swap;

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use async_graphql::{Enum, SimpleObject};
use chrono::{DateTime, Utc};
use models::{
	entity::{
		library, library_config, media, media_metadata, post_processing_task as task,
	},
	shared::{
		enums::LogLevel,
		post_processing::{
			PostProcessKind, PostProcessMode, PostProcessStatus, PostProcessingConfig,
		},
		trash::HeldFileReason,
	},
};
use sea_orm::{prelude::*, ActiveValue::Set, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::{
	config::StumpConfig,
	job::{
		error::JobError, JobContext, JobExecuteLog, JobLifecycle, JobOutputExt,
		JobProgress, JobTaskOutput, WorkingState,
	},
	media::processor::{generate_hashes, MediaProcessorOptions},
	trash::held,
	CoreResult,
};

/// Which books a post-processing run looks at
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Enum)]
pub enum PostProcessScope {
	/// Books a scan added after the step was turned on (what scans enqueue)
	New,
	/// Every matching book of the library (the library settings' "run now")
	All,
}

/// What one post-processing run does
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessJobConfig {
	pub library_id: String,
	pub scope: PostProcessScope,
	pub grey16: bool,
	pub pdf_ocr: bool,
	pub pdf_shrink: bool,
	/// Only estimate the 16-grey savings (a few pages per volume); nothing is changed
	pub dry_run: bool,
}

impl PostProcessJobConfig {
	/// The run a scan enqueues when it added books: every step set to new books
	pub fn for_new_books(
		library_id: String,
		config: &PostProcessingConfig,
	) -> Option<Self> {
		config.any_new().then(|| Self {
			library_id,
			scope: PostProcessScope::New,
			grey16: config.grey16 == PostProcessMode::New,
			pdf_ocr: config.pdf_ocr == PostProcessMode::New,
			pdf_shrink: config.pdf_shrink == PostProcessMode::New,
			dry_run: false,
		})
	}

	pub fn describe(&self) -> String {
		let mut steps = Vec::new();
		if self.grey16 {
			steps.push(if self.dry_run {
				"16-grey estimate"
			} else {
				"16 greys"
			});
		}
		if self.pdf_ocr {
			steps.push("PDF OCR");
		}
		if self.pdf_shrink {
			steps.push("PDF shrink");
		}
		let scope = match self.scope {
			PostProcessScope::New => "new books",
			PostProcessScope::All => "whole library",
		};
		format!("Post-processing ({scope}): {}", steps.join(", "))
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, SimpleObject)]
pub struct PostProcessOutput {
	/// CBZ volumes looked at
	pub volumes: u64,
	/// Volumes whose 16-grey copy replaced the original
	pub converted: u64,
	/// Volumes with nothing to do (already 16 greys, no grey PNG pages)
	pub skipped: u64,
	/// Volumes processed but kept (the saving was too small)
	pub rejected: u64,
	pub failed: u64,
	/// Volumes estimated by a dry run
	pub estimated: u64,
	/// Sizes before and after (or expected after, for a dry run) of the processed volumes
	pub bytes_before: u64,
	pub bytes_after: u64,
	/// PDFs written to the PDF worker's queue
	pub pdfs_queued: u64,
}

impl JobOutputExt for PostProcessOutput {
	fn update(&mut self, updated: Self) {
		self.volumes += updated.volumes;
		self.converted += updated.converted;
		self.skipped += updated.skipped;
		self.rejected += updated.rejected;
		self.failed += updated.failed;
		self.estimated += updated.estimated;
		self.bytes_before += updated.bytes_before;
		self.bytes_after += updated.bytes_after;
		self.pdfs_queued += updated.pdfs_queued;
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grey16Task {
	pub task_id: String,
	pub media_id: String,
	pub path: String,
}

pub struct PostProcessJob {
	pub config: PostProcessJobConfig,
}

/// A replacement is worth it from this saving on (≥ 1 MB and ≥ 5 %)
pub fn grey16_worth_it(bytes_in: u64, bytes_out: u64) -> bool {
	let saved = bytes_in.saturating_sub(bytes_out) as f64;
	saved >= (1_000_000f64).max(0.05 * bytes_in as f64)
}

fn mb(bytes: u64) -> String {
	format!("{:.1} MB", bytes as f64 / 1e6)
}

/// Whether the library-relative path of a book matches one of the exclusions
pub fn is_excluded(library_path: &str, book_path: &str, exclude: &[String]) -> bool {
	let rel = book_path.strip_prefix(library_path).unwrap_or(book_path);
	let rel = if rel.starts_with('/') {
		rel.to_string()
	} else {
		format!("/{rel}")
	};
	exclude
		.iter()
		.filter(|pattern| !pattern.trim().is_empty())
		.any(|pattern| rel.contains(pattern.trim()))
}

/// The library and its post-processing settings
async fn load_library(
	conn: &DatabaseConnection,
	library_id: &str,
) -> Result<(library::Model, library_config::Model), JobError> {
	let library = library::Entity::find_by_id(library_id.to_string())
		.one(conn)
		.await?
		.ok_or_else(|| JobError::InitFailed(format!("library {library_id} not found")))?;
	let config = library_config::Entity::find_by_id(library.config_id)
		.one(conn)
		.await?
		.ok_or_else(|| JobError::InitFailed("library config not found".to_string()))?;
	Ok((library, config))
}

/// The existing tasks of one kind for a library, by book
async fn tasks_by_media(
	conn: &DatabaseConnection,
	library_id: &str,
	kind: PostProcessKind,
) -> Result<HashMap<String, task::Model>, JobError> {
	let rows = task::Entity::find()
		.filter(task::Column::LibraryId.eq(library_id))
		.filter(task::Column::Kind.eq(kind))
		.order_by_asc(task::Column::UpdatedAt)
		.all(conn)
		.await?;
	Ok(rows
		.into_iter()
		.filter_map(|row| row.media_id.clone().map(|id| (id, row)))
		.collect())
}

/// Whether an existing task keeps a book out of a run: a settled task for the same file
/// (same size), or any task at all for a new-books run (failures are retried by "run now")
fn already_handled(
	existing: Option<&task::Model>,
	book: &media::Model,
	scope: PostProcessScope,
) -> bool {
	let Some(row) = existing else {
		return false;
	};
	if matches!(
		row.status,
		PostProcessStatus::Pending | PostProcessStatus::Running
	) {
		return true;
	}
	match scope {
		PostProcessScope::New => row.status != PostProcessStatus::Estimated,
		PostProcessScope::All => {
			row.status.settles()
				&& (row.bytes_after == Some(book.size)
					|| row.bytes_before == Some(book.size))
		},
	}
}

/// What to write into a task row
#[derive(Default)]
pub struct TaskUpdate {
	pub status: Option<PostProcessStatus>,
	pub reason: Option<Option<String>>,
	pub bytes_before: Option<Option<i64>>,
	pub bytes_after: Option<Option<i64>>,
	pub details: Option<Option<serde_json::Value>>,
	pub path: Option<String>,
	pub steps: Option<Option<String>>,
	pub finished: bool,
}

/// Update a task row in place
pub async fn update_task(
	conn: &DatabaseConnection,
	id: &str,
	update: TaskUpdate,
) -> Result<(), DbErr> {
	let mut active = task::ActiveModel {
		id: Set(id.to_string()),
		..Default::default()
	};
	if let Some(status) = update.status {
		active.status = Set(status);
	}
	if let Some(reason) = update.reason {
		active.reason = Set(reason);
	}
	if let Some(bytes) = update.bytes_before {
		active.bytes_before = Set(bytes);
	}
	if let Some(bytes) = update.bytes_after {
		active.bytes_after = Set(bytes);
	}
	if let Some(details) = update.details {
		active.details = Set(details);
	}
	if let Some(path) = update.path {
		active.path = Set(path);
	}
	if let Some(steps) = update.steps {
		active.steps = Set(steps);
	}
	if update.finished {
		active.finished_at = Set(Some(Utc::now()));
	} else if update.status.is_some() {
		active.finished_at = Set(None);
	}
	active.update(conn).await?;
	Ok(())
}

/// The task row of a book and kind (created when missing), reset to `status`
async fn upsert_task(
	conn: &DatabaseConnection,
	existing: Option<&task::Model>,
	library_id: &str,
	book: &media::Model,
	kind: PostProcessKind,
	status: PostProcessStatus,
	steps: Option<String>,
	details: Option<serde_json::Value>,
) -> Result<String, DbErr> {
	if let Some(row) = existing {
		update_task(
			conn,
			&row.id,
			TaskUpdate {
				status: Some(status),
				reason: Some(None),
				bytes_before: Some(Some(book.size)),
				bytes_after: Some(None),
				details: Some(details),
				path: Some(book.path.clone()),
				steps: Some(steps),
				finished: false,
			},
		)
		.await?;
		return Ok(row.id.clone());
	}
	let id = Uuid::new_v4().to_string();
	task::ActiveModel {
		id: Set(id.clone()),
		library_id: Set(library_id.to_string()),
		media_id: Set(Some(book.id.clone())),
		kind: Set(kind),
		steps: Set(steps),
		status: Set(status),
		path: Set(book.path.clone()),
		reason: Set(None),
		bytes_before: Set(Some(book.size)),
		bytes_after: Set(None),
		details: Set(details),
		finished_at: Set(None),
		..Default::default()
	}
	.insert(conn)
	.await?;
	Ok(id)
}

fn has_extension(book: &media::Model, ext: &str) -> bool {
	book.extension.eq_ignore_ascii_case(ext)
		|| book.path.to_ascii_lowercase().ends_with(&format!(".{ext}"))
}

/// Books of a library with the given extension, not removed; for new-books runs, added since
async fn candidates(
	conn: &DatabaseConnection,
	library_id: &str,
	ext: &str,
	since: Option<DateTime<Utc>>,
) -> Result<Vec<media::Model>, DbErr> {
	Ok(media::Entity::find()
		.filter(media::Column::LibraryId.eq(library_id))
		.filter(media::Column::DeletedAt.is_null())
		.filter(media::Column::Extension.is_in([ext.to_string(), ext.to_uppercase()]))
		.order_by_asc(media::Column::Path)
		.all(conn)
		.await?
		.into_iter()
		.filter(|book| has_extension(book, ext))
		// dates are compared here: SQLite would compare the stored text
		.filter(|book| {
			since.is_none_or(|since| book.created_at.with_timezone(&Utc) >= since)
		})
		.collect())
}

/// The file's size and modification time, to tell whether it changed while it was processed
fn file_stamp(path: &Path) -> std::io::Result<(u64, i64)> {
	let meta = std::fs::metadata(path)?;
	let mtime = meta
		.modified()?
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs() as i64)
		.unwrap_or_default();
	Ok((meta.len(), mtime))
}

/// After a swap: the book's size, modification time and hashes follow the new file, so the
/// next scan does not see it as changed. Pages, metadata and thumbnail stay as they are (the
/// processed file has the same pages and metadata)
pub async fn refresh_media_after_swap(
	conn: &DatabaseConnection,
	config: &StumpConfig,
	library_config: &library_config::Model,
	book: &media::Model,
) -> CoreResult<()> {
	let path = PathBuf::from(&book.path);
	let size = tokio::fs::metadata(&path).await?.len() as i64;
	let mut active = media::ActiveModel {
		id: Set(book.id.clone()),
		size: Set(size),
		modified_at: Set(Some(Utc::now().into())),
		..Default::default()
	};
	let wants_hash = book.hash.is_some() || library_config.generate_file_hashes;
	let wants_koreader =
		book.koreader_hash.is_some() || library_config.generate_koreader_hashes;
	if wants_hash || wants_koreader {
		let mut options = MediaProcessorOptions::new(library_config, config);
		options.generate_file_hashes = wants_hash;
		options.generate_koreader_hashes = wants_koreader;
		match generate_hashes(&path, options, config).await {
			Ok(hashes) => {
				if wants_hash {
					active.hash = Set(hashes.stump);
				}
				if wants_koreader {
					active.koreader_hash = Set(hashes.koreader);
				}
			},
			Err(error) => {
				tracing::warn!(?error, path = %book.path, "Could not rehash a processed book");
			},
		}
	}
	active.update(conn).await?;
	Ok(())
}

/// After a swap: the original held in the trash folder gets its row, so *Deleted books* lists
/// it (and can put it back). A failure only leaves an unlisted file in the trash folder
pub async fn record_held_original(
	conn: &DatabaseConnection,
	book: &media::Model,
	held_relative: &Path,
	bytes: u64,
	reason: HeldFileReason,
	steps: Option<String>,
	task_id: &str,
) {
	let title = held::book_title(conn, book).await;
	if let Err(error) = held::record(
		conn,
		book,
		title,
		held_relative,
		bytes,
		held::HeldFileInfo {
			reason,
			steps,
			task_id: Some(task_id.to_string()),
			held_by: None,
		},
	)
	.await
	{
		tracing::error!(?error, path = %book.path, held = %held_relative.display(), "Could not list a replaced original in Deleted books");
	}
}

impl PostProcessJob {
	/// Write the PDFs of this run to the PDF worker's queue
	async fn queue_pdfs(
		&self,
		ctx: &JobContext,
		library: &library::Model,
		config: &PostProcessingConfig,
	) -> Result<(u64, Vec<JobExecuteLog>), JobError> {
		let job = &self.config;
		let conn = ctx.conn();
		let mut logs = Vec::new();
		// (step, since) for the steps of this run
		let mut steps: Vec<(&str, Option<DateTime<Utc>>)> = Vec::new();
		if job.pdf_ocr {
			steps.push(("ocr", config.pdf_ocr_since));
		}
		if job.pdf_shrink {
			steps.push(("shrink", config.pdf_shrink_since));
		}
		if steps.is_empty() {
			return Ok((0, logs));
		}
		let since = match job.scope {
			PostProcessScope::All => None,
			PostProcessScope::New => {
				// books are new for a step from that step's own start
				let Some(earliest) = steps.iter().filter_map(|(_, s)| *s).min() else {
					return Ok((0, logs));
				};
				Some(earliest)
			},
		};
		let root = ctx.config().pdf_worker_directory();
		pdf_queue::ensure_dirs(&root).map_err(|e| JobError::InitFailed(e.to_string()))?;
		let books = candidates(conn, &library.id, "pdf", since).await?;
		let existing = tasks_by_media(conn, &library.id, PostProcessKind::Pdf).await?;
		let languages: HashMap<String, String> = media_metadata::Entity::find()
			.filter(
				media_metadata::Column::MediaId
					.is_in(books.iter().map(|b| b.id.clone()).collect::<Vec<_>>()),
			)
			.all(conn)
			.await?
			.into_iter()
			.filter_map(|m| Some((m.media_id?, m.language?)))
			.collect();
		let mut queued = 0;
		for book in books {
			let book_steps: Vec<String> = steps
				.iter()
				.filter(|(_, since)| match (job.scope, since) {
					(PostProcessScope::New, Some(since)) => {
						book.created_at.with_timezone(&Utc) >= *since
					},
					(PostProcessScope::New, None) => false,
					(PostProcessScope::All, _) => true,
				})
				.map(|(step, _)| String::from(*step))
				.collect();
			if book_steps.is_empty()
				|| already_handled(existing.get(&book.id), &book, job.scope)
			{
				continue;
			}
			let Ok((bytes, mtime)) = file_stamp(Path::new(&book.path)) else {
				logs.push(JobExecuteLog::warn(&format!(
					"{}: file not found, not queued",
					book.path
				)));
				continue;
			};
			let steps_label = book_steps
				.iter()
				.map(|s| s.to_uppercase())
				.collect::<Vec<_>>()
				.join("+");
			let task_id = upsert_task(
				conn,
				existing.get(&book.id),
				&library.id,
				&book,
				PostProcessKind::Pdf,
				PostProcessStatus::Pending,
				Some(steps_label),
				Some(
					serde_json::json!({ "request_bytes": bytes, "request_mtime": mtime }),
				),
			)
			.await?;
			let request = pdf_queue::PdfRequest {
				version: pdf_queue::QUEUE_VERSION,
				id: task_id.clone(),
				steps: book_steps,
				path: book.path.clone(),
				library_path: library.path.clone(),
				bytes,
				mtime,
				language: languages.get(&book.id).cloned(),
				created_at: Utc::now(),
			};
			if let Err(error) = pdf_queue::write_request(&root, &request) {
				update_task(
					conn,
					&task_id,
					TaskUpdate {
						status: Some(PostProcessStatus::Failed),
						reason: Some(Some(format!("could not queue: {error}"))),
						finished: true,
						..Default::default()
					},
				)
				.await?;
				continue;
			}
			queued += 1;
		}
		if queued > 0 {
			logs.push(JobExecuteLog::new(
				format!(
					"{queued} PDFs queued for the PDF worker in {}",
					root.display()
				),
				LogLevel::Info,
			));
		}
		Ok((queued, logs))
	}

	async fn run_grey16(
		&self,
		ctx: &JobContext,
		task: Grey16Task,
	) -> Result<PostProcessOutput, JobError> {
		let conn = ctx.conn();
		let mut output = PostProcessOutput {
			volumes: 1,
			..Default::default()
		};
		let Some(book) = media::Entity::find_by_id(task.media_id.clone())
			.one(conn)
			.await?
		else {
			update_task(
				conn,
				&task.task_id,
				TaskUpdate {
					status: Some(PostProcessStatus::Failed),
					reason: Some(Some("the book is gone".to_string())),
					finished: true,
					..Default::default()
				},
			)
			.await?;
			output.failed = 1;
			return Ok(output);
		};
		let src = PathBuf::from(&book.path);
		let threads = priority::worker_threads();

		if self.config.dry_run {
			let path = src.clone();
			let estimate =
				run_low_priority(move || grey16::estimate_volume(&path, 6)).await;
			match estimate {
				Ok((expected, report)) => {
					update_task(
						conn,
						&task.task_id,
						TaskUpdate {
							status: Some(PostProcessStatus::Estimated),
							reason: Some(Some(format!(
								"expected {} -> {} (from {} sampled pages)",
								mb(report.bytes_in),
								mb(expected),
								report.pages.values().sum::<u32>()
							))),
							bytes_after: Some(Some(expected as i64)),
							details: Some(serde_json::to_value(&report).ok()),
							finished: true,
							..Default::default()
						},
					)
					.await?;
					output.estimated = 1;
					output.bytes_before = report.bytes_in;
					output.bytes_after = expected;
				},
				Err(error) => {
					update_task(
						conn,
						&task.task_id,
						TaskUpdate {
							status: Some(PostProcessStatus::Failed),
							reason: Some(Some(error.to_string())),
							finished: true,
							..Default::default()
						},
					)
					.await?;
					output.failed = 1;
				},
			}
			return Ok(output);
		}

		update_task(
			conn,
			&task.task_id,
			TaskUpdate {
				status: Some(PostProcessStatus::Running),
				..Default::default()
			},
		)
		.await?;

		let work_dir = ctx.config().post_processing_work_directory();
		tokio::fs::create_dir_all(&work_dir)
			.await
			.map_err(|e| JobError::TaskFailed(e.to_string()))?;
		let work = work_dir.join(format!("{}.cbz", task.task_id));
		let before = file_stamp(&src).ok();

		let cancel = Arc::new(AtomicBool::new(false));
		let watcher = {
			let cancel = cancel.clone();
			let token = ctx.cancel_token.clone();
			tokio::spawn(async move {
				token.cancelled().await;
				cancel.store(true, Ordering::Relaxed);
			})
		};
		let converted = {
			let (src, work, cancel) = (src.clone(), work.clone(), cancel.clone());
			run_low_priority(move || {
				grey16::convert_volume(&src, &work, threads, &cancel)
			})
			.await
		};
		watcher.abort();

		let outcome = self
			.finish_grey16(ctx, &book, &task, converted, before, &work)
			.await;
		let _ = tokio::fs::remove_file(&work).await;
		let (status, reason, bytes_after, details) = outcome?;
		match status {
			PostProcessStatus::Done => {
				output.converted = 1;
				output.bytes_before = book.size as u64;
				output.bytes_after = bytes_after.unwrap_or_default() as u64;
			},
			PostProcessStatus::Skipped => output.skipped = 1,
			PostProcessStatus::Rejected => output.rejected = 1,
			_ => output.failed = 1,
		}
		update_task(
			conn,
			&task.task_id,
			TaskUpdate {
				status: Some(status),
				reason: Some(Some(reason)),
				bytes_after: Some(bytes_after),
				details: Some(details),
				finished: true,
				..Default::default()
			},
		)
		.await?;
		Ok(output)
	}

	#[allow(clippy::too_many_arguments)]
	async fn finish_grey16(
		&self,
		ctx: &JobContext,
		book: &media::Model,
		task: &Grey16Task,
		converted: Result<grey16::VolumeReport, grey16::Grey16Error>,
		before: Option<(u64, i64)>,
		work: &Path,
	) -> Result<
		(
			PostProcessStatus,
			String,
			Option<i64>,
			Option<serde_json::Value>,
		),
		JobError,
	> {
		let report = match converted {
			Ok(report) => report,
			Err(grey16::Grey16Error::Cancelled) => {
				return Err(JobError::Cancelled);
			},
			Err(error) => {
				return Ok((PostProcessStatus::Failed, error.to_string(), None, None));
			},
		};
		let details = serde_json::to_value(&report).ok();
		let pages = report.count(grey16::PageOutcome::Grey16);
		if pages == 0 {
			let reason = if report.count(grey16::PageOutcome::AlreadyGrey16) > 0 {
				"already 16 greys"
			} else {
				"no grey PNG page to convert"
			};
			return Ok((
				PostProcessStatus::Skipped,
				reason.to_string(),
				None,
				details,
			));
		}
		if !grey16_worth_it(report.bytes_in, report.bytes_out) {
			return Ok((
				PostProcessStatus::Rejected,
				format!(
					"saves only {} ({:.0}%)",
					mb(report.bytes_in.saturating_sub(report.bytes_out)),
					100.0 * report.bytes_in.saturating_sub(report.bytes_out) as f64
						/ report.bytes_in.max(1) as f64
				),
				Some(report.bytes_out as i64),
				details,
			));
		}
		let src = PathBuf::from(&book.path);
		if file_stamp(&src).ok() != before {
			return Ok((
				PostProcessStatus::Failed,
				"the book changed while it was converted".to_string(),
				None,
				details,
			));
		}
		let (library, library_config) =
			load_library(ctx.conn(), &book.library_id).await?;
		let (held_absolute, held_relative) =
			held::destination(ctx.config(), Path::new(&library.path), &src, Utc::now());
		let swapped = {
			let (src, work) = (src.clone(), work.to_path_buf());
			tokio::task::spawn_blocking(move || {
				swap::swap_into_place(&src, &work, &held_absolute)
			})
			.await
			.map_err(|e| JobError::TaskFailed(e.to_string()))?
		};
		match swapped {
			Ok(swap) => {
				record_held_original(
					ctx.conn(),
					book,
					&held_relative,
					swap.bytes_before,
					HeldFileReason::Grey16,
					None,
					&task.task_id,
				)
				.await;
				if let Err(error) = refresh_media_after_swap(
					ctx.conn(),
					ctx.config(),
					&library_config,
					book,
				)
				.await
				{
					tracing::warn!(?error, "Could not refresh a converted book");
				}
				tracing::info!(
					path = %book.path,
					before = swap.bytes_before,
					after = swap.bytes_after,
					task = %task.task_id,
					"Replaced a CBZ with its 16-grey copy"
				);
				Ok((
					PostProcessStatus::Done,
					format!(
						"{} -> {}; {pages} pages converted, {} colour pages kept",
						mb(swap.bytes_before),
						mb(swap.bytes_after),
						report.count(grey16::PageOutcome::Colour)
					),
					Some(swap.bytes_after as i64),
					details.map(|mut d| {
						d["held_path"] =
							serde_json::json!(held_relative.to_string_lossy());
						d
					}),
				))
			},
			Err(error) => Ok((
				PostProcessStatus::Failed,
				format!("could not replace the file: {error}"),
				None,
				details,
			)),
		}
	}
}

/// After a scan added books: enqueue the post-processing run for the steps set to new books
pub async fn enqueue_for_new_books(
	ctx: &JobContext,
	library_id: Option<String>,
	config: Option<&library_config::Model>,
) {
	let (Some(library_id), Some(config)) = (library_id, config) else {
		return;
	};
	let Some(job) =
		PostProcessJobConfig::for_new_books(library_id, &config.post_processing())
	else {
		return;
	};
	if let Err(error) = ctx.enqueue(crate::job::StumpJob::post_process(job)).await {
		tracing::error!(?error, "Failed to enqueue post-processing of new books");
	}
}

/// Run blocking work on its own thread at idle priority
async fn run_low_priority<T, F>(work: F) -> T
where
	F: FnOnce() -> T + Send + 'static,
	T: Send + 'static,
{
	let (tx, rx) = tokio::sync::oneshot::channel();
	std::thread::spawn(move || {
		priority::lower_current_thread();
		let _ = tx.send(work());
	});
	rx.await.expect("post-processing thread panicked")
}

#[async_trait::async_trait]
impl JobLifecycle for PostProcessJob {
	const NAME: &'static str = "post_processing";

	type Output = PostProcessOutput;
	type Task = Grey16Task;

	fn description(&self) -> Option<String> {
		Some(self.config.describe())
	}

	async fn init(
		&mut self,
		ctx: &JobContext,
	) -> Result<WorkingState<Self::Output, Self::Task>, JobError> {
		let conn = ctx.conn();
		let (library, library_config) =
			load_library(conn, &self.config.library_id).await?;
		let settings = library_config.post_processing();
		let mut output = PostProcessOutput::default();
		let mut logs = Vec::new();
		let mut tasks = std::collections::VecDeque::new();

		if !self.config.dry_run && (self.config.pdf_ocr || self.config.pdf_shrink) {
			let (queued, pdf_logs) = self.queue_pdfs(ctx, &library, &settings).await?;
			output.pdfs_queued = queued;
			logs.extend(pdf_logs);
		}

		if self.config.grey16 {
			let since = match self.config.scope {
				PostProcessScope::All => None,
				PostProcessScope::New => match settings.grey16_since {
					Some(since) => Some(since),
					None => {
						return Ok(WorkingState {
							output: Some(output),
							tasks,
							logs,
						})
					},
				},
			};
			let books = candidates(conn, &library.id, "cbz", since).await?;
			let existing =
				tasks_by_media(conn, &library.id, PostProcessKind::Grey16).await?;
			for book in books {
				if is_excluded(&library.path, &book.path, &settings.grey16_exclude) {
					continue;
				}
				let existing_row = existing.get(&book.id);
				let status = if self.config.dry_run {
					// an estimate never overwrites a real result
					if existing_row.is_some_and(|row| {
						!matches!(
							row.status,
							PostProcessStatus::Estimated | PostProcessStatus::Failed
						)
					}) {
						continue;
					}
					PostProcessStatus::Estimated
				} else {
					if already_handled(existing_row, &book, self.config.scope) {
						continue;
					}
					PostProcessStatus::Pending
				};
				let task_id = upsert_task(
					conn,
					existing_row,
					&library.id,
					&book,
					PostProcessKind::Grey16,
					status,
					None,
					None,
				)
				.await?;
				tasks.push_back(Grey16Task {
					task_id,
					media_id: book.id.clone(),
					path: book.path.clone(),
				});
			}
			if !tasks.is_empty() {
				ctx.report_progress(JobProgress::msg(&format!(
					"{} volumes to {}",
					tasks.len(),
					if self.config.dry_run {
						"estimate"
					} else {
						"convert"
					}
				)));
			}
		}

		Ok(WorkingState {
			output: Some(output),
			tasks,
			logs,
		})
	}

	async fn execute_task(
		&self,
		ctx: &JobContext,
		task: Self::Task,
	) -> Result<JobTaskOutput<Self>, JobError> {
		ctx.report_progress(JobProgress::msg(&task.path));
		let path = task.path.clone();
		let task_id = task.task_id.clone();
		match self.run_grey16(ctx, task).await {
			Ok(output) => Ok(JobTaskOutput {
				output,
				subtasks: vec![],
				logs: vec![],
			}),
			Err(JobError::Cancelled) => {
				let _ = update_task(
					ctx.conn(),
					&task_id,
					TaskUpdate {
						status: Some(PostProcessStatus::Failed),
						reason: Some(Some("cancelled".to_string())),
						finished: true,
						..Default::default()
					},
				)
				.await;
				Err(JobError::Cancelled)
			},
			Err(error) => {
				let _ = update_task(
					ctx.conn(),
					&task_id,
					TaskUpdate {
						status: Some(PostProcessStatus::Failed),
						reason: Some(Some(error.to_string())),
						finished: true,
						..Default::default()
					},
				)
				.await;
				Ok(JobTaskOutput {
					output: PostProcessOutput {
						volumes: 1,
						failed: 1,
						..Default::default()
					},
					subtasks: vec![],
					logs: vec![JobExecuteLog::error(format!("{path}: {error}"))],
				})
			},
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn worth_it_rule_needs_a_megabyte_and_five_percent() {
		// at least 1 MB and 5 %
		assert!(grey16_worth_it(100_000_000, 94_000_000));
		assert!(!grey16_worth_it(100_000_000, 96_000_000));
		assert!(!grey16_worth_it(10_000_000, 9_200_000));
		assert!(grey16_worth_it(10_000_000, 8_900_000));
	}

	#[test]
	fn exclusions_match_library_relative_paths() {
		let exclude = vec!["/Berserk/".to_string(), " ".to_string()];
		assert!(is_excluded(
			"/biblioteca/biblioteca",
			"/biblioteca/biblioteca/_Manga/Berserk/Berserk v01.cbz",
			&exclude
		));
		assert!(!is_excluded(
			"/biblioteca/biblioteca",
			"/biblioteca/biblioteca/_Manga/Yotsuba&!/Yotsuba&! v01.cbz",
			&exclude
		));
		assert!(is_excluded(
			"/lib",
			"/lib/Berserk/v1.cbz",
			&["/Berserk/".to_string()]
		));
	}

	#[test]
	fn stored_output_reads_back_as_post_processing() {
		let output = PostProcessOutput {
			volumes: 2,
			converted: 1,
			..Default::default()
		};
		let json = serde_json::to_vec(&output).unwrap();
		let read: crate::job::CoreJobOutput = serde_json::from_slice(&json).unwrap();
		assert!(
			matches!(read, crate::job::CoreJobOutput::PostProcess(o) if o.converted == 1)
		);
	}

	#[test]
	fn new_books_run_only_for_steps_set_to_new() {
		let mut config = PostProcessingConfig::default();
		assert!(PostProcessJobConfig::for_new_books("l".into(), &config).is_none());
		config.grey16 = PostProcessMode::New;
		let job = PostProcessJobConfig::for_new_books("l".into(), &config).unwrap();
		assert!(job.grey16 && !job.pdf_ocr && !job.pdf_shrink && !job.dry_run);
		assert_eq!(job.scope, PostProcessScope::New);
	}
}
