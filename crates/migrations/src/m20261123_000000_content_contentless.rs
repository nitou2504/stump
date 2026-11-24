use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

/// The content index without the text (`m20261117_000000_content_index` kept a second copy of every
/// book in `content_fts`: ~10 GB for ~9,000 books, against a 0.4 GB database before it).
///
/// - `content_idx`: an FTS5 table without content (`content=''`, `contentless_delete=1`, the same
///   tokenizer), rowid = `content_sections.id`. It keeps the words and their positions, so searches,
///   phrases and ranking are the same; what it can't do is give the text back, which `core::content::text`
///   reads from the book's file instead.
/// - `content_sections.source`: where in the file a section's text is (EPUB item or PDF pages). NULL for
///   the sections of the first index: they are read by their number until the index job (which reads a
///   book of `content_books.layout` 0 again, quietly) gives them one.
/// - The rows of `content_fts` are copied over in SQL (no book is read), then `content_fts` goes. Every
///   step can be run again: a database where `scripts/fork/content-prewarm.sh` has already copied most of
///   it (while Stump ran) only copies what changed since.
///
/// After it the database file keeps its size until it is vacuumed (`VACUUM`, with Stump stopped).
/// SQLite only; on another database the migration does nothing.
#[derive(DeriveMigrationName)]
pub struct Migration;

/// What the first index's `content_fts` had that `content_idx` has not, in one statement (the
/// migration runs it, and so does the pre-warm script, a range of ids at a time)
pub const COPY_MISSING: &str = "INSERT INTO content_idx (rowid, title, text)
	SELECT rowid, title, text FROM content_fts
	WHERE rowid NOT IN (SELECT id FROM content_idx_docsize)";

/// The statements of the migration for a database that has none of the first index's data (the test
/// database of the other crates runs them after the first migration's)
pub const UP: &[&str] = &[
	"ALTER TABLE content_sections ADD COLUMN source TEXT",
	"ALTER TABLE content_books ADD COLUMN layout INTEGER NOT NULL DEFAULT 0",
	"CREATE VIRTUAL TABLE IF NOT EXISTS content_idx USING fts5(
		title, text, content = '', contentless_delete = 1, tokenize = 'unicode61 remove_diacritics 2'
	)",
	"DROP TRIGGER IF EXISTS content_sections_fts_delete",
	"CREATE TRIGGER IF NOT EXISTS content_sections_idx_delete
		AFTER DELETE ON content_sections
		BEGIN
			DELETE FROM content_idx WHERE rowid = old.id;
		END",
	"DROP TABLE IF EXISTS content_fts",
];

/// Back to the first index's tables, EMPTY (the text of the books can't be got back from an index
/// without it): every book is forgotten, so the previous image's job reads them all again
const DOWN: &[&str] = &[
	"DROP TRIGGER IF EXISTS content_sections_idx_delete",
	"DROP TABLE IF EXISTS content_idx",
	"CREATE VIRTUAL TABLE IF NOT EXISTS content_fts USING fts5(
		title, text, tokenize = 'unicode61 remove_diacritics 2'
	)",
	"CREATE TRIGGER IF NOT EXISTS content_sections_fts_delete
		AFTER DELETE ON content_sections
		BEGIN
			DELETE FROM content_fts WHERE rowid = old.id;
		END",
	"DELETE FROM content_sections",
	"DELETE FROM content_books",
];

async fn has_column(db: &SchemaManager<'_>, table: &str, column: &str) -> Result<bool, DbErr> {
	db.has_column(table, column).await
}

async fn count(conn: &impl sea_orm::ConnectionTrait, sql: &str) -> Result<i64, DbErr> {
	let row = conn
		.query_one_raw(sea_orm::Statement::from_string(
			DatabaseBackend::Sqlite,
			sql.to_string(),
		))
		.await?
		.ok_or_else(|| DbErr::Custom(format!("no row for: {sql}")))?;
	row.try_get::<i64>("", "n")
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		if manager.get_database_backend() != DatabaseBackend::Sqlite {
			return Ok(());
		}
		let conn = manager.get_connection();
		if !has_column(manager, "content_sections", "source").await? {
			conn.execute_unprepared(UP[0]).await?;
		}
		if !has_column(manager, "content_books", "layout").await? {
			conn.execute_unprepared(UP[1]).await?;
		}
		conn.execute_unprepared(UP[2]).await?;
		if manager.has_table("content_fts").await? {
			conn.execute_unprepared(COPY_MISSING).await?;
			// what the first index has lost since a pre-warm copied it (books deleted, read again)
			conn.execute_unprepared(
				"DELETE FROM content_idx WHERE rowid NOT IN (SELECT id FROM content_sections)",
			)
			.await?;
			// never drop the only copy of the text before every row is in the new index
			let old = count(conn, "SELECT COUNT(*) AS n FROM content_fts").await?;
			let new = count(conn, "SELECT COUNT(*) AS n FROM content_idx_docsize").await?;
			if old != new {
				return Err(DbErr::Custom(format!(
					"content_idx has {new} sections and content_fts {old}: not dropping content_fts"
				)));
			}
		}
		conn.execute_unprepared(UP[3]).await?;
		conn.execute_unprepared(UP[4]).await?;
		conn.execute_unprepared(UP[5]).await?;
		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		if manager.get_database_backend() != DatabaseBackend::Sqlite {
			return Ok(());
		}
		let conn = manager.get_connection();
		for statement in DOWN {
			conn.execute_unprepared(statement).await?;
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use crate::{Migrator, MigratorTrait};
	use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
	use sea_orm_migration::SchemaManager;

	const NAME: &str = "m20261123_000000_content_contentless";

	async fn exec(db: &DatabaseConnection, sql: &str) {
		db.execute_unprepared(sql)
			.await
			.unwrap_or_else(|e| panic!("{sql}: {e}"));
	}

	async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
		db.query_one_raw(Statement::from_string(
			db.get_database_backend(),
			sql.to_string(),
		))
		.await
		.expect("should query")
		.expect("should have a row")
		.try_get::<i64>("", "n")
		.expect("should read the count")
	}

	/// A database at the first index, with two books of three sections whose text is in `content_fts`
	async fn first_index() -> DatabaseConnection {
		let db = Database::connect("sqlite::memory:")
			.await
			.expect("should connect to in-memory sqlite");
		let position = Migrator::migrations()
			.iter()
			.position(|m| m.name() == NAME)
			.expect("the migration should be registered") as u32;
		Migrator::up(&db, Some(position))
			.await
			.expect("earlier migrations should apply");
		exec(&db, "PRAGMA foreign_keys = OFF").await;
		for id in ["m1", "m2"] {
			exec(
				&db,
				&format!(
					"INSERT INTO media (id, name, size, extension, pages, path, status, library_id, created_at)
					 VALUES ('{id}', 'Book', 1, 'epub', 1, '/l/{id}.epub', 'READY', 'l', CURRENT_TIMESTAMP)"
				),
			)
			.await;
		}
		exec(&db, "PRAGMA foreign_keys = ON").await;
		for (id, media, n, text) in [
			(1, "m1", 0, "La canción del pájaro"),
			(2, "m1", 1, "Meditation and mindfulness"),
			(3, "m2", 0, "Otro libro, otra canción"),
		] {
			exec(
				&db,
				&format!(
					"INSERT INTO content_sections (id, media_id, n, title) VALUES ({id}, '{media}', {n}, 'T{n}')"
				),
			)
			.await;
			exec(
				&db,
				&format!("INSERT INTO content_fts (rowid, title, text) VALUES ({id}, 'T{n}', '{text}')"),
			)
			.await;
		}
		for media in ["m1", "m2"] {
			exec(
				&db,
				&format!(
					"INSERT INTO content_books (media_id, status, kind, extractor, sections)
					 VALUES ('{media}', 'indexed', 'epub', 1, 1)"
				),
			)
			.await;
		}
		db
	}

	async fn migrate(db: &DatabaseConnection) {
		Migrator::up(db, Some(1))
			.await
			.expect("the migration should apply");
	}

	#[tokio::test]
	async fn the_text_moves_to_an_index_that_keeps_no_text_and_finds_the_same() {
		let db = first_index().await;
		migrate(&db).await;
		let manager = SchemaManager::new(&db);
		assert!(manager.has_table("content_idx").await.unwrap());
		assert!(!manager.has_table("content_fts").await.unwrap(), "the text copy is gone");
		for (query, hits) in [("CANCION", 2), ("pajaro", 1), ("\"meditation and mindfulness\"", 1)] {
			assert_eq!(
				count(
					&db,
					&format!("SELECT COUNT(*) AS n FROM content_idx WHERE content_idx MATCH '{query}'")
				)
				.await,
				hits,
				"{query}"
			);
		}
		// no text: the index can't give it back
		let error = db
			.query_one_raw(Statement::from_string(
				db.get_database_backend(),
				"SELECT text FROM content_idx WHERE rowid = 1".to_string(),
			))
			.await
			.ok()
			.flatten()
			.and_then(|row| row.try_get::<Option<String>>("", "text").ok().flatten());
		assert!(error.is_none(), "a contentless table returns no text");
		// the books keep their state and layout 0, the sections no source
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM content_books WHERE layout = 0").await, 2);
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM content_sections WHERE source IS NULL").await, 3);
	}

	#[tokio::test]
	async fn what_a_prewarm_already_copied_is_not_copied_again_and_what_changed_since_is() {
		let db = first_index().await;
		// the pre-warm copied everything, then a section went and another came
		exec(
			&db,
			"CREATE VIRTUAL TABLE content_idx USING fts5(
				title, text, content = '', contentless_delete = 1, tokenize = 'unicode61 remove_diacritics 2')",
		)
		.await;
		exec(&db, super::COPY_MISSING).await;
		exec(&db, "DELETE FROM content_sections WHERE id = 2").await; // the old trigger drops it from content_fts only
		exec(
			&db,
			"INSERT INTO content_sections (id, media_id, n, title) VALUES (4, 'm2', 1, 'T1')",
		)
		.await;
		exec(&db, "INSERT INTO content_fts (rowid, title, text) VALUES (4, 'T1', 'un cuarto texto')").await;
		migrate(&db).await;
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM content_idx_docsize").await, 3);
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM content_idx WHERE content_idx MATCH 'mindfulness'").await,
			0,
			"the section deleted after the pre-warm is not found"
		);
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM content_idx WHERE content_idx MATCH 'cuarto'").await,
			1
		);
	}

	#[tokio::test]
	async fn deleting_sections_or_their_book_removes_them_from_the_index() {
		let db = first_index().await;
		migrate(&db).await;
		exec(&db, "DELETE FROM content_sections WHERE id = 1").await;
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM content_idx_docsize").await, 2);
		exec(&db, "DELETE FROM media WHERE id = 'm2'").await;
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM content_idx_docsize").await, 1);
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM content_sections").await, 1);
	}

	#[tokio::test]
	async fn the_migration_is_not_applied_twice_and_runs_again_after_a_stop() {
		let db = first_index().await;
		// a run that stopped after the columns: they exist, the migration is not recorded
		exec(&db, "ALTER TABLE content_sections ADD COLUMN source TEXT").await;
		migrate(&db).await;
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM content_idx_docsize").await, 3);
	}

	#[tokio::test]
	async fn down_makes_the_first_index_tables_empty_for_the_previous_image() {
		let db = first_index().await;
		migrate(&db).await;
		Migrator::down(&db, Some(1)).await.expect("the migration should roll back");
		let manager = SchemaManager::new(&db);
		assert!(manager.has_table("content_fts").await.unwrap());
		assert!(!manager.has_table("content_idx").await.unwrap());
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM content_books").await, 0);
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM content_sections").await, 0);
	}

	#[tokio::test]
	async fn the_manual_down_does_the_same() {
		let db = first_index().await;
		migrate(&db).await;
		exec(&db, include_str!("../manual-down/m20261123_000000_content_contentless.sql")).await;
		let manager = SchemaManager::new(&db);
		assert!(manager.has_table("content_fts").await.unwrap());
		assert!(!manager.has_table("content_idx").await.unwrap());
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM content_books").await, 0);
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM seaql_migrations WHERE version = 'm20261123_000000_content_contentless'").await,
			0
		);
	}
}
