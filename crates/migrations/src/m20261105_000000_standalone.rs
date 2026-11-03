use sea_orm_migration::prelude::*;

/// Stump on its own (no external tooling around it):
///
/// - `held_files`: the files Stump takes out of a library and keeps in `<config>/trash` for the
///   *Deleted books* retention (originals replaced by post-processing), so they can be put back;
///   the external quarantine folder is no longer used;
/// - `server_config.google_books_api_key`: an optional key for the online lookups (encrypted);
/// - `library_intake_settings.csv_path` goes: *Add books* no longer appends to a CSV file (the
///   filed items in the database are the record).
///
/// Books already in *Deleted books* are untouched: they keep their removal date, who removed
/// them and so their countdown.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(HeldFiles::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(HeldFiles::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(HeldFiles::MediaId).text().null())
					.col(ColumnDef::new(HeldFiles::LibraryId).text().null())
					.col(ColumnDef::new(HeldFiles::Title).text().not_null())
					.col(ColumnDef::new(HeldFiles::OriginalPath).text().not_null())
					.col(ColumnDef::new(HeldFiles::HeldPath).text().not_null())
					.col(ColumnDef::new(HeldFiles::Bytes).big_integer().not_null())
					.col(ColumnDef::new(HeldFiles::Reason).text().not_null())
					.col(ColumnDef::new(HeldFiles::Steps).text().null())
					.col(ColumnDef::new(HeldFiles::TaskId).text().null())
					.col(
						ColumnDef::new(HeldFiles::HeldAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(ColumnDef::new(HeldFiles::HeldBy).text().null())
					.to_owned(),
			)
			.await?;
		manager
			.create_index(
				Index::create()
					.name("held_files_held_at_idx")
					.table(HeldFiles::Table)
					.col(HeldFiles::HeldAt)
					.to_owned(),
			)
			.await?;
		manager
			.create_index(
				Index::create()
					.name("held_files_media_id_idx")
					.table(HeldFiles::Table)
					.col(HeldFiles::MediaId)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ServerConfig::Table)
					.add_column(
						ColumnDef::new(ServerConfig::GoogleBooksApiKey)
							.text()
							.null(),
					)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(LibraryIntakeSettings::Table)
					.drop_column(LibraryIntakeSettings::CsvPath)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(LibraryIntakeSettings::Table)
					.add_column(
						ColumnDef::new(LibraryIntakeSettings::CsvPath).text().null(),
					)
					.to_owned(),
			)
			.await?;
		manager
			.alter_table(
				Table::alter()
					.table(ServerConfig::Table)
					.drop_column(ServerConfig::GoogleBooksApiKey)
					.to_owned(),
			)
			.await?;
		manager
			.drop_table(Table::drop().table(HeldFiles::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum HeldFiles {
	Table,
	Id,
	MediaId,
	LibraryId,
	Title,
	OriginalPath,
	HeldPath,
	Bytes,
	Reason,
	Steps,
	TaskId,
	HeldAt,
	HeldBy,
}

#[derive(DeriveIden)]
enum ServerConfig {
	Table,
	GoogleBooksApiKey,
}

#[derive(DeriveIden)]
enum LibraryIntakeSettings {
	Table,
	CsvPath,
}

#[cfg(test)]
mod tests {
	use crate::{Migrator, MigratorTrait};
	use sea_orm::{
		ConnectionTrait, Database, DatabaseConnection, FromQueryResult, Statement,
	};
	use sea_orm_migration::SchemaManager;

	async fn exec(db: &DatabaseConnection, sql: &str) {
		db.execute_raw(Statement::from_string(db.get_database_backend(), sql))
			.await
			.unwrap_or_else(|error| panic!("{sql}: {error}"));
	}

	#[derive(Debug, FromQueryResult)]
	struct DeletedBook {
		id: String,
		deleted_at: String,
		deleted_by: Option<String>,
	}

	#[derive(Debug, FromQueryResult)]
	struct IntakeSettings {
		library_id: String,
		manga_dir: String,
	}

	const NAME: &str = "m20261105_000000_standalone";

	/// The whole migration chain on an empty SQLite database: books in *Deleted books* keep
	/// their removal (and so their countdown), the intake settings lose their CSV path, and
	/// rolling back puts the column back
	#[tokio::test]
	async fn test_standalone_migration_on_the_full_chain() {
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

		exec(
			&db,
			"INSERT INTO server_config (initial_wal_setup_complete) VALUES (1)",
		)
		.await;
		exec(
			&db,
			r"INSERT INTO library_configs (convert_rar_to_zip, hard_delete_conversions,
				default_reading_dir, default_reading_mode, default_reading_image_scale_fit,
				generate_file_hashes, generate_koreader_hashes, process_metadata, watch,
				library_pattern)
			VALUES (0, 0, 'ltr', 'paged', 'height', 0, 0, 1, 0, 'SERIES_BASED')",
		)
		.await;
		exec(
			&db,
			r"INSERT INTO libraries (id, name, path, status, created_at, config_id)
			VALUES ('lib', 'Library', '/lib', 'READY', '2026-11-01T00:00:00+00:00',
				(SELECT id FROM library_configs LIMIT 1))",
		)
		.await;
		exec(
			&db,
			r"INSERT INTO media (id, name, size, extension, pages, path, status, created_at,
				deleted_at, deleted_by, is_oneshot, library_id)
			VALUES ('book', 'Book', 10, 'epub', 1, '/lib/Book.epub', 'READY',
				'2026-11-01T00:00:00+00:00', '2026-11-03T16:06:19.948279473+00:00', 'u1', 0,
				'lib')",
		)
		.await;
		exec(
			&db,
			r"INSERT INTO library_intake_settings (library_id, manga_dir, light_novel_dir,
				csv_path, online_lookup)
			VALUES ('lib', '_Manga', '_Novela ligera', '/lib/stump_intake.csv', 1)",
		)
		.await;

		Migrator::up(&db, Some(1))
			.await
			.expect("the migration should apply");
		let manager = SchemaManager::new(&db);
		assert!(manager.has_table("held_files").await.unwrap());
		assert!(manager
			.has_column("server_config", "google_books_api_key")
			.await
			.unwrap());
		assert!(!manager
			.has_column("library_intake_settings", "csv_path")
			.await
			.unwrap());
		let settings = IntakeSettings::find_by_statement(Statement::from_string(
			db.get_database_backend(),
			"SELECT library_id, manga_dir FROM library_intake_settings",
		))
		.one(&db)
		.await
		.unwrap()
		.expect("the library's intake settings should still be there");
		assert_eq!(
			(settings.library_id.as_str(), settings.manga_dir.as_str()),
			("lib", "_Manga")
		);
		let book = DeletedBook::find_by_statement(Statement::from_string(
			db.get_database_backend(),
			"SELECT id, deleted_at, deleted_by FROM media WHERE deleted_at IS NOT NULL",
		))
		.one(&db)
		.await
		.unwrap()
		.expect("the removed book should still be in Deleted books");
		assert_eq!(book.id, "book");
		assert_eq!(book.deleted_at, "2026-11-03T16:06:19.948279473+00:00");
		assert_eq!(book.deleted_by.as_deref(), Some("u1"));
		exec(
			&db,
			r"INSERT INTO held_files (media_id, library_id, title, original_path, held_path,
				bytes, reason, steps, task_id, held_by)
			VALUES ('book', 'lib', 'Book', '/lib/Book.cbz', '2026-11-05/Book.cbz', 10,
				'GREY16', NULL, 't1', NULL)",
		)
		.await;

		Migrator::down(&db, Some(1))
			.await
			.expect("the rollback should apply");
		assert!(!manager.has_table("held_files").await.unwrap());
		assert!(manager
			.has_column("library_intake_settings", "csv_path")
			.await
			.unwrap());
		assert!(!manager
			.has_column("server_config", "google_books_api_key")
			.await
			.unwrap());
	}
}
