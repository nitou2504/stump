//! The metadata batch API: many books edited in one call, each through the same save as the
//! web editor ([`super::edit::save_edit`]: the book file written, read back and swapped in,
//! the database row, the locks and the edit history), one book at a time. One book failing
//! doesn't stop the others. Stump's own writes don't set the library watcher off, and the
//! books whose files were rewritten are read again by ONE targeted rescan job at the end
//! ([`crate::scan::rescan`]) instead of a library scan per book.

use models::entity::{media, media_metadata};

use super::edit::{save_edit, ChangedField, Editor, FileEditOutcome, SaveOptions};
use crate::{job::StumpJob, CoreResult, Ctx};

/// The most books one call may edit
pub const MAX_BATCH_ITEMS: usize = 500;

/// One book of a batch, as the caller named it
#[derive(Debug, Clone)]
pub struct BatchItem {
	/// The book, when the id or path named one the caller can see
	pub book: Option<media::Model>,
	/// The edited values (fields left unset stay as they are)
	pub active: media_metadata::ActiveModel,
	/// Why the item can't be saved (a bad value, a missing book, ...)
	pub problems: Vec<String>,
}

/// What happened to one book
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchItemStatus {
	/// Saved (in the file too, unless the outcome says otherwise)
	Saved,
	/// Nothing to change: the book already had these values
	Unchanged,
	/// Dry run: these changes would be saved
	WouldSave,
	/// Not saved: see the errors
	Failed,
	/// No book with that id or path (or not one the caller can see)
	NotFound,
}

#[derive(Debug, Clone)]
pub struct BatchItemResult {
	pub status: BatchItemStatus,
	pub book: Option<media::Model>,
	/// The changed fields (or, for a dry run, the ones that would change)
	pub changed: Vec<ChangedField>,
	pub outcome: Option<FileEditOutcome>,
	pub save_id: Option<String>,
	pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BatchResult {
	pub items: Vec<BatchItemResult>,
	/// The books queued for the targeted rescan (their files were rewritten)
	pub rescan_books: usize,
}

impl BatchResult {
	pub fn count(&self, status: BatchItemStatus) -> usize {
		self.items.iter().filter(|i| i.status == status).count()
	}
}

fn failed(
	book: Option<media::Model>,
	status: BatchItemStatus,
	errors: Vec<String>,
) -> BatchItemResult {
	BatchItemResult {
		status,
		book,
		changed: vec![],
		outcome: None,
		save_id: None,
		errors,
	}
}

/// Saves every item, in order, then queues one rescan of the books whose files changed
pub async fn save_batch(
	ctx: &Ctx,
	items: Vec<BatchItem>,
	editor: Editor<'_>,
	options: SaveOptions,
) -> CoreResult<BatchResult> {
	let mut result = BatchResult::default();
	let mut rewritten: Vec<String> = vec![];
	for item in items {
		let Some(book) = item.book else {
			result
				.items
				.push(failed(None, BatchItemStatus::NotFound, item.problems));
			continue;
		};
		if !item.problems.is_empty() {
			result
				.items
				.push(failed(Some(book), BatchItemStatus::Failed, item.problems));
			continue;
		}
		let saved = Box::pin(save_edit(
			ctx,
			book.clone(),
			item.active,
			editor,
			None,
			options,
		))
		.await;
		match saved {
			Ok(saved) => {
				let status = if saved.changed.is_empty() {
					BatchItemStatus::Unchanged
				} else if options.dry_run {
					BatchItemStatus::WouldSave
				} else {
					BatchItemStatus::Saved
				};
				if !options.dry_run
					&& saved.outcome.status == super::edit::FileWriteStatus::Written
					&& !rewritten.contains(&saved.book.id)
				{
					rewritten.push(saved.book.id.clone());
				}
				let errors = match saved.outcome.status {
					super::edit::FileWriteStatus::Failed => {
						saved.outcome.message.clone().into_iter().collect()
					},
					_ => vec![],
				};
				result.items.push(BatchItemResult {
					status,
					book: Some(saved.book),
					changed: saved.changed,
					outcome: Some(saved.outcome),
					save_id: saved.save_id,
					errors,
				});
			},
			Err(error) => {
				tracing::warn!(?error, media_id = %book.id, "A batch metadata edit failed");
				result.items.push(failed(
					Some(book),
					BatchItemStatus::Failed,
					vec![error.to_string()],
				));
			},
		}
	}
	if !rewritten.is_empty() {
		result.rescan_books = rewritten.len();
		let job = StumpJob::RescanBooks {
			media_ids: rewritten,
			reason: "metadata batch edit".to_string(),
		};
		if let Err(error) = ctx.enqueue(job).await {
			tracing::error!(
				?error,
				"Failed to queue the rescan after a metadata batch edit"
			);
			result.rescan_books = 0;
		}
	}
	Ok(result)
}
