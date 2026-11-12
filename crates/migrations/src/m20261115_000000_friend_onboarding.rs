use sea_orm_migration::prelude::*;

/// Friend onboarding: people make their own accounts from a link the owner makes, sign in from
/// the internet on any device (up to a limit), and read from reading apps through a key.
///
/// - `access_signup_links`: the links that make accounts. One use is spent by one conditional
///   update. Only the SHA-256 of the token is stored.
/// - `access_people`: what *Public access* knows about an account beyond the account (may it sign
///   in from the internet, how many devices, are new libraries hidden from it). An account with
///   no row is not allowed in from the internet. The row goes with the account. A satellite
///   table and not columns of `users`: it is not part of Stump's account model, an older image
///   never reads a column it does not know, and rolling back is dropping it.
/// - `access_library_decisions`: each (person, library) pair is decided once, so the rule that
///   hides libraries made later never overwrites the owner's own choices.
/// - `access_devices.kind`: `browser` (every existing row) or `opds` (a reading-app key).
///
/// Three tables and one column, nothing else of Stump's schema moves.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(AccessSignupLinks::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AccessSignupLinks::Id)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(
						ColumnDef::new(AccessSignupLinks::TokenHash)
							.text()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(AccessSignupLinks::Label).text().not_null())
					.col(ColumnDef::new(AccessSignupLinks::Kind).text().not_null())
					.col(
						ColumnDef::new(AccessSignupLinks::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(AccessSignupLinks::CreatedBy)
							.text()
							.not_null(),
					)
					.col(
						ColumnDef::new(AccessSignupLinks::ExpiresAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(AccessSignupLinks::MaxUses)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(AccessSignupLinks::Uses)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(AccessSignupLinks::RevokedAt)
							.timestamp_with_time_zone(),
					)
					.col(ColumnDef::new(AccessSignupLinks::Preset).text().not_null())
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(AccessPeople::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AccessPeople::UserId)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(
						ColumnDef::new(AccessPeople::PublicSignin)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(
						ColumnDef::new(AccessPeople::MaxDevices)
							.integer()
							.not_null()
							.default(3),
					)
					.col(
						ColumnDef::new(AccessPeople::NewLibrariesHidden)
							.boolean()
							.not_null()
							.default(true),
					)
					.col(ColumnDef::new(AccessPeople::SignupLinkId).text())
					.col(
						ColumnDef::new(AccessPeople::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-access_people-user")
							.from(AccessPeople::Table, AccessPeople::UserId)
							.to(Users::Table, Users::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-access_people-signup_link")
							.from(AccessPeople::Table, AccessPeople::SignupLinkId)
							.to(AccessSignupLinks::Table, AccessSignupLinks::Id)
							.on_delete(ForeignKeyAction::SetNull)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;
		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("access_people_link_idx")
					.table(AccessPeople::Table)
					.col(AccessPeople::SignupLinkId)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(AccessLibraryDecisions::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AccessLibraryDecisions::UserId)
							.text()
							.not_null(),
					)
					.col(
						ColumnDef::new(AccessLibraryDecisions::LibraryId)
							.text()
							.not_null(),
					)
					.primary_key(
						Index::create()
							.col(AccessLibraryDecisions::UserId)
							.col(AccessLibraryDecisions::LibraryId),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-access_library_decisions-user")
							.from(
								AccessLibraryDecisions::Table,
								AccessLibraryDecisions::UserId,
							)
							.to(Users::Table, Users::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-access_library_decisions-library")
							.from(
								AccessLibraryDecisions::Table,
								AccessLibraryDecisions::LibraryId,
							)
							.to(Libraries::Table, Libraries::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(AccessDevices::Table)
					.add_column(
						ColumnDef::new(AccessDevices::Kind)
							.text()
							.not_null()
							.default("browser"),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// reading-app keys mean nothing without the column
		manager
			.get_connection()
			.execute_unprepared("DELETE FROM access_devices WHERE kind = 'opds'")
			.await?;
		manager
			.alter_table(
				Table::alter()
					.table(AccessDevices::Table)
					.drop_column(AccessDevices::Kind)
					.to_owned(),
			)
			.await?;
		// the tables that point at others first
		manager
			.drop_table(
				Table::drop()
					.table(AccessLibraryDecisions::Table)
					.if_exists()
					.to_owned(),
			)
			.await?;
		manager
			.drop_table(
				Table::drop()
					.table(AccessPeople::Table)
					.if_exists()
					.to_owned(),
			)
			.await?;
		manager
			.drop_table(
				Table::drop()
					.table(AccessSignupLinks::Table)
					.if_exists()
					.to_owned(),
			)
			.await
	}
}

#[derive(DeriveIden)]
enum AccessSignupLinks {
	Table,
	Id,
	TokenHash,
	Label,
	Kind,
	CreatedAt,
	CreatedBy,
	ExpiresAt,
	MaxUses,
	Uses,
	RevokedAt,
	Preset,
}

#[derive(DeriveIden)]
enum AccessPeople {
	Table,
	UserId,
	PublicSignin,
	MaxDevices,
	NewLibrariesHidden,
	SignupLinkId,
	CreatedAt,
}

#[derive(DeriveIden)]
enum AccessLibraryDecisions {
	Table,
	UserId,
	LibraryId,
}

#[derive(DeriveIden)]
enum AccessDevices {
	Table,
	Kind,
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
	use crate::{Migrator, MigratorTrait};
	use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
	use sea_orm_migration::SchemaManager;

	const NAME: &str = "m20261115_000000_friend_onboarding";

	const TABLES: [&str; 3] = [
		"access_signup_links",
		"access_people",
		"access_library_decisions",
	];

	/// An in-memory database migrated to the step before this migration (with foreign keys on, as
	/// the server has them), with rows an older image would have written
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
			 VALUES ('u1', 'ana', 'x', 0, 0, CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert a user");
		db.execute_unprepared(
			"INSERT INTO access_devices (id, person_id, label, created_at)
			 VALUES ('ana-phone-k3f9x', 'u1', 'phone', CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert a device");
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

	/// The columns of a table, as `name type` text in order: two databases with the same schema
	/// give the same text
	async fn schema_of(db: &DatabaseConnection, table: &str) -> Vec<String> {
		db.query_all_raw(Statement::from_string(
			db.get_database_backend(),
			format!("SELECT name, type FROM pragma_table_info('{table}') ORDER BY cid"),
		))
		.await
		.expect("should read the table")
		.into_iter()
		.map(|row| {
			format!(
				"{} {}",
				row.try_get::<String>("", "name").unwrap(),
				row.try_get::<String>("", "type").unwrap()
			)
		})
		.collect()
	}

	#[tokio::test]
	async fn the_migration_makes_three_tables_and_a_column_and_keeps_old_devices() {
		let db = migrated().await;
		let manager = SchemaManager::new(&db);
		for table in TABLES {
			assert!(manager.has_table(table).await.unwrap(), "{table}");
		}
		for (table, column) in [
			("access_signup_links", "token_hash"),
			("access_signup_links", "preset"),
			("access_signup_links", "max_uses"),
			("access_people", "public_signin"),
			("access_people", "max_devices"),
			("access_people", "new_libraries_hidden"),
			("access_people", "signup_link_id"),
			("access_library_decisions", "library_id"),
			("access_devices", "kind"),
		] {
			assert!(
				manager.has_column(table, column).await.unwrap(),
				"{table}.{column}"
			);
		}
		// every device an older image made is a browser
		assert_eq!(
			count(
				&db,
				"SELECT COUNT(*) AS n FROM access_devices WHERE kind = 'browser'"
			)
			.await,
			1
		);
		// the defaults of a person's row
		db.execute_unprepared(
			"INSERT INTO access_people (user_id, created_at) VALUES ('u1', CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert a person");
		assert_eq!(
			count(
				&db,
				"SELECT COUNT(*) AS n FROM access_people WHERE public_signin = 0 AND max_devices = 3 AND new_libraries_hidden = 1"
			)
			.await,
			1
		);
	}

	#[tokio::test]
	async fn down_restores_the_schema_and_drops_reading_app_keys() {
		let db = before().await;
		let devices_before = schema_of(&db, "access_devices").await;
		Migrator::up(&db, Some(1)).await.expect("up");
		db.execute_unprepared(
			"INSERT INTO access_devices (id, person_id, label, created_at, kind)
			 VALUES ('kobo-9x2mq', 'u1', 'Kobo', CURRENT_TIMESTAMP, 'opds')",
		)
		.await
		.expect("should insert a reading-app key");

		Migrator::down(&db, Some(1)).await.expect("down");
		let manager = SchemaManager::new(&db);
		for table in TABLES {
			assert!(!manager.has_table(table).await.unwrap(), "{table}");
		}
		assert_eq!(schema_of(&db, "access_devices").await, devices_before);
		assert!(!applied_versions(&db).await.contains(&NAME.to_string()));
		// the browser stays, the key goes
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM access_devices").await,
			1
		);
	}

	/// The manual rollback the owner runs for an older image: THIS FILE, run on a database that
	/// has data in the new tables, gives the schema the older image expects
	#[tokio::test]
	async fn the_manual_down_leaves_a_database_the_previous_image_can_use() {
		let before_db = before().await;
		let expected = schema_of(&before_db, "access_devices").await;
		let db = migrated().await;
		db.execute_unprepared(
			"INSERT INTO access_signup_links (id, token_hash, label, kind, created_at, created_by, expires_at, max_uses, preset)
			 VALUES ('l1', 'h1', 'club', 'open', CURRENT_TIMESTAMP, 'owner', CURRENT_TIMESTAMP, 5, '{}')",
		)
		.await
		.expect("should insert a link");
		db.execute_unprepared(
			"INSERT INTO access_people (user_id, public_signin, signup_link_id, created_at) VALUES ('u1', 1, 'l1', CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert a person");
		db.execute_unprepared(
			"INSERT INTO access_devices (id, person_id, label, created_at, kind)
			 VALUES ('kobo-9x2mq', 'u1', 'Kobo', CURRENT_TIMESTAMP, 'opds')",
		)
		.await
		.expect("should insert a key");
		let versions: Vec<String> = applied_versions(&db)
			.await
			.into_iter()
			.filter(|version| version != NAME)
			.collect();

		// the file the owner runs, as the text of this test so the two can't drift apart
		let manual_down =
			include_str!("../manual-down/m20261115_000000_friend_onboarding.sql");
		db.execute_unprepared(manual_down)
			.await
			.expect("the manual down should run");

		let manager = SchemaManager::new(&db);
		for table in TABLES {
			assert!(!manager.has_table(table).await.unwrap(), "{table}");
		}
		assert_eq!(schema_of(&db, "access_devices").await, expected);
		assert_eq!(applied_versions(&db).await, versions);
		// what the older image reads is untouched: the user and the browser pass stay
		assert_eq!(count(&db, "SELECT COUNT(*) AS n FROM users").await, 1);
		assert_eq!(
			count(
				&db,
				"SELECT COUNT(*) AS n FROM access_devices WHERE id = 'ana-phone-k3f9x'"
			)
			.await,
			1
		);
		assert_eq!(
			count(
				&db,
				"SELECT COUNT(*) AS n FROM access_devices WHERE id = 'kobo-9x2mq'"
			)
			.await,
			0
		);
	}

	/// A signup link that goes is forgotten by the person it made (the account stays); a person's
	/// row and decisions go with the account; a decision goes with its library
	#[tokio::test]
	async fn rows_follow_their_foreign_keys() {
		let db = migrated().await;
		for sql in [
			"INSERT INTO library_configs (id, convert_rar_to_zip, hard_delete_conversions, default_reading_dir, default_reading_mode, default_reading_image_scale_fit, generate_file_hashes, generate_koreader_hashes, process_metadata, watch, library_pattern) VALUES (1, 0, 0, 'ltr', 'paged', 'height', 0, 0, 1, 0, 'SERIES_BASED')",
			"INSERT INTO libraries (id, name, path, status, created_at, config_id) VALUES ('lib1', 'Biblioteca', '/b', 'READY', '2026-11-08 00:00:00', 1)",
		] {
			db.execute_unprepared(sql)
				.await
				.expect("the fixture rows should insert");
		}

		db.execute_unprepared(
			"INSERT INTO access_signup_links (id, token_hash, label, kind, created_at, created_by, expires_at, max_uses, preset)
			 VALUES ('l1', 'h1', 'club', 'single', CURRENT_TIMESTAMP, 'owner', CURRENT_TIMESTAMP, 1, '{}')",
		)
		.await
		.expect("should insert a link");
		// two links cannot share a token
		assert!(
			db.execute_unprepared(
				"INSERT INTO access_signup_links (id, token_hash, label, kind, created_at, created_by, expires_at, max_uses, preset)
				 VALUES ('l2', 'h1', 'x', 'single', CURRENT_TIMESTAMP, 'owner', CURRENT_TIMESTAMP, 1, '{}')",
			)
			.await
			.is_err()
		);
		db.execute_unprepared(
			"INSERT INTO access_people (user_id, public_signin, signup_link_id, created_at) VALUES ('u1', 1, 'l1', CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert a person");
		db.execute_unprepared(
			"INSERT INTO access_library_decisions (user_id, library_id) VALUES ('u1', 'lib1')",
		)
		.await
		.expect("should insert a decision");
		// one decision per pair
		assert!(
			db.execute_unprepared(
				"INSERT INTO access_library_decisions (user_id, library_id) VALUES ('u1', 'lib1')"
			)
			.await
			.is_err()
		);

		// the link is deleted: the person stays, without the link
		db.execute_unprepared("DELETE FROM access_signup_links WHERE id = 'l1'")
			.await
			.expect("should delete the link");
		assert_eq!(
			count(
				&db,
				"SELECT COUNT(*) AS n FROM access_people WHERE signup_link_id IS NULL"
			)
			.await,
			1
		);

		// the library is deleted: its decision goes
		db.execute_unprepared("DELETE FROM libraries WHERE id = 'lib1'")
			.await
			.expect("should delete the library");
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM access_library_decisions").await,
			0
		);

		// the user is deleted: the person's row goes (the device row stays: it has no foreign key)
		db.execute_unprepared("DELETE FROM users WHERE id = 'u1'")
			.await
			.expect("should delete the user");
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM access_people").await,
			0
		);
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM access_devices").await,
			1
		);
	}
}
