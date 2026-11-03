//! Deleted books (the trash).
//!
//! Removing a book (`deleteMedia`) only hides it: `media.deleted_at` and `deleted_by` are set,
//! its file and everything attached to it (reading progress, annotations, bookmarks, tags, smart
//! list matches) stay, so restoring it brings it back as it was. Rescans keep it hidden.
//!
//! A book is deleted permanently by hand, or by the daily automatic purge once it has been in
//! *Deleted books* for the retention period (14 days unless configured, 0 = never). Book files
//! are never destroyed: a permanent deletion
//!
//! 1. moves the file to `<quarantine>/deleted/<YYYY-MM-DD>/<its path in the library>`
//!    (`STUMP_QUARANTINE_DIR`) and appends a row to that day's `restore_log.csv`;
//! 2. then deletes the book's row, which cascades to its metadata, reading sessions,
//!    annotations, bookmarks, tags, favorites and the rest, drops its virtual series if it was
//!    the last book in it, and logs the deletion in `media_purges`;
//! 3. then removes its thumbnail, its no-cover marker and its pages in the PDF page cache.
//!
//! Without a quarantine folder, or when the file can't be moved, nothing is deleted and the
//! book stays in *Deleted books* with the reason. A book whose file is already gone only has
//! its data deleted. The file is no longer in the library, so a scan can't add it back; *Restore
//! from quarantine* moves it back and scans its folder (it comes back as a new book).

pub mod job;
pub mod quarantine;

use std::{
	collections::{HashMap, HashSet},
	io,
	path::{Path, PathBuf},
	str::FromStr,
};

use chrono::{DateTime, Duration, Timelike, Utc};
use croner::Cron;
use models::{
	entity::{library, library_config, media, media_purge, server_config, user},
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

/// Why a book with a file can't be deleted permanently on a server without a quarantine folder
pub const NO_QUARANTINE: &str =
	"No quarantine folder is set (STUMP_QUARANTINE_DIR): book \
                                 files are never deleted, so the book was kept";

/// How many books a purge loads at once
const PURGE_CHUNK: usize = 100;

/// Permanent deletions and restores from the quarantine folder run one at a time, so a book
/// purged by hand while the automatic purge runs is handled once
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

/// The automatic purge: a job when books have expired (so it shows in the job list with what
/// it did), else only a note that nothing was due
pub async fn start_automatic_purge(ctx: &Ctx) -> CoreResult<()> {
	let conn = ctx.conn.as_ref();
	let retention = retention_days(conn).await?;
	if retention <= 0 {
		tracing::debug!("Deleted books are kept until deleted by hand");
		return Ok(());
	}
	let now = Utc::now();
	let expired = expired_book_ids(conn, retention, now).await?;
	if expired.is_empty() {
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
		count = expired.len(),
		"Deleting expired books in Deleted books permanently"
	);
	ctx.enqueue(StumpJob::TrashPurge).await
}

/// What happened to one book
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurgeOutcome {
	/// Its file went to the quarantine folder and its data was deleted
	Quarantined { path: PathBuf, bytes: u64 },
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
		self.count(|o| matches!(o, PurgeOutcome::Quarantined { .. }))
	}

	pub fn missing_files(&self) -> u64 {
		self.count(|o| matches!(o, PurgeOutcome::FileMissing))
	}

	pub fn kept(&self) -> u64 {
		self.count(|o| matches!(o, PurgeOutcome::Kept(_)))
	}

	pub fn quarantined_bytes(&self) -> u64 {
		self.items
			.iter()
			.map(|item| match item.outcome {
				PurgeOutcome::Quarantined { bytes, .. } => bytes,
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
	library_path: PathBuf,
	library_name: String,
	deleted_by_name: Option<String>,
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
	let user_ids: HashSet<String> = rows
		.iter()
		.filter_map(|row| row.media.deleted_by.clone())
		.collect();
	let usernames: HashMap<String, String> = if user_ids.is_empty() {
		HashMap::new()
	} else {
		user::Entity::find()
			.select_only()
			.column(user::Column::Id)
			.column(user::Column::Username)
			.filter(user::Column::Id.is_in(user_ids))
			.into_tuple::<(String, String)>()
			.all(conn)
			.await?
			.into_iter()
			.collect()
	};

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
			let deleted_by_name = row
				.media
				.deleted_by
				.as_ref()
				.and_then(|id| usernames.get(id).cloned());
			Some(TrashedBook {
				media: row.media,
				title,
				authors,
				library_path: PathBuf::from(&library.path),
				library_name: library.name.clone(),
				deleted_by_name,
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

struct Moved {
	path: PathBuf,
	bytes: u64,
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
	let library_paths: Vec<PathBuf> = library::Entity::find()
		.select_only()
		.column(library::Column::Path)
		.into_tuple::<String>()
		.all(conn)
		.await?
		.into_iter()
		.map(PathBuf::from)
		.collect();

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
			let outcome = purge_one(
				conn,
				config,
				book,
				purged_by,
				&library_paths,
				&mut leftovers,
			)
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

	clear_leftovers(config, leftovers).await;
	Ok(report)
}

async fn purge_one(
	conn: &DatabaseConnection,
	config: &StumpConfig,
	book: &TrashedBook,
	purged_by: Option<&str>,
	library_paths: &[PathBuf],
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

	// the page cache names its files after the file's version: read it before the file moves
	let pdf_cache = if present && book.media.extension.eq_ignore_ascii_case("pdf") {
		let (path, pages, config) = (path.clone(), book.media.pages, config.clone());
		tokio::task::spawn_blocking(move || pdf_cache_files(&path, pages, &config))
			.await
			.unwrap_or_default()
	} else {
		PdfCacheFiles::default()
	};

	let moved = if present {
		Some(quarantine_file(config, book, &path, library_paths, now).await?)
	} else {
		None
	};

	match delete_data(conn, book, moved.as_ref(), purged_by, now).await {
		Ok(series_thumbnails) => {
			leftovers.book_ids.push(book.media.id.clone());
			leftovers.series_thumbnails.extend(series_thumbnails);
			if let Some(thumbnail) = &book.media.thumbnail_path {
				leftovers.thumbnail_paths.push(PathBuf::from(thumbnail));
			}
			leftovers.pdf_cache.prefixes.extend(pdf_cache.prefixes);
			leftovers.pdf_cache.names.extend(pdf_cache.names);
			Ok(match moved {
				Some(Moved { path, bytes }) => PurgeOutcome::Quarantined { path, bytes },
				None => PurgeOutcome::FileMissing,
			})
		},
		Err(error) => {
			if let Some(moved) = moved {
				// the book stays in Deleted books, so its file goes back
				let (from, to) = (moved.path, path);
				let back = tokio::task::spawn_blocking(move || {
					quarantine::move_file(&from, &to).map_err(|e| (e, from))
				})
				.await;
				if let Ok(Err((error, from))) = back {
					tracing::error!(
						?error,
						quarantined = %from.display(),
						"Failed to move a book file back from the quarantine folder"
					);
				}
			}
			Err(error)
		},
	}
}

/// Moves the book's file into today's folder in the quarantine folder and logs it there
async fn quarantine_file(
	config: &StumpConfig,
	book: &TrashedBook,
	path: &Path,
	library_paths: &[PathBuf],
	now: DateTime<Utc>,
) -> Result<Moved, String> {
	let root = config
		.quarantine_directory()
		.ok_or_else(|| NO_QUARANTINE.to_string())?;
	// a quarantine folder that isn't mounted would fill the container's own disk
	if !tokio::fs::metadata(&root)
		.await
		.is_ok_and(|meta| meta.is_dir())
	{
		return Err(format!(
			"The quarantine folder {} doesn't exist: the book was kept",
			root.display()
		));
	}

	let day = quarantine::day_dir(&root, now);
	let rel = quarantine::library_relative(&book.library_path, path);
	let (from, title, authors) = (
		path.to_path_buf(),
		book.title.clone(),
		book.authors.clone().unwrap_or_default(),
	);
	let deleted_by = book.deleted_by_name.clone().unwrap_or_default();
	let deleted_at = book
		.media
		.deleted_at
		.map(|at| at.to_utc().to_rfc3339())
		.unwrap_or_default();
	let library_paths = library_paths.to_vec();

	tokio::task::spawn_blocking(move || {
		let dest = quarantine::free_path(&day, &rel);
		if library_paths
			.iter()
			.any(|library| dest.starts_with(library))
		{
			return Err(
				"The quarantine folder is inside a library, where a scan would find the book again"
					.to_string(),
			);
		}
		let bytes = quarantine::move_file(&from, &dest).map_err(|error| {
			format!("Its file couldn't be moved to the quarantine folder: {error}")
		})?;
		let (original_path, quarantine_path) = (
			from.to_string_lossy().into_owned(),
			dest.to_string_lossy().into_owned(),
		);
		let row = quarantine::RestoreLogRow {
			original_path: &original_path,
			quarantine_path: &quarantine_path,
			bytes,
			deleted_by: &deleted_by,
			deleted_at: &deleted_at,
			title: &title,
			authors: &authors,
		};
		if let Err(error) = quarantine::append_restore_log(&day, &row) {
			// a file in quarantine without its log row would be hard to find: it goes back
			if let Err(back) = quarantine::move_file(&dest, &from) {
				tracing::error!(error = ?back, quarantined = %dest.display(), "Failed to move a book file back from the quarantine folder");
			}
			return Err(format!("The quarantine folder's restore_log.csv couldn't be written: {error}"));
		}
		Ok(Moved { path: dest, bytes })
	})
	.await
	.map_err(|error| format!("Its file couldn't be moved: {error}"))?
}

/// Deletes the book's row (its dependent rows cascade), its virtual series if it was the last
/// book in it, and logs the deletion, in one transaction. Returns the thumbnails of the deleted
/// series
async fn delete_data(
	conn: &DatabaseConnection,
	book: &TrashedBook,
	moved: Option<&Moved>,
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
			quarantine_path: Set(moved.map(|m| m.path.to_string_lossy().into_owned())),
			bytes: Set(moved.map(|m| m.bytes as i64)),
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

/// Moves a file deleted permanently back to where it was and scans its folder, so its library
/// shows it again, as a new book: its reading progress, annotations and the rest were deleted
/// with it
pub async fn restore_purged(
	ctx: &Ctx,
	purge_id: i32,
	restored_by: &str,
) -> CoreResult<media_purge::Model> {
	let _guard = PURGE_LOCK.lock().await;
	let conn = ctx.conn.as_ref();
	let row = media_purge::Entity::find_by_id(purge_id)
		.one(conn)
		.await?
		.ok_or_else(|| CoreError::NotFound("Deleted book".to_string()))?;
	if row.restored_at.is_some() {
		return Err(CoreError::BadRequest(
			"Its file was already put back in the library".to_string(),
		));
	}
	let quarantined = row.quarantine_path.clone().ok_or_else(|| {
		CoreError::BadRequest(
			"Its file was already missing when it was deleted: there is nothing to restore"
				.to_string(),
		)
	})?;
	let library = match &row.library_id {
		Some(id) => library::Entity::find_by_id(id.clone()).one(conn).await?,
		None => None,
	}
	.ok_or_else(|| CoreError::BadRequest("Its library no longer exists".to_string()))?;
	let original = PathBuf::from(&row.original_path);
	if !original.starts_with(&library.path) {
		return Err(CoreError::BadRequest(format!(
			"{} is no longer inside its library",
			original.display()
		)));
	}

	let (from, to) = (PathBuf::from(&quarantined), original.clone());
	tokio::task::spawn_blocking(move || quarantine::move_file(&from, &to))
		.await?
		.map_err(|error| match error.kind() {
			io::ErrorKind::NotFound => CoreError::BadRequest(format!(
				"Its file is no longer in the quarantine folder ({quarantined})"
			)),
			io::ErrorKind::AlreadyExists => CoreError::BadRequest(format!(
				"A file is already at {}: move it away first",
				original.display()
			)),
			_ => {
				CoreError::BadRequest(format!("Its file couldn't be moved back: {error}"))
			},
		})?;

	let mut active = row.into_active_model();
	active.restored_at = Set(Some(Utc::now()));
	active.restored_by = Set(Some(restored_by.to_string()));
	let restored = active.update(conn).await?;

	let config = library_config::Entity::find()
		.filter(library_config::Column::LibraryId.eq(&library.id))
		.one(conn)
		.await?;
	let folder = original.parent().unwrap_or(Path::new(&library.path));
	crate::intake::enqueue_scan(ctx, &library, config.as_ref(), folder).await;
	Ok(restored)
}

#[cfg(test)]
mod tests;
