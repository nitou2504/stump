use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

/// The content index (`/api/v2/content`): the text of each EPUB and PDF, per section, in SQLite
/// tables and an FTS5 table, so the books can be searched, listed by section and read by passage.
///
/// - `content_books`: one row per indexed book (its state, the size and modification time of the
///   file it was read from, so an unchanged file is skipped, and why it failed)
/// - `content_sections`: one row per section (a chapter, or a PDF outline entry or page)
/// - `content_fts`: the sections' title and text (`unicode61 remove_diacritics 2`: accents and case
///   don't matter), its rowid is `content_sections.id`; a trigger removes a section's text with it
/// - `content_index_runs`: the last indexing run, for `GET /api/v2/content/status`
///
/// Nothing of Stump's other tables moves; an older image never reads these tables and rolling back is
/// dropping them (`manual-down/`). SQLite only: on another database the migration does nothing.
#[derive(DeriveMigrationName)]
pub struct Migration;

/// The statements of the migration (the test database of the other crates runs them too)
pub const UP: &[&str] = &[
	"CREATE TABLE IF NOT EXISTS content_books (
		media_id TEXT NOT NULL PRIMARY KEY REFERENCES media(id) ON DELETE CASCADE ON UPDATE CASCADE,
		status TEXT NOT NULL,
		kind TEXT,
		file_size BIGINT NOT NULL DEFAULT 0,
		file_mtime BIGINT NOT NULL DEFAULT 0,
		extractor INTEGER NOT NULL DEFAULT 0,
		sections INTEGER NOT NULL DEFAULT 0,
		chars BIGINT NOT NULL DEFAULT 0,
		error TEXT,
		indexed_at TEXT
	)",
	"CREATE TABLE IF NOT EXISTS content_sections (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		media_id TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE ON UPDATE CASCADE,
		n INTEGER NOT NULL,
		title TEXT NOT NULL DEFAULT '',
		level INTEGER NOT NULL DEFAULT 1,
		chars INTEGER NOT NULL DEFAULT 0,
		page_start INTEGER,
		page_marks TEXT,
		UNIQUE (media_id, n)
	)",
	"CREATE VIRTUAL TABLE IF NOT EXISTS content_fts USING fts5(
		title, text, tokenize = 'unicode61 remove_diacritics 2'
	)",
	"CREATE TRIGGER IF NOT EXISTS content_sections_fts_delete
		AFTER DELETE ON content_sections
		BEGIN
			DELETE FROM content_fts WHERE rowid = old.id;
		END",
	"CREATE TABLE IF NOT EXISTS content_index_runs (
		id INTEGER PRIMARY KEY CHECK (id = 1),
		started_at TEXT NOT NULL,
		finished_at TEXT,
		ok BOOLEAN
	)",
];

const DOWN: &[&str] = &[
	"DROP TRIGGER IF EXISTS content_sections_fts_delete",
	"DROP TABLE IF EXISTS content_fts",
	"DROP TABLE IF EXISTS content_sections",
	"DROP TABLE IF EXISTS content_books",
	"DROP TABLE IF EXISTS content_index_runs",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		if manager.get_database_backend() != DatabaseBackend::Sqlite {
			return Ok(());
		}
		let conn = manager.get_connection();
		for statement in UP {
			conn.execute_unprepared(statement).await?;
		}
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

	const NAME: &str = "m20261117_000000_content_index";

	/// An in-memory database migrated to the step before this migration (foreign keys on, as the
	/// server has them), with a library's book
	async fn before() -> DatabaseConnection {
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
		db.execute_unprepared("PRAGMA foreign_keys = ON")
			.await
			.expect("should turn on foreign keys");
		db
	}

	async fn migrated() -> DatabaseConnection {
		let db = before().await;
		Migrator::up(&db, Some(1))
			.await
			.expect("the migration should apply");
		db
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

	async fn applied_versions(db: &DatabaseConnection) -> Vec<String> {
		db.query_all_raw(Statement::from_string(
			db.get_database_backend(),
			"SELECT version FROM seaql_migrations ORDER BY version".to_string(),
		))
		.await
		.expect("should query")
		.into_iter()
		.map(|row| row.try_get::<String>("", "version").expect("a version"))
		.collect()
	}

	/// The bundled SQLite has FTS5 (the migration would fail otherwise)
	#[tokio::test]
	async fn the_migration_makes_the_tables_and_fts5_works() {
		let db = migrated().await;
		let manager = SchemaManager::new(&db);
		for table in [
			"content_books",
			"content_sections",
			"content_fts",
			"content_index_runs",
		] {
			assert!(manager.has_table(table).await.unwrap(), "{table}");
		}
		db.execute_unprepared(
			"INSERT INTO content_fts (rowid, title, text) VALUES (1, 'Capítulo', 'La canción del pájaro')",
		)
		.await
		.expect("should insert");
		// accents and case don't matter
		assert_eq!(
			count(
				&db,
				"SELECT COUNT(*) AS n FROM content_fts WHERE content_fts MATCH 'CANCION'"
			)
			.await,
			1
		);
		assert_eq!(
			count(
				&db,
				"SELECT COUNT(*) AS n FROM content_fts WHERE content_fts MATCH 'pajaro'"
			)
			.await,
			1
		);
	}

	/// Removing a book removes its sections and their text
	#[tokio::test]
	async fn deleting_a_book_removes_its_text() {
		let db = migrated().await;
		db.execute_unprepared("PRAGMA foreign_keys = OFF")
			.await
			.unwrap();
		db.execute_unprepared(
			"INSERT INTO libraries (id, name, path, status, config_id, created_at, emoji, is_standalone)
			 VALUES ('l', 'L', '/l', 'READY', 'c', CURRENT_TIMESTAMP, NULL, 0)",
		)
		.await
		.ok();
		db.execute_unprepared(
			"INSERT INTO media (id, name, size, extension, pages, path, status, library_id, created_at)
			 VALUES ('m1', 'Book', 1, 'epub', 1, '/l/b.epub', 'READY', 'l', CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert a book");
		db.execute_unprepared("PRAGMA foreign_keys = ON")
			.await
			.unwrap();
		db.execute_unprepared(
			"INSERT INTO content_sections (id, media_id, n, title) VALUES (7, 'm1', 0, 'One')",
		)
		.await
		.expect("should insert a section");
		db.execute_unprepared(
			"INSERT INTO content_fts (rowid, title, text) VALUES (7, 'One', 'some text')",
		)
		.await
		.unwrap();
		db.execute_unprepared("DELETE FROM media WHERE id = 'm1'")
			.await
			.expect("should delete the book");
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM content_sections").await,
			0
		);
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM content_fts").await, 0);
	}

	#[tokio::test]
	async fn down_drops_the_tables_and_the_record() {
		let db = migrated().await;
		Migrator::down(&db, Some(1))
			.await
			.expect("the migration should roll back");
		let manager = SchemaManager::new(&db);
		for table in ["content_books", "content_sections", "content_fts"] {
			assert!(!manager.has_table(table).await.unwrap(), "{table}");
		}
		assert!(!applied_versions(&db).await.contains(&NAME.to_string()));
	}

	#[tokio::test]
	async fn the_manual_down_leaves_a_database_the_previous_image_can_use() {
		let db = migrated().await;
		let versions: Vec<String> = applied_versions(&db)
			.await
			.into_iter()
			.filter(|version| version != NAME)
			.collect();
		let manual_down = include_str!("../manual-down/m20261117_000000_content_index.sql");
		db.execute_unprepared(manual_down)
			.await
			.expect("the manual down should run");
		let manager = SchemaManager::new(&db);
		for table in ["content_books", "content_sections", "content_fts"] {
			assert!(!manager.has_table(table).await.unwrap(), "{table}");
		}
		assert_eq!(applied_versions(&db).await, versions);
	}
}
