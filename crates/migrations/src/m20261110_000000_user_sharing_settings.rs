use sea_orm_migration::prelude::*;

/// `user_sharing_settings`: what a user lets the other people on the server see of them. For now
/// one switch, `share_reading_history` (the *My reading* page: what they have finished and are
/// reading now). A user without a row shares nothing, and the row goes with the user.
///
/// It is a table of its own and not a column of `user_preferences`: the web app sends every
/// preference back whenever it changes one, so a stale tab would silently turn the sharing back
/// on.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(UserSharingSettings::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(UserSharingSettings::UserId)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(
						ColumnDef::new(UserSharingSettings::ShareReadingHistory)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(
						ColumnDef::new(UserSharingSettings::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-user_sharing_settings-user")
							.from(UserSharingSettings::Table, UserSharingSettings::UserId)
							.to(Users::Table, Users::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(UserSharingSettings::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum UserSharingSettings {
	Table,
	UserId,
	ShareReadingHistory,
	UpdatedAt,
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

	const NAME: &str = "m20261110_000000_user_sharing_settings";

	/// An in-memory database migrated up to and including this migration
	async fn migrated() -> DatabaseConnection {
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
		Migrator::up(&db, Some(1))
			.await
			.expect("the migration should apply");
		db
	}

	async fn applied_versions(db: &DatabaseConnection) -> Vec<String> {
		db.query_all_raw(Statement::from_string(
			db.get_database_backend(),
			"SELECT version FROM seaql_migrations ORDER BY version".to_string(),
		))
		.await
		.expect("should read the applied migrations")
		.iter()
		.map(|row| row.try_get::<String>("", "version").expect("a version"))
		.collect()
	}

	/// The table is created with its columns, and rolling back drops it
	#[tokio::test]
	async fn test_user_sharing_settings_migration() {
		let db = migrated().await;
		let manager = SchemaManager::new(&db);
		for column in ["user_id", "share_reading_history", "updated_at"] {
			assert!(manager
				.has_column("user_sharing_settings", column)
				.await
				.unwrap());
		}
		Migrator::down(&db, Some(1))
			.await
			.expect("the rollback should apply");
		assert!(!manager.has_table("user_sharing_settings").await.unwrap());
		assert!(!applied_versions(&db).await.contains(&NAME.to_string()));
	}

	/// The manual rollback of the plan (Appendix B), for an older image started on a database that
	/// has this migration: it leaves the other migrations and every other table as they were
	#[tokio::test]
	async fn the_manual_down_leaves_a_database_the_previous_image_can_use() {
		let db = migrated().await;
		let before: Vec<String> = applied_versions(&db)
			.await
			.into_iter()
			.filter(|version| version != NAME)
			.collect();

		db.execute_unprepared(
			"BEGIN; DROP TABLE IF EXISTS user_sharing_settings;
			 DELETE FROM seaql_migrations WHERE version = 'm20261110_000000_user_sharing_settings'; COMMIT;",
		)
		.await
		.expect("the manual down should run");

		let manager = SchemaManager::new(&db);
		assert!(!manager.has_table("user_sharing_settings").await.unwrap());
		assert_eq!(applied_versions(&db).await, before);
		// the users table (what the new table pointed at) is untouched
		assert!(manager.has_table("users").await.unwrap());
	}

	/// A row goes with its user, and a user without one is simply not sharing
	#[tokio::test]
	async fn a_row_is_deleted_with_its_user() {
		let db = migrated().await;
		db.execute_unprepared("PRAGMA foreign_keys = ON")
			.await
			.expect("should turn on foreign keys");
		db.execute_unprepared(
			"INSERT INTO users (id, username, hashed_password, is_server_owner, created_at, is_locked)
			 VALUES ('u1', 'ana', 'x', 0, CURRENT_TIMESTAMP, 0)",
		)
		.await
		.expect("should insert a user");
		db.execute_unprepared(
			"INSERT INTO user_sharing_settings (user_id, share_reading_history) VALUES ('u1', 1)",
		)
		.await
		.expect("should insert the setting");

		db.execute_unprepared("DELETE FROM users WHERE id = 'u1'")
			.await
			.expect("should delete the user");

		let rows = db
			.query_all_raw(Statement::from_string(
				db.get_database_backend(),
				"SELECT user_id FROM user_sharing_settings".to_string(),
			))
			.await
			.expect("should read the settings");
		assert!(rows.is_empty());
	}
}
