//! The automatic purge of *Deleted books*: the books kept for the retention period are
//! deleted permanently (see [`super`]), a few at a time, and what happened is recorded for the
//! *Deleted books* page

use async_graphql::SimpleObject;
use chrono::Utc;
use models::shared::{
	enums::LogLevel,
	trash::{TrashPurgeError, TrashPurgeSummary},
};
use serde::{Deserialize, Serialize};

use crate::job::{
	error::JobError, JobContext, JobExecuteLog, JobLifecycle, JobOutputExt, JobProgress,
	JobTaskOutput, WorkingState,
};

use super::{expired_book_ids, purge_books, record_purge, retention_days, PurgeOutcome};

/// Books purged per task
const BOOKS_PER_TASK: usize = 10;

/// Errors kept in the job's output and the page's summary
const KEPT_ERRORS: usize = 20;

#[derive(Debug, Clone, Default, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct TrashPurgeOutput {
	/// Books whose file went to the quarantine folder and whose data was deleted
	pub purged_books: u64,
	/// Books whose file was already gone: only their data was deleted
	pub missing_book_files: u64,
	/// Books kept in Deleted books because their file couldn't be moved
	pub kept_books: u64,
	/// The size of the files moved to the quarantine folder
	pub quarantined_bytes: u64,
	/// Why books were kept (the first few)
	pub errors: Vec<TrashPurgeError>,
}

impl JobOutputExt for TrashPurgeOutput {
	fn update(&mut self, updated: Self) {
		self.purged_books += updated.purged_books;
		self.missing_book_files += updated.missing_book_files;
		self.kept_books += updated.kept_books;
		self.quarantined_bytes += updated.quarantined_bytes;
		let room = KEPT_ERRORS.saturating_sub(self.errors.len());
		self.errors.extend(updated.errors.into_iter().take(room));
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashPurgeTask {
	pub media_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TrashPurgeJob;

#[async_trait::async_trait]
impl JobLifecycle for TrashPurgeJob {
	const NAME: &'static str = "trash_purge";

	type Output = TrashPurgeOutput;
	type Task = TrashPurgeTask;

	fn description(&self) -> Option<String> {
		Some("Delete the expired books in Deleted books permanently".to_string())
	}

	async fn init(
		&mut self,
		ctx: &JobContext,
	) -> Result<WorkingState<Self::Output, Self::Task>, JobError> {
		let conn = ctx.conn();
		let retention = retention_days(conn).await?;
		// read again: books may have been restored since the job was queued
		let ids = expired_book_ids(conn, retention, Utc::now()).await?;
		let mut logs = Vec::new();
		if retention <= 0 {
			logs.push(JobExecuteLog::new(
				"Deleted books are kept until deleted by hand".to_string(),
				LogLevel::Info,
			));
		}
		Ok(WorkingState {
			output: Some(TrashPurgeOutput::default()),
			tasks: ids
				.chunks(BOOKS_PER_TASK)
				.map(|chunk| TrashPurgeTask {
					media_ids: chunk.to_vec(),
				})
				.collect(),
			logs,
		})
	}

	async fn execute_task(
		&self,
		ctx: &JobContext,
		task: Self::Task,
	) -> Result<JobTaskOutput<Self>, JobError> {
		ctx.report_progress(JobProgress::msg("Deleting expired books permanently"));
		let report = purge_books(ctx.conn(), ctx.config(), &task.media_ids, None).await?;
		let logs = report
			.items
			.iter()
			.map(|item| match &item.outcome {
				PurgeOutcome::Quarantined { path, .. } => JobExecuteLog::new(
					format!(
						"Deleted “{}” permanently: its file is in {}",
						item.title,
						path.display()
					),
					LogLevel::Info,
				),
				PurgeOutcome::FileMissing => JobExecuteLog::new(
					format!(
						"Deleted “{}” permanently: its file was already gone",
						item.title
					),
					LogLevel::Info,
				),
				PurgeOutcome::Kept(reason) => JobExecuteLog::error(format!(
					"Kept “{}” in Deleted books: {reason}",
					item.title
				))
				.with_ctx(item.media_id.clone()),
			})
			.collect();
		Ok(JobTaskOutput {
			output: TrashPurgeOutput {
				purged_books: report.purged(),
				missing_book_files: report.missing_files(),
				kept_books: report.kept(),
				quarantined_bytes: report.quarantined_bytes(),
				errors: report.errors().into_iter().take(KEPT_ERRORS).collect(),
			},
			subtasks: vec![],
			logs,
		})
	}

	async fn finalize(
		&self,
		ctx: &JobContext,
		output: &Self::Output,
	) -> Result<(), JobError> {
		record_purge(
			ctx.conn(),
			TrashPurgeSummary {
				finished_at: Utc::now(),
				job_id: Some(ctx.job_id.clone()),
				purged: output.purged_books,
				missing_files: output.missing_book_files,
				failed: output.kept_books,
				errors: output.errors.clone(),
			},
		)
		.await?;
		Ok(())
	}
}
