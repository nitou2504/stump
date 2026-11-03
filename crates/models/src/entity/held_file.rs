use chrono::Utc;
use sea_orm::{entity::prelude::*, prelude::async_trait::async_trait, ActiveValue};

use crate::shared::trash::HeldFileReason;

/// A file Stump took out of a library and keeps in its trash folder (`<config>/trash`) for the
/// *Deleted books* retention, so it can be put back: the original of a book that
/// post-processing replaced (16 greys, PDF OCR or shrinking), or the version a put-back
/// replaced. When the retention is over (or it is deleted by hand) the file is deleted and the
/// row goes. The row outlives the book's other data, so nothing here is a foreign key.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "held_files")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = true)]
	pub id: i32,
	/// The book the file belongs to
	#[sea_orm(column_type = "Text", nullable)]
	pub media_id: Option<String>,
	#[sea_orm(column_type = "Text", nullable)]
	pub library_id: Option<String>,
	/// The book's title when the file was held
	#[sea_orm(column_type = "Text")]
	pub title: String,
	/// Where the file was in the library (the book's path)
	#[sea_orm(column_type = "Text")]
	pub original_path: String,
	/// The file in the trash folder, relative to it
	#[sea_orm(column_type = "Text")]
	pub held_path: String,
	pub bytes: i64,
	/// Why it left the library
	pub reason: HeldFileReason,
	/// For a PDF: the post-processing steps asked for (`OCR`, `SHRINK`, `OCR+SHRINK`)
	#[sea_orm(column_type = "Text", nullable)]
	pub steps: Option<String>,
	/// The post-processing task that replaced it
	#[sea_orm(column_type = "Text", nullable)]
	pub task_id: Option<String>,
	pub held_at: DateTimeUtc,
	/// Who made Stump take it out (a user id); NULL for post-processing
	#[sea_orm(column_type = "Text", nullable)]
	pub held_by: Option<String>,
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
	async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
	where
		C: ConnectionTrait,
	{
		if insert && self.held_at.is_not_set() {
			self.held_at = ActiveValue::Set(Utc::now());
		}
		Ok(self)
	}
}
