use sea_orm_migration::prelude::*;

/// The "Add books" intake: per-user inbox items (staged uploads with their proposed and
/// reviewed metadata) and per-library intake settings.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(IntakeItems::Table)
					.if_not_exists()
					.col(ColumnDef::new(IntakeItems::Id).text().not_null().primary_key())
					.col(ColumnDef::new(IntakeItems::UserId).text().not_null())
					.col(ColumnDef::new(IntakeItems::LibraryId).text().not_null())
					.col(ColumnDef::new(IntakeItems::State).text().not_null())
					.col(ColumnDef::new(IntakeItems::Kind).text().not_null())
					.col(ColumnDef::new(IntakeItems::Name).text().not_null())
					.col(ColumnDef::new(IntakeItems::StagingDir).text().not_null())
					.col(ColumnDef::new(IntakeItems::Files).json().not_null())
					.col(ColumnDef::new(IntakeItems::Proposal).json())
					.col(ColumnDef::new(IntakeItems::Decision).json())
					.col(ColumnDef::new(IntakeItems::Destination).json())
					.col(ColumnDef::new(IntakeItems::Result).json())
					.col(ColumnDef::new(IntakeItems::Error).text())
					.col(
						ColumnDef::new(IntakeItems::CreatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(IntakeItems::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(ColumnDef::new(IntakeItems::FiledAt).timestamp_with_time_zone())
					.foreign_key(
						ForeignKey::create()
							.name("fk-intake_items-user")
							.from(IntakeItems::Table, IntakeItems::UserId)
							.to(Users::Table, Users::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-intake_items-library")
							.from(IntakeItems::Table, IntakeItems::LibraryId)
							.to(Libraries::Table, Libraries::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("intake_items_user_state_idx")
					.table(IntakeItems::Table)
					.col(IntakeItems::UserId)
					.col(IntakeItems::State)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("intake_items_filed_at_idx")
					.table(IntakeItems::Table)
					.col(IntakeItems::FiledAt)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(LibraryIntakeSettings::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(LibraryIntakeSettings::LibraryId)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(
						ColumnDef::new(LibraryIntakeSettings::MangaDir)
							.text()
							.not_null()
							.default("_Manga"),
					)
					.col(
						ColumnDef::new(LibraryIntakeSettings::LightNovelDir)
							.text()
							.not_null()
							.default("_Novela ligera"),
					)
					.col(ColumnDef::new(LibraryIntakeSettings::CsvPath).text())
					.col(
						ColumnDef::new(LibraryIntakeSettings::OnlineLookup)
							.boolean()
							.not_null()
							.default(true),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-library_intake_settings-library")
							.from(LibraryIntakeSettings::Table, LibraryIntakeSettings::LibraryId)
							.to(Libraries::Table, Libraries::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(LibraryIntakeSettings::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(IntakeItems::Table).to_owned())
			.await?;
		// ADD_BOOKS grants are left in users.permissions: unknown permissions are ignored
		Ok(())
	}
}

#[derive(DeriveIden)]
enum IntakeItems {
	Table,
	Id,
	UserId,
	LibraryId,
	State,
	Kind,
	Name,
	StagingDir,
	Files,
	Proposal,
	Decision,
	Destination,
	Result,
	Error,
	CreatedAt,
	UpdatedAt,
	FiledAt,
}

#[derive(DeriveIden)]
enum LibraryIntakeSettings {
	Table,
	LibraryId,
	MangaDir,
	LightNovelDir,
	CsvPath,
	OnlineLookup,
}

#[derive(DeriveIden)]
enum Users {
	Table,
	Id,
}

#[derive(DeriveIden)]
enum Libraries {
	Table,
	Id,
}

#[cfg(test)]
mod tests {
	use super::*;
	use sea_orm::{ConnectionTrait, Database, Statement};

	#[tokio::test]
	async fn creates_the_intake_tables_with_their_defaults() {
		let db = Database::connect("sqlite::memory:")
			.await
			.expect("should connect to in-memory sqlite");
		let backend = db.get_database_backend();
		for stmt in [
			"CREATE TABLE users (id TEXT NOT NULL PRIMARY KEY)",
			"CREATE TABLE libraries (id TEXT NOT NULL PRIMARY KEY)",
			"INSERT INTO libraries (id) VALUES ('lib')",
		] {
			db.execute_raw(Statement::from_string(backend, stmt))
				.await
				.expect("should prepare");
		}

		let manager = SchemaManager::new(&db);
		Migration.up(&manager).await.expect("migration should run");
		assert!(manager.has_table("intake_items").await.unwrap());
		assert!(manager.has_table("library_intake_settings").await.unwrap());

		db.execute_raw(Statement::from_string(
			backend,
			"INSERT INTO library_intake_settings (library_id) VALUES ('lib')",
		))
		.await
		.expect("defaults should fill the other columns");
		let row = db
			.query_one_raw(Statement::from_string(
				backend,
				"SELECT manga_dir, light_novel_dir, online_lookup FROM library_intake_settings",
			))
			.await
			.unwrap()
			.unwrap();
		assert_eq!(row.try_get::<String>("", "manga_dir").unwrap(), "_Manga");
		assert_eq!(
			row.try_get::<String>("", "light_novel_dir").unwrap(),
			"_Novela ligera"
		);
		assert!(row.try_get::<bool>("", "online_lookup").unwrap());

		Migration.down(&manager).await.expect("down should run");
		assert!(!manager.has_table("intake_items").await.unwrap());
	}
}
