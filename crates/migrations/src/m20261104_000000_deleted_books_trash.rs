use sea_orm_migration::prelude::*;

/// Deleted books (the trash): who removed a book, how long removed books are kept before they
/// are deleted permanently (14 days, 0 = never), the last automatic purge, and a log of the
/// books deleted permanently (their files went to the quarantine folder)
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// SQLite takes one column per ALTER TABLE
		manager
			.alter_table(
				Table::alter()
					.table(Media::Table)
					.add_column(ColumnDef::new(Media::DeletedBy).text().null())
					.to_owned(),
			)
			.await?;
		manager
			.alter_table(
				Table::alter()
					.table(ServerConfig::Table)
					.add_column(
						ColumnDef::new(ServerConfig::TrashRetentionDays)
							.integer()
							.not_null()
							.default(14),
					)
					.to_owned(),
			)
			.await?;
		manager
			.alter_table(
				Table::alter()
					.table(ServerConfig::Table)
					.add_column(
						ColumnDef::new(ServerConfig::TrashLastPurgeAt)
							.timestamp_with_time_zone()
							.null(),
					)
					.to_owned(),
			)
			.await?;
		manager
			.alter_table(
				Table::alter()
					.table(ServerConfig::Table)
					.add_column(
						ColumnDef::new(ServerConfig::TrashLastPurge).json().null(),
					)
					.to_owned(),
			)
			.await?;

		// no foreign keys: a row outlives its book, library and users
		manager
			.create_table(
				Table::create()
					.table(MediaPurges::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(MediaPurges::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(MediaPurges::MediaId).text().not_null())
					.col(ColumnDef::new(MediaPurges::LibraryId).text().null())
					.col(ColumnDef::new(MediaPurges::LibraryName).text().null())
					.col(ColumnDef::new(MediaPurges::Title).text().not_null())
					.col(ColumnDef::new(MediaPurges::Authors).text().null())
					.col(ColumnDef::new(MediaPurges::OriginalPath).text().not_null())
					.col(ColumnDef::new(MediaPurges::QuarantinePath).text().null())
					.col(ColumnDef::new(MediaPurges::Bytes).big_integer().null())
					.col(ColumnDef::new(MediaPurges::DeletedBy).text().null())
					.col(
						ColumnDef::new(MediaPurges::DeletedAt)
							.timestamp_with_time_zone()
							.null(),
					)
					.col(ColumnDef::new(MediaPurges::PurgedBy).text().null())
					.col(
						ColumnDef::new(MediaPurges::PurgedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(MediaPurges::RestoredAt)
							.timestamp_with_time_zone()
							.null(),
					)
					.col(ColumnDef::new(MediaPurges::RestoredBy).text().null())
					.to_owned(),
			)
			.await?;
		manager
			.create_index(
				Index::create()
					.name("media_purges_purged_at_idx")
					.table(MediaPurges::Table)
					.col(MediaPurges::PurgedAt)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(MediaPurges::Table).to_owned())
			.await?;
		for column in [
			ServerConfig::TrashLastPurge,
			ServerConfig::TrashLastPurgeAt,
			ServerConfig::TrashRetentionDays,
		] {
			manager
				.alter_table(
					Table::alter()
						.table(ServerConfig::Table)
						.drop_column(column)
						.to_owned(),
				)
				.await?;
		}
		manager
			.alter_table(
				Table::alter()
					.table(Media::Table)
					.drop_column(Media::DeletedBy)
					.to_owned(),
			)
			.await
	}
}

#[derive(DeriveIden)]
enum Media {
	Table,
	DeletedBy,
}

#[derive(DeriveIden)]
enum ServerConfig {
	Table,
	TrashRetentionDays,
	TrashLastPurgeAt,
	TrashLastPurge,
}

#[derive(DeriveIden)]
enum MediaPurges {
	Table,
	Id,
	MediaId,
	LibraryId,
	LibraryName,
	Title,
	Authors,
	OriginalPath,
	QuarantinePath,
	Bytes,
	DeletedBy,
	DeletedAt,
	PurgedBy,
	PurgedAt,
	RestoredAt,
	RestoredBy,
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
	struct Retention {
		trash_retention_days: i32,
	}

	#[derive(Debug, FromQueryResult)]
	struct DeletedBook {
		id: String,
		deleted_by: Option<String>,
	}

	/// The whole migration chain on an empty SQLite database: the books already in the trash
	/// keep their removal date (who removed them is unknown) and the server keeps them 14
	/// days; rolling back drops what was added
	#[tokio::test]
	async fn test_trash_migration_on_the_full_chain() {
		let db = Database::connect("sqlite::memory:")
			.await
			.expect("should connect to in-memory sqlite");
		let total = Migrator::migrations().len() as u32;
		assert_eq!(
			Migrator::migrations()
				.last()
				.map(|m| m.name().to_string())
				.as_deref(),
			Some("m20261104_000000_deleted_books_trash"),
			"this migration should be the last one"
		);
		Migrator::up(&db, Some(total - 1))
			.await
			.expect("earlier migrations should apply");

		// a server and a book removed before the upgrade
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
				deleted_at, is_oneshot, library_id)
			VALUES ('book', 'Book', 10, 'epub', 1, '/lib/Book.epub', 'READY',
				'2026-11-01T00:00:00+00:00', '2026-11-03T16:06:19+00:00', 0, 'lib')",
		)
		.await;

		Migrator::up(&db, Some(1))
			.await
			.expect("the trash migration should apply");

		let manager = SchemaManager::new(&db);
		assert!(manager.has_column("media", "deleted_by").await.unwrap());
		assert!(manager.has_table("media_purges").await.unwrap());
		for column in [
			"trash_retention_days",
			"trash_last_purge_at",
			"trash_last_purge",
		] {
			assert!(
				manager.has_column("server_config", column).await.unwrap(),
				"server_config.{column} should exist"
			);
		}
		let retention = Retention::find_by_statement(Statement::from_string(
			db.get_database_backend(),
			"SELECT trash_retention_days FROM server_config",
		))
		.one(&db)
		.await
		.unwrap()
		.expect("the server config should still be there");
		assert_eq!(retention.trash_retention_days, 14);
		let book = DeletedBook::find_by_statement(Statement::from_string(
			db.get_database_backend(),
			"SELECT id, deleted_by FROM media WHERE deleted_at IS NOT NULL",
		))
		.one(&db)
		.await
		.unwrap()
		.expect("the removed book should still be in the trash");
		assert_eq!((book.id.as_str(), book.deleted_by), ("book", None));

		// the log takes rows without a book, library or user behind them
		exec(
			&db,
			r"INSERT INTO media_purges (media_id, title, original_path, quarantine_path, bytes)
			VALUES ('gone', 'Gone', '/lib/Gone.epub', '/q/deleted/2026-11-04/Gone.epub', 10)",
		)
		.await;

		Migrator::down(&db, Some(1))
			.await
			.expect("the rollback should apply");
		assert!(!manager.has_column("media", "deleted_by").await.unwrap());
		assert!(!manager.has_table("media_purges").await.unwrap());
		assert!(!manager
			.has_column("server_config", "trash_retention_days")
			.await
			.unwrap());
		// the book is still in the trash after the rollback
		let book = DeletedBook::find_by_statement(Statement::from_string(
			db.get_database_backend(),
			"SELECT id, NULL AS deleted_by FROM media WHERE deleted_at IS NOT NULL",
		))
		.one(&db)
		.await
		.unwrap();
		assert!(book.is_some());
	}
}
