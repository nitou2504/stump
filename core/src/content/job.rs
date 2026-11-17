//! The job that reads the books' text into the content index. It runs on the post-processing
//! worker (one at a time, never holding up scans) with the EPUB reading on a thread at idle
//! priority, reads only the books that are new, changed (size or modification time) or never
//! indexed, and keeps each book's result as it goes: a run that stops (a restart, a cancel) is
//! simply run again and goes on with what is left.

use std::{collections::HashMap, path::Path, time::Duration};

use async_graphql::SimpleObject;
use models::entity::media;
use sea_orm::{prelude::*, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};

use super::{
	extract::{extract_epub, extract_pdf, Extracted},
	kind_of, store, ContentError, EXTRACT_TIMEOUT_SECS,
};
use crate::job::{
	error::JobError, JobContext, JobExecuteLog, JobLifecycle, JobOutputExt, JobProgress,
	JobTaskOutput, WorkingState,
};

/// What one run does
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentIndexConfig {
	/// Books that failed before are tried again (`POST /api/v2/content/reindex`); a scan's run
	/// leaves them alone until their file changes
	pub retry_failed: bool,
	/// The id the run's job gets, so `POST /reindex` can tell the caller which job it started
	#[serde(default)]
	pub job_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct ContentIndexOutput {
	/// Books read in this run
	pub indexed_books: u64,
	/// Of them, books with no text (a scan without a text layer)
	pub books_without_text: u64,
	pub failed_books: u64,
	/// Books left alone: already indexed, file unchanged
	pub unchanged_books: u64,
}

impl JobOutputExt for ContentIndexOutput {
	fn update(&mut self, updated: Self) {
		self.indexed_books += updated.indexed_books;
		self.books_without_text += updated.books_without_text;
		self.failed_books += updated.failed_books;
		self.unchanged_books += updated.unchanged_books;
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentIndexTask {
	pub media_id: String,
	pub path: String,
	pub kind: String,
	pub size: i64,
	pub mtime: i64,
}

pub struct ContentIndexJob {
	pub config: ContentIndexConfig,
}

fn file_stamp(path: &Path) -> std::io::Result<(i64, i64)> {
	let meta = std::fs::metadata(path)?;
	let mtime = meta
		.modified()?
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs() as i64)
		.unwrap_or_default();
	Ok((meta.len() as i64, mtime))
}

/// Reads one book's sections (`kind` is `epub` or `pdf`), for at most [`EXTRACT_TIMEOUT_SECS`]
pub async fn extract_book(
	config: &crate::config::StumpConfig,
	kind: &str,
	path: &str,
) -> Result<Extracted, ContentError> {
	extract_book_within(
		Duration::from_secs(EXTRACT_TIMEOUT_SECS),
		config,
		kind,
		path,
	)
	.await
}

async fn extract_book_within(
	limit: Duration,
	config: &crate::config::StumpConfig,
	kind: &str,
	path: &str,
) -> Result<Extracted, ContentError> {
	match kind {
		"pdf" => with_timeout(limit, extract_pdf(config, Path::new(path))).await,
		_ => {
			let path = path.to_string();
			// On a timeout the idle-priority thread is not stopped (a blocking read cannot be
			// cancelled): it is left to finish on its own and its result is dropped.
			with_timeout(
				limit,
				crate::postproc::run_low_priority(move || {
					// a damaged book must not take the worker down
					std::panic::catch_unwind(|| extract_epub(&path)).unwrap_or_else(|_| {
						Err(ContentError::Extract("the reader failed on this file".into()))
					})
				}),
			)
			.await
		},
	}
}

/// Gives the read `limit`; a book that takes longer is a failed book, and the run goes on
async fn with_timeout<F>(limit: Duration, read: F) -> Result<Extracted, ContentError>
where
	F: std::future::Future<Output = Result<Extracted, ContentError>>,
{
	tokio::time::timeout(limit, read)
		.await
		.unwrap_or_else(|_| Err(ContentError::Extract("timed out reading the book".into())))
}

/// Indexes one book: reads it and stores the result (or the reason it could not be read)
pub async fn index_book(
	conn: &DatabaseConnection,
	config: &crate::config::StumpConfig,
	task: &ContentIndexTask,
) -> Result<store::BookState, ContentError> {
	match extract_book(config, &task.kind, &task.path).await {
		Ok(extracted) => {
			store::save_book(
				conn,
				&task.media_id,
				&task.kind,
				task.size,
				task.mtime,
				extracted.sections,
			)
			.await?;
		},
		Err(ContentError::Db(error)) => return Err(ContentError::Db(error)),
		Err(error) => {
			store::save_failure(
				conn,
				&task.media_id,
				&task.kind,
				task.size,
				task.mtime,
				&error.to_string(),
			)
			.await?;
		},
	}
	store::book_state(conn, &task.media_id).await?.ok_or_else(|| {
		ContentError::Db(DbErr::RecordNotFound(format!(
			"content_books row of {}",
			task.media_id
		)))
	})
}

#[async_trait::async_trait]
impl JobLifecycle for ContentIndexJob {
	const NAME: &'static str = "content_index";

	type Output = ContentIndexOutput;
	type Task = ContentIndexTask;

	fn description(&self) -> Option<String> {
		Some("Index the text of the books".to_string())
	}

	async fn init(
		&mut self,
		ctx: &JobContext,
	) -> Result<WorkingState<Self::Output, Self::Task>, JobError> {
		let conn = ctx.conn();
		store::run_started(conn).await.map_err(to_job_error)?;
		store::remove_orphans(conn).await.map_err(to_job_error)?;
		let states: HashMap<String, store::BookState> = store::book_states(conn)
			.await
			.map_err(to_job_error)?
			.into_iter()
			.map(|s| (s.media_id.clone(), s))
			.collect();
		let books = media::Entity::find()
			.filter(media::Column::DeletedAt.is_null())
			.order_by_asc(media::Column::Path)
			.select_only()
			.columns([
				media::Column::Id,
				media::Column::Path,
				media::Column::Extension,
			])
			.into_tuple::<(String, String, String)>()
			.all(conn)
			.await?;
		let retry_failed = self.config.retry_failed;
		// thousands of synchronous stat calls: off the async runtime
		let (output, tasks, logs) = tokio::task::spawn_blocking(move || {
			let mut output = ContentIndexOutput::default();
			let mut tasks = std::collections::VecDeque::new();
			let mut logs = Vec::new();
			for (id, path, extension) in books {
				let Some(kind) = kind_of(&extension, &path) else {
					continue;
				};
				let (size, mtime) = match file_stamp(Path::new(&path)) {
					Ok(stamp) => stamp,
					Err(error) => {
						logs.push(
							JobExecuteLog::warn(&format!("{path}: {error}")).with_ctx(id),
						);
						continue;
					},
				};
				if !store::needs_indexing(states.get(&id), size, mtime, retry_failed) {
					output.unchanged_books += 1;
					continue;
				}
				tasks.push_back(ContentIndexTask {
					media_id: id,
					path,
					kind: kind.to_string(),
					size,
					mtime,
				});
			}
			(output, tasks, logs)
		})
		.await
		.map_err(|error| JobError::TaskFailed(error.to_string()))?;
		ctx.report_progress(JobProgress::msg(&format!(
			"{} books to index",
			tasks.len()
		)));
		Ok(WorkingState {
			output: Some(output),
			tasks,
			logs,
		})
	}

	async fn execute_task(
		&self,
		ctx: &JobContext,
		task: Self::Task,
	) -> Result<JobTaskOutput<Self>, JobError> {
		ctx.report_progress(JobProgress::msg(&task.path));
		let state = index_book(ctx.conn(), ctx.config(), &task)
			.await
			.map_err(to_job_error)?;
		let mut output = ContentIndexOutput::default();
		let mut logs = Vec::new();
		match state.status.as_str() {
			store::FAILED => {
				output.failed_books = 1;
				logs.push(
					JobExecuteLog::error(format!("{}: could not be read", task.path))
						.with_ctx(task.media_id.clone()),
				);
			},
			store::NO_TEXT => {
				output.indexed_books = 1;
				output.books_without_text = 1;
			},
			_ => output.indexed_books = 1,
		}
		Ok(JobTaskOutput {
			output,
			subtasks: vec![],
			logs,
		})
	}

	async fn finalize(
		&self,
		ctx: &JobContext,
		output: &Self::Output,
	) -> Result<(), JobError> {
		store::run_finished(ctx.conn(), output.failed_books == 0)
			.await
			.map_err(to_job_error)?;
		Ok(())
	}
}

fn to_job_error(error: ContentError) -> JobError {
	match error {
		ContentError::Db(error) => JobError::DbError(error),
		other => JobError::TaskFailed(other.to_string()),
	}
}

/// After a scan that added or changed books: enqueue a run (it skips what is unchanged)
pub async fn enqueue_after_scan(ctx: &JobContext) {
	if let Err(error) = ctx
		.enqueue(crate::job::StumpJob::content_index(ContentIndexConfig::default()))
		.await
	{
		tracing::error!(?error, "Failed to enqueue the content index run");
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn a_read_that_runs_over_the_limit_is_a_failure() {
		let slow = async {
			tokio::time::sleep(Duration::from_secs(30)).await;
			Ok(Extracted::default())
		};
		let error = with_timeout(Duration::from_millis(20), slow).await.unwrap_err();
		assert!(matches!(&error, ContentError::Extract(m) if m == "timed out reading the book"));
		// not a Db error, so index_book records it with save_failure and the run goes on
		assert!(!matches!(error, ContentError::Db(_)));
		let fast = with_timeout(Duration::from_secs(5), async { Ok(Extracted::default()) }).await;
		assert!(fast.is_ok());
	}
}
