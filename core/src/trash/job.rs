//! The automatic purge of *Deleted books*: the books and held files kept for the retention
//! period are deleted permanently (see [`super`]), a few at a time, and what happened is
//! recorded for the *Deleted books* page

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

use super::{
	expired_book_ids, held, purge_books, record_purge, retention_days, PurgeOutcome,
};

/// Books purged per task
const BOOKS_PER_TASK: usize = 10;

/// Errors kept in the job's output and the page's summary
const KEPT_ERRORS: usize = 20;

#[derive(Debug, Clone, Default, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct TrashPurgeOutput {
	/// Books deleted permanently (their file and their data)
	pub purged_books: u64,
	/// Books whose file was already gone: only their data was deleted
	pub missing_book_files: u64,
	/// Books kept in Deleted books because their file or data couldn't be deleted
	pub kept_books: u64,
	/// Held files (originals replaced by post-processing) deleted
	#[serde(default)]
	pub purged_held_files: u64,
	/// The disk space given back (deleted book files and held files)
	#[serde(default, alias = "quarantinedBytes")]
	pub freed_bytes: u64,
	/// Why books were kept (the first few)
	pub errors: Vec<TrashPurgeError>,
}

impl JobOutputExt for TrashPurgeOutput {
	fn update(&mut self, updated: Self) {
		self.purged_books += updated.purged_books;
		self.missing_book_files += updated.missing_book_files;
		self.kept_books += updated.kept_books;
		self.purged_held_files += updated.purged_held_files;
		self.freed_bytes += updated.freed_bytes;
		let room = KEPT_ERRORS.saturating_sub(self.errors.len());
		self.errors.extend(updated.errors.into_iter().take(room));
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrashPurgeTask {
	Books { media_ids: Vec<String> },
	HeldFiles { ids: Vec<i32> },
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
		let now = Utc::now();
		let ids = expired_book_ids(conn, retention, now).await?;
		let held_ids = held::expired_held_ids(conn, retention, now).await?;
		let mut logs = Vec::new();
		if retention <= 0 {
			logs.push(JobExecuteLog::new(
				"Deleted books are kept until deleted by hand".to_string(),
				LogLevel::Info,
			));
		}
		let mut tasks: std::collections::VecDeque<TrashPurgeTask> = ids
			.chunks(BOOKS_PER_TASK)
			.map(|chunk| TrashPurgeTask::Books {
				media_ids: chunk.to_vec(),
			})
			.collect();
		tasks.extend(held_ids.chunks(BOOKS_PER_TASK).map(|chunk| {
			TrashPurgeTask::HeldFiles {
				ids: chunk.to_vec(),
			}
		}));
		Ok(WorkingState {
			output: Some(TrashPurgeOutput::default()),
			tasks,
			logs,
		})
	}

	async fn execute_task(
		&self,
		ctx: &JobContext,
		task: Self::Task,
	) -> Result<JobTaskOutput<Self>, JobError> {
		let media_ids = match task {
			TrashPurgeTask::Books { media_ids } => media_ids,
			TrashPurgeTask::HeldFiles { ids } => {
				ctx.report_progress(JobProgress::msg("Deleting expired held files"));
				let purge = held::purge_held(ctx.conn(), ctx.config(), &ids).await?;
				let mut logs = vec![JobExecuteLog::new(
					format!(
						"Deleted {} held {} ({:.1} MB)",
						purge.deleted,
						if purge.deleted == 1 { "file" } else { "files" },
						purge.bytes as f64 / 1e6
					),
					LogLevel::Info,
				)];
				logs.extend(purge.kept.iter().map(|(id, title, why)| {
					JobExecuteLog::error(format!(
						"Kept the held file of “{title}”: {why}"
					))
					.with_ctx(id.to_string())
				}));
				return Ok(JobTaskOutput {
					output: TrashPurgeOutput {
						purged_held_files: purge.deleted,
						freed_bytes: purge.bytes,
						..Default::default()
					},
					subtasks: vec![],
					logs,
				});
			},
		};
		ctx.report_progress(JobProgress::msg("Deleting expired books permanently"));
		let report = purge_books(ctx.conn(), ctx.config(), &media_ids, None).await?;
		let logs = report
			.items
			.iter()
			.map(|item| match &item.outcome {
				PurgeOutcome::Deleted { bytes } => JobExecuteLog::new(
					format!(
						"Deleted “{}” permanently ({:.1} MB)",
						item.title,
						*bytes as f64 / 1e6
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
				purged_held_files: 0,
				freed_bytes: report.freed_bytes(),
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
				originals: output.purged_held_files,
				errors: output.errors.clone(),
			},
		)
		.await?;
		Ok(())
	}
}
