//! The "Add books" intake: upload into a per-user inbox, review, file into the library.

use std::{
	io::Seek,
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
use stump_core::{
	fs_utils::bounded,
	intake::{self as core_intake, extension_of, naming::fold_name, BOOK_EXTENSIONS},
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
	pub online_lookup: bool,
}

/// One file to stage, before it becomes part of an item
struct Staged {
	group: Option<String>,
	original_name: String,
	format: String,
	/// The bytes: the uploaded file, or a temporary file holding a book taken out of a pack
	source: std::fs::File,
	size: i64,
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

/// A book taken out of an uploaded .zip: its name, its bytes in an anonymous temporary file
/// (rewound), and how many they are
type PackBook = (String, std::fs::File, u64);

/// The books inside an uploaded .zip of a series (by name, in archive order).
///
/// The zip is somebody else's file, and a few KiB of it can inflate to gigabytes: no book is
/// held in memory (each goes to a temporary file as it inflates), none may inflate past
/// `max_size`, however small the zip says it is, the pack may not unpack past the configured
/// limit or hold more than the configured number of books, and an entry that inflates a
/// thousand to one is turned away
fn books_in_zip(file: &mut std::fs::File, max_size: usize) -> Result<Vec<PackBook>> {
	read_pack(
		file,
		max_size as u64,
		bounded::max_unpacked_bytes(),
		bounded::max_pack_books(),
	)
}

fn read_pack(
	file: &mut std::fs::File,
	max_size: u64,
	max_unpacked: u64,
	max_books: usize,
) -> Result<Vec<PackBook>> {
	let mut archive = ZipArchive::new(&mut *file).map_err(|_| "Error opening zip archive")?;
	let mut books: Vec<PackBook> = vec![];
	let (mut declared_total, mut unpacked_total) = (0u64, 0u64);
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
		if books.len() >= max_books {
			return Err(format!("The zip holds more than {max_books} books").into());
		}
		if entry.size() > max_size {
			return Err(format!("{name} is larger than the maximum upload size").into());
		}
		if bounded::check_ratio(entry.size(), entry.compressed_size()).is_err() {
			return Err(format!("{name} inflates far more than a book does").into());
		}
		declared_total = declared_total.saturating_add(entry.size());
		if declared_total > max_unpacked {
			return Err("The zip unpacks to more than the maximum size".into());
		}
		let mut book = tempfile::tempfile()?;
		let size = match bounded::copy_capped(&mut entry, &mut book, max_size) {
			Ok(size) => size,
			Err(error) if bounded::is_too_large(&error) => {
				return Err(format!("{name} is larger than the maximum upload size").into())
			},
			Err(error) => return Err(error.into()),
		};
		// what the zip says is only what it says: the total counts what came out
		unpacked_total = unpacked_total.saturating_add(size);
		if unpacked_total > max_unpacked {
			return Err("The zip unpacks to more than the maximum size".into());
		}
		book.rewind()?;
		books.push((name, book, size));
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
				for (name, book, size) in books {
					staged.push(Staged {
						group: (!single).then(|| zip_name.clone()),
						original_name: format!("{zip_name}/{name}"),
						format: extension_of(&name),
						size: size as i64,
						source: book,
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
				source: value.content,
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
				let mut source = tokio::fs::File::from_std(file.source);
				let mut out = tokio::fs::File::create(&target).await?;
				tokio::io::copy(&mut source, &mut out).await?;
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
			match core_intake::file_item(core, item.clone()).await {
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

	/// A zip of the entries (name, bytes, compressed), as an open file at its start
	fn zip_of(entries: &[(&str, &[u8], bool)]) -> std::fs::File {
		use std::io::Write;
		use zip::{write::SimpleFileOptions, CompressionMethod};

		let mut file = tempfile::tempfile().unwrap();
		{
			let mut writer = zip::ZipWriter::new(&mut file);
			for (name, bytes, compressed) in entries {
				let method = if *compressed {
					CompressionMethod::Deflated
				} else {
					CompressionMethod::Stored
				};
				writer
					.start_file(*name, SimpleFileOptions::default().compression_method(method))
					.unwrap();
				writer.write_all(bytes).unwrap();
			}
			writer.finish().unwrap();
		}
		file.rewind().unwrap();
		file
	}

	/// The same zip with the uncompressed size its central directory declares for every entry
	/// replaced: a zip that lies about how much an entry inflates to
	fn declaring(mut zip: std::fs::File, declared: u32) -> std::fs::File {
		use std::io::{Read, Write};

		let mut bytes = vec![];
		zip.read_to_end(&mut bytes).unwrap();
		let mut at = 0;
		while let Some(found) = bytes[at..].windows(4).position(|w| w == b"PK\x01\x02") {
			let header = at + found;
			// signature 4, versions 4, flags 2, method 2, time 4, crc 4, compressed 4: then this
			bytes[header + 24..header + 28].copy_from_slice(&declared.to_le_bytes());
			at = header + 4;
		}
		let mut lying = tempfile::tempfile().unwrap();
		lying.write_all(&bytes).unwrap();
		lying.rewind().unwrap();
		lying
	}

	fn read_back(mut book: std::fs::File) -> Vec<u8> {
		use std::io::Read;
		let mut bytes = vec![];
		book.read_to_end(&mut bytes).unwrap();
		bytes
	}

	const MIB: u64 = 1024 * 1024;

	#[test]
	fn a_pack_gives_its_books_in_order_and_skips_the_rest() {
		let mut zip = zip_of(&[
			("Series/", b"", false),
			("Series/v01.epub", b"first book", false),
			("Series/notes.txt", b"not a book", false),
			("Series/.hidden.epub", b"hidden", false),
			("Series/v02.cbz", b"second book", true),
		]);
		let books = read_pack(&mut zip, 10 * MIB, 100 * MIB, 10).unwrap();
		let names: Vec<_> = books.iter().map(|b| b.0.as_str()).collect();
		assert_eq!(names, ["v01.epub", "v02.cbz"]);
		let sizes: Vec<_> = books.iter().map(|b| b.2).collect();
		assert_eq!(sizes, [10, 11]);
		let mut contents = books.into_iter().map(|b| read_back(b.1));
		assert_eq!(contents.next().unwrap(), b"first book");
		assert_eq!(contents.next().unwrap(), b"second book");
	}

	#[test]
	fn a_book_over_the_maximum_is_refused() {
		let mut zip = zip_of(&[("big.epub", &vec![7u8; 2000], false)]);
		let error = read_pack(&mut zip, 1000, 100 * MIB, 10).unwrap_err();
		assert!(error.message.contains("big.epub is larger than"), "{}", error.message);
	}

	#[test]
	fn a_zip_that_lies_about_the_size_of_a_book_is_stopped_while_reading() {
		// 3 MiB of zeros (a few KiB of deflate) that the zip declares as 100 bytes
		let honest = zip_of(&[("bomb.epub", &vec![0u8; 3 * MIB as usize], true)]);
		let mut lying = declaring(honest, 100);
		{
			// the test's premise: the zip says 100 bytes, and the entry really holds 3 MiB
			let mut archive = ZipArchive::new(&mut lying).unwrap();
			let mut entry = archive.by_index(0).unwrap();
			assert_eq!(entry.size(), 100);
			assert_eq!(std::io::copy(&mut entry, &mut std::io::sink()).unwrap(), 3 * MIB);
		}
		lying.rewind().unwrap();
		let error = read_pack(&mut lying, MIB, 100 * MIB, 10).unwrap_err();
		assert!(error.message.contains("bomb.epub is larger than"), "{}", error.message);
	}

	#[test]
	fn a_book_that_inflates_a_thousand_to_one_is_refused() {
		let mut zip = zip_of(&[("zeros.pdf", &vec![0u8; 5 * MIB as usize], true)]);
		let error = read_pack(&mut zip, 10 * MIB, 100 * MIB, 10).unwrap_err();
		assert!(error.message.contains("inflates far more"), "{}", error.message);
	}

	#[test]
	fn a_pack_that_unpacks_past_the_maximum_is_refused() {
		let mut zip = zip_of(&[
			("a.epub", &vec![1u8; 1000], false),
			("b.epub", &vec![2u8; 1000], false),
		]);
		assert!(read_pack(&mut zip, 10 * MIB, 2000, 10).is_ok());
		let mut zip = zip_of(&[
			("a.epub", &vec![1u8; 1000], false),
			("b.epub", &vec![2u8; 1000], false),
		]);
		let error = read_pack(&mut zip, 10 * MIB, 1500, 10).unwrap_err();
		assert!(error.message.contains("unpacks to more than"), "{}", error.message);
	}

	#[test]
	fn a_pack_with_too_many_books_is_refused() {
		let entries: Vec<(String, Vec<u8>)> =
			(0..3).map(|i| (format!("{i}.epub"), vec![i as u8; 10])).collect();
		let entries: Vec<(&str, &[u8], bool)> =
			entries.iter().map(|(n, b)| (n.as_str(), b.as_slice(), false)).collect();
		let mut zip = zip_of(&entries);
		assert!(read_pack(&mut zip, 10 * MIB, 100 * MIB, 3).is_ok());
		let mut zip = zip_of(&entries);
		let error = read_pack(&mut zip, 10 * MIB, 100 * MIB, 2).unwrap_err();
		assert!(error.message.contains("more than 2 books"), "{}", error.message);
	}
}
