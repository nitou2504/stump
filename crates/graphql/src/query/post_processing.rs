use async_graphql::{Context, Object, Result, SimpleObject, ID};
use chrono::{DateTime, Utc};
use models::{
	entity::{library, library_config, post_processing_task as task},
	shared::{
		enums::UserPermission,
		post_processing::{PostProcessKind, PostProcessStatus, PostProcessingConfig},
	},
};
use sea_orm::{prelude::*, QueryOrder, QuerySelect};
use stump_core::postproc::pdf_queue::{worker_status, PdfWorkerStatus};

use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
};

/// A library's post-processing settings and where its work stands
#[derive(Debug, Clone, SimpleObject)]
pub struct LibraryPostProcessing {
	pub config: PostProcessingConfig,
	/// Where replaced originals go (nothing is replaced while this is unset)
	pub quarantine_dir: Option<String>,
	/// The PDF queue shared with the PDF worker container, as Stump sees it
	pub pdf_queue_dir: String,
	/// The PDF worker container, from its heartbeat in the queue folder
	pub pdf_worker: PdfWorker,
	/// Task counts by kind and status
	pub counts: Vec<PostProcessingCount>,
	/// Bytes saved by the replaced files
	pub bytes_saved: i64,
}

/// The PDF worker container (OCR and size reduction of scanned PDFs)
#[derive(Debug, Clone, SimpleObject)]
pub struct PdfWorker {
	/// Its heartbeat is recent: the container is running
	pub alive: bool,
	/// `idle` or `busy`
	pub state: Option<String>,
	/// The worker image's revision
	pub version: Option<String>,
	/// When its heartbeat was last refreshed
	pub last_seen: Option<DateTime<Utc>>,
	/// The task ids it is working on
	pub current: Option<String>,
}

impl From<PdfWorkerStatus> for PdfWorker {
	fn from(status: PdfWorkerStatus) -> Self {
		Self {
			alive: status.alive,
			state: status.state,
			version: status.version,
			last_seen: status.last_seen,
			current: status.current,
		}
	}
}

#[derive(Debug, Clone, SimpleObject)]
pub struct PostProcessingCount {
	pub kind: PostProcessKind,
	pub status: PostProcessStatus,
	pub count: i64,
}

pub(crate) async fn library_with_config(
	ctx: &Context<'_>,
	library_id: &str,
) -> Result<(library::Model, library_config::Model)> {
	let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
	let conn = ctx.data::<CoreContext>()?.conn.as_ref();
	let library = library::Entity::find_for_user(user)
		.filter(library::Column::Id.eq(library_id))
		.one(conn)
		.await?
		.ok_or("Library not found")?;
	let config = library_config::Entity::find_by_id(library.config_id)
		.one(conn)
		.await?
		.ok_or("Library config not found")?;
	Ok((library, config))
}

pub(crate) async fn summary(
	ctx: &Context<'_>,
	library_id: &str,
	config: PostProcessingConfig,
) -> Result<LibraryPostProcessing> {
	let core = ctx.data::<CoreContext>()?;
	let conn = core.conn.as_ref();
	let rows = task::Entity::find()
		.filter(task::Column::LibraryId.eq(library_id))
		.all(conn)
		.await?;
	let mut counts: Vec<PostProcessingCount> = Vec::new();
	let mut bytes_saved = 0;
	for row in &rows {
		match counts
			.iter_mut()
			.find(|c| c.kind == row.kind && c.status == row.status)
		{
			Some(c) => c.count += 1,
			None => counts.push(PostProcessingCount {
				kind: row.kind,
				status: row.status,
				count: 1,
			}),
		}
		if row.status == PostProcessStatus::Done {
			if let (Some(before), Some(after)) = (row.bytes_before, row.bytes_after) {
				bytes_saved += (before - after).max(0);
			}
		}
	}
	let pdf_queue = core.config.pdf_worker_directory();
	Ok(LibraryPostProcessing {
		config,
		quarantine_dir: core
			.config
			.quarantine_directory()
			.map(|p| p.to_string_lossy().to_string()),
		pdf_worker: worker_status(&pdf_queue, Utc::now()).into(),
		pdf_queue_dir: pdf_queue.to_string_lossy().to_string(),
		counts,
		bytes_saved,
	})
}

#[derive(Default)]
pub struct PostProcessingQuery;

#[Object]
impl PostProcessingQuery {
	/// A library's post-processing settings and task counts
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn library_post_processing(
		&self,
		ctx: &Context<'_>,
		library_id: ID,
	) -> Result<LibraryPostProcessing> {
		let (library, config) = library_with_config(ctx, &library_id).await?;
		summary(ctx, &library.id, config.post_processing()).await
	}

	/// Post-processing tasks, most recently updated first
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn post_processing_tasks(
		&self,
		ctx: &Context<'_>,
		library_id: Option<ID>,
		kind: Option<PostProcessKind>,
		statuses: Option<Vec<PostProcessStatus>>,
		#[graphql(default = 100)] limit: u64,
	) -> Result<Vec<task::Model>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let library_ids: Vec<String> = library::Entity::find_for_user(user)
			.select_only()
			.column(library::Column::Id)
			.into_tuple()
			.all(conn)
			.await?;
		let mut query =
			task::Entity::find().filter(task::Column::LibraryId.is_in(library_ids));
		if let Some(id) = library_id {
			query = query.filter(task::Column::LibraryId.eq(id.to_string()));
		}
		if let Some(kind) = kind {
			query = query.filter(task::Column::Kind.eq(kind));
		}
		if let Some(statuses) = statuses.filter(|s| !s.is_empty()) {
			query = query.filter(task::Column::Status.is_in(statuses));
		}
		Ok(query
			.order_by_desc(task::Column::UpdatedAt)
			.limit(limit.min(1000))
			.all(conn)
			.await?)
	}
}
