use chrono::Utc;
use sea_orm::{entity::prelude::*, prelude::async_trait::async_trait, ActiveValue};

use super::{media, user};

/// One field changed by a metadata edit made by hand. The fields changed by one save share
/// its `save_id`; a save that reverts another one names it in `reverts_save_id`. Values are
/// stored the way the metadata row stores them (lists comma-separated), `NULL` when empty.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "media_metadata_edits")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = true)]
	pub id: i32,
	#[sea_orm(column_type = "Text")]
	pub media_id: String,
	#[sea_orm(column_type = "Text")]
	pub save_id: String,
	/// The field's camelCase name (`genres`, `writers`, ...)
	#[sea_orm(column_type = "Text")]
	pub field: String,
	#[sea_orm(column_type = "Text", nullable)]
	pub old_value: Option<String>,
	#[sea_orm(column_type = "Text", nullable)]
	pub new_value: Option<String>,
	/// The user who saved it
	#[sea_orm(column_type = "Text", nullable)]
	pub edited_by: Option<String>,
	pub edited_at: DateTimeUtc,
	/// Whether the book file carries the new value (else it is kept in Stump only)
	pub written_to_file: bool,
	#[sea_orm(column_type = "Text", nullable)]
	pub reverts_save_id: Option<String>,
	#[sea_orm(
		belongs_to,
		from = "media_id",
		to = "id",
		on_update = "Cascade",
		on_delete = "Cascade"
	)]
	pub media: BelongsTo<media::Entity>,
	#[sea_orm(
		belongs_to,
		from = "edited_by",
		to = "id",
		on_update = "Cascade",
		on_delete = "SetNull"
	)]
	pub editor: BelongsTo<Option<user::Entity>>,
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
	async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
	where
		C: ConnectionTrait,
	{
		if insert && self.edited_at.is_not_set() {
			self.edited_at = ActiveValue::Set(Utc::now());
		}
		Ok(self)
	}
}
