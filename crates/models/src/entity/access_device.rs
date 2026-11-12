use sea_orm::entity::prelude::*;

/// A browser that holds a pass (the signed cookie the public edge asks for). A row is made when
/// an invite is redeemed, or the first time the app sees a pass it did not mint (one the terminal
/// script made: a *legacy* device, with no person).
///
/// `person_id` has no foreign key on purpose: a cascade would delete the row of a hard-deleted
/// user, and the next request with that pass (valid for months) would register it again as an
/// active device
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "access_devices")]
pub struct Model {
	/// The pass name: `[a-z0-9-]{1,24}`
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub id: String,
	#[sea_orm(column_type = "Text")]
	pub invite_id: Option<String>,
	/// The user the device belongs to. `None` for a legacy pass: it is not bound to anyone
	#[sea_orm(column_type = "Text")]
	pub person_id: Option<String>,
	#[sea_orm(column_type = "Text")]
	pub label: String,
	pub created_at: DateTimeUtc,
	pub first_seen_at: Option<DateTimeUtc>,
	pub last_seen_at: Option<DateTimeUtc>,
	#[sea_orm(column_type = "Text", default_value = "")]
	pub last_ip: String,
	#[sea_orm(column_type = "Text", default_value = "")]
	pub last_agent: String,
	pub revoked_at: Option<DateTimeUtc>,
	#[sea_orm(column_type = "Text")]
	pub revoked_by: Option<String>,
	/// The epoch (seconds) of the last time the owner reset the device's daily sign-in budget at the
	/// edge; 0 = never
	#[sea_orm(default_value = "0")]
	pub budget_reset_at: i64,
	/// `browser` (a pass in a browser's cookie) or `opds` (a reading-app key: `<id>.<mac>` in an
	/// address). Both count as devices of the person, and both are revoked the same way
	#[sea_orm(column_type = "Text", default_value = "browser")]
	pub kind: String,
}

impl ActiveModelBehavior for ActiveModel {}
