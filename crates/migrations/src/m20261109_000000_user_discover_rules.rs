use sea_orm_migration::prelude::*;

/// `user_discover_rules`: each user's rules for the home's "Picks of the week" and "Surprise
/// me" (unread only, tags, genres and libraries to leave out, a minimum rating), as one JSON
/// text per user; a user without a row uses the defaults. Deleted with the user.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(UserDiscoverRules::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(UserDiscoverRules::UserId)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(ColumnDef::new(UserDiscoverRules::Rules).text().not_null())
					.col(
						ColumnDef::new(UserDiscoverRules::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-user_discover_rules-user")
							.from(UserDiscoverRules::Table, UserDiscoverRules::UserId)
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
			.drop_table(Table::drop().table(UserDiscoverRules::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum UserDiscoverRules {
	Table,
	UserId,
	Rules,
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

	const NAME: &str = "m20261109_000000_user_discover_rules";

	/// The table is created with its columns, and rolling back drops it
	#[tokio::test]
	async fn test_user_discover_rules_migration() {
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
		for column in ["user_id", "rules", "updated_at"] {
			assert!(manager
				.has_column("user_discover_rules", column)
				.await
				.unwrap());
		}
		Migrator::down(&db, Some(1))
			.await
			.expect("the rollback should apply");
		assert!(!manager.has_table("user_discover_rules").await.unwrap());
	}
}
