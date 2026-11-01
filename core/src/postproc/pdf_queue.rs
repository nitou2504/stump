//! The queue between Stump and the host runner that OCRs and shrinks scanned PDFs
//! (`scripts/fork/workers/pdf_runner.py`). Those tools (OCRmyPDF, Tesseract, Ghostscript, JBIG2,
//! PyMuPDF) do not belong in Stump's image, and nothing has to stay resident: Stump writes a
//! request, the host runner starts when the queue is not empty (systemd path unit or cron),
//! works through it and exits.
//!
//! Layout under `<config>/workers/pdf/`:
//! - `queue/<id>.json`: a [`PdfRequest`] written by Stump;
//! - `running/<id>.json`: the runner moved it there while it works on it;
//! - `done/<id>.json` (+ `done/<id>.pdf` when there is a new file): the runner's [`PdfResult`];
//! - `archive/<id>.json`: results Stump has applied.
//!
//! Stump checks `done/` every minute: it swaps a new file in (the original goes to quarantine,
//! never deleted) only if the library file is still the one it queued (same size and mtime).

use std::{
	collections::HashSet,
	fs,
	io::{self, Write},
	path::{Path, PathBuf},
	time::Duration,
};

use chrono::{DateTime, Utc};
use models::{
	entity::{library, library_config, media, post_processing_task as task},
	shared::post_processing::{PostProcessKind, PostProcessStatus},
};
use sea_orm::{prelude::*, DatabaseConnection};
use serde::{Deserialize, Serialize};

use super::{refresh_media_after_swap, swap, update_task, TaskUpdate};
use crate::{config::StumpConfig, CoreResult, Ctx};

pub const QUEUE_VERSION: u32 = 1;

/// How often Stump looks at the runner's results
pub const COLLECT_INTERVAL: Duration = Duration::from_secs(60);

/// A PDF for the host runner (paths as Stump sees them: the runner maps them to host paths
/// through the container's mounts)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PdfRequest {
	pub version: u32,
	/// The post-processing task id
	pub id: String,
	/// `ocr` and/or `shrink`
	pub steps: Vec<String>,
	pub path: String,
	pub library_path: String,
	/// The file's size and modification time (unix seconds) when it was queued
	pub bytes: u64,
	pub mtime: i64,
	/// The book's language from its metadata, a hint for the OCR language
	pub language: Option<String>,
	pub created_at: DateTime<Utc>,
}

/// What the runner did with a request
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PdfResult {
	pub id: String,
	/// `done` (a new file is in `done/<id>.pdf`), `not_worth`, `skipped` or `failed`
	pub status: String,
	#[serde(default)]
	pub reason: String,
	pub bytes_in: Option<u64>,
	pub bytes_out: Option<u64>,
	/// The library file the runner read (size and mtime), to check it did not change since
	pub source_bytes: Option<u64>,
	pub source_mtime: Option<i64>,
	#[serde(default)]
	pub details: Option<serde_json::Value>,
}

pub fn ensure_dirs(root: &Path) -> io::Result<()> {
	for dir in ["queue", "running", "done", "archive"] {
		fs::create_dir_all(root.join(dir))?;
	}
	Ok(())
}

fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
	let tmp = path.with_extension("json.tmp");
	{
		let mut fh = fs::File::create(&tmp)?;
		fh.write_all(data)?;
		fh.sync_all()?;
	}
	fs::rename(tmp, path)
}

/// Queue a request (written whole, then renamed, so the runner never reads half a file)
pub fn write_request(root: &Path, request: &PdfRequest) -> io::Result<()> {
	ensure_dirs(root)?;
	let data = serde_json::to_vec_pretty(request).map_err(io::Error::other)?;
	write_atomic(
		&root.join("queue").join(format!("{}.json", request.id)),
		&data,
	)
}

fn ids_in(dir: &Path) -> HashSet<String> {
	fs::read_dir(dir)
		.map(|entries| {
			entries
				.filter_map(Result::ok)
				.filter_map(|e| {
					let name = e.file_name().to_string_lossy().to_string();
					name.strip_suffix(".json").map(str::to_string)
				})
				.collect()
		})
		.unwrap_or_default()
}

/// The results the runner finished, oldest first
pub fn read_results(root: &Path) -> Vec<PdfResult> {
	let mut results: Vec<(std::time::SystemTime, PdfResult)> =
		fs::read_dir(root.join("done"))
			.map(|entries| {
				entries
					.filter_map(Result::ok)
					.filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
					.filter_map(|e| {
						let modified = e.metadata().and_then(|m| m.modified()).ok()?;
						let data = fs::read(e.path()).ok()?;
						match serde_json::from_slice::<PdfResult>(&data) {
							Ok(result) => Some((modified, result)),
							Err(error) => {
								tracing::warn!(?error, path = ?e.path(), "Unreadable PDF runner result");
								None
							},
						}
					})
					.collect()
			})
			.unwrap_or_default();
	results.sort_by_key(|(modified, _)| *modified);
	results.into_iter().map(|(_, r)| r).collect()
}

/// The new file of a `done` result
pub fn result_file(root: &Path, id: &str) -> PathBuf {
	root.join("done").join(format!("{id}.pdf"))
}

/// Move an applied result to `archive/` and drop its PDF
pub fn archive_result(root: &Path, id: &str) -> io::Result<()> {
	let _ = fs::remove_file(result_file(root, id));
	fs::create_dir_all(root.join("archive"))?;
	let from = root.join("done").join(format!("{id}.json"));
	if from.exists() {
		fs::rename(from, root.join("archive").join(format!("{id}.json")))?;
	}
	Ok(())
}

fn file_stamp(path: &Path) -> io::Result<(u64, i64)> {
	let meta = fs::metadata(path)?;
	let mtime = meta
		.modified()?
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs() as i64)
		.unwrap_or_default();
	Ok((meta.len(), mtime))
}

/// Mark queued tasks the runner picked up as running
async fn sync_running(conn: &DatabaseConnection, root: &Path) -> Result<(), DbErr> {
	let running = ids_in(&root.join("running"));
	if running.is_empty() {
		return Ok(());
	}
	let pending = task::Entity::find()
		.filter(task::Column::Kind.eq(PostProcessKind::Pdf))
		.filter(task::Column::Status.eq(PostProcessStatus::Pending))
		.all(conn)
		.await?;
	for row in pending.into_iter().filter(|row| running.contains(&row.id)) {
		update_task(
			conn,
			&row.id,
			TaskUpdate {
				status: Some(PostProcessStatus::Running),
				..Default::default()
			},
		)
		.await?;
	}
	Ok(())
}

/// What applying one result came to
struct Applied {
	status: PostProcessStatus,
	reason: String,
	bytes_after: Option<i64>,
}

async fn apply_result(
	conn: &DatabaseConnection,
	config: &StumpConfig,
	root: &Path,
	row: &task::Model,
	result: &PdfResult,
) -> CoreResult<Applied> {
	let reason = if result.reason.is_empty() {
		result.status.replace('_', " ")
	} else {
		result.reason.clone()
	};
	let status = match result.status.as_str() {
		"done" => PostProcessStatus::Done,
		"not_worth" => PostProcessStatus::Rejected,
		"skipped" => PostProcessStatus::Skipped,
		_ => PostProcessStatus::Failed,
	};
	if status != PostProcessStatus::Done {
		return Ok(Applied {
			status,
			reason,
			bytes_after: result.bytes_out.map(|b| b as i64),
		});
	}
	let fail = |reason: String| Applied {
		status: PostProcessStatus::Failed,
		reason,
		bytes_after: None,
	};
	let Some(book_id) = row.media_id.clone() else {
		return Ok(fail("the book is gone".to_string()));
	};
	let Some(book) = media::Entity::find_by_id(book_id).one(conn).await? else {
		return Ok(fail("the book is gone".to_string()));
	};
	let quarantine = match super::quarantine_root(config) {
		Ok(root) => root,
		Err(reason) => return Ok(fail(reason)),
	};
	let new_file = result_file(root, &row.id);
	let new_bytes = match fs::metadata(&new_file) {
		Ok(meta) => meta.len(),
		Err(_) => return Ok(fail("the runner's file is missing".to_string())),
	};
	if result.bytes_out.is_some_and(|b| b != new_bytes) {
		return Ok(fail("the runner's file has the wrong size".to_string()));
	}
	let requested = row.details.as_ref().and_then(|d| {
		Some((
			d.get("request_bytes")?.as_u64()?,
			d.get("request_mtime")?.as_i64()?,
		))
	});
	let original = PathBuf::from(&book.path);
	let current = file_stamp(&original).ok();
	if current.is_none() || current != requested {
		return Ok(Applied {
			status: PostProcessStatus::Rejected,
			reason: "the library file changed after it was queued; queue it again"
				.to_string(),
			bytes_after: None,
		});
	}
	let Some(library) = library::Entity::find_by_id(book.library_id.clone())
		.one(conn)
		.await?
	else {
		return Ok(fail("the library is gone".to_string()));
	};
	let Some(library_config) = library_config::Entity::find_by_id(library.config_id)
		.one(conn)
		.await?
	else {
		return Ok(fail("the library config is gone".to_string()));
	};
	let batch = swap::batch_dir(&quarantine, "pdf", Utc::now());
	let rel = swap::quarantine_rel(Path::new(&library.path), &original);
	let swapped = {
		let (original, new_file) = (original.clone(), new_file.clone());
		tokio::task::spawn_blocking(move || {
			swap::swap_into_place(&original, &new_file, &batch, &rel)
		})
		.await
		.map_err(|e| crate::CoreError::InternalError(e.to_string()))?
	};
	match swapped {
		Ok(out) => {
			if let Err(error) =
				refresh_media_after_swap(conn, config, &library_config, &book).await
			{
				tracing::warn!(?error, "Could not refresh a processed PDF");
			}
			tracing::info!(
				path = %book.path,
				before = out.bytes_before,
				after = out.bytes_after,
				"Replaced a PDF with the runner's version"
			);
			Ok(Applied {
				status: PostProcessStatus::Done,
				reason,
				bytes_after: Some(out.bytes_after as i64),
			})
		},
		Err(error) => Ok(fail(format!("could not replace the file: {error}"))),
	}
}

/// Apply every finished result of the host runner. Returns how many were handled
pub async fn collect_results(ctx: &Ctx) -> CoreResult<usize> {
	let root = ctx.config.pdf_worker_directory();
	if !root.exists() {
		return Ok(0);
	}
	let conn = ctx.conn.as_ref();
	sync_running(conn, &root).await?;
	let results = read_results(&root);
	let mut handled = 0;
	for result in results {
		let Some(row) = task::Entity::find_by_id(result.id.clone())
			.one(conn)
			.await?
		else {
			tracing::warn!(id = %result.id, "PDF runner result without a task, archived");
			let _ = archive_result(&root, &result.id);
			continue;
		};
		let applied = match apply_result(conn, &ctx.config, &root, &row, &result).await {
			Ok(applied) => applied,
			Err(error) => Applied {
				status: PostProcessStatus::Failed,
				reason: error.to_string(),
				bytes_after: None,
			},
		};
		let mut details = row.details.clone().unwrap_or_else(|| serde_json::json!({}));
		if let Some(runner) = &result.details {
			details["runner"] = runner.clone();
		}
		update_task(
			conn,
			&row.id,
			TaskUpdate {
				status: Some(applied.status),
				reason: Some(Some(applied.reason)),
				bytes_after: Some(applied.bytes_after),
				details: Some(Some(details)),
				finished: true,
				..Default::default()
			},
		)
		.await?;
		if let Err(error) = archive_result(&root, &row.id) {
			tracing::warn!(?error, id = %row.id, "Could not archive a PDF runner result");
		}
		handled += 1;
	}
	Ok(handled)
}

/// Check the runner's results now and every [`COLLECT_INTERVAL`]: one directory listing a
/// minute while nothing is queued
pub fn spawn_collector(ctx: std::sync::Arc<Ctx>) -> tokio::task::JoinHandle<()> {
	tokio::spawn(async move {
		let mut interval = tokio::time::interval(COLLECT_INTERVAL);
		interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
		loop {
			interval.tick().await;
			match collect_results(&ctx).await {
				Ok(0) => {},
				Ok(handled) => tracing::info!(handled, "Applied PDF runner results"),
				Err(error) => {
					tracing::error!(?error, "Failed to apply PDF runner results")
				},
			}
		}
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn request_round_trip_and_results() {
		let dir = tempfile::tempdir().unwrap();
		let root = dir.path().join("workers/pdf");
		let request = PdfRequest {
			version: QUEUE_VERSION,
			id: "abc".to_string(),
			steps: vec!["ocr".to_string(), "shrink".to_string()],
			path: "/biblioteca/biblioteca/A/B.pdf".to_string(),
			library_path: "/biblioteca/biblioteca".to_string(),
			bytes: 10,
			mtime: 20,
			language: Some("es".to_string()),
			created_at: Utc::now(),
		};
		write_request(&root, &request).unwrap();
		let data = fs::read(root.join("queue/abc.json")).unwrap();
		assert_eq!(
			serde_json::from_slice::<PdfRequest>(&data).unwrap(),
			request
		);
		assert!(!root.join("queue/abc.json.tmp").exists());

		fs::write(
			root.join("done/abc.json"),
			r#"{"id":"abc","status":"not_worth","reason":"saves 1.0 MB (4%)","bytes_in":100,"bytes_out":96}"#,
		)
		.unwrap();
		fs::write(root.join("done/abc.pdf"), b"%PDF").unwrap();
		let results = read_results(&root);
		assert_eq!(results.len(), 1);
		assert_eq!(results[0].status, "not_worth");
		archive_result(&root, "abc").unwrap();
		assert!(root.join("archive/abc.json").exists());
		assert!(!result_file(&root, "abc").exists());
		assert!(read_results(&root).is_empty());
	}
}
