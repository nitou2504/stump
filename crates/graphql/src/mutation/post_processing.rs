use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use models::{
	entity::library_config,
	shared::{enums::UserPermission, post_processing::PostProcessingConfigInput},
};
use sea_orm::{prelude::*, ActiveValue::Set};
use stump_core::{
	job::StumpJob,
	postproc::{PostProcessJobConfig, PostProcessScope},
};

use crate::{
	data::CoreContext,
	guard::PermissionGuard,
	query::post_processing::{library_with_config, summary, LibraryPostProcessing},
};

#[derive(Default)]
pub struct PostProcessingMutation;

#[Object]
impl PostProcessingMutation {
	/// Change a library's post-processing settings. Turning a step on for new books processes
	/// the books scans add from now on
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn update_library_post_processing(
		&self,
		ctx: &Context<'_>,
		library_id: ID,
		input: PostProcessingConfigInput,
	) -> Result<LibraryPostProcessing> {
		let (library, config) = library_with_config(ctx, &library_id).await?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let updated = config.post_processing().apply(input, Utc::now());
		library_config::ActiveModel {
			id: Set(config.id),
			post_processing: Set(Some(updated.clone())),
			..Default::default()
		}
		.update(conn)
		.await?;
		summary(ctx, &library.id, updated).await
	}

	/// Run post-processing on the whole library now: the 16-grey step (or its dry-run
	/// estimate) and/or the PDF steps (queued for the host runner)
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn run_post_processing(
		&self,
		ctx: &Context<'_>,
		library_id: ID,
		#[graphql(default = false)] grey16: bool,
		#[graphql(default = false)] pdf_ocr: bool,
		#[graphql(default = false)] pdf_shrink: bool,
		#[graphql(default = false)] dry_run: bool,
	) -> Result<bool> {
		if !grey16 && !pdf_ocr && !pdf_shrink {
			return Err("Choose at least one step".into());
		}
		if dry_run && (pdf_ocr || pdf_shrink) {
			return Err("Only the 16-grey step has a dry run".into());
		}
		let (library, _) = library_with_config(ctx, &library_id).await?;
		let core = ctx.data::<CoreContext>()?;
		core.enqueue(StumpJob::post_process(PostProcessJobConfig {
			library_id: library.id,
			scope: PostProcessScope::All,
			grey16,
			pdf_ocr,
			pdf_shrink,
			dry_run,
		}))
		.await?;
		Ok(true)
	}
}
