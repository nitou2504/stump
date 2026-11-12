use sea_orm::entity::prelude::*;

/// "This library has been decided for this person": its exclusion was made, or the owner chose
/// to leave it visible. Each (person, library) pair is decided exactly once, so the owner's own
/// later changes to who sees a library are never overwritten by the rule that hides new libraries
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "access_library_decisions")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub user_id: String,
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub library_id: String,
}

impl ActiveModelBehavior for ActiveModel {}
