use sea_orm::Statement;
use sea_orm_migration::prelude::*;

/// Book removal becomes a user permission (`REMOVE_BOOK`) that an admin grants per user,
/// instead of a preference users could switch on themselves. Users who had switched the
/// preference on and could manage libraries (both were needed to remove books) get the
/// permission, then the preference column is dropped.
#[derive(DeriveMigrationName)]
pub struct Migration;

/// `permissions` is a comma-separated list, e.g. `MANAGE_LIBRARY,EDIT_LIBRARY`
const HAS_MANAGE_LIBRARY: &str = r"((',' || COALESCE(permissions, '') || ',') LIKE '%,MANAGE_LIBRARY,%'
	OR (',' || COALESCE(permissions, '') || ',') LIKE '%,DELETE_LIBRARY,%')";
const HAS_REMOVE_BOOK: &str =
	r"(',' || COALESCE(permissions, '') || ',') LIKE '%,REMOVE_BOOK,%'";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let conn = manager.get_connection();
		let backend = conn.get_database_backend();

		conn.execute_raw(Statement::from_string(
			backend,
			format!(
				r"
				UPDATE users
				SET permissions = CASE
					WHEN permissions IS NULL OR permissions = '' THEN 'REMOVE_BOOK'
					ELSE permissions || ',REMOVE_BOOK'
				END
				WHERE is_server_owner = FALSE
					AND (
						id IN (SELECT user_id FROM user_preferences WHERE enable_book_removal = TRUE)
						OR user_preferences_id IN (
							SELECT id FROM user_preferences WHERE enable_book_removal = TRUE
						)
					)
					AND {HAS_MANAGE_LIBRARY}
					AND NOT ({HAS_REMOVE_BOOK})
				"
			),
		))
		.await?;

		manager
			.alter_table(
				Table::alter()
					.table(UserPreferences::Table)
					.drop_column(UserPreferences::EnableBookRemoval)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(UserPreferences::Table)
					.add_column(
						ColumnDef::new(UserPreferences::EnableBookRemoval)
							.boolean()
							.not_null()
							.default(false),
					)
					.to_owned(),
			)
			.await?;

		let conn = manager.get_connection();
		let backend = conn.get_database_backend();

		conn.execute_raw(Statement::from_string(
			backend,
			format!(
				r"
				UPDATE user_preferences
				SET enable_book_removal = TRUE
				WHERE user_id IN (SELECT id FROM users WHERE {HAS_REMOVE_BOOK})
				"
			),
		))
		.await?;

		// REMOVE_BOOK is not a substring of any other permission
		conn.execute_raw(Statement::from_string(
			backend,
			r"
			UPDATE users
			SET permissions = REPLACE(REPLACE(REPLACE(
				permissions, ',REMOVE_BOOK', ''), 'REMOVE_BOOK,', ''), 'REMOVE_BOOK', '')
			WHERE permissions LIKE '%REMOVE_BOOK%'
			",
		))
		.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum UserPreferences {
	Table,
	EnableBookRemoval,
}

#[cfg(test)]
mod tests {
	use super::*;
	use sea_orm::{ConnectionTrait, Database, DatabaseConnection, FromQueryResult};

	async fn setup_db() -> DatabaseConnection {
		let db = Database::connect("sqlite::memory:")
			.await
			.expect("should connect to in-memory sqlite");
		let backend = db.get_database_backend();

		// only the columns this migration reads or writes
		let stmts = [
			r"CREATE TABLE user_preferences (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				user_id TEXT,
				enable_book_removal BOOLEAN NOT NULL DEFAULT FALSE
			)",
			r"CREATE TABLE users (
				id TEXT NOT NULL PRIMARY KEY,
				is_server_owner BOOLEAN NOT NULL,
				permissions TEXT,
				user_preferences_id INTEGER
			)",
		];
		for stmt in stmts {
			db.execute_raw(Statement::from_string(backend, stmt))
				.await
				.expect("should create table");
		}

		db
	}

	/// a user with their preferences row; `None` permissions are stored as NULL
	async fn insert_user(
		db: &DatabaseConnection,
		id: &str,
		is_server_owner: bool,
		permissions: Option<&str>,
		enable_book_removal: bool,
	) {
		let backend = db.get_database_backend();
		db.execute_raw(Statement::from_sql_and_values(
			backend,
			"INSERT INTO user_preferences (user_id, enable_book_removal) VALUES (?, ?)",
			vec![id.into(), enable_book_removal.into()],
		))
		.await
		.expect("should insert preferences");
		db.execute_raw(Statement::from_sql_and_values(
			backend,
			r"INSERT INTO users (id, is_server_owner, permissions, user_preferences_id)
			VALUES (?, ?, ?, (SELECT id FROM user_preferences WHERE user_id = ?))",
			vec![
				id.into(),
				is_server_owner.into(),
				permissions.map(String::from).into(),
				id.into(),
			],
		))
		.await
		.expect("should insert user");
	}

	#[derive(Debug, FromQueryResult)]
	struct PermissionsRow {
		permissions: Option<String>,
	}

	async fn permissions_of(db: &DatabaseConnection, id: &str) -> Option<String> {
		PermissionsRow::find_by_statement(Statement::from_sql_and_values(
			db.get_database_backend(),
			"SELECT permissions FROM users WHERE id = ?",
			vec![id.into()],
		))
		.one(db)
		.await
		.expect("query should succeed")
		.expect("user should exist")
		.permissions
	}

	#[derive(Debug, FromQueryResult)]
	struct PreferenceRow {
		enable_book_removal: bool,
	}

	async fn preference_of(db: &DatabaseConnection, id: &str) -> bool {
		PreferenceRow::find_by_statement(Statement::from_sql_and_values(
			db.get_database_backend(),
			"SELECT enable_book_removal FROM user_preferences WHERE user_id = ?",
			vec![id.into()],
		))
		.one(db)
		.await
		.expect("query should succeed")
		.expect("preferences should exist")
		.enable_book_removal
	}

	#[tokio::test]
	async fn test_up_grants_permission_to_users_who_could_remove_books() {
		let db = setup_db().await;
		insert_user(
			&db,
			"manager",
			false,
			Some("MANAGE_LIBRARY,EDIT_LIBRARY"),
			true,
		)
		.await;
		insert_user(&db, "deleter", false, Some("DELETE_LIBRARY"), true).await;
		insert_user(&db, "opted-out", false, Some("MANAGE_LIBRARY"), false).await;
		insert_user(&db, "reader", false, Some("DOWNLOAD_FILE"), true).await;
		insert_user(&db, "no-permissions", false, None, true).await;
		insert_user(&db, "owner", true, Some(""), true).await;
		insert_user(
			&db,
			"granted",
			false,
			Some("MANAGE_LIBRARY,REMOVE_BOOK"),
			true,
		)
		.await;

		let manager = SchemaManager::new(&db);
		Migration.up(&manager).await.expect("up should succeed");

		assert_eq!(
			permissions_of(&db, "manager").await.as_deref(),
			Some("MANAGE_LIBRARY,EDIT_LIBRARY,REMOVE_BOOK")
		);
		assert_eq!(
			permissions_of(&db, "deleter").await.as_deref(),
			Some("DELETE_LIBRARY,REMOVE_BOOK")
		);
		// the preference alone, or library management alone, never allowed removing books
		assert_eq!(
			permissions_of(&db, "opted-out").await.as_deref(),
			Some("MANAGE_LIBRARY")
		);
		assert_eq!(
			permissions_of(&db, "reader").await.as_deref(),
			Some("DOWNLOAD_FILE")
		);
		assert_eq!(permissions_of(&db, "no-permissions").await, None);
		// the server owner has every permission already
		assert_eq!(permissions_of(&db, "owner").await.as_deref(), Some(""));
		assert_eq!(
			permissions_of(&db, "granted").await.as_deref(),
			Some("MANAGE_LIBRARY,REMOVE_BOOK")
		);

		assert!(
			!manager
				.has_column("user_preferences", "enable_book_removal")
				.await
				.expect("should inspect the table"),
			"the preference column should be dropped"
		);
	}

	#[tokio::test]
	async fn test_down_restores_the_preference() {
		let db = setup_db().await;
		insert_user(&db, "manager", false, Some("MANAGE_LIBRARY"), true).await;
		insert_user(&db, "opted-out", false, Some("MANAGE_LIBRARY"), false).await;

		let manager = SchemaManager::new(&db);
		Migration.up(&manager).await.expect("up should succeed");
		// granted by an admin after the upgrade, as the only permission
		db.execute_raw(Statement::from_string(
			db.get_database_backend(),
			"UPDATE users SET permissions = 'REMOVE_BOOK' WHERE id = 'opted-out'",
		))
		.await
		.expect("should grant the permission");

		Migration.down(&manager).await.expect("down should succeed");

		assert!(preference_of(&db, "manager").await);
		assert!(preference_of(&db, "opted-out").await);
		assert_eq!(
			permissions_of(&db, "manager").await.as_deref(),
			Some("MANAGE_LIBRARY")
		);
		assert_eq!(permissions_of(&db, "opted-out").await.as_deref(), Some(""));
	}
}
