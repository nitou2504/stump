use sea_orm_migration::prelude::*;

/// `server_config.fetch_metadata_after_scan`: whether a scan asks the metadata providers
/// (Hardcover...) about the books it added, and the scheduled retries ask again. On by
/// default, as before; turning it off keeps the providers' daily quota for people's lookups
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(ServerConfig::Table)
					.add_column(
						ColumnDef::new(ServerConfig::FetchMetadataAfterScan)
							.boolean()
							.not_null()
							.default(true),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(ServerConfig::Table)
					.drop_column(ServerConfig::FetchMetadataAfterScan)
					.to_owned(),
			)
			.await
	}
}

#[derive(DeriveIden)]
enum ServerConfig {
	Table,
	FetchMetadataAfterScan,
}

#[cfg(test)]
mod tests {
	use crate::{Migrator, MigratorTrait};
	use sea_orm::{ConnectionTrait, Database, FromQueryResult, Statement};
	use sea_orm_migration::SchemaManager;

	#[derive(Debug, FromQueryResult)]
	struct Setting {
		fetch_metadata_after_scan: bool,
	}

	const NAME: &str = "m20261106_000000_metadata_fetch_after_scan";

	/// An existing server keeps fetching after scans (the column defaults to on), and rolling
	/// back drops the column
	#[tokio::test]
	async fn test_metadata_fetch_after_scan_migration() {
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
		db.execute_raw(Statement::from_string(
			db.get_database_backend(),
			"INSERT INTO server_config (initial_wal_setup_complete) VALUES (1)",
		))
		.await
		.expect("a server config row");

		Migrator::up(&db, Some(1))
			.await
			.expect("the migration should apply");
		let setting = Setting::find_by_statement(Statement::from_string(
			db.get_database_backend(),
			"SELECT fetch_metadata_after_scan FROM server_config",
		))
		.one(&db)
		.await
		.unwrap()
		.expect("the server config row");
		assert!(setting.fetch_metadata_after_scan);

		Migrator::down(&db, Some(1))
			.await
			.expect("the rollback should apply");
		assert!(!SchemaManager::new(&db)
			.has_column("server_config", "fetch_metadata_after_scan")
			.await
			.unwrap());
	}
}
