use sea_orm::entity::prelude::*;

/// A user's reading settings as the web reader saves them: one JSON text (theme, the
/// defaults for every EPUB, PDF or comic, each book's own settings), opaque to the server,
/// so they follow the user to every address, browser and device
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "user_reader_settings")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub user_id: String,
	#[sea_orm(column_type = "Text")]
	pub settings: String,
	pub updated_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
