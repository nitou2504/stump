use sea_orm::entity::prelude::*;

/// A link the owner made that lets somebody **make their own account** (see
/// `services::signup`): for one person (`single`, one use) or for several (`open`, a budget of
/// accounts), valid until `expires_at`, with the account's *preset* (the permissions, libraries,
/// age limit and device limit it gets). Only the SHA-256 of the token is kept: the link itself
/// is shown once, when it is made
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "access_signup_links")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub id: String,
	#[sea_orm(column_type = "Text", unique)]
	pub token_hash: String,
	/// The owner's note ("book club"), plain text
	#[sea_orm(column_type = "Text")]
	pub label: String,
	/// `single` or `open`
	#[sea_orm(column_type = "Text")]
	pub kind: String,
	pub created_at: DateTimeUtc,
	/// The owner's username when the link was made
	#[sea_orm(column_type = "Text")]
	pub created_by: String,
	pub expires_at: DateTimeUtc,
	/// The budget: how many accounts the link may create
	pub max_uses: i32,
	#[sea_orm(default_value = "0")]
	pub uses: i32,
	pub revoked_at: Option<DateTimeUtc>,
	/// The preset, as JSON (`services::signup::Preset`), checked at creation and again at redemption
	#[sea_orm(column_type = "Text")]
	pub preset: String,
}

impl ActiveModelBehavior for ActiveModel {}
