//! The way in for uploads: what may arrive, the stream to disk, and the items it makes.
//!
//! Everything that can be decided before the body is read is decided before: who may upload, to
//! which library, how big the request says it is, whether the user's quota and the disk have room
//! for it. The room is **reserved** while the body streams (two uploads of one user can't both
//! find the last bytes of the quota free). Then every byte is counted as it is written, so none
//! of it rests on what the client claims, a file that stalls is given up, and whatever stops an
//! upload (a limit, a refusal, a closed connection) removes everything it staged.
//!
//! The server doesn't parse a byte here: it counts, and looks at the first bytes of each file to
//! see that it is the kind of file its name says. Reading the books (`analyse`) comes after, as
//! it does for every book.

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc, LazyLock, Mutex,
	},
	time::Duration,
};

use models::{
	entity::{intake_item, library, user::AuthUser},
	shared::intake::{IntakeFiles, IntakeState, IntakeUploadMode},
};
use sea_orm::{prelude::*, QuerySelect};
use thiserror::Error;
use tokio::io::AsyncWriteExt;

use super::{
	approval::can_file_directly,
	extension_of,
	limits::{free_bytes, limits_for, min_free_bytes, total_max_bytes, IntakeLimits},
	staging::{
		books_in_zip_within, count_items, create_items, group_of, spawn_analysis, Staged,
		StagedSource,
	},
};
use crate::{CoreError, Ctx};

const MIB: u64 = 1024 * 1024;

/// What is said of a pack that would use up the disk's spare room
const POOR_DISK: &str = "The server is short of disk space for a zip that big";

/// The extensions that can be uploaded: the books, and a `.zip` of books
pub const UPLOAD_EXTENSIONS: &[&str] = &["epub", "pdf", "cbz", "cbr", "zip"];

/// What can go wrong with an upload, in the words that are shown, and which kind of answer each
/// is for the caller
#[derive(Debug, Error)]
pub enum UploadError {
	#[error("{0}")]
	NotFound(String),
	#[error("{0}")]
	Forbidden(String),
	/// Not open now: uploads that wait for approval are off, or the disk is full
	#[error("{0}")]
	Unavailable(String),
	#[error("{0}")]
	TooLarge(String),
	#[error("{0}")]
	TooMany(String),
	#[error("{0}")]
	UnsupportedType(String),
	#[error("{0}")]
	BadRequest(String),
	#[error("{0}")]
	TimedOut(String),
	#[error(transparent)]
	Core(#[from] CoreError),
	#[error(transparent)]
	Io(#[from] std::io::Error),
	#[error(transparent)]
	Db(#[from] DbErr),
}

/// One check of admission at a time: the quota is read and the room reserved without anybody in
/// between
static ADMISSION: LazyLock<tokio::sync::Mutex<()>> =
	LazyLock::new(|| tokio::sync::Mutex::new(()));

/// The bytes the uploads that are arriving right now have reserved, per user
static RESERVED: LazyLock<Mutex<HashMap<String, u64>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

/// The inbox items the uploads that are being made into items right now will make, per user
static RESERVED_ITEMS: LazyLock<Mutex<HashMap<String, u64>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

fn reserved_items_by(user_id: &str) -> u64 {
	RESERVED_ITEMS
		.lock()
		.ok()
		.and_then(|map| map.get(user_id).copied())
		.unwrap_or(0)
}

fn reserved_by(user_id: &str) -> u64 {
	RESERVED
		.lock()
		.ok()
		.and_then(|map| map.get(user_id).copied())
		.unwrap_or(0)
}

fn reserved_total() -> u64 {
	RESERVED.lock().map(|map| map.values().sum()).unwrap_or(0)
}

/// Room held for an upload that is arriving: bytes, and the items it is about to make. Given
/// back when it ends, however it ends.
struct Reservation {
	user_id: String,
	bytes: u64,
	items: u64,
}

impl Reservation {
	fn new(user_id: &str, bytes: u64) -> Self {
		if let Ok(mut map) = RESERVED.lock() {
			*map.entry(user_id.to_string()).or_default() += bytes;
		}
		Self {
			user_id: user_id.to_string(),
			bytes,
			items: 0,
		}
	}

	/// Holds `bytes` in all: what a pack turned out to unpack to is more than the compressed
	/// request that was let in
	fn grow_to(&mut self, bytes: u64) {
		if bytes > self.bytes {
			if let Ok(mut map) = RESERVED.lock() {
				*map.entry(self.user_id.clone()).or_default() += bytes - self.bytes;
			}
			self.bytes = bytes;
		}
	}

	/// Holds the room of `items` more inbox items, until they are stored
	fn hold_items(&mut self, items: u64) {
		if let Ok(mut map) = RESERVED_ITEMS.lock() {
			*map.entry(self.user_id.clone()).or_default() += items;
		}
		self.items += items;
	}

	/// The items are stored (and counted with the others now)
	fn release_items(&mut self) {
		release(&RESERVED_ITEMS, &self.user_id, self.items);
		self.items = 0;
	}
}

impl Drop for Reservation {
	fn drop(&mut self) {
		release(&RESERVED, &self.user_id, self.bytes);
		release(&RESERVED_ITEMS, &self.user_id, self.items);
	}
}

fn release(map: &Mutex<HashMap<String, u64>>, user_id: &str, held: u64) {
	if held == 0 {
		return;
	}
	if let Ok(mut map) = map.lock() {
		if let Some(all) = map.get_mut(user_id) {
			*all = all.saturating_sub(held);
			if *all == 0 {
				map.remove(user_id);
			}
		}
	}
}

/// The bytes of the files the inbox items of one user (or of everybody) hold on disk: every
/// item that isn't filed or discarded, a sent-back one included
async fn inbox_bytes(
	conn: &DatabaseConnection,
	user_id: Option<&str>,
) -> Result<u64, DbErr> {
	let mut query = intake_item::Entity::find()
		.select_only()
		.column(intake_item::Column::Files)
		.filter(
			intake_item::Column::State
				.is_not_in([IntakeState::Filed, IntakeState::Discarded]),
		);
	if let Some(user_id) = user_id {
		query = query.filter(intake_item::Column::UserId.eq(user_id.to_string()));
	}
	Ok(query
		.into_tuple::<IntakeFiles>()
		.all(conn)
		.await?
		.iter()
		.flat_map(|files| files.0.iter())
		.map(|file| file.size.max(0) as u64)
		.sum())
}

/// What a user has waiting in their inbox: its bytes, and its open items
pub async fn usage(
	conn: &DatabaseConnection,
	user_id: &str,
) -> Result<(u64, u64), DbErr> {
	Ok((
		inbox_bytes(conn, Some(user_id)).await?,
		open_items(conn, user_id).await?,
	))
}

/// How many items of a user wait for them, or for an approver: all but the filed, discarded and
/// sent back ones
async fn open_items(conn: &DatabaseConnection, user_id: &str) -> Result<u64, DbErr> {
	intake_item::Entity::find()
		.filter(intake_item::Column::UserId.eq(user_id.to_string()))
		.filter(intake_item::Column::State.is_not_in([
			IntakeState::Filed,
			IntakeState::Discarded,
			IntakeState::Rejected,
		]))
		.count(conn)
		.await
}

/// A figure made of two things that an upload moves from one to the other: the room that
/// uploads arriving right now hold, and what is stored in the database. An upload stores its
/// items first and gives its room back after, so the room is read FIRST: an upload that ends
/// between the two reads is then counted in both (too many for a moment, which only makes the
/// check stricter), never in neither. Read the other way round, the items that were stored just
/// after the database was read and released just before the room was read would not count.
async fn held_then_stored(
	held: impl FnOnce() -> u64,
	stored: impl std::future::Future<Output = Result<u64, DbErr>>,
) -> Result<u64, DbErr> {
	let held = held();
	Ok(stored.await?.saturating_add(held))
}

/// The open items of a user, and the items that uploads of theirs are about to make
async fn items_in_use(conn: &DatabaseConnection, user_id: &str) -> Result<u64, DbErr> {
	held_then_stored(|| reserved_items_by(user_id), open_items(conn, user_id)).await
}

/// The bytes of a user's inbox, and the bytes that uploads of theirs are receiving
async fn bytes_in_use(conn: &DatabaseConnection, user_id: &str) -> Result<u64, DbErr> {
	held_then_stored(|| reserved_by(user_id), inbox_bytes(conn, Some(user_id))).await
}

/// The bytes of every inbox of the server, and the bytes that uploads are receiving
async fn bytes_in_use_by_all(conn: &DatabaseConnection) -> Result<u64, DbErr> {
	held_then_stored(reserved_total, inbox_bytes(conn, None)).await
}

/// A file name for display and for the library, from what the client sent: no control
/// characters, no path separators that lead anywhere, at most 255 bytes
pub fn clean_name(name: &str) -> String {
	let cleaned: String = name
		.chars()
		.filter(|c| !c.is_control())
		.collect::<String>()
		.replace('\\', "/");
	let mut kept = String::new();
	for c in cleaned.trim().chars() {
		if kept.len() + c.len_utf8() > 255 {
			break;
		}
		kept.push(c);
	}
	kept
}

/// The format a file really is, from its first bytes, for the extension it was uploaded with:
/// `epub`, `cbz` and `zip` are zip files; `pdf` starts with `%PDF-`, within its first KiB; `cbr`
/// is a rar file, or (it is common) a zip file called `cbr`, which is a `cbz`; a `cbz` that is a
/// rar file is a `cbr`. Anything else is not what its name says.
pub fn sniff_format(extension: &str, head: &[u8]) -> Result<String, UploadError> {
	let zip = head.starts_with(b"PK\x03\x04");
	let rar = head.starts_with(b"Rar!\x1A\x07\x00")
		|| head.starts_with(b"Rar!\x1A\x07\x01\x00");
	let pdf = head
		.windows(5)
		.take(1024usize.saturating_sub(4))
		.any(|w| w == b"%PDF-");
	let mismatch = || {
		UploadError::UnsupportedType(format!(
			"The file is not a {} file, whatever its name says",
			extension.to_uppercase()
		))
	};
	match extension {
		"epub" if zip => Ok("epub".to_string()),
		"zip" if zip => Ok("zip".to_string()),
		"cbz" if zip => Ok("cbz".to_string()),
		"cbz" if rar => Ok("cbr".to_string()),
		"cbr" if rar => Ok("cbr".to_string()),
		"cbr" if zip => Ok("cbz".to_string()),
		"pdf" if pdf => Ok("pdf".to_string()),
		_ => Err(mismatch()),
	}
}

/// The format of a book taken out of a pack, from its first bytes: its name in the pack is only
/// a name, so a PDF or a RAR called `.epub` is not filed as one. The file is left at its start.
pub fn sniff_pack_book(
	name: &str,
	book: &mut std::fs::File,
) -> Result<String, UploadError> {
	use std::io::{Read, Seek};
	let mut head = Vec::with_capacity(1024);
	book.by_ref().take(1024).read_to_end(&mut head)?;
	book.rewind()?;
	sniff_format(&extension_of(name), &head)
}

/// A file that arrived whole
pub struct IncomingFile {
	path: PathBuf,
	/// The name it was uploaded with, or its path inside an uploaded folder
	original_name: String,
	format: String,
	size: u64,
}

/// One file being written to disk
pub struct UploadFile {
	file: tokio::fs::File,
	path: PathBuf,
	original_name: String,
	extension: String,
	size: u64,
	head: Vec<u8>,
	limits: IntakeLimits,
	received: Arc<AtomicU64>,
}

impl UploadFile {
	/// Writes the next piece of the file, if it is still within the limits
	pub async fn write(&mut self, chunk: &[u8]) -> Result<(), UploadError> {
		let len = chunk.len() as u64;
		self.size += len;
		if self.size > self.limits.max_file_bytes {
			return Err(UploadError::TooLarge(format!(
				"{} is larger than the {} MB a file can be",
				self.original_name,
				self.limits.max_file_bytes / MIB
			)));
		}
		if self.received.fetch_add(len, Ordering::Relaxed) + len
			> self.limits.max_request_bytes
		{
			return Err(UploadError::TooLarge(format!(
				"The upload is larger than the {} MB a request can hold",
				self.limits.max_request_bytes / MIB
			)));
		}
		if self.head.len() < 1024 {
			let take = (1024 - self.head.len()).min(chunk.len());
			self.head.extend_from_slice(&chunk[..take]);
		}
		self.file.write_all(chunk).await?;
		Ok(())
	}

	/// Ends the file: it is something, and the kind of file its name says
	pub async fn finish(mut self) -> Result<IncomingFile, UploadError> {
		self.file.flush().await?;
		if self.size == 0 {
			return Err(UploadError::BadRequest(format!(
				"{} is empty",
				self.original_name
			)));
		}
		let format =
			sniff_format(&self.extension, &self.head).map_err(|error| match error {
				UploadError::UnsupportedType(message) => UploadError::UnsupportedType(
					format!("{}: {message}", self.original_name),
				),
				other => other,
			})?;
		Ok(IncomingFile {
			path: self.path,
			original_name: self.original_name,
			format,
			size: self.size,
		})
	}
}

/// An upload request that was let in, and the files it has brought so far. Whatever it staged is
/// removed when it ends without becoming items.
pub struct IncomingUpload {
	ctx: Arc<Ctx>,
	user: AuthUser,
	library: library::Model,
	limits: IntakeLimits,
	dir: PathBuf,
	files: Vec<IncomingFile>,
	/// Files started, finished or not
	started: usize,
	received: Arc<AtomicU64>,
	approver: bool,
	_reservation: Reservation,
}

impl IncomingUpload {
	/// Lets an upload in, or says why not: who may upload at all, the library, the size the
	/// request declares, the room on disk, and the user's quota
	pub async fn begin(
		ctx: Arc<Ctx>,
		user: &AuthUser,
		library: library::Model,
		declared_len: u64,
	) -> Result<Self, UploadError> {
		let conn = ctx.conn.as_ref();
		let approver = can_file_directly(user);
		if !approver && ctx.config.intake_probe_mode != "inprocess" {
			return Err(UploadError::Unavailable(
				"Uploads that wait for approval aren't open on this server yet"
					.to_string(),
			));
		}
		let limits = limits_for(conn, &ctx.config, &user.id).await?;
		if declared_len > limits.max_request_bytes {
			return Err(UploadError::TooLarge(format!(
				"The upload is larger than the {} MB a request can hold",
				limits.max_request_bytes / MIB
			)));
		}

		let config_dir = ctx.config.config_directory();
		if let Some(free) = free_bytes(&config_dir) {
			if free < min_free_bytes(&ctx.config).saturating_add(declared_len) {
				return Err(UploadError::Unavailable(
					"The server is short of disk space: uploads are paused".to_string(),
				));
			}
		}

		let reservation = {
			let _admission = ADMISSION.lock().await;
			let everybody = bytes_in_use_by_all(conn).await?;
			if everybody.saturating_add(declared_len) > total_max_bytes(&ctx.config) {
				return Err(UploadError::Unavailable(
					"The inboxes of the server are full: uploads are paused".to_string(),
				));
			}
			if !approver {
				let used = bytes_in_use(conn, &user.id).await?;
				if used.saturating_add(declared_len) > limits.quota_bytes {
					return Err(UploadError::TooLarge(format!(
						"Your inbox can hold {} MB and this would go over it: remove books you no longer need",
						limits.quota_bytes / MIB
					)));
				}
				if items_in_use(conn, &user.id).await? >= limits.max_items {
					return Err(UploadError::TooMany(format!(
						"You have {} books waiting already: file, send or remove some first",
						limits.max_items
					)));
				}
			}
			Reservation::new(&user.id, declared_len)
		};

		let ticket = Uuid::new_v4().to_string();
		let dir = incoming_root(&ctx, &user.id).join(&ticket);
		tokio::fs::create_dir_all(&dir).await?;
		Ok(Self {
			ctx,
			user: user.clone(),
			library,
			limits,
			dir,
			files: vec![],
			started: 0,
			received: Arc::new(AtomicU64::new(0)),
			approver,
			_reservation: reservation,
		})
	}

	/// How long a file may send nothing before it is given up
	pub fn idle(&self) -> Duration {
		Duration::from_secs(self.ctx.config.intake_upload_idle_secs.max(1))
	}

	/// Starts the next file: its name must be of a book Stump reads, and there may not be more
	/// files than a request is allowed
	pub async fn begin_file(&mut self, name: &str) -> Result<UploadFile, UploadError> {
		if self.started as u64 >= self.limits.max_files_per_request {
			return Err(UploadError::TooMany(format!(
				"An upload can hold {} files at most",
				self.limits.max_files_per_request
			)));
		}
		let original_name = clean_name(name);
		let extension = extension_of(&original_name);
		if !UPLOAD_EXTENSIONS.contains(&extension.as_str()) {
			return Err(UploadError::UnsupportedType(if extension.is_empty() {
				format!("{original_name} has no extension: Stump reads EPUB, PDF, CBZ, CBR and ZIP files of those")
			} else {
				format!("Stump can't read .{extension} files: it reads EPUB, PDF, CBZ, CBR and ZIP files of those")
			}));
		}
		self.started += 1;
		let path = self.dir.join(format!("{:03}.upload", self.started));
		let file = tokio::fs::OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&path)
			.await?;
		Ok(UploadFile {
			file,
			path,
			original_name,
			extension,
			size: 0,
			head: Vec::with_capacity(1024),
			limits: self.limits,
			received: self.received.clone(),
		})
	}

	/// Keeps a file that arrived whole
	pub fn add(&mut self, file: IncomingFile) {
		self.files.push(file);
	}

	/// How many bytes of packs may be unpacked: what is left of the user's quota (what they hold,
	/// what their other uploads are bringing and the books of this request that aren't packs
	/// come off it), and half of what the disk has to spare (what is unpacked is staged by
	/// copying it, so both exist for a while). Says what to say when a pack goes past it.
	async fn pack_room(&self, plain_bytes: u64) -> Result<(u64, String), UploadError> {
		let mut room = u64::MAX;
		let mut too_much = "The zip unpacks to more than the maximum size".to_string();
		if let Some(free) = free_bytes(&self.ctx.config.config_directory()) {
			room = free.saturating_sub(min_free_bytes(&self.ctx.config)) / 2;
			too_much = POOR_DISK.to_string();
		}
		if !self.approver {
			let conn = self.ctx.conn.as_ref();
			// what they hold, and what their other uploads bring (this upload's own room is
			// not "other")
			let in_use = held_then_stored(
				|| reserved_by(&self.user.id).saturating_sub(self._reservation.bytes),
				inbox_bytes(conn, Some(&self.user.id)),
			)
			.await?;
			let left = self
				.limits
				.quota_bytes
				.saturating_sub(in_use)
				.saturating_sub(plain_bytes);
			if left <= room {
				room = left;
				too_much = format!(
					"This is more than your inbox can hold ({} MB)",
					self.limits.quota_bytes / MIB
				);
			}
		}
		Ok((room, too_much))
	}

	/// Makes items of what arrived (a `.zip` is opened, and its books are the files) and starts
	/// reading them. The room the upload reserved is given back; the items hold it now.
	///
	/// A pack is unpacked into the upload's own folder and no further than the room its user has
	/// left, so what a few KiB of zip says it holds can't fill the disk before the quota is looked
	/// at. What came out is checked again, and the room it needs is held, in one step with
	/// everybody else's: of two uploads that each fit alone, only the ones that fit together go in.
	pub async fn finish(
		mut self,
		mode: IntakeUploadMode,
	) -> Result<Vec<intake_item::Model>, UploadError> {
		if self.files.is_empty() {
			return Err(UploadError::BadRequest(
				"No files were uploaded".to_string(),
			));
		}
		let plain_bytes: u64 = self
			.files
			.iter()
			.filter(|file| file.format != "zip")
			.map(|file| file.size)
			.sum();
		let (mut room, too_much) = self.pack_room(plain_bytes).await?;
		let mut staged: Vec<Staged> = vec![];
		for file in std::mem::take(&mut self.files) {
			if file.format == "zip" {
				let zip_name = Path::new(&file.original_name)
					.file_stem()
					.map(|s| s.to_string_lossy().into_owned())
					.unwrap_or_else(|| "Series".to_string());
				let (path, max_file) = (file.path.clone(), self.limits.max_file_bytes);
				let (dir, message) = (self.dir.clone(), too_much.clone());
				let (left, pack_name) = (room, zip_name.clone());
				let books = tokio::task::spawn_blocking(move || {
					let mut zip = std::fs::File::open(path)?;
					// what is said when the room is used up is told as what it is: the quota is
					// too much (413), the disk is not open (503)
					let books =
						books_in_zip_within(&mut zip, max_file, left, &dir, &message)
							.map_err(|error| match error {
								CoreError::BadRequest(said) if said == message => {
									if said == POOR_DISK {
										UploadError::Unavailable(said)
									} else {
										UploadError::TooLarge(said)
									}
								},
								other => other.into(),
							})?;
					// the name of a book says nothing about what it is: look at its first bytes
					let mut sniffed = Vec::with_capacity(books.len());
					for (name, mut book, size) in books {
						let format = sniff_pack_book(&name, &mut book).map_err(
							|error| match error {
								UploadError::UnsupportedType(message) => {
									UploadError::UnsupportedType(format!(
										"{pack_name}/{name}: {message}"
									))
								},
								other => other,
							},
						)?;
						sniffed.push((name, book, size, format));
					}
					Ok::<_, UploadError>(sniffed)
				})
				.await
				.map_err(|e| CoreError::InternalError(e.to_string()))??;
				if books.is_empty() {
					return Err(UploadError::BadRequest(format!(
						"{} contains no books",
						file.original_name
					)));
				}
				let _ = tokio::fs::remove_file(&file.path).await;
				let single = books.len() == 1;
				for (name, book, size, format) in books {
					room = room.saturating_sub(size);
					staged.push(Staged {
						group: (!single).then(|| zip_name.clone()),
						original_name: format!("{zip_name}/{name}"),
						format,
						size: size as i64,
						source: StagedSource::Open(book),
					});
				}
			} else {
				staged.push(Staged {
					group: group_of(&file.original_name),
					original_name: file.original_name,
					format: file.format,
					size: file.size as i64,
					source: StagedSource::Moved(file.path),
				});
			}
		}
		if !self.approver {
			// what came out of the packs is staged now, and was not part of the request: the
			// room it needs is checked again, with what other uploads of this user hold, and held
			// from then on (the admission lock makes the check and the holding one step)
			let conn = self.ctx.conn.as_ref();
			let staged_total: u64 = staged.iter().map(|s| s.size.max(0) as u64).sum();
			let wanted = count_items(mode, &staged) as u64;
			let _admission = ADMISSION.lock().await;
			let in_use = held_then_stored(
				|| reserved_by(&self.user.id).saturating_sub(self._reservation.bytes),
				inbox_bytes(conn, Some(&self.user.id)),
			)
			.await?;
			if in_use.saturating_add(staged_total) > self.limits.quota_bytes {
				return Err(UploadError::TooLarge(format!(
					"This is more than your inbox can hold ({} MB)",
					self.limits.quota_bytes / MIB
				)));
			}
			if items_in_use(conn, &self.user.id)
				.await?
				.saturating_add(wanted)
				> self.limits.max_items
			{
				return Err(UploadError::TooMany(format!(
					"This would leave you with more than the {} books that can wait: file, send or remove some first",
					self.limits.max_items
				)));
			}
			self._reservation.grow_to(staged_total);
			self._reservation.hold_items(wanted);
		}
		let items =
			create_items(&self.ctx, &self.user, &self.library, mode, staged).await?;
		// they are items now, and counted as such
		self._reservation.release_items();
		spawn_analysis(
			self.ctx.clone(),
			items.iter().map(|i| i.id.clone()).collect(),
			None,
		);
		Ok(items)
	}
}

impl Drop for IncomingUpload {
	fn drop(&mut self) {
		// whatever is left (an upload that ended early, or the zips that were opened)
		let _ = std::fs::remove_dir_all(&self.dir);
	}
}

/// The folder the uploads of a user that are still arriving are staged in
fn incoming_root(ctx: &Ctx, user_id: &str) -> PathBuf {
	super::staging_root(ctx).join(user_id).join("_incoming")
}

/// Removes the folders of uploads that were still arriving when the server stopped
pub async fn remove_incoming(ctx: &Ctx) {
	let root = super::staging_root(ctx);
	let Ok(mut users) = tokio::fs::read_dir(&root).await else {
		return;
	};
	while let Ok(Some(user)) = users.next_entry().await {
		let _ = tokio::fs::remove_dir_all(user.path().join("_incoming")).await;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// An upload that ends while the two reads are being made moves its items from the room it
	/// held to the rows of the database. Whichever read is made first, the other one is made
	/// right after it, with that move in between: the room must be read first, or the items
	/// that moved are in neither figure.
	#[tokio::test]
	async fn the_room_held_is_read_before_the_rows() {
		use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};

		let held = AtomicU64::new(1); // one item is about to be stored
		let rows = AtomicU64::new(0); // and is not yet
		let moved = AtomicBool::new(false);
		// after the first read, the upload ends: its row is stored, its room given back
		let after_first_read = || {
			if !moved.swap(true, SeqCst) {
				rows.store(1, SeqCst);
				held.store(0, SeqCst);
			}
		};
		let counted = held_then_stored(
			|| {
				let read = held.load(SeqCst);
				after_first_read();
				read
			},
			async {
				let read = rows.load(SeqCst);
				after_first_read();
				Ok(read)
			},
		)
		.await
		.expect("a figure");
		assert!(
			counted >= 1,
			"the item that moved between the reads is in neither figure ({counted})"
		);
		// and the sum is the plain sum when nothing moves
		let plain = held_then_stored(|| 3, async { Ok(4) }).await.unwrap();
		assert_eq!(plain, 7);
	}

	#[test]
	fn a_file_is_what_its_name_says() {
		let zip = b"PK\x03\x04rest";
		let rar4 = b"Rar!\x1A\x07\x00rest";
		let rar5 = b"Rar!\x1A\x07\x01\x00rest";
		let pdf = b"%PDF-1.7\n";
		assert_eq!(sniff_format("epub", zip).unwrap(), "epub");
		assert_eq!(sniff_format("zip", zip).unwrap(), "zip");
		assert_eq!(sniff_format("cbz", zip).unwrap(), "cbz");
		assert_eq!(sniff_format("cbr", rar4).unwrap(), "cbr");
		assert_eq!(sniff_format("cbr", rar5).unwrap(), "cbr");
		assert_eq!(sniff_format("pdf", pdf).unwrap(), "pdf");
		// a cbr that is really a zip is a cbz, and the other way around
		assert_eq!(sniff_format("cbr", zip).unwrap(), "cbz");
		assert_eq!(sniff_format("cbz", rar4).unwrap(), "cbr");
		// anything else is not
		for (extension, head) in [
			("epub", &rar4[..]),
			("epub", b"hello"),
			("pdf", zip),
			("pdf", b""),
			("zip", pdf),
			("cbz", b"<html>"),
			("cbr", pdf),
		] {
			assert!(sniff_format(extension, head).is_err(), "{extension}");
		}
		// a PDF may start with some bytes of junk, but not far in
		let mut late = vec![b' '; 1000];
		late.extend_from_slice(b"%PDF-1.4");
		assert!(sniff_format("pdf", &late).is_ok());
		let mut too_late = vec![b' '; 1030];
		too_late.extend_from_slice(b"%PDF-1.4");
		assert!(sniff_format("pdf", &too_late).is_err());
	}

	#[test]
	fn a_name_is_clean() {
		assert_eq!(clean_name("  Book.epub "), "Book.epub");
		assert_eq!(clean_name("Series\\v01.cbz"), "Series/v01.cbz");
		assert_eq!(clean_name("bad\u{0}name\n.epub"), "badname.epub");
		let long = "é".repeat(300);
		assert!(clean_name(&long).len() <= 255);
		assert!(clean_name(&long).chars().all(|c| c == 'é'));
	}

	#[test]
	fn room_is_held_until_the_upload_ends() {
		let (a, b) = ("reservation-test-a", "reservation-test-b");
		assert_eq!(reserved_by(a), 0);
		let first = Reservation::new(a, 100);
		let second = Reservation::new(a, 50);
		let other = Reservation::new(b, 7);
		assert_eq!(reserved_by(a), 150);
		assert_eq!(reserved_by(b), 7);
		drop(first);
		assert_eq!(reserved_by(a), 50);
		drop(second);
		drop(other);
		assert_eq!(reserved_by(a), 0);
		assert_eq!(reserved_by(b), 0);
	}
}
