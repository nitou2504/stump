//! Deleted books (the trash): the types shared by the server config, the held files and the
//! purge job

use async_graphql::{Enum, SimpleObject};
use chrono::{DateTime, Utc};
use sea_orm::{prelude::*, DeriveActiveEnum, EnumIter, FromJsonQueryResult};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// How many days a removed book stays in *Deleted books* before it is deleted permanently,
/// unless the server is configured otherwise
pub const DEFAULT_RETENTION_DAYS: i32 = 14;

/// The longest retention the settings accept (10 years)
pub const MAX_RETENTION_DAYS: i32 = 3650;

/// Why Stump holds a file in its trash folder
#[derive(
	Debug,
	Clone,
	Copy,
	PartialEq,
	Eq,
	Hash,
	EnumIter,
	DeriveActiveEnum,
	Serialize,
	Deserialize,
	Enum,
	Display,
	EnumString,
)]
#[sea_orm(
	rs_type = "String",
	rename_all = "SCREAMING_SNAKE_CASE",
	db_type = "String(StringLen::None)"
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum HeldFileReason {
	/// The original of a manga volume reduced to 16 grey levels
	Grey16,
	/// The original of a PDF the PDF worker OCRed and/or shrank
	Pdf,
	/// The version of a book that was in the library when its held original was put back
	PutBack,
}

/// What an automatic purge of expired deleted books did
#[derive(
	Default,
	Debug,
	Clone,
	PartialEq,
	Eq,
	Serialize,
	Deserialize,
	SimpleObject,
	FromJsonQueryResult,
)]
#[serde(rename_all = "camelCase")]
#[graphql(name = "DeletedBooksPurgeSummary")]
pub struct TrashPurgeSummary {
	/// When it finished
	pub finished_at: DateTime<Utc>,
	/// The job that ran it, if it ran as a job (nothing had expired otherwise)
	pub job_id: Option<String>,
	/// Books deleted permanently (their file and their data)
	pub purged: u64,
	/// Books whose file was already gone: only their data was deleted
	pub missing_files: u64,
	/// Books kept in Deleted books because their file couldn't be deleted
	pub failed: u64,
	/// Held originals (and put-back versions) whose time was over, deleted
	#[serde(default)]
	pub originals: u64,
	/// Why books were kept (the first few)
	pub errors: Vec<TrashPurgeError>,
}

/// A book a purge couldn't delete permanently
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
#[graphql(name = "DeletedBooksPurgeError")]
pub struct TrashPurgeError {
	pub media_id: String,
	pub title: String,
	pub error: String,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_summary_round_trips_as_json() {
		let summary = TrashPurgeSummary {
			finished_at: "2026-11-04T04:00:01Z".parse().unwrap(),
			job_id: Some("job".to_string()),
			purged: 2,
			missing_files: 1,
			failed: 1,
			originals: 3,
			errors: vec![TrashPurgeError {
				media_id: "book".to_string(),
				title: "A book".to_string(),
				error: "Permission denied".to_string(),
			}],
		};
		let json = serde_json::to_string(&summary).unwrap();
		assert!(json.contains("\"missingFiles\":1"));
		assert_eq!(
			serde_json::from_str::<TrashPurgeSummary>(&json).unwrap(),
			summary
		);
	}
}
