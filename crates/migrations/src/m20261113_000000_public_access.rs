use sea_orm_migration::prelude::*;

/// Public access: what the owner manages from *Settings → Public access* instead of a terminal.
///
/// - `access_invites`: invite links for one person each. Only the SHA-256 of the token is stored,
///   so a read of the database never yields a working link.
/// - `access_devices`: the browsers that hold a pass (who they belong to, when and from where they
///   were last seen, whether they were revoked). `person_id` has no foreign key on purpose: a
///   cascade would delete the row of a hard-deleted user, and the next request with that pass
///   (still valid for months) would register it again as an active device.
/// - `access_audit`: what the owner did, and each redemption of an invite. Never a token, a link
///   or a password.
/// - `access_signin_marks`: what Stump knows about one row of `user_login_activity` beyond what
///   the row says: which pass the sign-in came with, why it failed, and whether the lock rule
///   counts it. A table of its own and not columns of `user_login_activity`, so the device ids
///   never reach the people who can read their own login activity, and the table upstream's
///   `failure_reason` TODO would change is left alone.
/// - `access_password_resets`: a person whose password the owner set, who has to choose their own
///   before they can use anything else.
///
/// Every table is new: rolling back is dropping them.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(AccessInvites::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AccessInvites::Id)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(
						ColumnDef::new(AccessInvites::TokenHash)
							.text()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(AccessInvites::PersonId).text().not_null())
					.col(ColumnDef::new(AccessInvites::Label).text().not_null())
					.col(
						ColumnDef::new(AccessInvites::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(AccessInvites::CreatedBy).text().not_null())
					.col(
						ColumnDef::new(AccessInvites::ExpiresAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(AccessInvites::MaxUses)
							.integer()
							.not_null()
							.default(3),
					)
					.col(
						ColumnDef::new(AccessInvites::Uses)
							.integer()
							.not_null()
							.default(0),
					)
					.col(ColumnDef::new(AccessInvites::RevokedAt).timestamp_with_time_zone())
					.foreign_key(
						ForeignKey::create()
							.name("fk-access_invites-person")
							.from(AccessInvites::Table, AccessInvites::PersonId)
							.to(Users::Table, Users::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(AccessDevices::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AccessDevices::Id)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(ColumnDef::new(AccessDevices::InviteId).text())
					.col(ColumnDef::new(AccessDevices::PersonId).text())
					.col(ColumnDef::new(AccessDevices::Label).text().not_null())
					.col(
						ColumnDef::new(AccessDevices::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(AccessDevices::FirstSeenAt).timestamp_with_time_zone())
					.col(ColumnDef::new(AccessDevices::LastSeenAt).timestamp_with_time_zone())
					.col(
						ColumnDef::new(AccessDevices::LastIp)
							.text()
							.not_null()
							.default(""),
					)
					.col(
						ColumnDef::new(AccessDevices::LastAgent)
							.text()
							.not_null()
							.default(""),
					)
					.col(ColumnDef::new(AccessDevices::RevokedAt).timestamp_with_time_zone())
					.col(ColumnDef::new(AccessDevices::RevokedBy).text())
					.col(
						ColumnDef::new(AccessDevices::BudgetResetAt)
							.big_integer()
							.not_null()
							.default(0),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-access_devices-invite")
							.from(AccessDevices::Table, AccessDevices::InviteId)
							.to(AccessInvites::Table, AccessInvites::Id)
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
					.name("access_devices_person_idx")
					.table(AccessDevices::Table)
					.col(AccessDevices::PersonId)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(AccessAudit::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AccessAudit::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(AccessAudit::At)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(AccessAudit::Actor).text().not_null())
					.col(ColumnDef::new(AccessAudit::Action).text().not_null())
					.col(
						ColumnDef::new(AccessAudit::Target)
							.text()
							.not_null()
							.default(""),
					)
					.col(
						ColumnDef::new(AccessAudit::Detail)
							.text()
							.not_null()
							.default(""),
					)
					.to_owned(),
			)
			.await?;
		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("access_audit_at_idx")
					.table(AccessAudit::Table)
					.col(AccessAudit::At)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(AccessSigninMarks::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AccessSigninMarks::LoginActivityId)
							.big_integer()
							.not_null()
							.primary_key(),
					)
					.col(ColumnDef::new(AccessSigninMarks::DeviceId).text())
					.col(ColumnDef::new(AccessSigninMarks::FailureReason).text())
					.col(
						ColumnDef::new(AccessSigninMarks::CountsTowardLock)
							.boolean()
							.not_null()
							.default(true),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk-access_signin_marks-login_activity")
							.from(
								AccessSigninMarks::Table,
								AccessSigninMarks::LoginActivityId,
							)
							.to(UserLoginActivity::Table, UserLoginActivity::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(AccessPasswordResets::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AccessPasswordResets::UserId)
							.text()
							.not_null()
							.primary_key(),
					)
					.col(
						ColumnDef::new(AccessPasswordResets::SetAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(AccessPasswordResets::SetBy).text().not_null())
					.foreign_key(
						ForeignKey::create()
							.name("fk-access_password_resets-user")
							.from(AccessPasswordResets::Table, AccessPasswordResets::UserId)
							.to(Users::Table, Users::Id)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// the tables that point at others first
		manager
			.drop_table(
				Table::drop()
					.table(AccessPasswordResets::Table)
					.if_exists()
					.to_owned(),
			)
			.await?;
		manager
			.drop_table(
				Table::drop()
					.table(AccessSigninMarks::Table)
					.if_exists()
					.to_owned(),
			)
			.await?;
		manager
			.drop_table(Table::drop().table(AccessAudit::Table).if_exists().to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(AccessDevices::Table).if_exists().to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(AccessInvites::Table).if_exists().to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum AccessInvites {
	Table,
	Id,
	TokenHash,
	PersonId,
	Label,
	CreatedAt,
	CreatedBy,
	ExpiresAt,
	MaxUses,
	Uses,
	RevokedAt,
}

#[derive(DeriveIden)]
enum AccessDevices {
	Table,
	Id,
	InviteId,
	PersonId,
	Label,
	CreatedAt,
	FirstSeenAt,
	LastSeenAt,
	LastIp,
	LastAgent,
	RevokedAt,
	RevokedBy,
	BudgetResetAt,
}

#[derive(DeriveIden)]
enum AccessAudit {
	Table,
	Id,
	At,
	Actor,
	Action,
	Target,
	Detail,
}

#[derive(DeriveIden)]
enum AccessSigninMarks {
	Table,
	LoginActivityId,
	DeviceId,
	FailureReason,
	CountsTowardLock,
}

#[derive(DeriveIden)]
enum AccessPasswordResets {
	Table,
	UserId,
	SetAt,
	SetBy,
}

#[derive(DeriveIden)]
enum Users {
	Table,
	Id,
}

#[derive(DeriveIden)]
enum UserLoginActivity {
	Table,
	Id,
}

#[cfg(test)]
mod tests {
	use crate::{Migrator, MigratorTrait};
	use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
	use sea_orm_migration::SchemaManager;

	const NAME: &str = "m20261113_000000_public_access";

	const TABLES: [&str; 5] = [
		"access_invites",
		"access_devices",
		"access_audit",
		"access_signin_marks",
		"access_password_resets",
	];

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

	async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
		db.query_one_raw(Statement::from_string(
			db.get_database_backend(),
			sql.to_string(),
		))
		.await
		.expect("should run the query")
		.expect("a row")
		.try_get::<i64>("", "n")
		.expect("a count")
	}

	async fn insert_user(db: &DatabaseConnection, id: &str, name: &str) {
		db.execute_unprepared(&format!(
			"INSERT INTO users (id, username, hashed_password, is_server_owner, created_at, is_locked)
			 VALUES ('{id}', '{name}', 'x', 0, CURRENT_TIMESTAMP, 0)"
		))
		.await
		.expect("should insert a user");
	}

	/// The tables are created with their columns, and rolling back drops them
	#[tokio::test]
	async fn test_public_access_migration() {
		let db = migrated().await;
		let manager = SchemaManager::new(&db);
		for table in TABLES {
			assert!(manager.has_table(table).await.unwrap(), "{table}");
		}
		for (table, column) in [
			("access_invites", "token_hash"),
			("access_invites", "max_uses"),
			("access_devices", "last_agent"),
			("access_devices", "budget_reset_at"),
			("access_audit", "detail"),
			("access_signin_marks", "counts_toward_lock"),
			("access_signin_marks", "device_id"),
			("access_password_resets", "set_by"),
		] {
			assert!(
				manager.has_column(table, column).await.unwrap(),
				"{table}.{column}"
			);
		}
		// the lead's R6: nothing is added to the table that holds the sign-ins
		for column in ["device_id", "failure_reason", "counts_toward_lock"] {
			assert!(
				!manager
					.has_column("user_login_activity", column)
					.await
					.unwrap(),
				"user_login_activity.{column} must not exist"
			);
		}

		Migrator::down(&db, Some(1))
			.await
			.expect("the rollback should apply");
		for table in TABLES {
			assert!(!manager.has_table(table).await.unwrap(), "{table}");
		}
		assert!(!applied_versions(&db).await.contains(&NAME.to_string()));
	}

	/// The manual rollback (for an older image started on a database that has this migration):
	/// it leaves the other migrations and every other table as they were
	#[tokio::test]
	async fn the_manual_down_leaves_a_database_the_previous_image_can_use() {
		let db = migrated().await;
		let before: Vec<String> = applied_versions(&db)
			.await
			.into_iter()
			.filter(|version| version != NAME)
			.collect();

		// the file the owner runs (a copy of it is in research/), as the text of this test so the
		// two can't drift apart
		let manual_down =
			include_str!("../manual-down/m20261113_000000_public_access.sql");
		db.execute_unprepared(manual_down)
			.await
			.expect("the manual down should run");

		let manager = SchemaManager::new(&db);
		for table in TABLES {
			assert!(!manager.has_table(table).await.unwrap(), "{table}");
		}
		assert_eq!(applied_versions(&db).await, before);
		// what the new tables pointed at is untouched
		assert!(manager.has_table("users").await.unwrap());
		assert!(manager.has_table("user_login_activity").await.unwrap());
	}

	/// An invite goes with its person, a device row stays (and loses the link to its invite),
	/// a sign-in mark goes with its sign-in, a password reset goes with its user
	#[tokio::test]
	async fn rows_follow_their_foreign_keys() {
		let db = migrated().await;
		db.execute_unprepared("PRAGMA foreign_keys = ON")
			.await
			.expect("should turn on foreign keys");
		insert_user(&db, "u1", "ana").await;
		db.execute_unprepared(
			"INSERT INTO access_invites (id, token_hash, person_id, label, created_at, created_by, expires_at)
			 VALUES ('i1', 'h1', 'u1', 'phone', CURRENT_TIMESTAMP, 'owner', CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert an invite");
		db.execute_unprepared(
			"INSERT INTO access_devices (id, invite_id, person_id, label, created_at)
			 VALUES ('ana-phone-k3f9x', 'i1', 'u1', 'phone', CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert a device");
		db.execute_unprepared(
			"INSERT INTO user_login_activity (ip_address, user_agent, authentication_successful, timestamp, user_id)
			 VALUES ('127.0.0.1', 'test', 0, CURRENT_TIMESTAMP, 'u1')",
		)
		.await
		.expect("should insert a login activity");
		db.execute_unprepared(
			"INSERT INTO access_signin_marks (login_activity_id, device_id, failure_reason)
			 VALUES (1, 'ana-phone-k3f9x', 'wrong_password')",
		)
		.await
		.expect("should insert a mark");
		db.execute_unprepared(
			"INSERT INTO access_password_resets (user_id, set_at, set_by) VALUES ('u1', CURRENT_TIMESTAMP, 'owner')",
		)
		.await
		.expect("should insert a reset");
		// a mark counts towards the lock unless it says otherwise
		assert_eq!(
			count(
				&db,
				"SELECT COUNT(*) AS n FROM access_signin_marks WHERE counts_toward_lock = 1"
			)
			.await,
			1
		);

		// the invite is deleted with the person; the device row stays, without the link
		db.execute_unprepared("DELETE FROM access_invites WHERE id = 'i1'")
			.await
			.expect("should delete the invite");
		assert_eq!(
			count(
				&db,
				"SELECT COUNT(*) AS n FROM access_devices WHERE invite_id IS NULL"
			)
			.await,
			1
		);

		// the sign-in is deleted (a user's login activity is cleared), its mark goes with it
		db.execute_unprepared("DELETE FROM user_login_activity")
			.await
			.expect("should delete the login activity");
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM access_signin_marks").await,
			0
		);

		// the user is deleted: the reset goes, the device row stays (no foreign key on person_id)
		db.execute_unprepared("DELETE FROM users WHERE id = 'u1'")
			.await
			.expect("should delete the user");
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM access_password_resets").await,
			0
		);
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM access_devices").await,
			1
		);
	}

	/// The invite of a deleted person is deleted with them
	#[tokio::test]
	async fn an_invite_is_deleted_with_its_person() {
		let db = migrated().await;
		db.execute_unprepared("PRAGMA foreign_keys = ON")
			.await
			.expect("should turn on foreign keys");
		insert_user(&db, "u1", "ana").await;
		db.execute_unprepared(
			"INSERT INTO access_invites (id, token_hash, person_id, label, created_at, created_by, expires_at)
			 VALUES ('i1', 'h1', 'u1', 'phone', CURRENT_TIMESTAMP, 'owner', CURRENT_TIMESTAMP)",
		)
		.await
		.expect("should insert an invite");
		db.execute_unprepared("DELETE FROM users WHERE id = 'u1'")
			.await
			.expect("should delete the user");
		assert_eq!(
			count(&db, "SELECT COUNT(*) AS n FROM access_invites").await,
			0
		);
	}

	/// A token hash is unique: two invites can't share a link
	#[tokio::test]
	async fn a_token_hash_is_unique() {
		let db = migrated().await;
		insert_user(&db, "u1", "ana").await;
		let insert = "INSERT INTO access_invites (id, token_hash, person_id, label, created_at, created_by, expires_at)
			 VALUES ('IDX', 'same', 'u1', 'phone', CURRENT_TIMESTAMP, 'owner', CURRENT_TIMESTAMP)";
		db.execute_unprepared(&insert.replace("IDX", "i1"))
			.await
			.expect("should insert an invite");
		assert!(db
			.execute_unprepared(&insert.replace("IDX", "i2"))
			.await
			.is_err());
	}
}
