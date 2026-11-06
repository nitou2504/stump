use sea_orm_migration::prelude::*;

/// `user_reader_settings`: each user's reading settings (theme, the defaults for every EPUB,
/// PDF or comic, and each book's own settings), as the web reader saves them, so they follow
/// the user to every address, browser and device. One JSON text per user, opaque to the
/// server; deleted with the user.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(UserReaderSettings::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(UserReaderSettings::UserId)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(ColumnDef::new(UserReaderSettings::Settings).text().not_null())
					.col(
						ColumnDef::new(UserReaderSettings::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-user_reader_settings-user")
							.from(UserReaderSettings::Table, UserReaderSettings::UserId)
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
			.drop_table(Table::drop().table(UserReaderSettings::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum UserReaderSettings {
	Table,
	UserId,
	Settings,
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
	use sea_orm::Database;
	use sea_orm_migration::SchemaManager;

	const NAME: &str = "m20261107_000000_user_reader_settings";

	/// The table is created with its columns, and rolling back drops it
	#[tokio::test]
	async fn test_user_reader_settings_migration() {
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
		let manager = SchemaManager::new(&db);
		for column in ["user_id", "settings", "updated_at"] {
			assert!(manager
				.has_column("user_reader_settings", column)
				.await
				.unwrap());
		}
		Migrator::down(&db, Some(1))
			.await
			.expect("the rollback should apply");
		assert!(!manager.has_table("user_reader_settings").await.unwrap());
	}
}
