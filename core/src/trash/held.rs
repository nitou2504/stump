//! Files Stump holds in its trash folder (`<config>/trash/<date>/<path in the library>`) for the
//! *Deleted books* retention: the originals post-processing replaced, and the version a put-back
//! replaced. Each one has a `held_files` row; the page lists them with the removed books. A held
//! file can be put back (the book's file becomes it again, and the version it replaces is held
//! in turn, so nothing is ever lost by a click), or deleted: by hand, by the daily purge once the
//! retention is over, or with its book when the book is deleted permanently.

use std::{
	io,
	path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use models::{
	entity::{held_file, library, library_config, media, post_processing_task as task},
	shared::{post_processing::PostProcessStatus, trash::HeldFileReason},
};
use sea_orm::{prelude::*, ActiveValue::Set, QueryOrder};

use super::{expires_at, files, is_expired, PURGE_LOCK};
use crate::{
	config::StumpConfig,
	postproc::{refresh_media_after_swap, update_task, TaskUpdate},
	CoreError, CoreResult, Ctx,
};

/// The absolute path of a held file
pub fn held_file_path(config: &StumpConfig, row: &held_file::Model) -> PathBuf {
	config.trash_directory().join(&row.held_path)
}

/// Where a book's file goes in the trash folder today: `<date>/<its path in the library>`,
/// relative to the trash folder (`.prevN` added when that name is taken)
pub fn destination(
	config: &StumpConfig,
	library_path: &Path,
	book_path: &Path,
	now: DateTime<Utc>,
) -> (PathBuf, PathBuf) {
	let root = config.trash_directory();
	let rel = files::library_relative(library_path, book_path);
	let absolute = files::free_path(&files::day_dir(&root, now), &rel);
	let relative = absolute
		.strip_prefix(&root)
		.map(Path::to_path_buf)
		.unwrap_or_else(|_| absolute.clone());
	(absolute, relative)
}

/// What a held file is, for its row
#[derive(Debug, Clone)]
pub struct HeldFileInfo {
	pub reason: HeldFileReason,
	pub steps: Option<String>,
	pub task_id: Option<String>,
	pub held_by: Option<String>,
}

/// Records a file already copied into the trash folder (`held` is relative to it)
pub async fn record<C: ConnectionTrait>(
	conn: &C,
	book: &media::Model,
	title: String,
	held: &Path,
	bytes: u64,
	info: HeldFileInfo,
) -> CoreResult<held_file::Model> {
	Ok(held_file::ActiveModel {
		media_id: Set(Some(book.id.clone())),
		library_id: Set(Some(book.library_id.clone())),
		title: Set(title),
		original_path: Set(book.path.clone()),
		held_path: Set(held.to_string_lossy().into_owned()),
		bytes: Set(bytes as i64),
		reason: Set(info.reason),
		steps: Set(info.steps),
		task_id: Set(info.task_id),
		held_by: Set(info.held_by),
		held_at: Set(Utc::now()),
		..Default::default()
	}
	.insert(conn)
	.await?)
}

/// The title a book is listed under
pub async fn book_title<C: ConnectionTrait>(conn: &C, book: &media::Model) -> String {
	models::entity::media_metadata::Entity::find()
		.filter(models::entity::media_metadata::Column::MediaId.eq(book.id.clone()))
		.one(conn)
		.await
		.ok()
		.flatten()
		.and_then(|m| m.title)
		.filter(|t| !t.trim().is_empty())
		.unwrap_or_else(|| book.name.clone())
}

/// When a held file's time is over (None: kept until deleted by hand)
pub fn held_expires_at(
	row: &held_file::Model,
	retention_days: i32,
) -> Option<DateTime<Utc>> {
	expires_at(row.held_at, retention_days)
}

/// The held files whose time is over
pub async fn expired_held_ids<C: ConnectionTrait>(
	conn: &C,
	retention_days: i32,
	now: DateTime<Utc>,
) -> CoreResult<Vec<i32>> {
	if retention_days <= 0 {
		return Ok(vec![]);
	}
	Ok(held_file::Entity::find()
		.order_by_asc(held_file::Column::HeldAt)
		.all(conn)
		.await?
		.into_iter()
		.filter(|row| is_expired(row.held_at, retention_days, now))
		.map(|row| row.id)
		.collect())
}

/// What deleting held files did
#[derive(Debug, Clone, Default)]
pub struct HeldPurge {
	pub deleted: u64,
	pub bytes: u64,
	/// (id, title, why) of the ones kept
	pub kept: Vec<(i32, String, String)>,
}

/// Deletes held files and their rows (a file already gone only loses its row)
pub async fn purge_held(
	conn: &DatabaseConnection,
	config: &StumpConfig,
	ids: &[i32],
) -> CoreResult<HeldPurge> {
	let mut out = HeldPurge::default();
	if ids.is_empty() {
		return Ok(out);
	}
	let rows = held_file::Entity::find()
		.filter(held_file::Column::Id.is_in(ids.to_vec()))
		.all(conn)
		.await?;
	for row in rows {
		let path = held_file_path(config, &row);
		match tokio::fs::remove_file(&path).await {
			Ok(()) => out.bytes += row.bytes.max(0) as u64,
			Err(error) if error.kind() == io::ErrorKind::NotFound => {},
			Err(error) => {
				out.kept.push((
					row.id,
					row.title.clone(),
					format!("Its file couldn't be deleted: {error}"),
				));
				continue;
			},
		}
		held_file::Entity::delete_by_id(row.id).exec(conn).await?;
		out.deleted += 1;
		remove_empty_parents(&config.trash_directory(), &path).await;
	}
	Ok(out)
}

/// The held files of books deleted permanently go with them
pub async fn purge_for_books(
	conn: &DatabaseConnection,
	config: &StumpConfig,
	book_ids: &[String],
) -> CoreResult<HeldPurge> {
	if book_ids.is_empty() {
		return Ok(HeldPurge::default());
	}
	let ids: Vec<i32> = held_file::Entity::find()
		.filter(held_file::Column::MediaId.is_in(book_ids.to_vec()))
		.all(conn)
		.await?
		.into_iter()
		.map(|row| row.id)
		.collect();
	purge_held(conn, config, &ids).await
}

/// Removes the empty folders a deleted file leaves in the trash folder (never the trash folder
/// itself)
async fn remove_empty_parents(root: &Path, file: &Path) {
	let mut dir = file.parent().map(Path::to_path_buf);
	while let Some(current) = dir {
		if current == root || !current.starts_with(root) {
			break;
		}
		if tokio::fs::remove_dir(&current).await.is_err() {
			break;
		}
		dir = current.parent().map(Path::to_path_buf);
	}
}

/// What putting a held file back did
#[derive(Debug, Clone)]
pub struct PutBack {
	pub book: media::Model,
	/// The version that was in the library, now held in turn
	pub replaced: Option<held_file::Model>,
}

/// Puts a held file back in its book's place. The book's current file (if any) is held in
/// turn first, so a put-back can itself be undone until the retention is over; the book's
/// size and hashes follow the file, and a post-processing task that replaced it is marked
/// reverted (so it isn't run again on its own)
pub async fn put_back(ctx: &Ctx, id: i32, user_id: &str) -> CoreResult<PutBack> {
	let _guard = PURGE_LOCK.lock().await;
	let conn = ctx.conn.as_ref();
	let row = held_file::Entity::find_by_id(id)
		.one(conn)
		.await?
		.ok_or_else(|| CoreError::NotFound("Held file".to_string()))?;
	let book = match &row.media_id {
		Some(media_id) => {
			media::Entity::find_by_id(media_id.clone())
				.one(conn)
				.await?
		},
		None => None,
	}
	.ok_or_else(|| {
		CoreError::BadRequest(
			"Its book is no longer in Stump: delete it instead".to_string(),
		)
	})?;
	// no metadata edit of the book while its file is swapped
	let _book_guard = crate::intake::edit::lock_book(&book.id).await;
	let (library, library_config) = library::Entity::find_by_id(book.library_id.clone())
		.find_also_related(library_config::Entity)
		.one(conn)
		.await?
		.ok_or_else(|| {
			CoreError::BadRequest("Its library no longer exists".to_string())
		})?;
	let library_config = library_config
		.ok_or_else(|| CoreError::NotFound("Library configuration".to_string()))?;
	let held = held_file_path(&ctx.config, &row);
	if !files::exists(&held) {
		return Err(CoreError::BadRequest(format!(
			"Its file is no longer in Stump's trash folder ({})",
			held.display()
		)));
	}
	let target = PathBuf::from(&book.path);
	if !target.starts_with(&library.path) {
		return Err(CoreError::BadRequest(format!(
			"{} is not inside its library",
			target.display()
		)));
	}

	// the version in the library is held first: putting back never loses a file
	let mut replaced = None;
	if files::exists(&target) {
		let (absolute, relative) =
			destination(&ctx.config, Path::new(&library.path), &target, Utc::now());
		let (from, to) = (target.clone(), absolute.clone());
		let bytes = tokio::task::spawn_blocking(move || files::copy_new(&from, &to))
			.await
			.map_err(|e| CoreError::InternalError(e.to_string()))?
			.map_err(|error| {
				CoreError::BadRequest(format!(
					"The book's current file couldn't be kept in the trash folder: {error}"
				))
			})?;
		let title = book_title(conn, &book).await;
		match record(
			conn,
			&book,
			title,
			&relative,
			bytes,
			HeldFileInfo {
				reason: HeldFileReason::PutBack,
				steps: row.steps.clone(),
				task_id: row.task_id.clone(),
				held_by: Some(user_id.to_string()),
			},
		)
		.await
		{
			Ok(model) => replaced = Some(model),
			Err(error) => {
				let _ = tokio::fs::remove_file(&absolute).await;
				return Err(error);
			},
		}
	}

	let (from, to) = (held.clone(), target.clone());
	let placed =
		tokio::task::spawn_blocking(move || files::replace_in_library(&from, &to))
			.await
			.map_err(|e| CoreError::InternalError(e.to_string()))?;
	if let Err(error) = placed {
		// nothing changed in the library: the version held a moment ago is not needed
		if let Some(model) = replaced.take() {
			let _ = tokio::fs::remove_file(held_file_path(&ctx.config, &model)).await;
			let _ = held_file::Entity::delete_by_id(model.id).exec(conn).await;
		}
		return Err(CoreError::BadRequest(format!(
			"Its file couldn't be put back: {error}"
		)));
	}

	held_file::Entity::delete_by_id(row.id).exec(conn).await?;
	let _ = tokio::fs::remove_file(&held).await;
	remove_empty_parents(&ctx.config.trash_directory(), &held).await;
	if let Err(error) =
		refresh_media_after_swap(conn, &ctx.config, &library_config, &book).await
	{
		tracing::warn!(?error, path = %book.path, "Could not refresh a book put back");
	}
	if let Some(task_id) = &row.task_id {
		let reason = match row.reason {
			HeldFileReason::PutBack => {
				"its processed version was put back from Deleted books"
			},
			_ => "its original was put back from Deleted books",
		};
		let status = match row.reason {
			HeldFileReason::PutBack => PostProcessStatus::Done,
			_ => PostProcessStatus::Reverted,
		};
		let exists = task::Entity::find_by_id(task_id.clone())
			.one(conn)
			.await?
			.is_some();
		if exists {
			update_task(
				conn,
				task_id,
				TaskUpdate {
					status: Some(status),
					reason: Some(Some(reason.to_string())),
					finished: true,
					..Default::default()
				},
			)
			.await?;
		}
	}
	let book = media::Entity::find_by_id(book.id.clone())
		.one(conn)
		.await?
		.unwrap_or(book);
	tracing::info!(path = %book.path, held = %held.display(), "Put a held file back in the library");
	Ok(PutBack { book, replaced })
}
