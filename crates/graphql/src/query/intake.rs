use std::path::Path;

use async_graphql::{Context, Object, Result, ID};
use models::{
	entity::{intake_item, library, library_intake_settings, user_intake_limits},
	shared::{
		enums::UserPermission,
		intake::{IntakeSeriesKind, IntakeState},
	},
};
use sea_orm::{prelude::*, QueryOrder, QuerySelect};
use stump_core::{
	intake::{
		self as core_intake,
		approval::{self, can_file_directly},
		library_index::library_index,
		limits::{limits_for, IntakeLimits},
		scope, upload,
	},
	Ctx,
};

use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
	object::{
		intake::{
			IntakeFolder, IntakeItem, IntakeItemPage, IntakeLimitValues,
			IntakeUploadLimits, IntakeVocabularyEntry, RecentlyAddedBook,
			UserIntakeLimits,
		},
		library::Library,
	},
	pagination::{list_limit, MAX_LIST_SIZE},
};

/// The most items `intakeItems` returns: a user's open items are capped by the server (100 by default),
/// and the web app's inbox page asks for every field of each, so the limit also keeps that query under
/// the limit of what a query may cost
const INTAKE_ITEMS_LIMIT: u64 = 100;

#[derive(Default)]
pub struct IntakeQuery;

async fn accessible_library(
	ctx: &Context<'_>,
	library_id: &str,
) -> Result<library::Model> {
	let AuthContext { user, .. } = ctx.data()?;
	let conn = ctx.data::<CoreContext>()?.conn.as_ref();
	library::Entity::find_for_user(user)
		.filter(library::Column::Id.eq(library_id))
		.one(conn)
		.await?
		.ok_or_else(|| "Library not found".into())
}

/// The limits of a user: the ones an admin gave them, and what applies to them now
pub(crate) async fn user_intake_limits_of(
	core: &Ctx,
	user_id: &str,
) -> Result<UserIntakeLimits> {
	let conn = core.conn.as_ref();
	let own = user_intake_limits::Entity::find_by_id(user_id.to_string())
		.one(conn)
		.await?;
	let own = own.map(|row| row.limits).unwrap_or_default();
	let effective = IntakeLimits::new(&core.config, Some(&own));
	let clamp = |value: Option<u64>| value.map(|v| i64::try_from(v).unwrap_or(i64::MAX));
	Ok(UserIntakeLimits {
		user_id: user_id.to_string().into(),
		max_file_mb: clamp(own.max_file_mb),
		max_files_per_request: clamp(own.max_files_per_request),
		quota_mb: clamp(own.quota_mb),
		max_items: clamp(own.max_items),
		effective: IntakeLimitValues::from(effective),
	})
}

fn matches_search(value: &str, search: &Option<String>) -> bool {
	match search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
		None => true,
		Some(search) => {
			let value = core_intake::library_index::norm(value);
			core_intake::library_index::norm(search)
				.split(' ')
				.all(|word| value.split(' ').any(|v| v.starts_with(word)))
		},
	}
}

#[Object]
impl IntakeQuery {
	/// The items of the user's "Add books" inbox, newest first (the latest 100 at most). By
	/// default everything but discarded items.
	#[graphql(
		guard = "PermissionGuard::one(UserPermission::AddBooks)",
		complexity = "crate::complexity::list_cost(Some(INTAKE_ITEMS_LIMIT), child_complexity)"
	)]
	async fn intake_items(
		&self,
		ctx: &Context<'_>,
		states: Option<Vec<IntakeState>>,
	) -> Result<Vec<IntakeItem>> {
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let mut query = intake_item::Entity::find_for_user(user)
			.order_by_desc(intake_item::Column::CreatedAt)
			.limit(INTAKE_ITEMS_LIMIT);
		query = match states {
			Some(states) if !states.is_empty() => {
				query.filter(intake_item::Column::State.is_in(states))
			},
			_ => query.filter(intake_item::Column::State.ne(IntakeState::Discarded)),
		};
		Ok(query
			.all(conn)
			.await?
			.into_iter()
			.map(IntakeItem::from)
			.collect())
	}

	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn intake_item(&self, ctx: &Context<'_>, id: ID) -> Result<Option<IntakeItem>> {
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(intake_item::Entity::find_for_user(user)
			.filter(intake_item::Column::Id.eq(id.to_string()))
			.one(conn)
			.await?
			.map(IntakeItem::from))
	}

	/// The libraries the user can add books to
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn intake_libraries(&self, ctx: &Context<'_>) -> Result<Vec<Library>> {
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(library::Entity::find_for_user(user)
			.order_by_asc(library::Column::Name)
			.all(conn)
			.await?
			.into_iter()
			.map(Library::from)
			.collect())
	}

	/// How the intake files books into a library (its defaults when never changed)
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn library_intake_settings(
		&self,
		ctx: &Context<'_>,
		library_id: ID,
	) -> Result<library_intake_settings::Model> {
		let library = accessible_library(ctx, &library_id).await?;
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let mut settings = core_intake::settings_for(conn, &library.id).await?;
		if !can_file_directly(user) {
			// the folders are the library's own layout
			settings.manga_dir = String::new();
			settings.light_novel_dir = String::new();
		}
		Ok(settings)
	}

	/// The tags the library uses (most used first), for the tag picker
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn intake_vocabulary(
		&self,
		ctx: &Context<'_>,
		library_id: ID,
		search: Option<String>,
		#[graphql(default = 30)] limit: usize,
	) -> Result<Vec<IntakeVocabularyEntry>> {
		let library = accessible_library(ctx, &library_id).await?;
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let limit = limit.min(MAX_LIST_SIZE as usize);
		let mut entries: Vec<IntakeVocabularyEntry> = if can_file_directly(user) {
			let settings = core_intake::settings_for(conn, &library.id).await?;
			let index = library_index(
				conn,
				&library.id,
				Path::new(&library.path),
				&settings.manga_dir,
				&settings.light_novel_dir,
			)
			.await?;
			index
				.vocabulary
				.iter()
				.filter(|v| matches_search(&v.value, &search))
				.map(|v| IntakeVocabularyEntry {
					value: v.value.clone(),
					count: v.count,
					kind: if v.facet { "FACET" } else { "GENRE" }.to_string(),
				})
				.collect()
		} else {
			// the tags of the books they can read, counted among those
			scope::viewer_tags(conn, user, &library.id)
				.await?
				.into_iter()
				.filter(|(tag, _, _)| matches_search(tag, &search))
				.map(|(value, count, facet)| IntakeVocabularyEntry {
					value,
					count,
					kind: if facet { "FACET" } else { "GENRE" }.to_string(),
				})
				.collect()
		};
		entries.sort_by(|a, b| b.count.cmp(&a.count).then(a.value.cmp(&b.value)));
		entries.truncate(limit);
		Ok(entries)
	}

	/// Existing author folders (no `seriesKind`) or series folders, for the folder picker of
	/// somebody who chooses where books go (an approver)
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn intake_folders(
		&self,
		ctx: &Context<'_>,
		library_id: ID,
		series_kind: Option<IntakeSeriesKind>,
		search: Option<String>,
		#[graphql(default = 30)] limit: usize,
	) -> Result<Vec<IntakeFolder>> {
		let library = accessible_library(ctx, &library_id).await?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let settings = core_intake::settings_for(conn, &library.id).await?;
		let index = library_index(
			conn,
			&library.id,
			Path::new(&library.path),
			&settings.manga_dir,
			&settings.light_novel_dir,
		)
		.await?;
		let folders = match series_kind {
			None => &index.author_folders,
			Some(IntakeSeriesKind::Manga) => &index.manga_folders,
			Some(IntakeSeriesKind::LightNovel) => &index.light_novel_folders,
		};
		let limit = limit.min(MAX_LIST_SIZE as usize);
		let mut out: Vec<IntakeFolder> = folders
			.iter()
			.filter(|f| matches_search(&f.name.replace('_', " "), &search))
			.map(|f| IntakeFolder {
				name: f.name.clone(),
				books: f.books,
			})
			.collect();
		if search.as_deref().is_none_or(|s| s.trim().is_empty()) {
			out.sort_by(|a, b| b.books.cmp(&a.books).then(a.name.cmp(&b.name)));
		}
		out.truncate(limit);
		Ok(out)
	}

	/// The items waiting for approval in the libraries the viewer can use, oldest first, for the
	/// approvers' review page
	#[graphql(
		guard = "PermissionGuard::one(UserPermission::ManageLibrary)",
		complexity = "crate::complexity::list_cost(Some(page_size), child_complexity)"
	)]
	async fn pending_intake_items(
		&self,
		ctx: &Context<'_>,
		library_id: Option<ID>,
		#[graphql(default = 1)] page: u64,
		#[graphql(default = 20)] page_size: u64,
	) -> Result<IntakeItemPage> {
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let size = list_limit(Some(page_size)).max(1);
		let offset = page.saturating_sub(1).saturating_mul(size);
		let (items, total) = approval::pending(
			conn,
			user,
			library_id.as_ref().map(|id| id.as_str()),
			crate::pagination::list_offset(Some(offset)),
			size,
		)
		.await?;
		Ok(IntakeItemPage {
			nodes: items.into_iter().map(IntakeItem::from).collect(),
			total_count: i64::try_from(total).unwrap_or(i64::MAX),
		})
	}

	/// How many items wait for approval in the libraries the viewer can use
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn pending_intake_count(&self, ctx: &Context<'_>) -> Result<i64> {
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(i64::try_from(approval::pending_count(conn, user).await?).unwrap_or(i64::MAX))
	}

	/// What the viewer may upload, how much of it they have used, and whether uploads are open
	/// for them now
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn intake_upload_limits(
		&self,
		ctx: &Context<'_>,
	) -> Result<IntakeUploadLimits> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();
		let limits = limits_for(conn, &core.config, &user.id).await?;
		let (used, open) = upload::usage(conn, &user.id).await?;
		let approver = can_file_directly(user);
		let (uploads_open, closed_reason) =
			if !approver && core.config.intake_probe_mode != "inprocess" {
				(
					false,
					Some(
						"Uploads that wait for approval aren't open on this server yet"
							.to_string(),
					),
				)
			} else if !core.config.enable_upload && approver {
				(
					false,
					Some("Uploads are turned off on this server".to_string()),
				)
			} else {
				(true, None)
			};
		let clamp = |value: u64| i64::try_from(value).unwrap_or(i64::MAX);
		Ok(IntakeUploadLimits {
			max_request_bytes: clamp(limits.max_request_bytes),
			max_file_bytes: clamp(limits.max_file_bytes),
			max_files_per_request: clamp(limits.max_files_per_request),
			quota_bytes: clamp(limits.quota_bytes),
			used_bytes: clamp(used),
			max_items: clamp(limits.max_items),
			open_items: clamp(open),
			quota_applies: !approver,
			can_file_directly: approver,
			uploads_open,
			closed_reason,
		})
	}

	/// The upload limits an admin gave a user, and what applies to them. A user can read their
	/// own; reading somebody else's takes managing users
	#[graphql(
		guard = "PermissionGuard::new(&[UserPermission::ManageUsers, UserPermission::AddBooks])"
	)]
	async fn user_intake_limits(
		&self,
		ctx: &Context<'_>,
		user_id: ID,
	) -> Result<UserIntakeLimits> {
		let AuthContext { user, .. } = ctx.data()?;
		if user.id != user_id.as_str()
			&& !user.has_permission(UserPermission::ManageUsers)
		{
			return Err(crate::error_message::FORBIDDEN_ACTION.into());
		}
		let core = ctx.data::<CoreContext>()?;
		user_intake_limits_of(core, &user_id).await
	}

	/// Books filed through the intake, newest first (who added what, when, where)
	#[graphql(guard = "PermissionGuard::one(UserPermission::ReadUsers)")]
	async fn recently_added_books(
		&self,
		ctx: &Context<'_>,
		user_id: Option<ID>,
		#[graphql(default = 50)] limit: u64,
		#[graphql(default = 0)] offset: u64,
	) -> Result<Vec<RecentlyAddedBook>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let rows = core_intake::recently_filed(
			conn,
			user_id.as_ref().map(|id| id.as_str()),
			limit.min(200),
			offset,
		)
		.await?;
		let mut out = vec![];
		for (item, user) in rows {
			let library = library::Entity::find_by_id(item.library_id.clone())
				.one(conn)
				.await?;
			let root = library.as_ref().map(|l| l.path.clone()).unwrap_or_default();
			let decision = item.decision.clone().unwrap_or_default();
			let absolute_paths = item.result.clone().map(|r| r.paths).unwrap_or_default();
			let paths = absolute_paths
				.iter()
				.map(|p| {
					Path::new(p)
						.strip_prefix(&root)
						.map(|r| r.to_string_lossy().into_owned())
						.unwrap_or_else(|_| p.clone())
				})
				.collect();
			out.push(RecentlyAddedBook {
				item_id: item.id.clone().into(),
				user_id: item.user_id.clone(),
				username: user.map(|u| u.username),
				title: decision
					.metadata
					.title
					.clone()
					.or(decision.metadata.series.clone())
					.unwrap_or_else(|| item.name.clone()),
				authors: decision.metadata.authors.clone(),
				series: decision.metadata.series.clone(),
				paths,
				library_id: item.library_id.clone(),
				library_name: library.map(|l| l.name).unwrap_or_default(),
				filed_at: item.filed_at,
				absolute_paths,
			});
		}
		Ok(out)
	}
}
