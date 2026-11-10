//! Password sign-ins (the web login and OPDS HTTP Basic auth): tracking, locking, unknown users

mod basic_auth;
mod login;

use std::time::{Duration, Instant};

use models::entity::{user, user_login_activity};
use sea_orm::{
	ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set,
};

use crate::common::TestApp;

/// How many failed attempts in a row it takes to lock an account: the 10th one, after 9 others
const FAILED_ATTEMPTS_TO_LOCK: usize = 10;

/// Puts a user straight into the database. Their bcrypt cost is the lowest there is, so signing
/// in over and over stays fast.
async fn insert_user(
	app: &TestApp,
	username: &str,
	password: &str,
	is_server_owner: bool,
) -> user::Model {
	user::ActiveModel {
		username: Set(username.to_string()),
		hashed_password: Set(bcrypt::hash(password, 4).expect("could not hash")),
		is_server_owner: Set(is_server_owner),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("could not insert user")
}

async fn is_locked(app: &TestApp, user: &user::Model) -> bool {
	user::Entity::find_by_id(user.id.clone())
		.one(app.conn())
		.await
		.expect("could not read user")
		.expect("user is gone")
		.is_locked
}

/// The login activity rows of the user, successful or failed ones
async fn tracked(app: &TestApp, user: &user::Model, successful: bool) -> u64 {
	user_login_activity::Entity::find()
		.filter(user_login_activity::Column::UserId.eq(user.id.clone()))
		.filter(user_login_activity::Column::AuthenticationSuccessful.eq(successful))
		.count(app.conn())
		.await
		.expect("could not count login activity")
}

/// How long one password check takes on this machine
fn one_password_check(app: &TestApp) -> Duration {
	let hash = bcrypt::hash("password", app.ctx.config.password_hash_cost)
		.expect("could not hash");
	let start = Instant::now();
	let _ = bcrypt::verify("not the password", &hash);
	start.elapsed()
}
