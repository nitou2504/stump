use sea_orm_migration::prelude::*;

/// Public access: "Needs your attention" can be cleared. The warnings are computed on request, so
/// to dismiss one the server remembers *what the owner had seen* of it: a dismissed warning is
/// hidden until something new or worse happens (a newer event, a higher count).
///
/// - `access_warning_reviews`: one row per dismissed warning id (`AtDeviceLimit:<user id>`,
///   `SigninFailures:-`, ...), with the count and the time of the newest event when it was
///   dismissed. Nothing else of Stump's schema moves; an older image never reads this table, and
///   rolling back is dropping it (the warnings then all show again).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(AccessWarningReviews::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AccessWarningReviews::WarningId)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(
						ColumnDef::new(AccessWarningReviews::DismissedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(AccessWarningReviews::DismissedBy).text())
					.col(
						ColumnDef::new(AccessWarningReviews::CountAtDismiss)
							.big_integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(AccessWarningReviews::LastAtDismiss)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-access_warning_reviews-dismissed_by")
							.from(
								AccessWarningReviews::Table,
								AccessWarningReviews::DismissedBy,
							)
							.to(Users::Table, Users::Id)
							.on_delete(ForeignKeyAction::SetNull)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(
				Table::drop()
					.table(AccessWarningReviews::Table)
					.if_exists()
					.to_owned(),
			)
			.await
	}
}

#[derive(DeriveIden)]
enum AccessWarningReviews {
	Table,
	WarningId,
	DismissedAt,
	DismissedBy,
	CountAtDismiss,
	LastAtDismiss,
}

#[derive(DeriveIden)]
enum Users {
	Table,
	Id,
}

#[cfg(test)]
mod tests {
	use crate::{Migrator, MigratorTrait};
	use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
	use sea_orm_migration::SchemaManager;

	const NAME: &str = "m20261116_000000_access_warning_reviews";

	/// An in-memory database migrated to the step before this migration (with foreign keys on, as
	/// the server has them), with an account an older image would have written
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
		db.execute_unprepared(
			"INSERT INTO users (id, username, hashed_password, is_server_owner, is_locked, created_at)
			 VALUES ('owner', 'nitou', 'x', 1, 0, CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert a user");
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

	#[tokio::test]
	async fn the_migration_makes_the_table() {
		let db = migrated().await;
		assert!(SchemaManager::new(&db)
			.has_table("access_warning_reviews")
			.await
			.unwrap());
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM access_warning_reviews").await,
			0
		);
	}

	#[tokio::test]
	async fn down_drops_the_table_and_the_record() {
		let db = migrated().await;
		Migrator::down(&db, Some(1))
			.await
			.expect("the migration should roll back");
		assert!(!SchemaManager::new(&db)
			.has_table("access_warning_reviews")
			.await
			.unwrap());
		assert!(!applied_versions(&db).await.contains(&NAME.to_string()));
	}

	#[tokio::test]
	async fn the_manual_down_leaves_a_database_the_previous_image_can_use() {
		let db = migrated().await;
		db.execute_unprepared(
			"INSERT INTO access_warning_reviews (warning_id, dismissed_at, dismissed_by, count_at_dismiss, last_at_dismiss)
			 VALUES ('AtDeviceLimit:u1', CURRENT_TIMESTAMP, 'owner', 3, CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert a review");
		let versions: Vec<String> = applied_versions(&db)
			.await
			.into_iter()
			.filter(|version| version != NAME)
			.collect();

		// the file the owner runs, as the text of this test so the two can't drift apart
		let manual_down =
			include_str!("../manual-down/m20261116_000000_access_warning_reviews.sql");
		db.execute_unprepared(manual_down)
			.await
			.expect("the manual down should run");

		assert!(!SchemaManager::new(&db)
			.has_table("access_warning_reviews")
			.await
			.unwrap());
		assert_eq!(applied_versions(&db).await, versions);
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM users").await, 1);
	}

	/// A review whose owner is deleted stays (it says nothing about who), a review is one per id
	#[tokio::test]
	async fn a_deleted_owner_keeps_the_review_and_an_id_is_reviewed_once() {
		let db = migrated().await;
		db.execute_unprepared(
			"INSERT INTO access_warning_reviews (warning_id, dismissed_at, dismissed_by, count_at_dismiss, last_at_dismiss)
			 VALUES ('GlobalBudget:-', CURRENT_TIMESTAMP, 'owner', 1, CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert a review");
		assert!(db
			.execute_unprepared(
				"INSERT INTO access_warning_reviews (warning_id, dismissed_at, count_at_dismiss, last_at_dismiss)
				 VALUES ('GlobalBudget:-', CURRENT_TIMESTAMP, 1, CURRENT_TIMESTAMP)",
			)
			.await
			.is_err());
		db.execute_unprepared("DELETE FROM users WHERE id = 'owner'")
			.await
			.expect("should delete the user");
		assert_eq!(
			count(
				&db,
				"SELECT COUNT(*) AS n FROM access_warning_reviews WHERE dismissed_by IS NULL"
			)
			.await,
			1
		);
	}
}
