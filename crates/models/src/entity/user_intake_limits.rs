use sea_orm::{entity::prelude::*, FromJsonQueryResult};
use serde::{Deserialize, Serialize};

/// A user's own limits for what they may upload through "Add books", where they differ from the
/// server's (`Stump.toml`): somebody who brings whole manga series may have a bigger quota. A
/// user without a row (or a field left out) uses the server's value, and no value goes past the
/// ceiling Stump compiles in. Not a foreign key, so the table can be dropped without a trace.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "user_intake_limits")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub user_id: String,
	#[sea_orm(column_type = "Json")]
	pub limits: UserIntakeLimitOverrides,
	pub updated_at: DateTimeWithTimeZone,
}

/// The limits a user may have of their own. Fields not set follow the server's configuration
#[derive(
	Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult,
)]
#[serde(default)]
pub struct UserIntakeLimitOverrides {
	/// The biggest file, in MB
	pub max_file_mb: Option<u64>,
	/// The most files in one upload request
	pub max_files_per_request: Option<u64>,
	/// The most MB of their uploads that wait in the inbox at once
	pub quota_mb: Option<u64>,
	/// The most inbox items that are not filed, discarded or sent back
	pub max_items: Option<u64>,
}

impl ActiveModelBehavior for ActiveModel {}
