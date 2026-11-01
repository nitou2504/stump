use async_graphql::SimpleObject;
use chrono::Utc;
use sea_orm::{entity::prelude::*, prelude::async_trait::async_trait, ActiveValue};

use crate::shared::post_processing::{PostProcessKind, PostProcessStatus};

use super::{library, media};

/// One post-processing step for one book: the 16-grey reduction of a CBZ, or a PDF sent to
/// the host runner for OCR and/or shrinking. There is at most one row per book and kind; a
/// new run updates it in place
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[sea_orm(table_name = "post_processing_tasks")]
#[graphql(name = "PostProcessingTaskModel")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub id: String,
	pub library_id: String,
	pub media_id: Option<String>,
	#[sea_orm(column_type = "Text")]
	pub kind: PostProcessKind,
	/// For PDFs, the requested steps (`OCR`, `SHRINK` or `OCR+SHRINK`)
	pub steps: Option<String>,
	#[sea_orm(column_type = "Text")]
	pub status: PostProcessStatus,
	/// The book's path when the task was created
	pub path: String,
	/// Why the task ended the way it did, in a few words
	pub reason: Option<String>,
	pub bytes_before: Option<i64>,
	/// The size of the processed file (for an estimate, the expected size)
	pub bytes_after: Option<i64>,
	/// Step details (pages converted, quality numbers, ...)
	#[sea_orm(column_type = "Json", nullable)]
	pub details: Option<serde_json::Value>,
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
	pub finished_at: Option<DateTimeUtc>,
	#[sea_orm(
		belongs_to,
		from = "library_id",
		to = "id",
		on_update = "Cascade",
		on_delete = "Cascade"
	)]
	#[graphql(skip)]
	pub library: BelongsTo<library::Entity>,
	#[sea_orm(
		belongs_to,
		from = "media_id",
		to = "id",
		on_update = "Cascade",
		on_delete = "SetNull"
	)]
	#[graphql(skip)]
	pub media: BelongsTo<Option<media::Entity>>,
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
	async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
	where
		C: ConnectionTrait,
	{
		let now = Utc::now();
		if insert && self.created_at.is_not_set() {
			self.created_at = ActiveValue::Set(now);
		}
		self.updated_at = ActiveValue::Set(now);
		Ok(self)
	}
}
