use sea_orm::{
	ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DbErr, EntityTrait,
	IntoActiveModel, QueryFilter, TransactionSession, TransactionTrait,
};

use crate::entity::{
	age_restriction, api_key, refresh_token, session, user, user_preferences,
};

/// Age restriction params for the to-be-created user
pub struct NewAgeRestriction {
	pub age: i32,
	pub restrict_on_unset: bool,
}

// TODO: once the circular fk dep is removed i should be good to use nested active models here

/// Create a user along with their default preferences row (and optionally an age
/// restriction) in a single transaction
pub async fn create_user_with_preferences<C>(
	conn: &C,
	new_user: user::ActiveModel,
	age_restriction: Option<NewAgeRestriction>,
) -> Result<user::Model, DbErr>
where
	C: TransactionTrait,
{
	let txn = conn.begin().await?;

	let user_model = new_user.insert(&txn).await?;

	let preferences = user_preferences::ActiveModel {
		user_id: Set(Some(user_model.id.clone())),
		..Default::default()
	}
	.insert(&txn)
	.await?;

	let mut user_active = user_model.into_active_model();
	user_active.user_preferences_id = Set(Some(preferences.id));
	let user_model = user_active.update(&txn).await?;

	if let Some(restriction) = age_restriction {
		age_restriction::ActiveModel {
			user_id: Set(user_model.id.clone()),
			age: Set(restriction.age),
			restrict_on_unset: Set(restriction.restrict_on_unset),
			..Default::default()
		}
		.insert(&txn)
		.await?;
	}

	txn.commit().await?;

	Ok(user_model)
}

/// Signs a user out everywhere: removes their sessions (the cookies stop working) and their
/// refresh tokens (each one would mint new access tokens for as long as it lives). The session
/// `keep_session_id` stays when given, for a user who changes their own password and should not
/// be signed out of the place they changed it from. Returns the sessions removed.
///
/// An access token already handed out can't be recalled, but a deleted or locked user is refused
/// with it on every request, and it runs out on its own.
pub async fn revoke_user_sessions<C>(
	conn: &C,
	user_id: &str,
	keep_session_id: Option<&str>,
) -> Result<Vec<session::Model>, DbErr>
where
	C: ConnectionTrait,
{
	let mut delete_sessions =
		session::Entity::delete_many().filter(session::Column::UserId.eq(user_id));
	if let Some(keep_session_id) = keep_session_id {
		delete_sessions =
			delete_sessions.filter(session::Column::SessionId.ne(keep_session_id));
	}
	let removed_sessions = delete_sessions.exec_with_returning(conn).await?;

	refresh_token::Entity::delete_many()
		.filter(refresh_token::Column::UserId.eq(user_id))
		.exec(conn)
		.await?;

	Ok(removed_sessions)
}

/// Removes the API keys of a user. A deleted user's keys are refused anyway; this makes sure
/// nothing brings them back. Returns how many were removed.
pub async fn revoke_user_api_keys<C>(conn: &C, user_id: &str) -> Result<u64, DbErr>
where
	C: ConnectionTrait,
{
	Ok(api_key::Entity::delete_many()
		.filter(api_key::Column::UserId.eq(user_id))
		.exec(conn)
		.await?
		.rows_affected)
}
