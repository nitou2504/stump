//! The file the edge follows: `state.json`, in a folder the app owns and the edge mounts read-only
//! (`deploy/public-edge/stump-gate.js` reads it once a second).
//!
//! It holds ids and numbers only: the devices that are revoked, and the epoch of each device's
//! last budget reset. Never a token, a key, a name or an address.
//!
//! The edge applies a file whenever its `v` differs from the one it holds, so `v` is the write
//! time in milliseconds and only ever grows. A write is a temporary file in the same folder,
//! synced, made readable by everyone, and renamed over the old one: the edge sees the old file or
//! the new one, never half of one.

use std::{
	io::Write,
	path::{Path, PathBuf},
	sync::{Arc, OnceLock},
	time::Duration as StdDuration,
};

use chrono::{DateTime, SecondsFormat, Utc};
use models::services::access::{edge_state, prune, EdgeState};
use sea_orm::{DatabaseConnection, DbErr};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::config::StumpConfig;

pub const STATE_FILE: &str = "state.json";

/// How often the housekeeping loop writes the file again (a harmless heartbeat) and prunes
pub const HOUSEKEEPING_INTERVAL: StdDuration = StdDuration::from_secs(6 * 60 * 60);

#[derive(Debug, thiserror::Error)]
pub enum StateError {
	#[error(transparent)]
	Db(#[from] DbErr),
	#[error("the edge's state file could not be written: {0}")]
	Io(String),
}

#[derive(Serialize)]
struct StateFile<'a> {
	v: i64,
	written: String,
	revoked: &'a [String],
	budget: &'a std::collections::BTreeMap<String, i64>,
}

/// The text of the file for `state`, as version `v` written at `now`
pub fn render(state: &EdgeState, v: i64, now: DateTime<Utc>) -> String {
	serde_json::to_string(&StateFile {
		v,
		written: now.to_rfc3339_opts(SecondsFormat::Secs, true),
		revoked: &state.revoked,
		budget: &state.budget,
	})
	.expect("a state file is always serialisable")
}

/// The version of the last file written by this process. It is also the lock that makes two
/// writes happen one after the other, with the database read inside it, so the older state can
/// never be written last
fn last_version() -> &'static Mutex<i64> {
	static LAST: OnceLock<Mutex<i64>> = OnceLock::new();
	LAST.get_or_init(|| Mutex::new(0))
}

/// Writes the state file for what the database says now. Returns the version written
pub async fn publish(
	conn: &DatabaseConnection,
	dir: &Path,
	now: DateTime<Utc>,
) -> Result<i64, StateError> {
	let mut last = last_version().lock().await;
	let state = edge_state(conn).await?;
	let v = now.timestamp_millis().max(*last + 1);
	let body = render(&state, v, now);
	let dir = dir.to_path_buf();
	tokio::task::spawn_blocking(move || write_atomically(&dir, body.as_bytes()))
		.await
		.map_err(|error| StateError::Io(error.to_string()))?
		.map_err(|error| StateError::Io(error.to_string()))?;
	*last = v;
	Ok(v)
}

fn write_atomically(dir: &Path, body: &[u8]) -> std::io::Result<()> {
	let temporary = dir.join(format!(".{STATE_FILE}.{}.tmp", std::process::id()));
	let result = (|| {
		let mut file = std::fs::File::create(&temporary)?;
		file.write_all(body)?;
		file.sync_all()?;
		// readable by everyone, whatever the umask: the edge runs as another user
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o644))?;
		}
		std::fs::rename(&temporary, dir.join(STATE_FILE))
	})();
	if result.is_err() {
		let _ = std::fs::remove_file(&temporary);
	}
	result
}

/// When the app last wrote the state file in `dir`, as the file says it (`None` when there is no
/// file, or it is not one of ours)
pub fn state_written_at(dir: &Path) -> Option<DateTime<Utc>> {
	let text = std::fs::read_to_string(dir.join(STATE_FILE)).ok()?;
	let value: serde_json::Value = serde_json::from_str(&text).ok()?;
	DateTime::parse_from_rfc3339(value.get("written")?.as_str()?)
		.ok()
		.map(|at| at.with_timezone(&Utc))
}

/// The folder the state file goes in, when the edge is configured with one
pub fn state_dir(config: &StumpConfig) -> Option<PathBuf> {
	config.edge.as_ref().and_then(|edge| edge.state_directory())
}

/// Writes the state file if the app has a folder for it. `Ok(false)` when it has none (nothing to
/// tell the edge); an error when it has one and the write failed
pub async fn publish_if_configured(
	conn: &DatabaseConnection,
	config: &StumpConfig,
) -> Result<bool, StateError> {
	let Some(dir) = state_dir(config) else {
		return Ok(false);
	};
	publish(conn, &dir, Utc::now()).await?;
	Ok(true)
}

/// Once at start, then every six hours: forget what is old enough to forget, and write the state
/// file again (so the edge can tell the app is alive, and a file someone deleted comes back)
pub fn spawn_housekeeping(conn: Arc<DatabaseConnection>, config: Arc<StumpConfig>) {
	if config.edge.is_none() {
		return;
	}
	tokio::spawn(async move {
		loop {
			tokio::time::sleep(HOUSEKEEPING_INTERVAL).await;
			if let Err(error) = prune(conn.as_ref(), Utc::now()).await {
				tracing::error!(?error, "Public access: pruning failed");
			}
			if let Err(error) = publish_if_configured(conn.as_ref(), &config).await {
				tracing::error!(?error, "Public access: the state file was not written");
			}
		}
	});
}

/// How often libraries made by something that doesn't know about the hide-new-libraries rule (a
/// script that inserts into `libraries`) are hidden from the people the rule is for
pub const LIBRARY_RECONCILE_INTERVAL: StdDuration = StdDuration::from_secs(10 * 60);

/// Once at start, then every ten minutes: hides every library that has no decision yet from the
/// people whose "hide new libraries" choice is on (`access_people::reconcile_libraries`). It does
/// **not** depend on the edge settings: libraries must stay hidden even if they are removed
pub fn spawn_library_reconcile(conn: Arc<DatabaseConnection>) {
	tokio::spawn(async move {
		loop {
			match models::services::access_people::reconcile_libraries(conn.as_ref())
				.await
			{
				Ok(0) => {},
				Ok(hidden) => {
					tracing::info!(
						hidden,
						"Hid new libraries from the people they are hidden from"
					)
				},
				Err(error) => {
					tracing::error!(?error, "Public access: hiding new libraries failed")
				},
			}
			tokio::time::sleep(LIBRARY_RECONCILE_INTERVAL).await;
		}
	});
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::BTreeMap;

	#[test]
	fn the_file_holds_ids_and_numbers_only() {
		let state = EdgeState {
			revoked: vec!["ana-phone-k3f9x".to_string()],
			budget: BTreeMap::from([("ana-laptop-2m8qd".to_string(), 1_794_420_000)]),
		};
		let now = DateTime::parse_from_rfc3339("2026-11-11T14:03:11Z")
			.unwrap()
			.with_timezone(&Utc);
		let text = render(&state, 1_794_423_791_150, now);
		let value: serde_json::Value = serde_json::from_str(&text).unwrap();
		assert_eq!(value["v"], 1_794_423_791_150_i64);
		assert_eq!(value["written"], "2026-11-11T14:03:11Z");
		assert_eq!(value["revoked"], serde_json::json!(["ana-phone-k3f9x"]));
		assert_eq!(
			value["budget"],
			serde_json::json!({"ana-laptop-2m8qd": 1_794_420_000_i64})
		);
		assert_eq!(value.as_object().unwrap().len(), 4);
	}

	#[test]
	fn a_write_replaces_the_file_whole_and_readable() {
		let dir = tempfile::tempdir().unwrap();
		write_atomically(dir.path(), b"{\"v\":1}").unwrap();
		write_atomically(dir.path(), b"{\"v\":2}").unwrap();
		let path = dir.path().join(STATE_FILE);
		assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"v\":2}");
		// nothing is left next to it
		let names: Vec<_> = std::fs::read_dir(dir.path())
			.unwrap()
			.map(|entry| entry.unwrap().file_name())
			.collect();
		assert_eq!(names.len(), 1, "{names:?}");
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
			assert_eq!(mode, 0o644);
		}
	}

	#[test]
	fn a_missing_folder_is_an_error_and_leaves_nothing() {
		let dir = tempfile::tempdir().unwrap();
		let missing = dir.path().join("nope");
		assert!(write_atomically(&missing, b"x").is_err());
		assert!(!missing.exists());
	}
}
