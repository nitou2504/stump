use async_graphql::{Enum, SimpleObject};
use chrono::{DateTime, Utc};
use metadata_integrations::MetadataField;
use models::{
	entity::media_metadata_edit,
	shared::intake::{IntakeGenreSuggestion, IntakeSimilarBook, OnlineLookup},
};
use stump_core::intake::{
	batch::{BatchItemStatus, BatchResult},
	edit::{
		ChangedField, CurrentTagFit, EditHistorySave, ExistingBookSuggestions,
		FileEditOutcome, FileWriteStatus,
	},
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
	/// The save asked to keep the changes in Stump only (`writeToFile: false`)
	Skipped,
}

impl From<FileWriteStatus> for MetadataFileWriteStatus {
	fn from(status: FileWriteStatus) -> Self {
		match status {
			FileWriteStatus::Written => Self::Written,
			FileWriteStatus::NotNeeded => Self::NotNeeded,
			FileWriteStatus::Failed => Self::Failed,
			FileWriteStatus::Skipped => Self::Skipped,
		}
	}
}

/// Where the changes of a metadata edit went
#[derive(Debug, Clone, SimpleObject)]
pub struct MetadataEditFile {
	pub status: MetadataFileWriteStatus,
	/// The changed fields (camelCase) now in the book file
	pub file_fields: Vec<String>,
	/// The changed fields kept in Stump only
	pub stump_only_fields: Vec<String>,
	/// The locks that keep the edited fields through rescans
	pub locked: Vec<MetadataField>,
	/// Why the file was not written, or a note about it
	pub message: Option<String>,
}

impl From<FileEditOutcome> for MetadataEditFile {
	fn from(outcome: FileEditOutcome) -> Self {
		Self {
			status: outcome.status.into(),
			file_fields: outcome.file_fields.iter().map(|f| f.to_string()).collect(),
			stump_only_fields: outcome
				.db_only_fields
				.iter()
				.map(|f| f.to_string())
				.collect(),
			locked: outcome.locks,
			message: outcome.message,
		}
	}
}

/// The result of saving a book's metadata
#[derive(Debug, Clone, SimpleObject)]
pub struct MetadataEditResult {
	pub media: Media,
	pub file: MetadataEditFile,
	/// The save in the book's edit history (none when nothing changed)
	pub save_id: Option<String>,
	/// For a revert: the fields left as they are because they changed again since
	pub skipped_fields: Vec<String>,
}

/// One field changed by a save
#[derive(Debug, Clone, SimpleObject)]
pub struct MetadataEditChange {
	/// The field's camelCase name (`genres`, `writers`, ...)
	pub field: String,
	/// The value before (lists comma-separated)
	pub old_value: Option<String>,
	pub new_value: Option<String>,
	/// Whether the book file carries the new value (else it was kept in Stump only)
	pub written_to_file: bool,
}

impl From<media_metadata_edit::Model> for MetadataEditChange {
	fn from(row: media_metadata_edit::Model) -> Self {
		Self {
			field: row.field,
			old_value: row.old_value,
			new_value: row.new_value,
			written_to_file: row.written_to_file,
		}
	}
}

impl MetadataEditChange {
	fn of(change: &ChangedField, outcome: Option<&FileEditOutcome>) -> Self {
		let text = |v: &str| (!v.is_empty()).then(|| v.to_string());
		Self {
			field: change.field.to_string(),
			old_value: text(&change.old),
			new_value: text(&change.new),
			written_to_file: outcome
				.is_some_and(|o| o.file_fields.contains(&change.field)),
		}
	}
}

/// What happened to one book of a batch edit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum MetadataBatchItemStatus {
	/// Saved (see `file` for whether the book file has it)
	Saved,
	/// Nothing to change: the book already had these values
	Unchanged,
	/// Dry run: these changes would be saved
	WouldSave,
	/// Not saved: see `errors`
	Failed,
	/// No book with that id or path that the caller can see (removed books included)
	NotFound,
}

impl From<BatchItemStatus> for MetadataBatchItemStatus {
	fn from(status: BatchItemStatus) -> Self {
		match status {
			BatchItemStatus::Saved => Self::Saved,
			BatchItemStatus::Unchanged => Self::Unchanged,
			BatchItemStatus::WouldSave => Self::WouldSave,
			BatchItemStatus::Failed => Self::Failed,
			BatchItemStatus::NotFound => Self::NotFound,
		}
	}
}

/// One book of `editMediaMetadataBatch`, in the order sent
#[derive(Debug, Clone, SimpleObject)]
pub struct MediaMetadataBatchItemResult {
	/// Its position in `inputs`
	pub index: i32,
	/// The id and path the item was sent with
	pub requested_id: Option<String>,
	pub requested_path: Option<String>,
	/// The book found, if any
	pub media_id: Option<String>,
	pub path: Option<String>,
	pub status: MetadataBatchItemStatus,
	/// The fields changed (or, for a dry run, that would change), with old and new values
	pub changes: Vec<MetadataEditChange>,
	/// Where the changes went (or, for a dry run, would go)
	pub file: Option<MetadataEditFile>,
	/// The save in the book's edit history (revert it with `revertMediaMetadataEdit`)
	pub save_id: Option<String>,
	/// Why it wasn't saved, or why the file kept its old values
	pub errors: Vec<String>,
}

/// The result of `editMediaMetadataBatch`
#[derive(Debug, Clone, SimpleObject)]
pub struct MediaMetadataBatchResult {
	pub dry_run: bool,
	pub items: Vec<MediaMetadataBatchItemResult>,
	pub saved: i32,
	pub unchanged: i32,
	/// Items that would be saved (dry run)
	pub would_save: i32,
	pub failed: i32,
	pub not_found: i32,
	/// How many books the targeted rescan job queued at the end reads again (the ones whose
	/// files were rewritten; 0: no rescan was needed)
	pub rescan_books: i32,
}

impl MediaMetadataBatchResult {
	pub fn new(
		dry_run: bool,
		result: BatchResult,
		references: Vec<(Option<String>, Option<String>)>,
	) -> Self {
		let count = |status| result.count(status) as i32;
		Self {
			dry_run,
			saved: count(BatchItemStatus::Saved),
			unchanged: count(BatchItemStatus::Unchanged),
			would_save: count(BatchItemStatus::WouldSave),
			failed: count(BatchItemStatus::Failed),
			not_found: count(BatchItemStatus::NotFound),
			rescan_books: result.rescan_books as i32,
			items: result
				.items
				.into_iter()
				.zip(references)
				.enumerate()
				.map(|(index, (item, (requested_id, requested_path)))| {
					MediaMetadataBatchItemResult {
						index: index as i32,
						requested_id,
						requested_path,
						media_id: item.book.as_ref().map(|b| b.id.clone()),
						path: item.book.as_ref().map(|b| b.path.clone()),
						status: item.status.into(),
						changes: item
							.changed
							.iter()
							.map(|c| MetadataEditChange::of(c, item.outcome.as_ref()))
							.collect(),
						file: item.outcome.map(MetadataEditFile::from),
						save_id: item.save_id,
						errors: item.errors,
					}
				})
				.collect(),
		}
	}
}

/// One save of a book's metadata edit history
#[derive(Debug, Clone, SimpleObject)]
pub struct MetadataEditSave {
	pub save_id: String,
	pub edited_at: DateTime<Utc>,
	/// The username of who saved it
	pub edited_by: Option<String>,
	/// The save this one reverted
	pub reverts_save_id: Option<String>,
	/// The save that reverted this one
	pub reverted_by: Option<String>,
	pub changes: Vec<MetadataEditChange>,
}

impl From<EditHistorySave> for MetadataEditSave {
	fn from(save: EditHistorySave) -> Self {
		Self {
			save_id: save.save_id,
			edited_at: save.edited_at,
			edited_by: save.edited_by,
			reverts_save_id: save.reverts_save_id,
			reverted_by: save.reverted_by,
			changes: save
				.changes
				.into_iter()
				.map(MetadataEditChange::from)
				.collect(),
		}
	}
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
	/// Whether an online source found the book (a failed lookup isn't one)
	pub looked_up_online: bool,
	/// How each online source answered (empty when they weren't asked): found with N
	/// subjects, no subjects, no match, rate-limited (Google Books without an API key), error
	pub online_lookups: Vec<OnlineLookup>,
}

impl From<ExistingBookSuggestions> for MediaGenreSuggestions {
	fn from(s: ExistingBookSuggestions) -> Self {
		Self {
			suggestions: s.suggestions,
			current: s.current.into_iter().map(CurrentGenreFit::from).collect(),
			similar_books: s.similar_books,
			subjects: s.subjects,
			looked_up_online: s.looked_up_online,
			online_lookups: s.online_lookups,
		}
	}
}
