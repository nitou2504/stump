use sea_orm_migration::prelude::*;

/// Adds a drawing style to annotations (highlight, underline, strikethrough or invert, like
/// KOReader's). Existing annotations were all highlights, which is the default
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(MediaAnnotations::Table)
					.add_column(
						ColumnDef::new(MediaAnnotations::Style)
							.string()
							.not_null()
							.default("HIGHLIGHT"),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(MediaAnnotations::Table)
					.drop_column(MediaAnnotations::Style)
					.to_owned(),
			)
			.await
	}
}

#[derive(DeriveIden)]
enum MediaAnnotations {
	Table,
	Style,
}

#[cfg(test)]
mod tests {
	use super::*;
	use sea_orm::{
		ConnectionTrait, Database, DatabaseConnection, FromQueryResult, Statement,
	};

	async fn setup_db() -> DatabaseConnection {
		let db = Database::connect("sqlite::memory:")
			.await
			.expect("should connect to in-memory sqlite");
		db.execute_raw(Statement::from_string(
			db.get_database_backend(),
			// only the columns this migration touches, plus an id
			r"CREATE TABLE media_annotations (
				id TEXT NOT NULL PRIMARY KEY,
				annotation_text TEXT
			)",
		))
		.await
		.expect("should create table");
		db
	}

	async fn insert_annotation(db: &DatabaseConnection, id: &str) {
		db.execute_raw(Statement::from_sql_and_values(
			db.get_database_backend(),
			"INSERT INTO media_annotations (id, annotation_text) VALUES (?, ?)",
			vec![id.into(), "a note".into()],
		))
		.await
		.expect("should insert annotation");
	}

	#[derive(Debug, FromQueryResult)]
	struct StyleRow {
		style: String,
	}

	async fn style_of(db: &DatabaseConnection, id: &str) -> String {
		StyleRow::find_by_statement(Statement::from_sql_and_values(
			db.get_database_backend(),
			"SELECT style FROM media_annotations WHERE id = ?",
			vec![id.into()],
		))
		.one(db)
		.await
		.expect("should query style")
		.expect("annotation should exist")
		.style
	}

	/// existing annotations were drawn as highlights and stay highlights, and new rows that
	/// don't set a style get the same default
	#[tokio::test]
	async fn test_up_makes_existing_and_new_annotations_highlights() {
		let db = setup_db().await;
		insert_annotation(&db, "before").await;

		let manager = SchemaManager::new(&db);
		Migration.up(&manager).await.expect("up should succeed");

		assert_eq!(style_of(&db, "before").await, "HIGHLIGHT");

		insert_annotation(&db, "after").await;
		assert_eq!(style_of(&db, "after").await, "HIGHLIGHT");

		db.execute_raw(Statement::from_sql_and_values(
			db.get_database_backend(),
			"UPDATE media_annotations SET style = ? WHERE id = ?",
			vec!["INVERT".into(), "after".into()],
		))
		.await
		.expect("should update style");
		assert_eq!(style_of(&db, "after").await, "INVERT");
	}

	#[tokio::test]
	async fn test_down_drops_the_style() {
		let db = setup_db().await;
		let manager = SchemaManager::new(&db);
		Migration.up(&manager).await.expect("up should succeed");
		Migration.down(&manager).await.expect("down should succeed");

		assert!(!manager
			.has_column("media_annotations", "style")
			.await
			.expect("should inspect columns"));
	}
}
