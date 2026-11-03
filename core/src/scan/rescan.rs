//! A targeted rescan: only the given books are read again from their files (like a library
//! scan does for a changed file, keeping their locked fields), instead of walking their
//! libraries. The metadata batch API ends with one of these for the books whose files it
//! rewrote.

use std::{collections::HashMap, path::PathBuf};

use async_graphql::SimpleObject;
use models::entity::{library_config, media};
use sea_orm::{prelude::*, QuerySelect};
use serde::{Deserialize, Serialize};

use crate::{
	database::SQLITE_BIND_LIMIT,
	job::{
		error::JobError, JobContext, JobExecuteLog, JobLifecycle, JobOutputExt,
		JobProgress, JobTaskOutput, WorkingState,
	},
	scan::{
		options::BookVisitOperation,
		utils::media::{visit_and_update_media, MediaBuildOperation},
	},
};

/// The books of one library to read again
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RescanBooksTask {
	pub library_id: String,
	pub paths: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct RescanBooksOutput {
	/// Books read again from their files
	pub rescanned_books: u64,
	/// Books whose stored data changed
	pub updated_books: u64,
	/// Books that are no longer in Stump
	pub missing_books: u64,
}

impl JobOutputExt for RescanBooksOutput {
	fn update(&mut self, updated: Self) {
		self.rescanned_books += updated.rescanned_books;
		self.updated_books += updated.updated_books;
		self.missing_books += updated.missing_books;
	}
}

#[derive(Debug, Clone)]
pub struct RescanBooksJob {
	pub media_ids: Vec<String>,
	/// Why they are read again, for the job list
	pub reason: String,
}

#[async_trait::async_trait]
impl JobLifecycle for RescanBooksJob {
	const NAME: &'static str = "book_rescan";

	type Output = RescanBooksOutput;
	type Task = RescanBooksTask;

	fn description(&self) -> Option<String> {
		Some(format!(
			"Rescan of {} {} ({})",
			self.media_ids.len(),
			if self.media_ids.len() == 1 {
				"book"
			} else {
				"books"
			},
			self.reason
		))
	}

	async fn init(
		&mut self,
		ctx: &JobContext,
	) -> Result<WorkingState<Self::Output, Self::Task>, JobError> {
		let conn = ctx.conn();
		let mut by_library: HashMap<String, Vec<String>> = HashMap::new();
		let mut found = 0;
		for chunk in self.media_ids.chunks(SQLITE_BIND_LIMIT - 2) {
			let rows: Vec<(String, String)> = media::Entity::find()
				.select_only()
				.column(media::Column::LibraryId)
				.column(media::Column::Path)
				.filter(media::Column::Id.is_in(chunk.to_vec()))
				.filter(media::Column::DeletedAt.is_null())
				.into_tuple()
				.all(conn)
				.await?;
			found += rows.len();
			for (library_id, path) in rows {
				by_library.entry(library_id).or_default().push(path);
			}
		}
		let missing = self.media_ids.len().saturating_sub(found) as u64;
		let mut logs = vec![];
		if missing > 0 {
			logs.push(JobExecuteLog::warn(&format!(
				"{missing} of the books are no longer in Stump"
			)));
		}
		Ok(WorkingState {
			output: Some(RescanBooksOutput {
				missing_books: missing,
				..Default::default()
			}),
			tasks: by_library
				.into_iter()
				.map(|(library_id, paths)| RescanBooksTask { library_id, paths })
				.collect(),
			logs,
		})
	}

	async fn execute_task(
		&self,
		ctx: &JobContext,
		task: Self::Task,
	) -> Result<JobTaskOutput<Self>, JobError> {
		ctx.report_progress(JobProgress::msg(&format!(
			"Reading {} books again",
			task.paths.len()
		)));
		let Some(library_config) = library_config::Entity::find()
			.filter(library_config::Column::LibraryId.eq(task.library_id.clone()))
			.one(ctx.conn())
			.await?
		else {
			return Ok(JobTaskOutput {
				output: RescanBooksOutput {
					missing_books: task.paths.len() as u64,
					..Default::default()
				},
				subtasks: vec![],
				logs: vec![JobExecuteLog::warn("The books' library is gone")],
			});
		};
		let count = task.paths.len() as u64;
		let result = visit_and_update_media(
			MediaBuildOperation {
				series_id: None,
				library_config,
			},
			ctx,
			task.paths
				.into_iter()
				.map(|path| (PathBuf::from(path), BookVisitOperation::Rebuild))
				.collect(),
		)
		.await?;
		Ok(JobTaskOutput {
			output: RescanBooksOutput {
				rescanned_books: count,
				updated_books: result.updated_media,
				missing_books: 0,
			},
			subtasks: vec![],
			logs: result.logs,
		})
	}
}
