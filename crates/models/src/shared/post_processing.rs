//! Post-processing of library files after they are scanned: manga CBZ pages reduced to 16 grey
//! levels (inside Stump), and OCR / size reduction of scanned PDFs (queued for an on-demand host
//! runner, see `scripts/fork/workers/`)

use async_graphql::{Enum, InputObject, SimpleObject};
use chrono::{DateTime, Utc};
use sea_orm::{prelude::*, DeriveActiveEnum, EnumIter, FromJsonQueryResult};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// When a post-processing step runs for a library
#[derive(
	Debug,
	Default,
	Clone,
	Copy,
	PartialEq,
	Eq,
	Hash,
	Serialize,
	Deserialize,
	Enum,
	Display,
	EnumString,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum PostProcessMode {
	/// The step never runs on its own (it can still be started from the library settings)
	#[default]
	Off,
	/// The step runs on books a scan adds after it was turned on
	New,
}

/// The library series excluded from the 16-grey step by default: Berserk's pages are already
/// compact JPEGs, which grow as 16-grey PNGs (book-curator research/comics-size.md)
pub const DEFAULT_GREY16_EXCLUDE: &[&str] = &["/Berserk/"];

/// A library's post-processing settings (stored as JSON in `library_configs`)
#[derive(
	Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult, SimpleObject,
)]
#[serde(rename_all = "camelCase", default)]
#[graphql(name = "PostProcessingConfig")]
pub struct PostProcessingConfig {
	/// Reduce the grey pages of CBZ files to 16 grey levels (4-bit PNG); colour pages stay
	pub grey16: PostProcessMode,
	/// Parts of a book's path (relative to the library, e.g. `/Berserk/`) that keep a CBZ out
	/// of the 16-grey step
	pub grey16_exclude: Vec<String>,
	/// OCR scanned PDFs whose text layer is missing or poor (PDF worker container)
	pub pdf_ocr: PostProcessMode,
	/// Shrink scanned PDFs when it saves enough without visible loss (PDF worker container)
	pub pdf_shrink: PostProcessMode,
	/// When the 16-grey step was set to new books: books added from then on are processed
	pub grey16_since: Option<DateTime<Utc>>,
	/// When PDF OCR was set to new books
	pub pdf_ocr_since: Option<DateTime<Utc>>,
	/// When PDF shrinking was set to new books
	pub pdf_shrink_since: Option<DateTime<Utc>>,
}

impl Default for PostProcessingConfig {
	fn default() -> Self {
		Self {
			grey16: PostProcessMode::Off,
			grey16_exclude: DEFAULT_GREY16_EXCLUDE
				.iter()
				.map(|s| String::from(*s))
				.collect(),
			pdf_ocr: PostProcessMode::Off,
			pdf_shrink: PostProcessMode::Off,
			grey16_since: None,
			pdf_ocr_since: None,
			pdf_shrink_since: None,
		}
	}
}

impl PostProcessingConfig {
	/// Whether any step runs on new books
	pub fn any_new(&self) -> bool {
		self.grey16 == PostProcessMode::New
			|| self.pdf_ocr == PostProcessMode::New
			|| self.pdf_shrink == PostProcessMode::New
	}

	/// Apply new settings, stamping `*_since` when a step is switched on (and clearing it when
	/// it is switched off) so "new books" means books added from that moment
	pub fn apply(&self, input: PostProcessingConfigInput, now: DateTime<Utc>) -> Self {
		let since = |old_mode: PostProcessMode,
		             new_mode: PostProcessMode,
		             old_since: Option<DateTime<Utc>>| match new_mode {
			PostProcessMode::Off => None,
			PostProcessMode::New if old_mode == PostProcessMode::New => {
				old_since.or(Some(now))
			},
			PostProcessMode::New => Some(now),
		};
		let grey16_exclude = input
			.grey16_exclude
			.into_iter()
			.map(|s| String::from(s.trim()))
			.filter(|s| !s.is_empty())
			.collect();
		Self {
			grey16: input.grey16,
			grey16_exclude,
			pdf_ocr: input.pdf_ocr,
			pdf_shrink: input.pdf_shrink,
			grey16_since: since(self.grey16, input.grey16, self.grey16_since),
			pdf_ocr_since: since(self.pdf_ocr, input.pdf_ocr, self.pdf_ocr_since),
			pdf_shrink_since: since(
				self.pdf_shrink,
				input.pdf_shrink,
				self.pdf_shrink_since,
			),
		}
	}
}

/// The settable part of [`PostProcessingConfig`]
#[derive(Debug, Clone, Serialize, Deserialize, InputObject)]
pub struct PostProcessingConfigInput {
	pub grey16: PostProcessMode,
	pub grey16_exclude: Vec<String>,
	pub pdf_ocr: PostProcessMode,
	pub pdf_shrink: PostProcessMode,
}

/// What a post-processing task does
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
pub enum PostProcessKind {
	/// CBZ pages reduced to 16 grey levels (in Stump)
	Grey16,
	/// A PDF sent to the PDF worker for OCR and/or shrinking (see `steps`)
	Pdf,
}

/// Where a post-processing task stands
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
pub enum PostProcessStatus {
	/// Waiting in the queue (PDFs: written for the PDF worker)
	Pending,
	/// Being processed
	Running,
	/// The smaller (or OCRed) file replaced the original, which went to quarantine
	Done,
	/// Processed, but the result was not used (saves too little, or a quality check failed)
	Rejected,
	/// Nothing to do (already 16 greys, born-digital PDF, ...)
	Skipped,
	/// Something went wrong; the original is untouched
	Failed,
	/// A dry run's estimate (nothing was changed)
	Estimated,
}

impl PostProcessStatus {
	/// Whether a task in this state keeps a book from being picked again
	pub fn settles(self) -> bool {
		matches!(
			self,
			Self::Pending | Self::Running | Self::Done | Self::Rejected | Self::Skipped
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn input(grey16: PostProcessMode) -> PostProcessingConfigInput {
		PostProcessingConfigInput {
			grey16,
			grey16_exclude: vec![String::from(" /Berserk/ "), String::new()],
			pdf_ocr: PostProcessMode::Off,
			pdf_shrink: PostProcessMode::Off,
		}
	}

	#[test]
	fn default_excludes_berserk() {
		let config = PostProcessingConfig::default();
		assert_eq!(config.grey16_exclude, vec![String::from("/Berserk/")]);
		assert!(!config.any_new());
	}

	#[test]
	fn switching_on_stamps_since_and_keeps_it() {
		let t1 = Utc::now();
		let on = PostProcessingConfig::default().apply(input(PostProcessMode::New), t1);
		assert_eq!(on.grey16_since, Some(t1));
		assert_eq!(on.grey16_exclude, vec![String::from("/Berserk/")]);
		assert!(on.any_new());

		let t2 = t1 + chrono::Duration::hours(1);
		let still_on = on.apply(input(PostProcessMode::New), t2);
		assert_eq!(still_on.grey16_since, Some(t1));

		let off = still_on.apply(input(PostProcessMode::Off), t2);
		assert_eq!(off.grey16_since, None);
	}

	#[test]
	fn json_without_new_fields_uses_defaults() {
		let config: PostProcessingConfig =
			serde_json::from_str(r#"{"grey16":"NEW"}"#).unwrap();
		assert_eq!(config.grey16, PostProcessMode::New);
		assert_eq!(config.pdf_ocr, PostProcessMode::Off);
		assert_eq!(config.grey16_exclude, vec![String::from("/Berserk/")]);
	}
}
