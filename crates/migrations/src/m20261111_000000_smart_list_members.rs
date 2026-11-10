use models::catalog_revision as schema;
use sea_orm::{DbBackend, Statement, TransactionTrait};
use sea_orm_migration::prelude::*;

/// `smart_list_members`: what the smart lists hold, kept instead of worked out on every visit.
///
/// Three tables (the members of each list, the revision of the catalog each list was worked out at,
/// and the catalog's revision) and the `catalog_rev_*` triggers that bump the revision whenever
/// something a rule can look at changes. See [`models::catalog_revision`]. Nothing is copied: a
/// list is worked out the first time it is read, and by the server's warmer after start-up.
///
/// SQLite only. On another database this does nothing and every list is worked out whenever it is
/// read, as before.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let conn = manager.get_connection();
		if conn.get_database_backend() != DbBackend::Sqlite {
			return Ok(());
		}

		let txn = conn.begin().await?;
		for sql in schema::create_statements()
			.into_iter()
			.chain(schema::trigger_statements())
		{
			txn.execute_unprepared(&sql).await?;
		}
		txn.commit().await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let conn = manager.get_connection();
		if conn.get_database_backend() != DbBackend::Sqlite {
			return Ok(());
		}

		let txn = conn.begin().await?;
		// by what is there, not by what this version would create
		let triggers = txn
			.query_all_raw(Statement::from_string(
				DbBackend::Sqlite,
				format!(
					"SELECT name FROM sqlite_master WHERE type = 'trigger' AND name LIKE '{}%'",
					schema::TRIGGER_PREFIX
				),
			))
			.await?;
		for row in triggers {
			let name: String = row.try_get("", "name")?;
			txn.execute_unprepared(&format!("DROP TRIGGER IF EXISTS {name}"))
				.await?;
		}
		for table in [
			schema::MEMBERS_TABLE,
			schema::MATERIALIZATIONS_TABLE,
			schema::REVISION_TABLE,
		] {
			txn.execute_unprepared(&format!("DROP TABLE IF EXISTS {table}"))
				.await?;
		}
		txn.commit().await
	}
}

#[cfg(test)]
mod tests {
	use crate::{Migrator, MigratorTrait};
	use models::catalog_revision as schema;
	use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
	use sea_orm_migration::SchemaManager;

	const NAME: &str = "m20261111_000000_smart_list_members";

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
		strings(
			db,
			"SELECT version AS v FROM seaql_migrations ORDER BY version",
		)
		.await
	}

	async fn strings(db: &DatabaseConnection, sql: &str) -> Vec<String> {
		db.query_all_raw(Statement::from_string(
			db.get_database_backend(),
			sql.to_string(),
		))
		.await
		.expect("should run the query")
		.iter()
		.map(|row| row.try_get::<String>("", "v").expect("a string"))
		.collect()
	}

	async fn cache_triggers(db: &DatabaseConnection) -> Vec<String> {
		strings(
			db,
			"SELECT name AS v FROM sqlite_master WHERE type = 'trigger' AND name LIKE 'catalog_rev_%' ORDER BY name",
		)
		.await
	}

	/// The three tables with their columns, the revision's one row, and every trigger; and rolling
	/// back takes all of it away and leaves the other triggers (the library ones) alone
	#[tokio::test]
	async fn test_smart_list_members_migration() {
		let db = migrated().await;
		let manager = SchemaManager::new(&db);
		for (table, columns) in [
			(schema::MEMBERS_TABLE, vec!["smart_list_id", "media_id"]),
			(
				schema::MATERIALIZATIONS_TABLE,
				vec!["smart_list_id", "revision", "member_count", "computed_at"],
			),
			(schema::REVISION_TABLE, vec!["id", "revision"]),
		] {
			assert!(manager.has_table(table).await.unwrap(), "{table}");
			for column in columns {
				assert!(
					manager.has_column(table, column).await.unwrap(),
					"{table}.{column}"
				);
			}
		}
		assert_eq!(
			strings(
				&db,
				"SELECT revision || '' AS v FROM catalog_revision WHERE id = 1"
			)
			.await,
			vec!["0".to_string()]
		);

		let mut expected = schema::trigger_names();
		expected.sort();
		assert_eq!(cache_triggers(&db).await, expected);
		let library_triggers = strings(
			&db,
			"SELECT name AS v FROM sqlite_master WHERE type = 'trigger' AND name NOT LIKE 'catalog_rev_%' ORDER BY name",
		)
		.await;
		assert_eq!(library_triggers.len(), 3, "{library_triggers:?}");

		Migrator::down(&db, Some(1))
			.await
			.expect("the rollback should apply");
		for table in [
			schema::MEMBERS_TABLE,
			schema::MATERIALIZATIONS_TABLE,
			schema::REVISION_TABLE,
		] {
			assert!(!manager.has_table(table).await.unwrap(), "{table}");
		}
		assert!(cache_triggers(&db).await.is_empty());
		assert_eq!(
			strings(
				&db,
				"SELECT name AS v FROM sqlite_master WHERE type = 'trigger' ORDER BY name"
			)
			.await,
			library_triggers
		);
		assert!(!applied_versions(&db).await.contains(&NAME.to_string()));
	}

	/// Every column the triggers watch is a column of the real schema (a trigger on a column that
	/// isn't there would never fire)
	#[tokio::test]
	async fn every_watched_column_exists() {
		let db = migrated().await;
		let manager = SchemaManager::new(&db);
		for watched in schema::WATCHED {
			assert!(
				manager.has_table(watched.table).await.unwrap(),
				"{}",
				watched.table
			);
			for column in watched.columns {
				assert!(
					manager.has_column(watched.table, column).await.unwrap(),
					"{}.{column} is watched but is not a column",
					watched.table
				);
			}
		}
	}

	/// The manual rollback of the plan (Appendix B), for an older image started on a database that
	/// has this migration: the triggers are found by name and dropped, and nothing else changes
	#[tokio::test]
	async fn the_manual_down_leaves_a_database_the_previous_image_can_use() {
		let db = migrated().await;
		let before: Vec<String> = applied_versions(&db)
			.await
			.into_iter()
			.filter(|version| version != NAME)
			.collect();

		let drops = strings(
			&db,
			"SELECT 'DROP TRIGGER ' || name || ';' AS v FROM sqlite_master WHERE type = 'trigger' AND name LIKE 'catalog_rev_%'",
		)
		.await;
		assert_eq!(drops.len(), schema::trigger_names().len());
		db.execute_unprepared(&format!(
			"BEGIN; {}
			 DROP TABLE IF EXISTS smart_list_members; DROP TABLE IF EXISTS smart_list_materializations; DROP TABLE IF EXISTS catalog_revision;
			 DELETE FROM seaql_migrations WHERE version = 'm20261111_000000_smart_list_members'; COMMIT;",
			drops.join(" ")
		))
		.await
		.expect("the manual down should run");

		let manager = SchemaManager::new(&db);
		for table in [
			schema::MEMBERS_TABLE,
			schema::MATERIALIZATIONS_TABLE,
			schema::REVISION_TABLE,
		] {
			assert!(!manager.has_table(table).await.unwrap(), "{table}");
		}
		assert!(cache_triggers(&db).await.is_empty());
		assert_eq!(applied_versions(&db).await, before);
		// the tables the cache pointed at are untouched
		for table in ["smart_lists", "media", "libraries"] {
			assert!(manager.has_table(table).await.unwrap(), "{table}");
		}
	}

	/// The migration can be run again after a rollback
	#[tokio::test]
	async fn the_migration_applies_again_after_a_rollback() {
		let db = migrated().await;
		Migrator::down(&db, Some(1))
			.await
			.expect("the rollback should apply");
		Migrator::up(&db, Some(1))
			.await
			.expect("the migration should apply again");
		let mut expected = schema::trigger_names();
		expected.sort();
		assert_eq!(cache_triggers(&db).await, expected);
	}
}
