use chrono::Utc;
use sea_orm::{entity::prelude::*, prelude::async_trait::async_trait, ActiveValue};

/// A book deleted permanently from *Deleted books*: its file went to the quarantine folder
/// (`quarantine_path`, NULL when the file was already gone) and its data was deleted. The row
/// outlives the book, its library and the users it names, so nothing here is a foreign key.
/// `restored_at` is set once the file was moved back into the library.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "media_purges")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = true)]
	pub id: i32,
	#[sea_orm(column_type = "Text")]
	pub media_id: String,
	#[sea_orm(column_type = "Text", nullable)]
	pub library_id: Option<String>,
	#[sea_orm(column_type = "Text", nullable)]
	pub library_name: Option<String>,
	#[sea_orm(column_type = "Text")]
	pub title: String,
	/// Comma-separated, as the metadata stores them
	#[sea_orm(column_type = "Text", nullable)]
	pub authors: Option<String>,
	#[sea_orm(column_type = "Text")]
	pub original_path: String,
	#[sea_orm(column_type = "Text", nullable)]
	pub quarantine_path: Option<String>,
	pub bytes: Option<i64>,
	/// Who removed the book from its library (a user id)
	#[sea_orm(column_type = "Text", nullable)]
	pub deleted_by: Option<String>,
	pub deleted_at: Option<DateTimeUtc>,
	/// Who deleted it permanently (a user id); NULL for the automatic purge
	#[sea_orm(column_type = "Text", nullable)]
	pub purged_by: Option<String>,
	pub purged_at: DateTimeUtc,
	pub restored_at: Option<DateTimeUtc>,
	#[sea_orm(column_type = "Text", nullable)]
	pub restored_by: Option<String>,
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
	async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
	where
		C: ConnectionTrait,
	{
		if insert && self.purged_at.is_not_set() {
			self.purged_at = ActiveValue::Set(Utc::now());
		}
		Ok(self)
	}
}
