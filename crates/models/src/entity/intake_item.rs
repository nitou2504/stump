use sea_orm::{entity::prelude::*, prelude::async_trait::async_trait, ActiveValue};

use crate::shared::intake::{
	IntakeDecision, IntakeDestination, IntakeFiles, IntakeKind, IntakeProposal,
	IntakeResult, IntakeState,
};

use super::user::AuthUser;

/// An upload in a user's "Add books" inbox: one book, or a series of volumes. Its files
/// are staged under the config directory until it is filed into the library.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "intake_items")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub id: String,
	#[sea_orm(column_type = "Text")]
	pub user_id: String,
	#[sea_orm(column_type = "Text")]
	pub library_id: String,
	#[sea_orm(column_type = "Text")]
	pub state: IntakeState,
	#[sea_orm(column_type = "Text")]
	pub kind: IntakeKind,
	/// The uploaded file or folder name, for display
	#[sea_orm(column_type = "Text")]
	pub name: String,
	/// The absolute staging directory holding the uploaded files
	#[sea_orm(column_type = "Text")]
	pub staging_dir: String,
	#[sea_orm(column_type = "Json")]
	pub files: IntakeFiles,
	#[sea_orm(column_type = "Json", nullable)]
	pub proposal: Option<IntakeProposal>,
	#[sea_orm(column_type = "Json", nullable)]
	pub decision: Option<IntakeDecision>,
	#[sea_orm(column_type = "Json", nullable)]
	pub destination: Option<IntakeDestination>,
	#[sea_orm(column_type = "Json", nullable)]
	pub result: Option<IntakeResult>,
	#[sea_orm(column_type = "Text", nullable)]
	pub error: Option<String>,
	pub created_at: DateTimeWithTimeZone,
	pub updated_at: DateTimeWithTimeZone,
	pub filed_at: Option<DateTimeWithTimeZone>,
}

impl Entity {
	/// The items of one user's inbox
	pub fn find_for_user(user: &AuthUser) -> Select<Entity> {
		Entity::find().filter(Column::UserId.eq(&user.id))
	}
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
	async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
	where
		C: ConnectionTrait,
	{
		let now: DateTimeWithTimeZone = chrono::Utc::now().into();
		if insert {
			self.created_at = ActiveValue::Set(now);
		}
		self.updated_at = ActiveValue::Set(now);
		Ok(self)
	}
}
