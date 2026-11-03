//! Deleted books (the trash).
//!
//! Removing a book (`deleteMedia`) only hides it: `media.deleted_at` and `deleted_by` are set,
//! its file stays where it is in the library and everything attached to it (reading progress,
//! annotations, bookmarks, tags, smart list matches) stays, so restoring it brings it back as it
//! was. Rescans keep it hidden.
//!
//! Files Stump takes out of a library are held in its own trash folder (`<config>/trash`, see
//! [`held`]) and listed on the same page: the originals post-processing replaced (16 greys, PDF
//! OCR or shrinking), and the version a put-back replaced. They can be put back.
//!
//! Both kinds stay for the retention period (14 days unless configured, 0 = until deleted by
//! hand); then the daily automatic purge deletes them, or they are deleted by hand. Deleting a
//! removed book permanently
//!
//! 1. renames its file to a hidden name next to itself (scans skip it from then on);
//! 2. deletes the book's row, which cascades to its metadata, reading sessions, annotations,
//!    bookmarks, tags, favorites and the rest, drops its virtual series if it was the last book
//!    in it, and logs the deletion in `media_purges` (who, when, what, where it was);
//! 3. deletes the file, and the book's held files, thumbnail, no-cover marker and pages in the
//!    PDF page cache.
//!
//! When the file can't be renamed or the data can't be deleted, nothing is deleted and the book
//! stays in *Deleted books* with the reason. A book whose file is already gone only has its data
//! deleted.

pub mod files;
pub mod held;
pub mod job;

use std::{
	collections::{HashMap, HashSet},
	io,
	path::{Path, PathBuf},
	str::FromStr,
};

use chrono::{DateTime, Duration, Timelike, Utc};
use croner::Cron;
use models::{
	entity::{library, media, media_purge, server_config},
	shared::trash::{
		TrashPurgeError, TrashPurgeSummary, DEFAULT_RETENTION_DAYS, MAX_RETENTION_DAYS,
	},
};
use sea_orm::{
	prelude::*, ActiveValue::Set, IntoActiveModel, QuerySelect, TransactionTrait,
};
use tokio::sync::Mutex;

use crate::{
	config::StumpConfig,
	image::{
		bump_media_thumbnail_fallbacks, remove_thumbnails, thumbnail::forget_coverless,
	},
	job::StumpJob,
	metadata::membership::{refresh_series, remove_series_thumbnails},
	CoreError, CoreResult, Ctx,
};

/// The automatic purge runs every day at 04:00 UTC
pub const PURGE_SCHEDULE: &str = "0 4 * * *";

/// How many books a purge loads at once
const PURGE_CHUNK: usize = 100;

/// Permanent deletions and put-backs run one at a time, so a book purged by hand while the
/// automatic purge runs is handled once
static PURGE_LOCK: Mutex<()> = Mutex::const_new(());

fn purge_schedule() -> Cron {
	Cron::from_str(PURGE_SCHEDULE).expect("the purge schedule is a valid cron expression")
}

/// The first automatic purge after `after` (or at it, when `inclusive`), on the whole second
pub fn next_purge_run(after: DateTime<Utc>, inclusive: bool) -> Option<DateTime<Utc>> {
	// croner keeps the sub-second part of the time it starts from
	let after = after.with_nanosecond(0).unwrap_or(after);
	purge_schedule()
		.find_next_occurrence(&after, inclusive)
		.ok()
}

/// When a book removed at `deleted_at` has been kept long enough (None: kept until it is
/// deleted by hand)
pub fn expires_at(
	deleted_at: DateTime<Utc>,
	retention_days: i32,
) -> Option<DateTime<Utc>> {
	(retention_days > 0).then(|| deleted_at + Duration::days(retention_days.into()))
}

/// The automatic purge that will delete a book removed at `deleted_at`: the first one once it
/// has expired (the next one, if it is overdue)
pub fn purge_at(
	deleted_at: DateTime<Utc>,
	retention_days: i32,
	now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
	let expires = expires_at(deleted_at, retention_days)?;
	if expires <= now {
		next_purge_run(now, false)
	} else {
		next_purge_run(expires, true)
	}
}

pub fn is_expired(
	deleted_at: DateTime<Utc>,
	retention_days: i32,
	now: DateTime<Utc>,
) -> bool {
	expires_at(deleted_at, retention_days).is_some_and(|expires| expires <= now)
}

/// How many days removed books are kept, from the server config
pub async fn retention_days<C: ConnectionTrait>(conn: &C) -> CoreResult<i32> {
	Ok(server_config::Entity::find()
		.one(conn)
		.await?
		.map_or(DEFAULT_RETENTION_DAYS, |config| config.trash_retention_days))
}

/// Sets how many days removed books are kept (0 = until deleted by hand)
pub async fn set_retention_days<C: ConnectionTrait>(
	conn: &C,
	days: i32,
) -> CoreResult<i32> {
	if !(0..=MAX_RETENTION_DAYS).contains(&days) {
		return Err(CoreError::BadRequest(format!(
			"Deleted books are kept 0 (never deleted automatically) to {MAX_RETENTION_DAYS} days"
		)));
	}
	let config = server_config::Entity::find()
		.one(conn)
		.await?
		.ok_or_else(|| CoreError::NotFound("Server configuration".to_string()))?;
	let mut active = config.into_active_model();
	active.trash_retention_days = Set(days);
	active.update(conn).await?;
	Ok(days)
}

/// The books whose time in *Deleted books* is over
pub async fn expired_book_ids<C: ConnectionTrait>(
	conn: &C,
	retention_days: i32,
	now: DateTime<Utc>,
) -> CoreResult<Vec<String>> {
	if retention_days <= 0 {
		return Ok(Vec::new());
	}
	// compared here rather than in SQL: the column is text, written with offsets
	let rows: Vec<(String, Option<DateTimeWithTimeZone>)> = media::Entity::find()
		.select_only()
		.column(media::Column::Id)
		.column(media::Column::DeletedAt)
		.filter(media::Column::DeletedAt.is_not_null())
		.into_tuple()
		.all(conn)
		.await?;
	Ok(rows
		.into_iter()
		.filter(|(_, deleted_at)| {
			deleted_at.is_some_and(|at| is_expired(at.to_utc(), retention_days, now))
		})
		.map(|(id, _)| id)
		.collect())
}

/// Whether the automatic purge hasn't run for a day (or ever), e.g. the server was down when it
/// was due
pub async fn purge_is_overdue<C: ConnectionTrait>(
	conn: &C,
	now: DateTime<Utc>,
) -> CoreResult<bool> {
	let last = server_config::Entity::find()
		.one(conn)
		.await?
		.and_then(|config| config.trash_last_purge_at);
	Ok(last.is_none_or(|last| now - last >= Duration::days(1)))
}

/// Records what an automatic purge did, for *Deleted books*
pub async fn record_purge<C: ConnectionTrait>(
	conn: &C,
	summary: TrashPurgeSummary,
) -> CoreResult<()> {
	let Some(config) = server_config::Entity::find().one(conn).await? else {
		return Ok(());
	};
	let mut active = config.into_active_model();
	active.trash_last_purge_at = Set(Some(summary.finished_at));
	active.trash_last_purge = Set(Some(summary));
	active.update(conn).await?;
	Ok(())
}

/// The automatic purge: a job when books or held files have expired (so it shows in the job
/// list with what it did), else only a note that nothing was due
pub async fn start_automatic_purge(ctx: &Ctx) -> CoreResult<()> {
	let conn = ctx.conn.as_ref();
	let retention = retention_days(conn).await?;
	if retention <= 0 {
		tracing::debug!("Deleted books are kept until deleted by hand");
		return Ok(());
	}
	let now = Utc::now();
	let expired = expired_book_ids(conn, retention, now).await?;
	let expired_held = held::expired_held_ids(conn, retention, now).await?;
	if expired.is_empty() && expired_held.is_empty() {
		tracing::debug!("No deleted book is due for permanent deletion");
		return record_purge(
			conn,
			TrashPurgeSummary {
				finished_at: now,
				..Default::default()
			},
		)
		.await;
	}
	tracing::info!(
		books = expired.len(),
		held_files = expired_held.len(),
		"Deleting expired books and held files in Deleted books permanently"
	);
	ctx.enqueue(StumpJob::TrashPurge).await
}

/// What happened to one book
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurgeOutcome {
	/// Its file and its data were deleted
	Deleted { bytes: u64 },
	/// Its file was already gone: only its data was deleted
	FileMissing,
	/// It stays in *Deleted books*, for this reason
	Kept(String),
}

#[derive(Debug, Clone)]
pub struct PurgedItem {
	pub media_id: String,
	pub title: String,
	pub outcome: PurgeOutcome,
}

/// What a purge did, book by book
#[derive(Debug, Clone, Default)]
pub struct PurgeReport {
	pub items: Vec<PurgedItem>,
}

impl PurgeReport {
	pub fn purged(&self) -> u64 {
		self.count(|o| matches!(o, PurgeOutcome::Deleted { .. }))
	}

	pub fn missing_files(&self) -> u64 {
		self.count(|o| matches!(o, PurgeOutcome::FileMissing))
	}

	pub fn kept(&self) -> u64 {
		self.count(|o| matches!(o, PurgeOutcome::Kept(_)))
	}

	/// The size of the book files deleted
	pub fn freed_bytes(&self) -> u64 {
		self.items
			.iter()
			.map(|item| match item.outcome {
				PurgeOutcome::Deleted { bytes } => bytes,
				_ => 0,
			})
			.sum()
	}

	/// The books kept, with why
	pub fn errors(&self) -> Vec<TrashPurgeError> {
		self.items
			.iter()
			.filter_map(|item| match &item.outcome {
				PurgeOutcome::Kept(error) => Some(TrashPurgeError {
					media_id: item.media_id.clone(),
					title: item.title.clone(),
					error: error.clone(),
				}),
				_ => None,
			})
			.collect()
	}

	fn count(&self, matches: impl Fn(&PurgeOutcome) -> bool) -> u64 {
		self.items
			.iter()
			.filter(|item| matches(&item.outcome))
			.count() as u64
	}
}

/// A book in *Deleted books*, with what a purge needs to know about it
#[derive(Debug, Clone)]
struct TrashedBook {
	media: media::Model,
	title: String,
	authors: Option<String>,
	library_name: String,
}

async fn load_trashed<C: ConnectionTrait>(
	conn: &C,
	ids: &[String],
) -> CoreResult<Vec<TrashedBook>> {
	let rows = media::ModelWithMetadata::find()
		.filter(media::Column::Id.is_in(ids.to_vec()))
		.filter(media::Column::DeletedAt.is_not_null())
		.into_model::<media::ModelWithMetadata>()
		.all(conn)
		.await?;
	let library_ids: HashSet<String> = rows
		.iter()
		.map(|row| row.media.library_id.clone())
		.collect();
	let libraries: HashMap<String, library::Model> = library::Entity::find()
		.filter(library::Column::Id.is_in(library_ids))
		.all(conn)
		.await?
		.into_iter()
		.map(|library| (library.id.clone(), library))
		.collect();
	Ok(rows
		.into_iter()
		.filter_map(|row| {
			let library = libraries.get(&row.media.library_id)?;
			let metadata = row.metadata.as_ref();
			let title = metadata
				.and_then(|m| m.title.clone())
				.filter(|title| !title.trim().is_empty())
				.unwrap_or_else(|| row.media.name.clone());
			let authors = metadata.and_then(|m| m.writers.clone());
			Some(TrashedBook {
				media: row.media,
				title,
				authors,
				library_name: library.name.clone(),
			})
		})
		.collect())
}

/// What the page cache may hold for a PDF, named after the file's current version
#[derive(Debug, Default)]
struct PdfCacheFiles {
	prefixes: Vec<String>,
	names: HashSet<String>,
}

fn pdf_cache_files(path: &Path, pages: i32, config: &StumpConfig) -> PdfCacheFiles {
	PdfCacheFiles {
		prefixes: crate::pdf_reader::document_version(path)
			.ok()
			.map(|version| format!("pdfr_{version}_"))
			.into_iter()
			.collect(),
		names: (1..=pages.max(0))
			.filter_map(|page| {
				crate::media::processor::pdf::page_cache_file_name(path, page, config)
			})
			.collect(),
	}
}

/// What to clean up once the books' data is gone
#[derive(Debug, Default)]
struct Leftovers {
	book_ids: Vec<String>,
	thumbnail_paths: Vec<PathBuf>,
	series_thumbnails: Vec<String>,
	pdf_cache: PdfCacheFiles,
}

/// Deletes books in *Deleted books* permanently (see the module docs), one by one: a book that
/// can't be deleted stays in *Deleted books* and the others go on. `purged_by` is the user
/// who asked, None for the automatic purge
pub async fn purge_books(
	conn: &DatabaseConnection,
	config: &StumpConfig,
	ids: &[String],
	purged_by: Option<&str>,
) -> CoreResult<PurgeReport> {
	let _guard = PURGE_LOCK.lock().await;
	let mut report = PurgeReport::default();
	let mut leftovers = Leftovers::default();

	let mut unique = HashSet::new();
	let ids: Vec<String> = ids
		.iter()
		.filter(|id| unique.insert(id.as_str()))
		.cloned()
		.collect();
	for chunk in ids.chunks(PURGE_CHUNK) {
		let books = load_trashed(conn, chunk).await?;
		let found: HashSet<&str> = books.iter().map(|b| b.media.id.as_str()).collect();
		for id in chunk.iter().filter(|id| !found.contains(id.as_str())) {
			report.items.push(PurgedItem {
				media_id: id.clone(),
				title: id.clone(),
				outcome: PurgeOutcome::Kept("It isn't in Deleted books".to_string()),
			});
		}
		for book in &books {
			let outcome = purge_one(conn, config, book, purged_by, &mut leftovers)
				.await
				.unwrap_or_else(|reason| {
					tracing::warn!(
						media_id = %book.media.id,
						path = %book.media.path,
						%reason,
						"A deleted book was kept"
					);
					PurgeOutcome::Kept(reason)
				});
			report.items.push(PurgedItem {
				media_id: book.media.id.clone(),
				title: book.title.clone(),
				outcome,
			});
		}
	}

	// the originals held for the books deleted go with them
	match held::purge_for_books(conn, config, &leftovers.book_ids).await {
		Ok(purge) if purge.deleted > 0 => {
			tracing::info!(
				held_files = purge.deleted,
				"Deleted the held files of books deleted permanently"
			)
		},
		Ok(_) => {},
		Err(error) => {
			tracing::warn!(
				?error,
				"Failed to delete the held files of books deleted permanently"
			)
		},
	}
	clear_leftovers(config, leftovers).await;
	Ok(report)
}

async fn purge_one(
	conn: &DatabaseConnection,
	config: &StumpConfig,
	book: &TrashedBook,
	purged_by: Option<&str>,
	leftovers: &mut Leftovers,
) -> Result<PurgeOutcome, String> {
	let now = Utc::now();
	let path = PathBuf::from(&book.media.path);
	let present = match tokio::fs::symlink_metadata(&path).await {
		Ok(meta) if meta.is_dir() => {
			return Err("Its path is a folder, not a book file".to_string())
		},
		Ok(_) => true,
		Err(error) if error.kind() == io::ErrorKind::NotFound => false,
		Err(error) => return Err(format!("Its file can't be read: {error}")),
	};

	// the page cache names its files after the file's version: read it before the file goes
	let pdf_cache = if present && book.media.extension.eq_ignore_ascii_case("pdf") {
		let (path, pages, config) = (path.clone(), book.media.pages, config.clone());
		tokio::task::spawn_blocking(move || pdf_cache_files(&path, pages, &config))
			.await
			.unwrap_or_default()
	} else {
		PdfCacheFiles::default()
	};

	// the file is hidden first, so the data can go before it and it can come back on failure
	let doomed = if present {
		let from = path.clone();
		Some(
			tokio::task::spawn_blocking(move || files::Doomed::hide(&from))
				.await
				.map_err(|error| format!("Its file couldn't be deleted: {error}"))?
				.map_err(|error| format!("Its file couldn't be deleted: {error}"))?,
		)
	} else {
		None
	};
	let bytes = doomed.as_ref().map(|d| d.bytes);

	match delete_data(conn, book, bytes, purged_by, now).await {
		Ok(series_thumbnails) => {
			leftovers.book_ids.push(book.media.id.clone());
			leftovers.series_thumbnails.extend(series_thumbnails);
			if let Some(thumbnail) = &book.media.thumbnail_path {
				leftovers.thumbnail_paths.push(PathBuf::from(thumbnail));
			}
			leftovers.pdf_cache.prefixes.extend(pdf_cache.prefixes);
			leftovers.pdf_cache.names.extend(pdf_cache.names);
			Ok(match doomed {
				Some(doomed) => {
					let bytes = doomed.bytes;
					let deleted =
						tokio::task::spawn_blocking(move || doomed.delete()).await;
					if !matches!(deleted, Ok(Ok(()))) {
						// hidden, so no scan brings it back; only the space is wasted
						tracing::warn!(?deleted, path = %path.display(), "A deleted book's file couldn't be removed from disk");
					}
					PurgeOutcome::Deleted { bytes }
				},
				None => PurgeOutcome::FileMissing,
			})
		},
		Err(error) => {
			if let Some(doomed) = doomed {
				// the book stays in Deleted books, so its file goes back
				let back = tokio::task::spawn_blocking(move || doomed.restore()).await;
				if !matches!(back, Ok(Ok(()))) {
					tracing::error!(?back, path = %path.display(), "Failed to put back the file of a book kept in Deleted books");
				}
			}
			Err(error)
		},
	}
}

/// Deletes the book's row (its dependent rows cascade), its virtual series if it was the last
/// book in it, and logs the deletion, in one transaction. Returns the thumbnails of the deleted
/// series
async fn delete_data(
	conn: &DatabaseConnection,
	book: &TrashedBook,
	bytes: Option<u64>,
	purged_by: Option<&str>,
	now: DateTime<Utc>,
) -> Result<Vec<String>, String> {
	let run = async {
		let txn = conn.begin().await?;
		let deleted = media::Entity::delete_many()
			.filter(media::Column::Id.eq(&book.media.id))
			// restored meanwhile: keep it
			.filter(media::Column::DeletedAt.is_not_null())
			.exec(&txn)
			.await?
			.rows_affected;
		if deleted == 0 {
			return Ok(None);
		}
		let series_thumbnails =
			refresh_series(&txn, book.media.series_id.clone()).await?;
		bump_media_thumbnail_fallbacks(
			&txn,
			&book.media.library_id,
			book.media.series_id.as_deref(),
		)
		.await?;
		media_purge::ActiveModel {
			media_id: Set(book.media.id.clone()),
			library_id: Set(Some(book.media.library_id.clone())),
			library_name: Set(Some(book.library_name.clone())),
			title: Set(book.title.clone()),
			authors: Set(book.authors.clone()),
			original_path: Set(book.media.path.clone()),
			quarantine_path: Set(None),
			bytes: Set(bytes.map(|b| b as i64)),
			deleted_by: Set(book.media.deleted_by.clone()),
			deleted_at: Set(book.media.deleted_at.map(|at| at.to_utc())),
			purged_by: Set(purged_by.map(str::to_string)),
			purged_at: Set(now),
			..Default::default()
		}
		.insert(&txn)
		.await?;
		txn.commit().await?;
		Ok::<_, CoreError>(Some(series_thumbnails))
	};
	match run.await {
		Ok(Some(series_thumbnails)) => Ok(series_thumbnails),
		Ok(None) => Err("It was restored in the meantime".to_string()),
		Err(error) => Err(format!("Its data couldn't be deleted: {error}")),
	}
}

/// Thumbnails, no-cover markers, deleted series' thumbnails and PDF pages of the books gone.
/// Best effort: a leftover file is only wasted space
async fn clear_leftovers(config: &StumpConfig, leftovers: Leftovers) {
	if leftovers.book_ids.is_empty() {
		return;
	}
	let thumbnails_dir = config.thumbnails_directory();
	if let Err(error) = remove_thumbnails(&leftovers.book_ids, &thumbnails_dir).await {
		tracing::warn!(?error, "Failed to remove the thumbnails of deleted books");
	}
	// a saved thumbnail elsewhere in the thumbnails folder (never a file outside it)
	for path in leftovers.thumbnail_paths {
		if path.starts_with(&thumbnails_dir) {
			if let Err(error) = tokio::fs::remove_file(&path).await {
				if error.kind() != io::ErrorKind::NotFound {
					tracing::warn!(
						?error,
						?path,
						"Failed to remove a deleted book's thumbnail"
					);
				}
			}
		}
	}
	for id in &leftovers.book_ids {
		forget_coverless(config, id).await;
	}
	remove_series_thumbnails(leftovers.series_thumbnails).await;

	let PdfCacheFiles { prefixes, names } = leftovers.pdf_cache;
	if prefixes.is_empty() && names.is_empty() {
		return;
	}
	let cache_dir = config.pdf_cache_directory();
	let removed = tokio::task::spawn_blocking(move || {
		let Ok(entries) = std::fs::read_dir(&cache_dir) else {
			return 0;
		};
		entries
			.filter_map(Result::ok)
			.filter(|entry| {
				let name = entry.file_name();
				let name = name.to_string_lossy();
				names.contains(name.as_ref())
					|| prefixes.iter().any(|prefix| name.starts_with(prefix))
			})
			.filter(|entry| std::fs::remove_file(entry.path()).is_ok())
			.count()
	})
	.await
	.unwrap_or_default();
	tracing::debug!(
		removed,
		"Removed deleted books' pages from the PDF page cache"
	);
}

#[cfg(test)]
mod tests;
