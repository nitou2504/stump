use async_graphql::{Enum, SimpleObject};
use metadata_integrations::MetadataField;
use models::shared::intake::{IntakeGenreSuggestion, IntakeSimilarBook};
use stump_core::intake::edit::{
	CurrentTagFit, ExistingBookSuggestions, FileEditOutcome, FileWriteStatus,
};

use super::media::Media;

/// Whether an edit reached the book file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum MetadataFileWriteStatus {
	/// The book file now carries the changed values
	Written,
	/// None of the changes is something the file carries
	NotNeeded,
	/// The file could not take the changes: they are kept in Stump only (and locked)
	Failed,
}

impl From<FileWriteStatus> for MetadataFileWriteStatus {
	fn from(status: FileWriteStatus) -> Self {
		match status {
			FileWriteStatus::Written => Self::Written,
			FileWriteStatus::NotNeeded => Self::NotNeeded,
			FileWriteStatus::Failed => Self::Failed,
		}
	}
}

/// Where the changes of a metadata edit went
#[derive(Debug, Clone, SimpleObject)]
pub struct MetadataEditFile {
	pub status: MetadataFileWriteStatus,
	/// The changed fields (camelCase) now in the book file
	pub file_fields: Vec<String>,
	/// The changed fields kept in Stump only, locked so rescans keep them
	pub stump_only_fields: Vec<String>,
	/// The locks added for them
	pub locked: Vec<MetadataField>,
	/// Why the file was not written, or a note about it
	pub message: Option<String>,
	/// Where the original file was kept
	pub quarantine_path: Option<String>,
	/// The edits log the change was appended to
	pub csv_path: Option<String>,
}

impl From<FileEditOutcome> for MetadataEditFile {
	fn from(outcome: FileEditOutcome) -> Self {
		Self {
			status: outcome.status.into(),
			file_fields: outcome.file_fields.iter().map(|f| f.to_string()).collect(),
			stump_only_fields: outcome.db_only_fields.iter().map(|f| f.to_string()).collect(),
			locked: outcome.locks,
			message: outcome.message,
			quarantine_path: outcome.quarantine_path,
			csv_path: outcome.csv_path,
		}
	}
}

/// The result of saving a book's metadata
#[derive(Debug, Clone, SimpleObject)]
pub struct MetadataEditResult {
	pub media: Media,
	pub file: MetadataEditFile,
}

/// How well one of a book's current tags fits it
#[derive(Debug, Clone, SimpleObject)]
pub struct CurrentGenreFit {
	pub value: String,
	/// `GENRE` or `FACET`
	pub kind: String,
	/// The evidence for it besides the book itself, 0-1
	pub confidence: f64,
	/// Whether anything besides the book supports it (facets always fit)
	pub fits: bool,
	pub reasons: Vec<String>,
	/// How many books of the library carry it
	pub library_count: i32,
}

impl From<CurrentTagFit> for CurrentGenreFit {
	fn from(fit: CurrentTagFit) -> Self {
		Self {
			value: fit.value,
			kind: fit.kind.to_string(),
			confidence: fit.confidence,
			fits: fit.fits,
			reasons: fit.reasons,
			library_count: fit.library_count,
		}
	}
}

/// Tags proposed for a book already in the library, and how its current ones fit
#[derive(Debug, Clone, SimpleObject)]
pub struct MediaGenreSuggestions {
	/// The library's tags that fit the book, best first (`selected` marks the default
	/// choice)
	pub suggestions: Vec<IntakeGenreSuggestion>,
	/// The book's current tags, judged against the same evidence
	pub current: Vec<CurrentGenreFit>,
	/// Books of the library it was compared with
	pub similar_books: Vec<IntakeSimilarBook>,
	/// Subjects found online
	pub subjects: Vec<String>,
	pub looked_up_online: bool,
}

impl From<ExistingBookSuggestions> for MediaGenreSuggestions {
	fn from(s: ExistingBookSuggestions) -> Self {
		Self {
			suggestions: s.suggestions,
			current: s.current.into_iter().map(CurrentGenreFit::from).collect(),
			similar_books: s.similar_books,
			subjects: s.subjects,
			looked_up_online: s.looked_up_online,
		}
	}
}
