//! The "Add books" intake: upload into a per-user inbox, review, file into the library.

use std::{
	io::{Read, Seek},
	path::{Path, PathBuf},
};

use async_graphql::{Context, InputObject, Object, Result, Upload, ID};
use models::{
	entity::{intake_item, library, library_intake_settings, user::AuthUser},
	shared::{
		enums::UserPermission,
		intake::{IntakeFile, IntakeFiles, IntakeKind, IntakeReviewInput, IntakeState},
	},
};
use sea_orm::{prelude::*, ActiveValue::Set, IntoActiveModel};
use stump_core::intake::{
	self as core_intake, extension_of, naming::fold_name, BOOK_EXTENSIONS,
};
use zip::ZipArchive;

use crate::{
	data::{AuthContext, CoreContext},
	guard::{OptionalFeature, OptionalFeatureGuard, PermissionGuard},
	mutation::upload::{enforce_max_size, validate_book_file},
	object::intake::{IntakeItem, IntakeUploadMode},
};

#[derive(Default)]
pub struct IntakeMutation;

#[derive(Debug, InputObject)]
pub struct LibraryIntakeSettingsInput {
	pub manga_dir: String,
	pub light_novel_dir: String,
	/// Empty or omitted: next to `catalogo.csv` when the library's parent has one, else
	/// the library root
	pub csv_path: Option<String>,
	pub online_lookup: bool,
}

/// One file to stage, before it becomes part of an item
struct Staged {
	group: Option<String>,
	original_name: String,
	format: String,
	source: StagedSource,
	size: i64,
}

enum StagedSource {
	Upload(std::fs::File),
	Bytes(Vec<u8>),
}

/// A safe staged file name: the folded stem and the extension, prefixed by its position
fn staged_name(position: usize, original: &str) -> String {
	let base = Path::new(original)
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_default();
	let ext = extension_of(&base);
	let stem = base.strip_suffix(&format!(".{ext}")).unwrap_or(&base);
	let folded = fold_name(stem);
	format!("{position:03}_{}.{ext}", if folded.is_empty() { "file" } else { &folded })
}

/// The folder of a relative upload path (`Series/v01.cbz` -> `Series`)
fn group_of(relative: &str) -> Option<String> {
	let parts: Vec<&str> = relative
		.split(['/', '\\'])
		.filter(|p| !p.is_empty() && *p != "." && *p != "..")
		.collect();
	(parts.len() >= 2).then(|| parts[0].to_string())
}

/// The books inside an uploaded .zip of a series (by name, in archive order)
fn books_in_zip(file: &mut std::fs::File, max_size: usize) -> Result<Vec<(String, Vec<u8>)>> {
	let mut archive = ZipArchive::new(&mut *file).map_err(|_| "Error opening zip archive")?;
	let mut books = vec![];
	for i in 0..archive.len() {
		let mut entry = archive.by_index(i)?;
		if entry.is_dir() {
			continue;
		}
		let Some(enclosed) = entry.enclosed_name() else {
			return Err("The zip contains a malformed path".into());
		};
		let name = enclosed
			.file_name()
			.map(|n| n.to_string_lossy().into_owned())
			.unwrap_or_default();
		if name.starts_with('.') || !BOOK_EXTENSIONS.contains(&extension_of(&name).as_str()) {
			continue;
		}
		if entry.size() as usize > max_size {
			return Err(format!("{name} is larger than the maximum upload size").into());
		}
		let mut bytes = Vec::with_capacity(entry.size() as usize);
		entry.read_to_end(&mut bytes)?;
		books.push((name, bytes));
	}
	file.rewind()?;
	Ok(books)
}

fn spawn_analysis(core: &CoreContext, ids: Vec<String>, online: Option<bool>) {
	let core = core.clone();
	tokio::spawn(async move {
		for id in ids {
			if let Err(error) = core_intake::analyse(&core, &id, online).await {
				tracing::warn!(?error, id, "Intake analysis failed");
			}
		}
		core_intake::library_index::drop_stale_indexes();
	});
}

async fn own_item(ctx: &Context<'_>, user: &AuthUser, id: &str) -> Result<intake_item::Model> {
	let conn = ctx.data::<CoreContext>()?.conn.as_ref();
	intake_item::Entity::find_for_user(user)
		.filter(intake_item::Column::Id.eq(id))
		.one(conn)
		.await?
		.ok_or_else(|| "Inbox item not found".into())
}

#[Object]
impl IntakeMutation {
	/// Uploads books into the user's inbox for a library. Each book (or each folder / .zip
	/// of volumes) becomes an item that is analysed right away; poll `intakeItems`.
	#[graphql(
		guard = "OptionalFeatureGuard::new(OptionalFeature::Upload).and(PermissionGuard::one(UserPermission::AddBooks))"
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
				let books = tokio::task::block_in_place(|| books_in_zip(&mut value.content, max_size))?;
				if books.is_empty() {
					return Err(format!("{} contains no books", value.filename).into());
				}
				let single = books.len() == 1;
				for (name, bytes) in books {
					staged.push(Staged {
						group: (!single).then(|| zip_name.clone()),
						original_name: format!("{zip_name}/{name}"),
						format: extension_of(&name),
						size: bytes.len() as i64,
						source: StagedSource::Bytes(bytes),
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
				source: StagedSource::Upload(value.content),
			});
		}

		// group the files into items
		let mut groups: Vec<(Option<String>, Vec<Staged>)> = vec![];
		for file in staged {
			let key = match mode {
				IntakeUploadMode::Books => None,
				IntakeUploadMode::Series => Some(
					file.group
						.clone()
						.unwrap_or_else(|| "Series".to_string()),
				),
				IntakeUploadMode::Auto => file.group.clone(),
			};
			match key {
				Some(key) => match groups.iter_mut().find(|(k, _)| k.as_deref() == Some(&key)) {
					Some((_, files)) => files.push(file),
					None => groups.push((Some(key), vec![file])),
				},
				None => groups.push((None, vec![file])),
			}
		}

		let mut items = vec![];
		for (group, files) in groups {
			let id = Uuid::new_v4().to_string();
			let dir = core_intake::staging_dir(core, &user.id, &id);
			tokio::fs::create_dir_all(&dir).await?;
			let name = group.clone().unwrap_or_else(|| {
				files
					.first()
					.map(|f| {
						Path::new(&f.original_name)
							.file_name()
							.map(|n| n.to_string_lossy().into_owned())
							.unwrap_or_default()
					})
					.unwrap_or_default()
			});
			let kind = if group.is_some() && files.len() > 1 || mode == IntakeUploadMode::Series {
				IntakeKind::Series
			} else {
				IntakeKind::Book
			};
			let mut intake_files = vec![];
			for (position, file) in files.into_iter().enumerate() {
				let staged_name = staged_name(position, &file.original_name);
				let target: PathBuf = dir.join(&staged_name);
				match file.source {
					StagedSource::Upload(content) => {
						let mut source = tokio::fs::File::from_std(content);
						let mut out = tokio::fs::File::create(&target).await?;
						tokio::io::copy(&mut source, &mut out).await?;
					},
					StagedSource::Bytes(bytes) => tokio::fs::write(&target, bytes).await?,
				}
				intake_files.push(IntakeFile {
					original_name: file.original_name,
					staged_name,
					size: file.size,
					format: file.format,
					..Default::default()
				});
			}
			let model = intake_item::ActiveModel {
				id: Set(id.clone()),
				user_id: Set(user.id.clone()),
				library_id: Set(library.id.clone()),
				state: Set(IntakeState::Analysing),
				kind: Set(kind),
				name: Set(name),
				staging_dir: Set(dir.to_string_lossy().into_owned()),
				files: Set(IntakeFiles(intake_files)),
				..Default::default()
			}
			.insert(conn)
			.await?;
			items.push(model);
		}

		spawn_analysis(core, items.iter().map(|i| i.id.clone()).collect(), None);
		Ok(items.into_iter().map(IntakeItem::from).collect())
	}

	/// Saves the uploader's review of an item (omitted fields keep their value)
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn intake_review(
		&self,
		ctx: &Context<'_>,
		id: ID,
		input: IntakeReviewInput,
	) -> Result<IntakeItem> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let item = own_item(ctx, user, &id).await?;
		Ok(core_intake::apply_review(core, item, input).await?.into())
	}

	/// Analyses an item again (e.g. after a failure, or to look it up online)
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn intake_reanalyse(
		&self,
		ctx: &Context<'_>,
		id: ID,
		#[graphql(desc = "Override the library's online lookup setting")] online: Option<bool>,
	) -> Result<IntakeItem> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let item = own_item(ctx, user, &id).await?;
		if matches!(item.state, IntakeState::Filed | IntakeState::Discarded) {
			return Err("This item is no longer in the inbox".into());
		}
		let mut active = item.into_active_model();
		active.state = Set(IntakeState::Analysing);
		active.error = Set(None);
		let item = active.update(core.conn.as_ref()).await?;
		spawn_analysis(core, vec![item.id.clone()], online);
		Ok(item.into())
	}

	/// Files ready items into their library. Items that fail keep their state and get an
	/// `error`; the others become `FILED`.
	#[graphql(guard = "PermissionGuard::one(UserPermission::AddBooks)")]
	async fn intake_accept(&self, ctx: &Context<'_>, ids: Vec<ID>) -> Result<Vec<IntakeItem>> {
		let AuthContext { user, .. } = ctx.data()?;
		let core = ctx.data::<CoreContext>()?;
		let mut out = vec![];
		for id in ids {
			let item = own_item(ctx, user, &id).await?;
			match core_intake::file_item(core, item.clone(), &user.username).await {
				Ok(filed) => out.push(filed.into()),
				Err(error) => {
					let mut active = item.into_active_model();
					active.error = Set(Some(error.to_string()));
					out.push(active.update(core.conn.as_ref()).await?.into());
				},
			}
		}
		Ok(out)
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
		let plain = |dir: &str| core_intake::naming::is_plain_component(dir);
		if !plain(&input.manga_dir) || !plain(&input.light_novel_dir) {
			return Err("Series folders must be a single folder name".into());
		}
		let model = library_intake_settings::Model {
			library_id: library.id.clone(),
			manga_dir: input.manga_dir.trim().to_string(),
			light_novel_dir: input.light_novel_dir.trim().to_string(),
			csv_path: input
				.csv_path
				.map(|p| p.trim().to_string())
				.filter(|p| !p.is_empty()),
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
			csv_path: Set(model.csv_path.clone()),
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn stages_files_under_safe_names() {
		assert_eq!(staged_name(0, "Umberto Eco - El nombre.epub"), "000_Umberto_Eco_-_El_nombre.epub");
		assert_eq!(staged_name(3, "Series/../../etc/passwd.cbz"), "003_passwd.cbz");
		assert_eq!(group_of("Yotsuba/Yotsuba v01.cbz").as_deref(), Some("Yotsuba"));
		assert_eq!(group_of("book.epub"), None);
		assert_eq!(group_of("../book.epub"), None);
	}
}
