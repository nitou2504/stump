use std::{collections::HashMap, path::Path};

use async_graphql::{Result, SimpleObject};
use chrono::{DateTime, Utc};
use models::{
	entity::{media, media_purge, server_config, user},
	shared::trash::{TrashPurgeError, TrashPurgeSummary},
};
use sea_orm::{prelude::*, QuerySelect};
use stump_core::{
	trash::{self, PurgeReport},
	utils::serde::comma_separated_list_to_vec,
	Ctx,
};

use super::media::Media;

/// A book in *Deleted books*: removed from its library, hidden everywhere, its file still on
/// disk until it is deleted permanently
#[derive(Debug, Clone, SimpleObject)]
pub struct DeletedBook {
	/// The book as it was when it was removed (its progress, annotations and tags are kept)
	pub book: Media,
	pub deleted_at: DateTime<Utc>,
	/// Who removed it, when known
	pub deleted_by: Option<String>,
	/// When its time in Deleted books is over (null: kept until deleted by hand)
	pub expires_at: Option<DateTime<Utc>>,
	/// When the automatic purge will delete it permanently
	pub purge_at: Option<DateTime<Utc>>,
	pub library_id: String,
	pub library_name: String,
	/// Its path inside its library
	pub relative_path: String,
	/// Whether its file is still there (otherwise deleting it permanently only deletes its data)
	pub file_exists: bool,
}

/// How *Deleted books* works on this server
#[derive(Debug, Clone, SimpleObject)]
pub struct DeletedBooksSettings {
	/// How many days removed books stay in Deleted books (0: until deleted by hand)
	pub retention_days: i32,
	/// The folder book files go to when they are deleted permanently (library managers only)
	pub quarantine_dir: Option<String>,
	/// Whether books with a file can be deleted permanently: the quarantine folder is set and
	/// exists
	pub quarantine_ready: bool,
	/// When the automatic purge runs next (null when it is off)
	pub next_purge_at: Option<DateTime<Utc>>,
	/// When the automatic purge last ran (library managers only)
	pub last_purge_at: Option<DateTime<Utc>>,
	/// What it did (library managers only)
	pub last_purge: Option<TrashPurgeSummary>,
}

impl DeletedBooksSettings {
	/// `for_managers` adds the quarantine folder and the last purge
	pub async fn load(core: &Ctx, for_managers: bool) -> Result<Self> {
		let config = server_config::Entity::find()
			.one(core.conn.as_ref())
			.await?
			.ok_or("Server configuration not found")?;
		let retention_days = config.trash_retention_days;
		let quarantine = core.config.quarantine_directory();
		let quarantine_ready = match &quarantine {
			Some(dir) => tokio::fs::metadata(dir)
				.await
				.is_ok_and(|meta| meta.is_dir()),
			None => false,
		};
		Ok(Self {
			retention_days,
			quarantine_dir: quarantine
				.filter(|_| for_managers)
				.map(|dir| dir.to_string_lossy().into_owned()),
			quarantine_ready,
			next_purge_at: (retention_days > 0)
				.then(|| trash::next_purge_run(Utc::now(), false))
				.flatten(),
			last_purge_at: config.trash_last_purge_at.filter(|_| for_managers),
			last_purge: config.trash_last_purge.filter(|_| for_managers),
		})
	}
}

/// What deleting books permanently did
#[derive(Debug, Clone, SimpleObject)]
pub struct DeletedBooksPurgeResult {
	/// Books whose file went to the quarantine folder and whose data was deleted
	pub purged: u64,
	/// Books whose file was already gone: only their data was deleted
	pub missing_files: u64,
	/// The size of the files moved to the quarantine folder
	pub quarantined_bytes: u64,
	/// Books kept in Deleted books, with why
	pub kept: Vec<TrashPurgeError>,
}

impl From<PurgeReport> for DeletedBooksPurgeResult {
	fn from(report: PurgeReport) -> Self {
		Self {
			purged: report.purged(),
			missing_files: report.missing_files(),
			quarantined_bytes: report.quarantined_bytes(),
			kept: report.errors(),
		}
	}
}

/// A book deleted permanently: its file is in the quarantine folder (unless it was already
/// gone) and can be moved back
#[derive(Debug, Clone, SimpleObject)]
pub struct PurgedBook {
	pub id: i32,
	pub media_id: String,
	pub title: String,
	pub authors: Vec<String>,
	pub library_id: Option<String>,
	pub library_name: Option<String>,
	pub original_path: String,
	/// Where its file is (null: it was already gone)
	pub quarantine_path: Option<String>,
	pub bytes: Option<i64>,
	pub deleted_by: Option<String>,
	pub deleted_at: Option<DateTime<Utc>>,
	/// Who deleted it permanently, when known
	pub purged_by: Option<String>,
	/// Deleted by the automatic purge
	pub automatic: bool,
	pub purged_at: DateTime<Utc>,
	/// When its file was moved back into the library
	pub restored_at: Option<DateTime<Utc>>,
	pub restored_by: Option<String>,
	/// Whether its file can be moved back: it is still in the quarantine folder and nothing
	/// took its place in the library
	pub can_restore: bool,
}

impl PurgedBook {
	pub async fn from_rows(
		conn: &DatabaseConnection,
		rows: Vec<media_purge::Model>,
	) -> Result<Vec<Self>> {
		let user_ids: Vec<String> = rows
			.iter()
			.flat_map(|row| {
				[&row.deleted_by, &row.purged_by, &row.restored_by]
					.into_iter()
					.flatten()
					.cloned()
			})
			.collect();
		let names = usernames(conn, user_ids).await?;
		let name =
			|id: &Option<String>| id.as_ref().and_then(|id| names.get(id).cloned());

		let mut books = Vec::with_capacity(rows.len());
		for row in rows {
			let can_restore = row.restored_at.is_none()
				&& match &row.quarantine_path {
					Some(path) => {
						exists(Path::new(path)).await
							&& !exists(Path::new(&row.original_path)).await
					},
					None => false,
				};
			books.push(Self {
				id: row.id,
				deleted_by: name(&row.deleted_by),
				purged_by: name(&row.purged_by),
				automatic: row.purged_by.is_none(),
				restored_by: name(&row.restored_by),
				media_id: row.media_id,
				title: row.title,
				authors: row
					.authors
					.map(comma_separated_list_to_vec)
					.unwrap_or_default(),
				library_id: row.library_id,
				library_name: row.library_name,
				original_path: row.original_path,
				quarantine_path: row.quarantine_path,
				bytes: row.bytes,
				deleted_at: row.deleted_at,
				purged_at: row.purged_at,
				restored_at: row.restored_at,
				can_restore,
			});
		}
		Ok(books)
	}
}

/// Whether anything is at `path`
pub async fn exists(path: &Path) -> bool {
	tokio::fs::symlink_metadata(path).await.is_ok()
}

/// User names by id, for the ids that still exist
pub async fn usernames(
	conn: &DatabaseConnection,
	ids: Vec<String>,
) -> Result<HashMap<String, String>> {
	if ids.is_empty() {
		return Ok(HashMap::new());
	}
	Ok(user::Entity::find()
		.select_only()
		.column(user::Column::Id)
		.column(user::Column::Username)
		.filter(user::Column::Id.is_in(ids))
		.into_tuple::<(String, String)>()
		.all(conn)
		.await?
		.into_iter()
		.collect())
}

/// The ids among `ids` of books in Deleted books, in the given libraries
pub async fn trashed_ids(
	conn: &DatabaseConnection,
	ids: Option<Vec<String>>,
	libraries: sea_orm::sea_query::SelectStatement,
	library_id: Option<String>,
) -> Result<Vec<String>> {
	let mut query = media::Entity::find()
		.select_only()
		.column(media::Column::Id)
		.filter(media::Column::DeletedAt.is_not_null())
		.filter(media::Column::LibraryId.in_subquery(libraries));
	if let Some(ids) = ids {
		query = query.filter(media::Column::Id.is_in(ids));
	}
	if let Some(library_id) = library_id {
		query = query.filter(media::Column::LibraryId.eq(library_id));
	}
	Ok(query.into_tuple::<String>().all(conn).await?)
}
