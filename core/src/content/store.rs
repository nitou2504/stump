//! The index's tables: writing a book's sections, and the state of each book

use chrono::Utc;
use sea_orm::{
	ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value,
};

use super::{extract::Section, ContentResult, EXTRACTOR_VERSION};

fn stmt(sql: &str, values: Vec<Value>) -> Statement {
	Statement::from_sql_and_values(DbBackend::Sqlite, sql, values)
}

/// What is stored of a book
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookState {
	pub media_id: String,
	pub status: String,
	pub file_size: i64,
	pub file_mtime: i64,
	pub extractor: i32,
}

pub const INDEXED: &str = "indexed";
pub const NO_TEXT: &str = "no_text";
pub const FAILED: &str = "failed";

/// The state of every book that has one
pub async fn book_states(conn: &DatabaseConnection) -> ContentResult<Vec<BookState>> {
	let rows = conn
		.query_all_raw(stmt(
			"SELECT media_id, status, file_size, file_mtime, extractor FROM content_books",
			vec![],
		))
		.await?;
	let mut states = Vec::with_capacity(rows.len());
	for row in rows {
		states.push(BookState {
			media_id: row.try_get("", "media_id")?,
			status: row.try_get("", "status")?,
			file_size: row.try_get("", "file_size")?,
			file_mtime: row.try_get("", "file_mtime")?,
			extractor: row.try_get("", "extractor")?,
		});
	}
	Ok(states)
}

/// The state of one book (one indexed lookup, not a scan of the table)
pub async fn book_state(
	conn: &DatabaseConnection,
	media_id: &str,
) -> ContentResult<Option<BookState>> {
	let Some(row) = conn
		.query_one_raw(stmt(
			"SELECT media_id, status, file_size, file_mtime, extractor FROM content_books WHERE media_id = ?",
			vec![media_id.into()],
		))
		.await?
	else {
		return Ok(None);
	};
	Ok(Some(BookState {
		media_id: row.try_get("", "media_id")?,
		status: row.try_get("", "status")?,
		file_size: row.try_get("", "file_size")?,
		file_mtime: row.try_get("", "file_mtime")?,
		extractor: row.try_get("", "extractor")?,
	}))
}

/// Whether a book needs reading: never read, its file changed, or read by an older extractor.
/// A failed book is read again only when `retry_failed` (or its file changed).
pub fn needs_indexing(
	state: Option<&BookState>,
	file_size: i64,
	file_mtime: i64,
	retry_failed: bool,
) -> bool {
	let Some(state) = state else {
		return true;
	};
	if state.file_size != file_size
		|| state.file_mtime != file_mtime
		|| state.extractor != EXTRACTOR_VERSION
	{
		return true;
	}
	state.status == FAILED && retry_failed
}

/// Replaces a book's sections (one transaction: a book is never half indexed) and records its state
pub async fn save_book(
	conn: &DatabaseConnection,
	media_id: &str,
	kind: &str,
	file_size: i64,
	file_mtime: i64,
	sections: Vec<Section>,
) -> ContentResult<()> {
	let tx = conn.begin().await?;
	tx.execute_raw(stmt(
		"DELETE FROM content_sections WHERE media_id = ?",
		vec![media_id.into()],
	))
	.await?;
	let mut chars_total = 0i64;
	let sections_total = sections.len();
	for (n, section) in sections.into_iter().enumerate() {
		let chars = section.text.chars().count() as i64;
		chars_total += chars;
		let marks = if section.page_marks.is_empty() {
			None
		} else {
			serde_json::to_string(&section.page_marks).ok()
		};
		let row = tx
			.query_one_raw(stmt(
				"INSERT INTO content_sections (media_id, n, title, level, chars, page_start, page_marks)
				 VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
				vec![
					media_id.into(),
					(n as i64).into(),
					section.title.into(),
					(section.level as i64).into(),
					chars.into(),
					section.page_start.map(i64::from).into(),
					marks.into(),
				],
			))
			.await?
			.ok_or_else(|| sea_orm::DbErr::RecordNotInserted)?;
		let id: i64 = row.try_get("", "id")?;
		tx.execute_raw(stmt(
			"INSERT INTO content_fts (rowid, title, text)
			 SELECT id, title, ? FROM content_sections WHERE id = ?",
			vec![section.text.into(), id.into()],
		))
		.await?;
	}
	let status = if sections_total == 0 { NO_TEXT } else { INDEXED };
	write_state(
		&tx,
		media_id,
		status,
		Some(kind),
		file_size,
		file_mtime,
		sections_total as i64,
		chars_total,
		None,
	)
	.await?;
	tx.commit().await?;
	Ok(())
}

/// Records that a book could not be read (its old sections go: they are of an older file)
pub async fn save_failure(
	conn: &DatabaseConnection,
	media_id: &str,
	kind: &str,
	file_size: i64,
	file_mtime: i64,
	error: &str,
) -> ContentResult<()> {
	let tx = conn.begin().await?;
	tx.execute_raw(stmt(
		"DELETE FROM content_sections WHERE media_id = ?",
		vec![media_id.into()],
	))
	.await?;
	let mut message = error.to_string();
	super::truncate_chars(&mut message, 500);
	write_state(
		&tx,
		media_id,
		FAILED,
		Some(kind),
		file_size,
		file_mtime,
		0,
		0,
		Some(&message),
	)
	.await?;
	tx.commit().await?;
	Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn write_state(
	conn: &impl ConnectionTrait,
	media_id: &str,
	status: &str,
	kind: Option<&str>,
	file_size: i64,
	file_mtime: i64,
	sections: i64,
	chars: i64,
	error: Option<&str>,
) -> ContentResult<()> {
	conn.execute_raw(stmt(
		"INSERT INTO content_books (media_id, status, kind, file_size, file_mtime, extractor, sections, chars, error, indexed_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		 ON CONFLICT(media_id) DO UPDATE SET status = excluded.status, kind = excluded.kind,
		   file_size = excluded.file_size, file_mtime = excluded.file_mtime, extractor = excluded.extractor,
		   sections = excluded.sections, chars = excluded.chars, error = excluded.error,
		   indexed_at = excluded.indexed_at",
		vec![
			media_id.into(),
			status.into(),
			kind.map(str::to_string).into(),
			file_size.into(),
			file_mtime.into(),
			i64::from(EXTRACTOR_VERSION).into(),
			sections.into(),
			chars.into(),
			error.map(str::to_string).into(),
			Utc::now().to_rfc3339().into(),
		],
	))
	.await?;
	Ok(())
}

/// Removes the index rows of books that are gone (a database without foreign keys keeps them)
pub async fn remove_orphans(conn: &DatabaseConnection) -> ContentResult<u64> {
	let result = conn
		.execute_raw(stmt(
			"DELETE FROM content_sections WHERE media_id NOT IN (SELECT id FROM media)",
			vec![],
		))
		.await?;
	conn.execute_raw(stmt(
		"DELETE FROM content_books WHERE media_id NOT IN (SELECT id FROM media)",
		vec![],
	))
	.await?;
	Ok(result.rows_affected())
}

/// The last run
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LastRun {
	pub started: String,
	pub finished: Option<String>,
	pub ok: Option<bool>,
}

pub async fn run_started(conn: &DatabaseConnection) -> ContentResult<()> {
	conn.execute_raw(stmt(
		"INSERT INTO content_index_runs (id, started_at, finished_at, ok) VALUES (1, ?, NULL, NULL)
		 ON CONFLICT(id) DO UPDATE SET started_at = excluded.started_at, finished_at = NULL, ok = NULL",
		vec![Utc::now().to_rfc3339().into()],
	))
	.await?;
	Ok(())
}

pub async fn run_finished(conn: &DatabaseConnection, ok: bool) -> ContentResult<()> {
	conn.execute_raw(stmt(
		"UPDATE content_index_runs SET finished_at = ?, ok = ? WHERE id = 1",
		vec![Utc::now().to_rfc3339().into(), ok.into()],
	))
	.await?;
	Ok(())
}

pub async fn last_run(conn: &DatabaseConnection) -> ContentResult<Option<LastRun>> {
	let Some(row) = conn
		.query_one_raw(stmt(
			"SELECT started_at, finished_at, ok FROM content_index_runs WHERE id = 1",
			vec![],
		))
		.await?
	else {
		return Ok(None);
	};
	Ok(Some(LastRun {
		started: row.try_get("", "started_at")?,
		finished: row.try_get("", "finished_at")?,
		ok: row.try_get("", "ok")?,
	}))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn state(status: &str) -> BookState {
		BookState {
			media_id: "m".into(),
			status: status.into(),
			file_size: 10,
			file_mtime: 20,
			extractor: EXTRACTOR_VERSION,
		}
	}

	#[test]
	fn an_unchanged_book_is_skipped_and_a_new_or_changed_one_is_not() {
		assert!(needs_indexing(None, 10, 20, false));
		assert!(!needs_indexing(Some(&state(INDEXED)), 10, 20, false));
		assert!(!needs_indexing(Some(&state(NO_TEXT)), 10, 20, true));
		assert!(needs_indexing(Some(&state(INDEXED)), 11, 20, false));
		assert!(needs_indexing(Some(&state(INDEXED)), 10, 21, false));
		let mut old = state(INDEXED);
		old.extractor = EXTRACTOR_VERSION - 1;
		assert!(needs_indexing(Some(&old), 10, 20, false));
	}

	#[test]
	fn a_failed_book_waits_for_a_reindex_or_a_changed_file() {
		assert!(!needs_indexing(Some(&state(FAILED)), 10, 20, false));
		assert!(needs_indexing(Some(&state(FAILED)), 10, 20, true));
		assert!(needs_indexing(Some(&state(FAILED)), 11, 20, false));
	}
}
