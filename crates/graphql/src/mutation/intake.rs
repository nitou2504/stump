//! The "Add books" intake: upload into a per-user inbox, review, and file into the library. What
//! a user may do depends on whether they file books themselves (the server owner, or anyone who
//! manages libraries: an *approver*) or send them to an approver (see `stump_core::intake::approval`).

use std::path::Path;

use async_graphql::{Context, InputObject, Object, Result, Upload, ID};
use models::{
	entity::{
		intake_item, library,
		library_intake_settings::{self, typed_series_kind},
		user::AuthUser,
		user_intake_limits,
	},
	shared::{
		enums::UserPermission,
		intake::{IntakeReviewInput, IntakeState, IntakeUploadMode},
	},
};
use sea_orm::{prelude::*, ActiveValue::Set, IntoActiveModel};
use stump_core::intake::{
	self as core_intake,
	approval::{self, can_file_directly},
	extension_of,
	staging::{
		books_in_zip, create_items, group_of, spawn_analysis, Staged, StagedSource,
	},
	Reviewer,
};

use crate::{
	data::{AuthContext, CoreContext},
	guard::{OptionalFeature, OptionalFeatureGuard, PermissionGuard},
	mutation::upload::{enforce_max_size, validate_book_file},
	object::intake::{IntakeItem, UserIntakeLimits},
};

#[derive(Default)]
pub struct IntakeMutation;

#[derive(Debug, InputObject)]
pub struct LibraryIntakeSettingsInput {
	pub manga_dir: String,
	pub light_novel_dir: String,
	pub online_lookup: bool,
}

/// The limits an admin gives a user of their own. Left out (null), a limit follows the server's
#[derive(Debug, InputObject)]
pub struct UserIntakeLimitsInput {
	/// The biggest file, in MB
	pub max_file_mb: Option<u32>,
	/// The most files in one upload request
	pub max_files_per_request: Option<u32>,
	/// The most MB of their books that may wait in their inbox
	pub quota_mb: Option<u32>,
	/// The most inbox items that are not filed, discarded or sent back
	pub max_items: Option<u32>,
}

async fn own_item(
	ctx: &Context<'_>,
	user: &AuthUser,
	id: &str,
) -> Result<intake_item::Model> {
	let conn = ctx.data::<CoreContext>()?.conn.as_ref();
	intake_item::Entity::find_for_user(user)
		.filter(intake_item::Column::Id.eq(id))
		.one(conn)
		.await?
		.ok_or_else(|| "Inbox item not found".into())
}

/// An item the viewer may change: their own, or (for an approver) somebody else's that waits for
/// approval in a library they can use. What isn't either is not found.
async fn item_to_change(
	ctx: &Context<'_>,
	user: &AuthUser,
	id: &str,
) -> Result<(intake_item::Model, Reviewer)> {
	let conn = ctx.data::<CoreContext>()?.conn.as_ref();
	let approver = can_file_directly(user);
	if let Some(item) = intake_item::Entity::find_for_user(user)
		.filter(intake_item::Column::Id.eq(id))
		.one(conn)
		.await?
	{
		return Ok((
			item,
			Reviewer {
				approver,
				owns_item: true,
			},
		));
	}
	if approver {
		if let Ok(item) = approval::item_for_approver(conn, user, id).await {
			if item.state == IntakeState::PendingApproval {
				return Ok((
					item,
					Reviewer {
						approver: true,
						owns_item: false,
					},
				));
			}
		}
	}
	Err("Inbox item not found".into())
}

#[Object]
impl IntakeMutation {
	/// Uploads books into the user's inbox for a library, for somebody who files books
	/// themselves. Each book (or each folder / .zip of volumes) becomes an item that is analysed
	/// right away; poll `intakeItems`. Everybody else uploads through `POST /api/v2/intake/upload`,
	/// which streams the files to disk within the limits.
	#[graphql(
		guard = "OptionalFeatureGuard::new(OptionalFeature::Upload).and(PermissionGuard::all(&[UserPermission::AddBooks, UserPermission::ManageLibrary]))"
	)]
	async fn intake_upload(
		&self,
		ctx: &Context<'_>,
		library_id: ID,
		files: Vec<Upload>,
		#[graphql(desc = "The path of each file inside an uploaded folder (same order)")]
		relative_paths: Option<Vec<String>>,
		#[graphql(default)] mode: IntakeUploadMode,
	) -> Result<Vec<IntakeItem>> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();
		let library = library::Entity::find_for_user(user)
			.filter(library::Column::Id.eq(library_id.to_string()))
			.one(conn)
			.await?
			.ok_or("Library not found")?;
		if files.is_empty() {
			return Err("No files were uploaded".into());
		}

		let max_size = core.config.max_file_upload_size;
		let mut staged: Vec<Staged> = vec![];
		for (i, upload) in files.iter().enumerate() {
			let mut value = upload.value(ctx)?;
			enforce_max_size(&value, max_size)?;
			let relative = relative_paths
				.as_ref()
				.and_then(|p| p.get(i).cloned())
				.unwrap_or_else(|| value.filename.clone());
			let ext = extension_of(&value.filename);
			let size = value.size().unwrap_or(0) as i64;
			if ext == "zip" {
				let zip_name = Path::new(&value.filename)
					.file_stem()
					.map(|s| s.to_string_lossy().into_owned())
					.unwrap_or_else(|| "Series".to_string());
				let books = tokio::task::block_in_place(|| {
					books_in_zip(&mut value.content, max_size as u64)
				})
				.map_err(|error| async_graphql::Error::new(error.to_string()))?;
				if books.is_empty() {
					return Err(format!("{} contains no books", value.filename).into());
				}
				let single = books.len() == 1;
				for (name, book, size) in books {
					staged.push(Staged {
						group: (!single).then(|| zip_name.clone()),
						original_name: format!("{zip_name}/{name}"),
						format: extension_of(&name),
						size: size as i64,
						source: StagedSource::Open(book),
					});
				}
				continue;
			}
			validate_book_file(&mut value)?;
			staged.push(Staged {
				group: group_of(&relative),
				original_name: relative,
				format: ext,
				size,
				source: StagedSource::Open(value.content),
			});
		}

		let items = create_items(core, user, &library, mode, staged).await?;
		spawn_analysis(
			core.clone(),
			items.iter().map(|i| i.id.clone()).collect(),
			None,
		);
		Ok(items.into_iter().map(IntakeItem::from).collect())
	}

	/// Saves a review of an item (omitted fields keep their value). The uploader reviews their
	/// own; an approver can also review an item that waits for approval. Somebody who needs
	/// approval can't choose the folder, the file names or the kind of series, their texts are
	/// plain and of a sensible length, and the age rating can't go down.
	#[graphql(
		guard = "PermissionGuard::new(&[UserPermission::AddBooks, UserPermission::ManageLibrary])"
	)]
	async fn intake_review(
		&self,
		ctx: &Context<'_>,
		id: ID,
		input: IntakeReviewInput,
	) -> Result<IntakeItem> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let (item, reviewer) = item_to_change(ctx, user, &id).await?;
		Ok(core_intake::apply_review(core, item, input, reviewer)
			.await?
			.into())
	}

	/// Analyses an item again (e.g. after a failure, or to look it up online)
	#[graphql(
		guard = "PermissionGuard::new(&[UserPermission::AddBooks, UserPermission::ManageLibrary])"
	)]
	async fn intake_reanalyse(
		&self,
		ctx: &Context<'_>,
		id: ID,
		#[graphql(desc = "Override the library's online lookup setting")] online: Option<
			bool,
		>,
	) -> Result<IntakeItem> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let (item, reviewer) = item_to_change(ctx, user, &id).await?;
		if !core_intake::can_reanalyse(item.state, reviewer) {
			return Err(
				core_intake::closed_for_changes(item.state, "analysed again").into(),
			);
		}
		let mut active = item.into_active_model();
		active.state = Set(IntakeState::Analysing);
		active.error = Set(None);
		let item = active.update(core.conn.as_ref()).await?;
		spawn_analysis(core.clone(), vec![item.id.clone()], online);
		Ok(item.into())
	}

	/// Moves an item to another library (one the user can use) and analyses it again there: the
	/// proposal, the folder and the file names are made for the library the item is in. An
	/// approver can move an item that waits for approval, to a library its uploader can use too
	#[graphql(
		guard = "PermissionGuard::new(&[UserPermission::AddBooks, UserPermission::ManageLibrary])"
	)]
	async fn intake_change_library(
		&self,
		ctx: &Context<'_>,
		id: ID,
		library_id: ID,
	) -> Result<IntakeItem> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();
		let (item, reviewer) = item_to_change(ctx, user, &id).await?;
		let library = library::Entity::find_for_user(user)
			.filter(library::Column::Id.eq(library_id.to_string()))
			.one(conn)
			.await?
			.ok_or("Library not found")?;
		if !reviewer.owns_item {
			// the uploader sees the item in its library: it must be one they can use
			let uploader = approval::load_auth_user(conn, &item.user_id)
				.await?
				.ok_or("Library not found")?;
			library::Entity::find_for_user(&uploader)
				.filter(library::Column::Id.eq(library.id.clone()))
				.one(conn)
				.await?
				.ok_or("Library not found")?;
		}
		let item = core_intake::change_library(core, item, &library.id, reviewer).await?;
		spawn_analysis(core.clone(), vec![item.id.clone()], None);
		Ok(item.into())
	}

	/// Files ready items into their library, for somebody who files books themselves. Items that
	/// fail keep their state and get an `error`; the others become `FILED`.
	#[graphql(
		guard = "PermissionGuard::all(&[UserPermission::AddBooks, UserPermission::ManageLibrary])"
	)]
	async fn intake_accept(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
	) -> Result<Vec<IntakeItem>> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let mut out = vec![];
		for id in ids {
			let item = own_item(ctx, user, &id).await?;
			match approval::accept(core, user, item.clone()).await {
				Ok(filed) => out.push(filed.into()),
				Err(error) => {
					// the item is back where it was, with the reason; one that could not even
					// be claimed is told as it is
					let current = own_item(ctx, user, &id).await.unwrap_or(item);
					let mut active = current.into_active_model();
					active.error = Set(Some(error.to_string()));
					out.push(active.update(core.conn.as_ref()).await?.into());
				},
			}
		}
		Ok(out)
	}

	/// Sends ready items to be approved, for somebody who may not file books themselves. They
	/// wait in the approval queue (`PENDING_APPROVAL`) until an approver files or sends them back
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn intake_submit(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
	) -> Result<Vec<IntakeItem>> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
		Ok(approval::submit(core, user, &ids)
			.await?
			.into_iter()
			.map(IntakeItem::from)
			.collect())
	}

	/// Takes items back from the approval queue, to change them
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn intake_withdraw(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
	) -> Result<Vec<IntakeItem>> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
		Ok(approval::withdraw(core, user, &ids)
			.await?
			.into_iter()
			.map(IntakeItem::from)
			.collect())
	}

	/// Approves items waiting in the approval queue of libraries the approver can use. Each is
	/// filed in the background (`FILING`, then `FILED`); an item already being filed comes back
	/// as it is
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn intake_approve(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
	) -> Result<Vec<IntakeItem>> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
		Ok(approval::approve(core, user, &ids)
			.await?
			.into_iter()
			.map(IntakeItem::from)
			.collect())
	}

	/// Sends an item of the approval queue back to its uploader with a note (`reviewNote`): they
	/// can change it and send it again
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn intake_reject(
		&self,
		ctx: &Context<'_>,
		id: ID,
		note: String,
	) -> Result<IntakeItem> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		Ok(approval::reject(core, user, &id, &note).await?.into())
	}

	/// Throws items away with their staged files. Returns how many were discarded.
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn intake_discard(&self, ctx: &Context<'_>, ids: Vec<ID>) -> Result<i32> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let mut count = 0;
		for id in ids {
			let item = own_item(ctx, user, &id).await?;
			core_intake::discard(core, item).await?;
			count += 1;
		}
		Ok(count)
	}

	/// Gives a user limits of their own for what they may upload (the ones left out follow the
	/// server's), or takes them away with all fields null
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageUsers)")]
	async fn update_user_intake_limits(
		&self,
		ctx: &Context<'_>,
		user_id: ID,
		input: UserIntakeLimitsInput,
	) -> Result<UserIntakeLimits> {
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();
		let known = models::entity::user::Entity::find_by_id(user_id.to_string())
			.filter(models::entity::user::Column::DeletedAt.is_null())
			.one(conn)
			.await?;
		if known.is_none() {
			return Err("User not found".into());
		}
		let overrides = user_intake_limits::UserIntakeLimitOverrides {
			max_file_mb: input.max_file_mb.map(u64::from),
			max_files_per_request: input.max_files_per_request.map(u64::from),
			quota_mb: input.quota_mb.map(u64::from),
			max_items: input.max_items.map(u64::from),
		};
		let row = user_intake_limits::ActiveModel {
			user_id: Set(user_id.to_string()),
			limits: Set(overrides),
			updated_at: Set(chrono::Utc::now().into()),
		};
		if user_intake_limits::Entity::find_by_id(user_id.to_string())
			.one(conn)
			.await?
			.is_some()
		{
			row.update(conn).await?;
		} else {
			row.insert(conn).await?;
		}
		crate::query::intake::user_intake_limits_of(core, &user_id).await
	}

	/// Changes how the intake files books into a library
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn update_library_intake_settings(
		&self,
		ctx: &Context<'_>,
		library_id: ID,
		input: LibraryIntakeSettingsInput,
	) -> Result<library_intake_settings::Model> {
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let library = library::Entity::find_for_user(user)
			.filter(library::Column::Id.eq(library_id.to_string()))
			.one(conn)
			.await?
			.ok_or("Library not found")?;
		// a manga or light novel library may keep its series folders in its root: an empty folder
		let typed =
			typed_series_kind(core_intake::library_type_of(conn, &library.id).await?)
				.is_some();
		let plain = |dir: &str| {
			core_intake::naming::is_plain_component(dir)
				|| (typed && dir.trim().is_empty())
		};
		if !plain(&input.manga_dir) || !plain(&input.light_novel_dir) {
			let message = if typed {
				"Series folders must be empty or a single folder name"
			} else {
				"Series folders must be a single folder name"
			};
			return Err(message.into());
		}
		let model = library_intake_settings::Model {
			library_id: library.id.clone(),
			manga_dir: input.manga_dir.trim().to_string(),
			light_novel_dir: input.light_novel_dir.trim().to_string(),
			online_lookup: input.online_lookup,
		};
		let exists = library_intake_settings::Entity::find_by_id(library.id.clone())
			.one(conn)
			.await?
			.is_some();
		let active = library_intake_settings::ActiveModel {
			library_id: Set(model.library_id.clone()),
			manga_dir: Set(model.manga_dir.clone()),
			light_novel_dir: Set(model.light_novel_dir.clone()),
			online_lookup: Set(model.online_lookup),
		};
		let saved = if exists {
			active.update(conn).await?
		} else {
			active.insert(conn).await?
		};
		core_intake::library_index::invalidate_index(&library.id);
		Ok(saved)
	}
}
