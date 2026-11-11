use sea_orm::entity::prelude::*;

/// An invite link the owner made for one person (see `services::access`). Only the SHA-256 of the
/// token is kept: the link itself is shown once, when it is made
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "access_invites")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub id: String,
	#[sea_orm(column_type = "Text", unique)]
	pub token_hash: String,
	/// The user the link is for. Their invites go with them
	#[sea_orm(column_type = "Text")]
	pub person_id: String,
	/// The device name the owner typed ("Ana's phone")
	#[sea_orm(column_type = "Text")]
	pub label: String,
	pub created_at: DateTimeUtc,
	/// The admin's username when the link was made
	#[sea_orm(column_type = "Text")]
	pub created_by: String,
	pub expires_at: DateTimeUtc,
	#[sea_orm(default_value = "3")]
	pub max_uses: i32,
	#[sea_orm(default_value = "0")]
	pub uses: i32,
	pub revoked_at: Option<DateTimeUtc>,
}

impl ActiveModelBehavior for ActiveModel {}
