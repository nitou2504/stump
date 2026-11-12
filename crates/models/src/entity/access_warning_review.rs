use sea_orm::entity::prelude::*;

/// A warning of *Public access -> Needs your attention* the owner marked as seen. The warnings
/// are computed on request, so this remembers what the owner had seen of one: it stays hidden
/// while its count and its newest event are no higher than they were then. A newer event, or a
/// worse state, shows it again. The owner's account may go: the row stays and forgets who
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "access_warning_reviews")]
pub struct Model {
	/// The warning's stable id, like `AtDeviceLimit:<user id>` or `SigninFailures:-`
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub warning_id: String,
	pub dismissed_at: DateTimeUtc,
	#[sea_orm(column_type = "Text")]
	pub dismissed_by: Option<String>,
	/// The warning's count when it was dismissed
	pub count_at_dismiss: i64,
	/// The time of the newest event behind the warning when it was dismissed
	pub last_at_dismiss: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
