//! What somebody may upload through "Add books": the server's limits (`Stump.toml`, or
//! `STUMP_INTAKE_*` in the environment) and, where an admin gave one, a user's own, held under the
//! ceilings Stump compiles in.
//!
//! The limits are read once per request and enforced before the body is: the request size is
//! checked against its `Content-Length`, the quota is reserved for it, and while the body streams
//! to disk every byte is counted again, so none of them depends on what the client says.

use models::entity::user_intake_limits::{self, UserIntakeLimitOverrides};
use sea_orm::prelude::*;

use crate::{config::StumpConfig, CoreResult};

const MIB: u64 = 1024 * 1024;

/// The biggest single file anybody can be given, whatever the configuration says
pub const HARD_MAX_FILE_MB: u64 = 2048;
/// The most files in one request anybody can be given
pub const HARD_MAX_FILES_PER_REQUEST: u64 = 500;

const DEFAULT_MAX_REQUEST_MB: u64 = 300;
const DEFAULT_MAX_FILE_MB: u64 = 300;
const DEFAULT_MAX_FILES_PER_REQUEST: u64 = 50;
const DEFAULT_QUOTA_MB: u64 = 2048;
const DEFAULT_MAX_ITEMS: u64 = 100;

/// What one user may upload (sizes in bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntakeLimits {
	/// The most bytes in one upload request
	pub max_request_bytes: u64,
	/// The biggest single file (never more than a request)
	pub max_file_bytes: u64,
	/// The most files in one request
	pub max_files_per_request: u64,
	/// The most bytes of this user's uploads waiting in their inbox at once
	pub quota_bytes: u64,
	/// The most open inbox items
	pub max_items: u64,
}

/// A configured value, where 0 means "the default" (nobody means "no uploads" or "no limit" by it)
fn configured(value: u64, default: u64) -> u64 {
	if value == 0 {
		default
	} else {
		value
	}
}

/// A user's own value when they have one, else the server's
fn picked(own: Option<u64>, server: u64, default: u64) -> u64 {
	match own.filter(|value| *value > 0) {
		Some(own) => own,
		None => configured(server, default),
	}
}

impl IntakeLimits {
	/// The limits of a user, whoever they are: for somebody who files books themselves (an
	/// approver) the files one request may hold are the ceiling, as the GraphQL upload this
	/// route replaced gave them, so a folder of a few hundred volumes goes in one request. The
	/// number the server gives to people who need approval does not hold them back; a number an
	/// admin gave that user does. Sizes and the rest are the same for everybody.
	pub fn for_user(
		config: &StumpConfig,
		own: Option<&UserIntakeLimitOverrides>,
		approver: bool,
	) -> Self {
		let mut limits = Self::new(config, own);
		let has_own_files = own
			.and_then(|own| own.max_files_per_request)
			.is_some_and(|files| files > 0);
		if approver && !has_own_files {
			limits.max_files_per_request = HARD_MAX_FILES_PER_REQUEST;
		}
		limits
	}

	/// The server's limits with a user's own applied, and the ceilings
	pub fn new(config: &StumpConfig, own: Option<&UserIntakeLimitOverrides>) -> Self {
		let own = own.cloned().unwrap_or_default();
		let max_request_bytes =
			configured(config.intake_max_request_mb, DEFAULT_MAX_REQUEST_MB)
				.saturating_mul(MIB);
		let file_mb = picked(
			own.max_file_mb,
			config.intake_max_file_mb,
			DEFAULT_MAX_FILE_MB,
		)
		.min(HARD_MAX_FILE_MB);
		Self {
			max_request_bytes,
			max_file_bytes: file_mb.saturating_mul(MIB).min(max_request_bytes),
			max_files_per_request: picked(
				own.max_files_per_request,
				config.intake_max_files_per_request,
				DEFAULT_MAX_FILES_PER_REQUEST,
			)
			.min(HARD_MAX_FILES_PER_REQUEST),
			quota_bytes: picked(
				own.quota_mb,
				config.intake_user_quota_mb,
				DEFAULT_QUOTA_MB,
			)
			.saturating_mul(MIB),
			max_items: picked(
				own.max_items,
				config.intake_user_max_items,
				DEFAULT_MAX_ITEMS,
			),
		}
	}
}

/// The limits of a user: the server's and their own, if an admin set any. `approver` is whether
/// they file books themselves (see [`IntakeLimits::for_user`])
pub async fn limits_for(
	conn: &DatabaseConnection,
	config: &StumpConfig,
	user_id: &str,
	approver: bool,
) -> CoreResult<IntakeLimits> {
	let own = user_intake_limits::Entity::find_by_id(user_id.to_string())
		.one(conn)
		.await?;
	Ok(IntakeLimits::for_user(
		config,
		own.as_ref().map(|row| &row.limits),
		approver,
	))
}

/// The most bytes of uploads that wait in all inboxes together
pub fn total_max_bytes(config: &StumpConfig) -> u64 {
	configured(config.intake_total_max_gb, 50).saturating_mul(1024 * MIB)
}

/// The free space the disk holding the config folder must keep, in bytes
pub fn min_free_bytes(config: &StumpConfig) -> u64 {
	// 0 is a value here: somebody who sets 0 doesn't want the check
	config.intake_min_free_gb.saturating_mul(1024 * MIB)
}

/// The bytes free on the disk that holds `path`, where the system can say
#[cfg(target_os = "linux")]
pub fn free_bytes(path: &std::path::Path) -> Option<u64> {
	use std::os::unix::ffi::OsStrExt;

	let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
	// SAFETY: `path` is a valid C string and `stats` is written by the call before it is read
	let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
	let status = unsafe { libc::statvfs(path.as_ptr(), &mut stats) };
	(status == 0).then(|| (stats.f_bavail as u64).saturating_mul(stats.f_frsize as u64))
}

#[cfg(not(target_os = "linux"))]
pub fn free_bytes(_path: &std::path::Path) -> Option<u64> {
	None
}

#[cfg(test)]
mod tests {
	use super::*;

	fn config() -> StumpConfig {
		StumpConfig::default()
	}

	#[test]
	fn the_servers_limits_are_the_defaults() {
		let limits = IntakeLimits::new(&config(), None);
		assert_eq!(limits.max_request_bytes, 300 * MIB);
		assert_eq!(limits.max_file_bytes, 300 * MIB);
		assert_eq!(limits.max_files_per_request, 50);
		assert_eq!(limits.quota_bytes, 2048 * MIB);
		assert_eq!(limits.max_items, 100);
	}

	#[test]
	fn a_user_can_have_limits_of_their_own() {
		let own = UserIntakeLimitOverrides {
			quota_mb: Some(10_000),
			max_items: Some(5),
			..Default::default()
		};
		let limits = IntakeLimits::new(&config(), Some(&own));
		assert_eq!(limits.quota_bytes, 10_000 * MIB);
		assert_eq!(limits.max_items, 5);
		// what they don't set follows the server
		assert_eq!(limits.max_files_per_request, 50);
	}

	#[test]
	fn somebody_who_files_books_themselves_may_send_as_many_files_as_a_request_holds() {
		let mut few = config();
		few.intake_max_files_per_request = 5;
		// those who need approval follow the server's number
		assert_eq!(
			IntakeLimits::for_user(&few, None, false).max_files_per_request,
			5
		);
		// an approver's folder of volumes is not held to it
		let approver = IntakeLimits::for_user(&few, None, true);
		assert_eq!(approver.max_files_per_request, HARD_MAX_FILES_PER_REQUEST);
		// and nothing else of theirs changes
		let other = IntakeLimits::for_user(&few, None, false);
		assert_eq!(approver.max_request_bytes, other.max_request_bytes);
		assert_eq!(approver.max_file_bytes, other.max_file_bytes);
		assert_eq!(approver.quota_bytes, other.quota_bytes);
		assert_eq!(approver.max_items, other.max_items);
	}

	#[test]
	fn a_number_an_admin_gave_an_approver_still_holds() {
		let own = UserIntakeLimitOverrides {
			max_files_per_request: Some(30),
			..Default::default()
		};
		assert_eq!(
			IntakeLimits::for_user(&config(), Some(&own), true).max_files_per_request,
			30
		);
		// 0 means no number of their own, as everywhere else
		let zero = UserIntakeLimitOverrides {
			max_files_per_request: Some(0),
			..Default::default()
		};
		assert_eq!(
			IntakeLimits::for_user(&config(), Some(&zero), true).max_files_per_request,
			HARD_MAX_FILES_PER_REQUEST
		);
		// never more than the ceiling, even for them
		let greedy = UserIntakeLimitOverrides {
			max_files_per_request: Some(1_000_000),
			..Default::default()
		};
		assert_eq!(
			IntakeLimits::for_user(&config(), Some(&greedy), true).max_files_per_request,
			HARD_MAX_FILES_PER_REQUEST
		);
	}

	#[test]
	fn a_file_is_never_bigger_than_a_request_or_the_ceiling() {
		let mut small_requests = config();
		small_requests.intake_max_request_mb = 100;
		assert_eq!(
			IntakeLimits::new(&small_requests, None).max_file_bytes,
			100 * MIB
		);

		let greedy = UserIntakeLimitOverrides {
			max_file_mb: Some(1_000_000),
			max_files_per_request: Some(1_000_000),
			..Default::default()
		};
		let mut big_requests = config();
		big_requests.intake_max_request_mb = 10_000_000;
		let limits = IntakeLimits::new(&big_requests, Some(&greedy));
		assert_eq!(limits.max_file_bytes, HARD_MAX_FILE_MB * MIB);
		assert_eq!(limits.max_files_per_request, HARD_MAX_FILES_PER_REQUEST);
	}

	#[test]
	fn zero_means_the_default() {
		let mut zeros = config();
		zeros.intake_user_quota_mb = 0;
		zeros.intake_user_max_items = 0;
		let own = UserIntakeLimitOverrides {
			quota_mb: Some(0),
			..Default::default()
		};
		let limits = IntakeLimits::new(&zeros, Some(&own));
		assert_eq!(limits.quota_bytes, DEFAULT_QUOTA_MB * MIB);
		assert_eq!(limits.max_items, DEFAULT_MAX_ITEMS);
	}

	#[cfg(target_os = "linux")]
	#[test]
	fn the_free_space_of_a_disk_can_be_read() {
		let dir = tempfile::tempdir().unwrap();
		assert!(free_bytes(dir.path()).is_some_and(|free| free > 0));
		assert!(free_bytes(&dir.path().join("missing")).is_none());
	}
}
