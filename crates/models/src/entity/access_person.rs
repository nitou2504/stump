use sea_orm::entity::prelude::*;

/// What *Public access* knows about one account beyond the account itself. One row per account
/// the owner has made a decision about (or that a signup link made); an account **without a
/// row** is not allowed in from the internet and has the defaults below. The row goes with the
/// account
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "access_people")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub user_id: String,
	/// The account may sign in through the public address (and use it afterwards)
	#[sea_orm(default_value = "false")]
	pub public_signin: bool,
	/// How many devices the person may have (browsers and reading-app keys), 1 to 10
	#[sea_orm(default_value = "3")]
	pub max_devices: i32,
	/// Libraries made after the account was set up are hidden from it, decided once each
	/// (`access_library_decisions`)
	#[sea_orm(default_value = "true")]
	pub new_libraries_hidden: bool,
	/// The signup link that made the account, while that link is still kept
	#[sea_orm(column_type = "Text")]
	pub signup_link_id: Option<String>,
	pub created_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
