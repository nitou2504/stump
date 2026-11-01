use std::path::Path;

use async_graphql::{Context, Object, Result, ID};
use models::{
	entity::{intake_item, library, library_intake_settings},
	shared::{
		enums::UserPermission,
		intake::{IntakeSeriesKind, IntakeState},
	},
};
use sea_orm::{prelude::*, QueryOrder};
use stump_core::intake::{self as core_intake, library_index::library_index};

use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
	object::{
		intake::{IntakeFolder, IntakeItem, IntakeVocabularyEntry, RecentlyAddedBook},
		library::Library,
	},
};

#[derive(Default)]
pub struct IntakeQuery;

async fn accessible_library(ctx: &Context<'_>, library_id: &str) -> Result<library::Model> {
	let AuthContext { user, .. } = ctx.data()?;
	let conn = ctx.data::<CoreContext>()?.conn.as_ref();
	library::Entity::find_for_user(user)
		.filter(library::Column::Id.eq(library_id))
		.one(conn)
		.await?
		.ok_or_else(|| "Library not found".into())
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
	/// The items of the user's "Add books" inbox, newest first. By default everything but
	/// discarded items.
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn intake_items(
		&self,
		ctx: &Context<'_>,
		states: Option<Vec<IntakeState>>,
	) -> Result<Vec<IntakeItem>> {
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let mut query = intake_item::Entity::find_for_user(user)
			.order_by_desc(intake_item::Column::CreatedAt);
		query = match states {
			Some(states) if !states.is_empty() => {
				query.filter(intake_item::Column::State.is_in(states))
			},
			_ => query.filter(intake_item::Column::State.ne(IntakeState::Discarded)),
		};
		Ok(query.all(conn).await?.into_iter().map(IntakeItem::from).collect())
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
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(core_intake::settings_for(conn, &library.id).await?)
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
		let mut entries: Vec<IntakeVocabularyEntry> = index
			.vocabulary
			.iter()
			.filter(|v| matches_search(&v.value, &search))
			.map(|v| IntakeVocabularyEntry {
				value: v.value.clone(),
				count: v.count,
				kind: if v.facet { "FACET" } else { "GENRE" }.to_string(),
			})
			.collect();
		entries.sort_by(|a, b| b.count.cmp(&a.count).then(a.value.cmp(&b.value)));
		entries.truncate(limit);
		Ok(entries)
	}

	/// Existing author folders (no `seriesKind`) or series folders, for the folder picker
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
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
