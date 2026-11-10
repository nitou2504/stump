use sea_orm::entity::prelude::*;

/// A user whose password the owner set: until they choose their own, they can use nothing but the
/// way to change it. A user without a row has no such requirement; the row goes with the user, and
/// when they change their password themselves
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "access_password_resets")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub user_id: String,
	pub set_at: DateTimeUtc,
	/// The username of whoever set the password
	#[sea_orm(column_type = "Text")]
	pub set_by: String,
}

impl ActiveModelBehavior for ActiveModel {}
