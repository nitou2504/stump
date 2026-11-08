use sea_orm_migration::prelude::*;

/// Reading lists: named lists of books a user puts together by hand (`reading_lists`), their
/// books in the list's order (`reading_list_items`, a book once per list) and who else may see
/// a list (`reading_list_rules`). The entities and GraphQL API existed without their tables.
/// A list goes with its owner; its entries go with the list or with the book (deleting a book
/// permanently cascades to everything attached to it).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(ReadingLists::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(ReadingLists::Id)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(ColumnDef::new(ReadingLists::Name).text().not_null())
					.col(ColumnDef::new(ReadingLists::Description).text())
					.col(
						ColumnDef::new(ReadingLists::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(ReadingLists::Visibility)
							.text()
							.not_null()
							.default("PRIVATE"),
					)
					.col(
						ColumnDef::new(ReadingLists::Ordering)
							.text()
							.not_null()
							.default("MANUAL"),
					)
					.col(
						ColumnDef::new(ReadingLists::CreatingUserId)
							.text()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-reading_lists-user")
							.from(ReadingLists::Table, ReadingLists::CreatingUserId)
							.to(Users::Table, Users::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("reading_lists_creating_user_id_idx")
					.table(ReadingLists::Table)
					.col(ReadingLists::CreatingUserId)
					.if_not_exists()
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(ReadingListItems::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(ReadingListItems::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(ReadingListItems::DisplayOrder)
							.integer()
							.not_null(),
					)
					.col(ColumnDef::new(ReadingListItems::MediaId).text().not_null())
					.col(
						ColumnDef::new(ReadingListItems::ReadingListId)
							.text()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-reading_list_items-media")
							.from(ReadingListItems::Table, ReadingListItems::MediaId)
							.to(Media::Table, Media::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-reading_list_items-reading_list")
							.from(
								ReadingListItems::Table,
								ReadingListItems::ReadingListId,
							)
							.to(ReadingLists::Table, ReadingLists::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// A book is in a list once; also the lookup of a list's books
		manager
			.create_index(
				Index::create()
					.name("reading_list_items_reading_list_id_media_id_idx")
					.table(ReadingListItems::Table)
					.col(ReadingListItems::ReadingListId)
					.col(ReadingListItems::MediaId)
					.unique()
					.if_not_exists()
					.to_owned(),
			)
			.await?;

		// The lists a book is in
		manager
			.create_index(
				Index::create()
					.name("reading_list_items_media_id_idx")
					.table(ReadingListItems::Table)
					.col(ReadingListItems::MediaId)
					.if_not_exists()
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(ReadingListRules::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(ReadingListRules::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(ReadingListRules::Role).integer().not_null())
					.col(ColumnDef::new(ReadingListRules::UserId).text().not_null())
					.col(
						ColumnDef::new(ReadingListRules::ReadingListId)
							.text()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-reading_list_rules-user")
							.from(ReadingListRules::Table, ReadingListRules::UserId)
							.to(Users::Table, Users::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-reading_list_rules-reading_list")
							.from(
								ReadingListRules::Table,
								ReadingListRules::ReadingListId,
							)
							.to(ReadingLists::Table, ReadingLists::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(ReadingListRules::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(ReadingListItems::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(ReadingLists::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum ReadingLists {
	Table,
	Id,
	Name,
	Description,
	UpdatedAt,
	Visibility,
	Ordering,
	CreatingUserId,
}

#[derive(DeriveIden)]
enum ReadingListItems {
	Table,
	Id,
	DisplayOrder,
	MediaId,
	ReadingListId,
}

#[derive(DeriveIden)]
enum ReadingListRules {
	Table,
	Id,
	Role,
	UserId,
	ReadingListId,
}

#[derive(DeriveIden)]
enum Users {
	Table,
	Id,
}

#[derive(DeriveIden)]
enum Media {
	Table,
	Id,
}

#[cfg(test)]
mod tests {
	use crate::{Migrator, MigratorTrait};
	use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
	use sea_orm_migration::SchemaManager;

	const NAME: &str = "m20261108_000000_reading_lists";

	/// The tables are created; a book is in a list once, and deleting the book or the list
	/// takes its entries along; rolling back drops the tables
	#[tokio::test]
	async fn test_reading_lists_migration() {
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
		for table in ["reading_lists", "reading_list_items", "reading_list_rules"] {
			assert!(
				manager.has_table(table).await.unwrap(),
				"{table} should exist"
			);
		}

		let conn = &db;
		let exec = |sql: &'static str| conn.execute_unprepared(sql);
		let count = |sql: &'static str| async move {
			conn.query_one_raw(Statement::from_string(DbBackend::Sqlite, sql))
				.await
				.unwrap()
				.expect("a count should return a row")
				.try_get_by_index::<i64>(0)
				.unwrap()
		};
		for sql in [
			"INSERT INTO users (id, username, hashed_password, is_server_owner, is_locked, created_at) VALUES ('u1', 'reader', 'x', 0, 0, '2026-11-08 00:00:00')",
			"INSERT INTO library_configs (id, convert_rar_to_zip, hard_delete_conversions, default_reading_dir, default_reading_mode, default_reading_image_scale_fit, generate_file_hashes, generate_koreader_hashes, process_metadata, watch, library_pattern) VALUES (1, 0, 0, 'ltr', 'paged', 'height', 0, 0, 1, 0, 'SERIES_BASED')",
			"INSERT INTO libraries (id, name, path, status, created_at, config_id) VALUES ('l1', 'Library', '/l', 'READY', '2026-11-08 00:00:00', 1)",
			"INSERT INTO media (id, name, size, extension, pages, path, status, created_at, library_id) VALUES ('m1', 'Book', 1, 'epub', 1, '/l/b.epub', 'READY', '2026-11-08 00:00:00', 'l1')",
			"INSERT INTO reading_lists (id, name, updated_at, creating_user_id) VALUES ('r1', 'Favourites', '2026-11-08 00:00:00', 'u1')",
			"INSERT INTO reading_list_items (display_order, media_id, reading_list_id) VALUES (0, 'm1', 'r1')",
		] {
			exec(sql).await.expect("the fixture rows should insert");
		}
		assert_eq!(
			count("SELECT COUNT(*) FROM reading_lists WHERE visibility = 'PRIVATE' AND ordering = 'MANUAL'").await,
			1,
			"a list should be private and in manual order by default"
		);
		assert!(
			exec("INSERT INTO reading_list_items (display_order, media_id, reading_list_id) VALUES (1, 'm1', 'r1')")
				.await
				.is_err(),
			"a book should be in a list once"
		);

		// deleting the book takes its entries along, deleting the user takes their lists
		exec("DELETE FROM media WHERE id = 'm1'").await.unwrap();
		assert_eq!(count("SELECT COUNT(*) FROM reading_list_items").await, 0);
		exec("DELETE FROM users WHERE id = 'u1'").await.unwrap();
		assert_eq!(count("SELECT COUNT(*) FROM reading_lists").await, 0);

		Migrator::down(&db, Some(1))
			.await
			.expect("the rollback should apply");
		for table in ["reading_lists", "reading_list_items", "reading_list_rules"] {
			assert!(
				!manager.has_table(table).await.unwrap(),
				"{table} should be dropped"
			);
		}
	}
}
