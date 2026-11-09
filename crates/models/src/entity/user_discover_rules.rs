use sea_orm::entity::prelude::*;

/// A user's rules for the home's picks (see `shared::discover::DiscoverRules`), as JSON text
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "user_discover_rules")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub user_id: String,
	#[sea_orm(column_type = "Text")]
	pub rules: String,
	pub updated_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
