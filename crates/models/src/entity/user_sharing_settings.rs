use sea_orm::entity::prelude::*;

/// What a user lets the other people on the server see of them. One row per user who has ever
/// changed a setting; a user without a row shares nothing
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "user_sharing_settings")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub user_id: String,
	/// Whether the *My reading* page of the user (what they have finished and are reading now)
	/// can be seen by everyone else on the server
	#[sea_orm(default_value = "false")]
	pub share_reading_history: bool,
	pub updated_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
	/// The ids of the users who let everyone see their reading history
	pub fn sharing_reading_history() -> Select<Entity> {
		Entity::find().filter(Column::ShareReadingHistory.eq(true))
	}
}
