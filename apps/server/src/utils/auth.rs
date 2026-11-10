use std::sync::OnceLock;

use models::entity::user::{self, AuthUser, LoginUser};
use sea_orm::{ColumnTrait, DatabaseConnection, QueryFilter};
use stump_core::config::StumpConfig;
use tower_sessions::Session;

use crate::{
	config::session::SESSION_USER_KEY,
	errors::{APIError, AuthError},
};

/// A struct to represent the decoded username and (plaintext) password from a base64-encoded
/// string
#[derive(Debug)]
pub struct DecodedCredentials {
	pub username: String,
	pub password: String,
}

pub fn hash_password(password: &str, config: &StumpConfig) -> Result<String, AuthError> {
	Ok(bcrypt::hash(password, config.password_hash_cost)?)
}

/// Verify a password against a hash using the bcrypt algorithm. A check takes a quarter of a
/// second of CPU at the default cost, so it runs on the blocking thread pool: a burst of sign-ins
/// must not hold up the async workers that serve everybody else.
pub async fn verify_password(hash: String, password: String) -> Result<bool, APIError> {
	tokio::task::spawn_blocking(move || bcrypt::verify(password, &hash))
		.await
		.map_err(|error| {
			tracing::error!(?error, "The password check did not finish");
			APIError::InternalServerError("Failed to check the password".to_string())
		})?
		.map_err(APIError::from)
}

/// Checks a password against a throwaway hash and drops the result. It is for logins that have
/// no real hash to check (an unknown username): refusing them then takes as long as refusing a
/// wrong password for a real account, so the response time doesn't show which usernames exist.
pub async fn verify_dummy_password(password: String, config: &StumpConfig) {
	let cost = config.password_hash_cost;
	let _ = tokio::task::spawn_blocking(move || {
		let _ = bcrypt::verify(password, dummy_hash(cost));
	})
	.await;
}

/// A hash of a fixed password that nobody uses, made once with the configured cost so it is as
/// slow to check as the real ones
fn dummy_hash(cost: u32) -> &'static str {
	static DUMMY_HASH: OnceLock<String> = OnceLock::new();
	DUMMY_HASH
		.get_or_init(|| bcrypt::hash("stump-dummy-password", cost).unwrap_or_default())
}

// TODO(axum-upgrade): rebase with develop to get relevant fixes for this
/// Decode a base64-encoded string into a username and password pair
pub fn decode_base64_credentials(
	bytes: Vec<u8>,
) -> Result<DecodedCredentials, AuthError> {
	let decoded = String::from_utf8(bytes).map_err(|_| AuthError::BadCredentials)?;

	match decoded.split_once(':') {
		Some((username, password)) => {
			if username.is_empty() || password.is_empty() {
				Err(AuthError::BadCredentials)
			} else {
				Ok(DecodedCredentials {
					username: username.to_string(),
					password: password.to_string(),
				})
			}
		},
		None => Err(AuthError::BadCredentials),
	}
}

pub async fn fetch_session_user(
	session: &Session,
	conn: &DatabaseConnection,
) -> Result<Option<AuthUser>, APIError> {
	if let Some(user_id) = session.get::<String>(SESSION_USER_KEY).await? {
		// a deleted user's cookie stops working even if its session was left behind
		let user = LoginUser::find_by_id(user_id)
			.filter(user::Column::DeletedAt.is_null())
			.into_model::<LoginUser>()
			.one(conn)
			.await?
			.ok_or(APIError::Unauthorized)?;

		if user.is_locked {
			return Err(APIError::AccountLocked);
		}

		Ok(Some(user.into()))
	} else {
		tracing::debug!("No user found in session");
		Ok(None)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_verify_password() {
		let hash = bcrypt::hash("password", bcrypt::DEFAULT_COST).unwrap();
		assert!(verify_password(hash.clone(), "password".into())
			.await
			.unwrap());
		assert!(!verify_password(hash, "passw0rd".into()).await.unwrap());
	}

	#[tokio::test]
	async fn test_dummy_hash_is_a_valid_hash_nobody_matches() {
		let config = StumpConfig::debug();
		let hash = dummy_hash(config.password_hash_cost);
		assert!(hash.starts_with("$2"), "not a bcrypt hash: {hash:?}");
		assert!(!verify_password(hash.to_string(), "password".into())
			.await
			.unwrap());
		// the same hash every time, and no panic when used
		assert_eq!(hash, dummy_hash(config.password_hash_cost));
		verify_dummy_password("password".into(), &config).await;
	}

	#[test]
	fn test_decode_64_credentials_with_colon_in_password() {
		let testcreds = decode_base64_credentials("username:pass:$%^word".into());
		assert_eq!(testcreds.unwrap().password, String::from("pass:$%^word"));
	}

	#[test]
	fn test_decode_64_credentials_32_chars_password() {
		let testcreds =
			decode_base64_credentials("username:wp*r@hj!1b:o4sZ#5TdvyzBd$n-bqaPi".into());
		assert_eq!(
			testcreds.unwrap().password,
			String::from("wp*r@hj!1b:o4sZ#5TdvyzBd$n-bqaPi")
		);
	}

	#[test]
	fn test_decode_64_credentials_64_chars_password() {
		let testcreds = decode_base64_credentials(
			"username:wp*r@hj!1b:o4sZ#5TdvyzBd$n-bqaPiwp*r@hj!1b:o4sZ#5TdvyzBd$n-bqaPi"
				.into(),
		);
		assert_eq!(
			testcreds.unwrap().password,
			String::from(
				"wp*r@hj!1b:o4sZ#5TdvyzBd$n-bqaPiwp*r@hj!1b:o4sZ#5TdvyzBd$n-bqaPi"
			)
		);
	}
}
