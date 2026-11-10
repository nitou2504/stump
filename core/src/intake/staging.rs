//! Putting uploaded files into inbox items: the names they are staged under, the books inside an
//! uploaded `.zip` pack, and the items themselves. Both ways in (the GraphQL `intakeUpload` of an
//! approver and the REST upload everybody uses) end here.

use std::{
	io::Seek,
	path::{Path, PathBuf},
	sync::Arc,
};

use models::{
	entity::{intake_item, library, user::AuthUser},
	shared::intake::{
		IntakeFile, IntakeFiles, IntakeKind, IntakeState, IntakeUploadMode,
	},
};
use sea_orm::{prelude::*, ActiveValue::Set};
use zip::ZipArchive;

use super::{extension_of, naming::fold_name, BOOK_EXTENSIONS};
use crate::{fs_utils::bounded, CoreError, CoreResult, Ctx};

/// Where the bytes of a staged file are: an open file to copy (a spooled upload, or a book taken
/// out of a pack), or a file already in the staging area that is moved into its item's folder
pub enum StagedSource {
	Open(std::fs::File),
	Moved(PathBuf),
}

/// One file to stage, before it becomes part of an item
pub struct Staged {
	pub group: Option<String>,
	pub original_name: String,
	pub format: String,
	pub source: StagedSource,
	pub size: i64,
}

/// A safe staged file name: the folded stem and the extension, prefixed by its position
pub fn staged_name(position: usize, original: &str) -> String {
	let base = Path::new(original)
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_default();
	let ext = extension_of(&base);
	let stem = base.strip_suffix(&format!(".{ext}")).unwrap_or(&base);
	let folded = fold_name(stem);
	format!(
		"{position:03}_{}.{ext}",
		if folded.is_empty() { "file" } else { &folded }
	)
}

/// The folder of a relative upload path (`Series/v01.cbz` -> `Series`)
pub fn group_of(relative: &str) -> Option<String> {
	let parts: Vec<&str> = relative
		.split(['/', '\\'])
		.filter(|p| !p.is_empty() && *p != "." && *p != "..")
		.collect();
	(parts.len() >= 2).then(|| parts[0].to_string())
}

/// A book taken out of an uploaded .zip: its name, its bytes in an anonymous temporary file
/// (rewound), and how many they are
pub type PackBook = (String, std::fs::File, u64);

/// The books inside an uploaded .zip of a series (by name, in archive order).
///
/// The zip is somebody else's file, and a few KiB of it can inflate to gigabytes: no book is
/// held in memory (each goes to a temporary file as it inflates), none may inflate past
/// `max_size`, however small the zip says it is, the pack may not unpack past the configured
/// limit or hold more than the configured number of books, and an entry that inflates a
/// thousand to one is turned away
pub fn books_in_zip(
	file: &mut std::fs::File,
	max_size: u64,
) -> CoreResult<Vec<PackBook>> {
	read_pack(
		file,
		max_size,
		bounded::max_unpacked_bytes(),
		bounded::max_pack_books(),
	)
}

pub fn read_pack(
	file: &mut std::fs::File,
	max_size: u64,
	max_unpacked: u64,
	max_books: usize,
) -> CoreResult<Vec<PackBook>> {
	let bad = |message: String| CoreError::BadRequest(message);
	let mut archive = ZipArchive::new(&mut *file)
		.map_err(|_| bad("Error opening zip archive".to_string()))?;
	let mut books: Vec<PackBook> = vec![];
	let (mut declared_total, mut unpacked_total) = (0u64, 0u64);
	for i in 0..archive.len() {
		let mut entry = archive
			.by_index(i)
			.map_err(|error| bad(format!("Error reading the zip: {error}")))?;
		if entry.is_dir() {
			continue;
		}
		let Some(enclosed) = entry.enclosed_name() else {
			return Err(bad("The zip contains a malformed path".to_string()));
		};
		let name = enclosed
			.file_name()
			.map(|n| n.to_string_lossy().into_owned())
			.unwrap_or_default();
		if name.starts_with('.')
			|| !BOOK_EXTENSIONS.contains(&extension_of(&name).as_str())
		{
			continue;
		}
		if books.len() >= max_books {
			return Err(bad(format!("The zip holds more than {max_books} books")));
		}
		if entry.size() > max_size {
			return Err(bad(format!(
				"{name} is larger than the maximum upload size"
			)));
		}
		if bounded::check_ratio(entry.size(), entry.compressed_size()).is_err() {
			return Err(bad(format!("{name} inflates far more than a book does")));
		}
		declared_total = declared_total.saturating_add(entry.size());
		if declared_total > max_unpacked {
			return Err(bad(
				"The zip unpacks to more than the maximum size".to_string()
			));
		}
		let mut book = tempfile::tempfile()?;
		let size = match bounded::copy_capped(&mut entry, &mut book, max_size) {
			Ok(size) => size,
			Err(error) if bounded::is_too_large(&error) => {
				return Err(bad(format!(
					"{name} is larger than the maximum upload size"
				)))
			},
			Err(error) => return Err(error.into()),
		};
		// what the zip says is only what it says: the total counts what came out
		unpacked_total = unpacked_total.saturating_add(size);
		if unpacked_total > max_unpacked {
			return Err(bad(
				"The zip unpacks to more than the maximum size".to_string()
			));
		}
		book.rewind()?;
		books.push((name, book, size));
	}
	file.rewind()?;
	Ok(books)
}

/// Starts the analysis of items in the background, one after the other. The library index it
/// builds is dropped minutes later.
pub fn spawn_analysis(ctx: Arc<Ctx>, ids: Vec<String>, online: Option<bool>) {
	tokio::spawn(async move {
		for id in ids {
			if let Err(error) = super::analyse(&ctx, &id, online).await {
				tracing::warn!(?error, id, "Intake analysis failed");
			}
		}
		super::library_index::drop_stale_indexes();
	});
}

/// The item a staged file belongs to, by the name of its group: `None` is a book of its own
fn group_key(mode: IntakeUploadMode, file: &Staged) -> Option<String> {
	match mode {
		IntakeUploadMode::Books => None,
		IntakeUploadMode::Series => {
			Some(file.group.clone().unwrap_or_else(|| "Series".to_string()))
		},
		IntakeUploadMode::Auto => file.group.clone(),
	}
}

/// How many items these files make
pub fn count_items(mode: IntakeUploadMode, staged: &[Staged]) -> usize {
	let mut groups = std::collections::HashSet::new();
	let mut loose = 0;
	for file in staged {
		match group_key(mode, file) {
			Some(key) => {
				groups.insert(key);
			},
			None => loose += 1,
		}
	}
	groups.len() + loose
}

/// Groups staged files into items, stages each item's files under its own folder and stores the
/// items (waiting to be analysed). A group is one book, or the volumes of a series.
pub async fn create_items(
	ctx: &Ctx,
	user: &AuthUser,
	library: &library::Model,
	mode: IntakeUploadMode,
	staged: Vec<Staged>,
) -> CoreResult<Vec<intake_item::Model>> {
	let conn = ctx.conn.as_ref();
	let mut groups: Vec<(Option<String>, Vec<Staged>)> = vec![];
	for file in staged {
		match group_key(mode, &file) {
			Some(key) => {
				match groups.iter_mut().find(|(k, _)| k.as_deref() == Some(&key)) {
					Some((_, files)) => files.push(file),
					None => groups.push((Some(key), vec![file])),
				}
			},
			None => groups.push((None, vec![file])),
		}
	}

	let mut items: Vec<intake_item::Model> = vec![];
	for (group, files) in groups {
		let id = Uuid::new_v4().to_string();
		let dir = super::staging_dir(ctx, &user.id, &id);
		if let Err(error) = tokio::fs::create_dir_all(&dir).await {
			discard_created(conn, &items).await;
			return Err(error.into());
		}
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
		let kind =
			if group.is_some() && files.len() > 1 || mode == IntakeUploadMode::Series {
				IntakeKind::Series
			} else {
				IntakeKind::Book
			};
		let staged = stage_files(&dir, files).await;
		let intake_files = match staged {
			Ok(files) => files,
			Err(error) => {
				let _ = tokio::fs::remove_dir_all(&dir).await;
				discard_created(conn, &items).await;
				return Err(error);
			},
		};
		let model = intake_item::ActiveModel {
			id: Set(id),
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
		.await;
		match model {
			Ok(model) => items.push(model),
			Err(error) => {
				let _ = tokio::fs::remove_dir_all(&dir).await;
				discard_created(conn, &items).await;
				return Err(error.into());
			},
		}
	}
	Ok(items)
}

/// Takes back the items an upload had made before it failed: a request that failed is one that
/// made nothing, and an item left behind would wait to be analysed for good
async fn discard_created(conn: &DatabaseConnection, items: &[intake_item::Model]) {
	for item in items {
		let _ = intake_item::Entity::delete_by_id(item.id.clone())
			.exec(conn)
			.await;
		let _ = tokio::fs::remove_dir_all(&item.staging_dir).await;
	}
}

/// Puts the files of one item into its staging folder under safe names
async fn stage_files(dir: &Path, files: Vec<Staged>) -> CoreResult<Vec<IntakeFile>> {
	let mut intake_files = vec![];
	for (position, file) in files.into_iter().enumerate() {
		let staged_name = staged_name(position, &file.original_name);
		let target: PathBuf = dir.join(&staged_name);
		match file.source {
			StagedSource::Open(source) => {
				let mut source = tokio::fs::File::from_std(source);
				let mut out = tokio::fs::File::create(&target).await?;
				tokio::io::copy(&mut source, &mut out).await?;
			},
			StagedSource::Moved(from) => {
				// the same disk: a rename moves nothing. Falls back to a copy across disks
				if tokio::fs::rename(&from, &target).await.is_err() {
					tokio::fs::copy(&from, &target).await?;
					let _ = tokio::fs::remove_file(&from).await;
				}
			},
		}
		intake_files.push(IntakeFile {
			original_name: file.original_name,
			staged_name,
			size: file.size,
			format: file.format,
			..Default::default()
		});
	}
	Ok(intake_files)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn stages_files_under_safe_names() {
		assert_eq!(
			staged_name(0, "Umberto Eco - El nombre.epub"),
			"000_Umberto_Eco_-_El_nombre.epub"
		);
		assert_eq!(
			staged_name(3, "Series/../../etc/passwd.cbz"),
			"003_passwd.cbz"
		);
		assert_eq!(
			group_of("Yotsuba/Yotsuba v01.cbz").as_deref(),
			Some("Yotsuba")
		);
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
					.start_file(
						*name,
						SimpleFileOptions::default().compression_method(method),
					)
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
		assert!(
			error.to_string().contains("big.epub is larger than"),
			"{error}"
		);
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
			assert_eq!(
				std::io::copy(&mut entry, &mut std::io::sink()).unwrap(),
				3 * MIB
			);
		}
		lying.rewind().unwrap();
		let error = read_pack(&mut lying, MIB, 100 * MIB, 10).unwrap_err();
		assert!(
			error.to_string().contains("bomb.epub is larger than"),
			"{error}"
		);
	}

	#[test]
	fn a_book_that_inflates_a_thousand_to_one_is_refused() {
		let mut zip = zip_of(&[("zeros.pdf", &vec![0u8; 5 * MIB as usize], true)]);
		let error = read_pack(&mut zip, 10 * MIB, 100 * MIB, 10).unwrap_err();
		assert!(error.to_string().contains("inflates far more"), "{error}");
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
		assert!(
			error.to_string().contains("unpacks to more than"),
			"{error}"
		);
	}

	#[test]
	fn a_pack_with_too_many_books_is_refused() {
		let entries: Vec<(String, Vec<u8>)> = (0..3)
			.map(|i| (format!("{i}.epub"), vec![i as u8; 10]))
			.collect();
		let entries: Vec<(&str, &[u8], bool)> = entries
			.iter()
			.map(|(n, b)| (n.as_str(), b.as_slice(), false))
			.collect();
		let mut zip = zip_of(&entries);
		assert!(read_pack(&mut zip, 10 * MIB, 100 * MIB, 3).is_ok());
		let mut zip = zip_of(&entries);
		let error = read_pack(&mut zip, 10 * MIB, 100 * MIB, 2).unwrap_err();
		assert!(error.to_string().contains("more than 2 books"), "{error}");
	}
}
