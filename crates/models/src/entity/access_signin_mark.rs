use sea_orm::entity::prelude::*;

/// What Stump knows about one row of `user_login_activity` beyond what the row says: the pass the
/// sign-in came with, why a failure failed, and whether the lock rule counts it. A row without a
/// mark counts, as it always has. The row goes with its sign-in
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "access_signin_marks")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false)]
	pub login_activity_id: i64,
	/// The pass id of a sign-in through the public edge
	#[sea_orm(column_type = "Text")]
	pub device_id: Option<String>,
	/// `wrong_password`, `locked`, `owner_via_edge` or `wrong_person`
	#[sea_orm(column_type = "Text")]
	pub failure_reason: Option<String>,
	/// `false` for a failure the lock rule ignores: an attempt that was not a guess at the
	/// account's password (the owner's through the edge, a pass of someone else's), and the
	/// failures the owner forgave when they unblocked the account
	#[sea_orm(default_value = "true")]
	pub counts_toward_lock: bool,
}

impl ActiveModelBehavior for ActiveModel {}
