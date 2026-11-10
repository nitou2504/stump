use sea_orm_migration::prelude::*;

/// Uploads that wait for an approver (the server owner, or anyone who manages libraries) before
/// they enter a library.
///
/// - `intake_items` gets `submitted_at`, `reviewed_at`, `reviewed_by`, `review_note` and
///   `cover_file`: plain nullable columns, with no foreign key and no index, so a rollback can
///   `DROP COLUMN` them (SQLite can't drop a column that is part of a foreign key or an index).
///   The new states (`PENDING_APPROVAL`, `REJECTED`, `FILING`) are plain text and need no schema
///   change.
/// - `user_intake_limits` holds a user's own upload limits where they differ from the server's.
///   No foreign key and no index, for the same reason: its rollback is a `DROP TABLE`.
///
/// The rollback puts the new states back to ones an older version knows first.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// SQLite takes one change per statement
		for column in [
			ColumnDef::new(IntakeItems::SubmittedAt)
				.timestamp_with_time_zone()
				.to_owned(),
			ColumnDef::new(IntakeItems::ReviewedAt)
				.timestamp_with_time_zone()
				.to_owned(),
			ColumnDef::new(IntakeItems::ReviewedBy).text().to_owned(),
			ColumnDef::new(IntakeItems::ReviewNote).text().to_owned(),
			ColumnDef::new(IntakeItems::CoverFile).text().to_owned(),
		] {
			manager
				.alter_table(
					Table::alter()
						.table(IntakeItems::Table)
						.add_column(column)
						.to_owned(),
				)
				.await?;
		}

		manager
			.create_table(
				Table::create()
					.table(UserIntakeLimits::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(UserIntakeLimits::UserId)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(ColumnDef::new(UserIntakeLimits::Limits).json().not_null())
					.col(
						ColumnDef::new(UserIntakeLimits::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// an older version can't read the new states: waiting and half-filed items are ready
		// again, and a sent-back one needs another look
		manager
			.get_connection()
			.execute_unprepared(
				"UPDATE intake_items SET state = CASE state \
				 WHEN 'PENDING_APPROVAL' THEN 'READY' \
				 WHEN 'FILING' THEN 'READY' \
				 WHEN 'REJECTED' THEN 'NEEDS_REVIEW' \
				 ELSE state END",
			)
			.await?;

		manager
			.drop_table(Table::drop().table(UserIntakeLimits::Table).to_owned())
			.await?;
		for column in [
			IntakeItems::SubmittedAt,
			IntakeItems::ReviewedAt,
			IntakeItems::ReviewedBy,
			IntakeItems::ReviewNote,
			IntakeItems::CoverFile,
		] {
			manager
				.alter_table(
					Table::alter()
						.table(IntakeItems::Table)
						.drop_column(column)
						.to_owned(),
				)
				.await?;
		}
		Ok(())
	}
}

#[derive(DeriveIden)]
enum IntakeItems {
	Table,
	SubmittedAt,
	ReviewedAt,
	ReviewedBy,
	ReviewNote,
	CoverFile,
}

#[derive(DeriveIden)]
enum UserIntakeLimits {
	Table,
	UserId,
	Limits,
	UpdatedAt,
}

#[cfg(test)]
mod tests {
	use crate::{Migrator, MigratorTrait};
	use sea_orm::{ConnectionTrait, Database, FromQueryResult, Statement};
	use sea_orm_migration::SchemaManager;

	const NAME: &str = "m20261112_000000_intake_approval";

	#[derive(Debug, FromQueryResult)]
	struct Row {
		id: String,
		state: String,
	}

	/// The columns and the table are created, an existing item keeps its state, and rolling back
	/// puts the new states back to ones an older version reads and drops what was added
	#[tokio::test]
	async fn test_intake_approval_migration() {
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

		let backend = db.get_database_backend();
		// the rows below need no user or library of their own
		db.execute_unprepared("PRAGMA foreign_keys = OFF")
			.await
			.expect("foreign keys off");
		db.execute_raw(Statement::from_string(
			backend,
			"INSERT INTO intake_items (id, user_id, library_id, state, kind, name, \
			 staging_dir, files) VALUES ('i', 'u', 'l', 'READY', 'BOOK', 'a.epub', \
			 '/staging', '[]')",
		))
		.await
		.expect("an inbox item");

		Migrator::up(&db, Some(1))
			.await
			.expect("the migration should apply");
		let manager = SchemaManager::new(&db);
		for column in [
			"submitted_at",
			"reviewed_at",
			"reviewed_by",
			"review_note",
			"cover_file",
		] {
			assert!(
				manager.has_column("intake_items", column).await.unwrap(),
				"{column}"
			);
		}
		for column in ["user_id", "limits", "updated_at"] {
			assert!(
				manager
					.has_column("user_intake_limits", column)
					.await
					.unwrap(),
				"{column}"
			);
		}
		let state = |db: sea_orm::DatabaseConnection| async move {
			Row::find_by_statement(Statement::from_string(
				db.get_database_backend(),
				"SELECT id, state FROM intake_items ORDER BY id",
			))
			.all(&db)
			.await
			.unwrap()
		};
		let rows = state(db.clone()).await;
		assert_eq!(rows[0].state, "READY", "an existing item keeps its state");

		// items in the new states, as they would be after a time in use
		for (id, new_state) in [
			("p", "PENDING_APPROVAL"),
			("f", "FILING"),
			("r", "REJECTED"),
		] {
			db.execute_raw(Statement::from_string(
				backend,
				format!(
					"INSERT INTO intake_items (id, user_id, library_id, state, kind, \
					 name, staging_dir, files, review_note) VALUES ('{id}', 'u', 'l', \
					 '{new_state}', 'BOOK', 'a.epub', '/staging', '[]', 'a note')"
				),
			))
			.await
			.expect("an item in a new state");
		}

		Migrator::down(&db, Some(1))
			.await
			.expect("the rollback should apply");
		for column in [
			"submitted_at",
			"reviewed_at",
			"reviewed_by",
			"review_note",
			"cover_file",
		] {
			assert!(
				!manager.has_column("intake_items", column).await.unwrap(),
				"{column}"
			);
		}
		assert!(!manager.has_table("user_intake_limits").await.unwrap());
		let rows: Vec<(String, String)> = state(db.clone())
			.await
			.into_iter()
			.map(|row| (row.id, row.state))
			.collect();
		assert_eq!(
			rows,
			[
				("f".to_string(), "READY".to_string()),
				("i".to_string(), "READY".to_string()),
				("p".to_string(), "READY".to_string()),
				("r".to_string(), "NEEDS_REVIEW".to_string()),
			],
			"the new states are put back to ones an older version reads"
		);
	}
}
