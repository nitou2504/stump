use sea_orm::entity::prelude::*;

/// What the owner did in *Public access* (and each redemption of an invite). `detail` is built
/// from ids and labels only: never a token, a link or a password
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "access_audit")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = true)]
	pub id: i32,
	pub at: DateTimeUtc,
	/// The admin's username, or `public` for a redemption
	#[sea_orm(column_type = "Text")]
	pub actor: String,
	/// `invite.create`, `device.revoke`, ...
	#[sea_orm(column_type = "Text")]
	pub action: String,
	/// An invite id, a device id or a user id
	#[sea_orm(column_type = "Text", default_value = "")]
	pub target: String,
	#[sea_orm(column_type = "Text", default_value = "")]
	pub detail: String,
}

impl ActiveModelBehavior for ActiveModel {}
