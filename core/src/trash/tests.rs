use std::{fs, path::Path, sync::Arc};

use ::tests::{db::test_database, fake_data};
use apalis::prelude::{Data, MemoryStorage};
use chrono::{DateTime, Duration, Utc};
use models::{
	entity::{
		bookmark, held_file, media, media_metadata, media_purge, media_tag,
		post_processing_task, reading_session, server_config, tag,
	},
	shared::{
		post_processing::{PostProcessKind, PostProcessStatus},
		trash::{HeldFileReason, TrashPurgeSummary},
	},
};
use sea_orm::{prelude::*, ActiveValue::Set, DatabaseConnection, IntoActiveModel};
use tempfile::TempDir;

use super::*;
use crate::job::{context::ApalisWorkerState, dispatch_job};

struct Setup {
	_dir: TempDir,
	conn: DatabaseConnection,
	config: StumpConfig,
	library_path: PathBuf,
	user_id: String,
	series_id: String,
}

/// A library folder and Stump's config folder in a temporary folder, and a database with a
/// library, a series, a user and the server config
async fn setup() -> Setup {
	let dir = tempfile::tempdir().unwrap();
	let library_path = dir.path().join("lib");
	fs::create_dir_all(&library_path).unwrap();
	let config = StumpConfig {
		config_dir: dir.path().join("config").to_string_lossy().into_owned(),
		..StumpConfig::debug()
	};
	fs::create_dir_all(config.thumbnails_directory()).unwrap();

	let conn = test_database().await;
	server_config::ActiveModel {
		..Default::default()
	}
	.insert(&conn)
	.await
	.unwrap();
	let library = fake_data::Library {
		path: Some(library_path.to_string_lossy().into_owned()),
		..Default::default()
	}
	.insert(&conn)
	.await;
	let series = fake_data::Series {
		library_id: Some(library.id),
		..Default::default()
	}
	.insert(&conn)
	.await;
	let user = fake_data::User::new("remover").insert(&conn).await;

	Setup {
		_dir: dir,
		conn,
		config,
		library_path,
		user_id: user.id,
		series_id: series.id,
	}
}

impl Setup {
	/// A book in Deleted books, removed `days_ago`, with its file at `rel` in the library
	/// unless `with_file` is false
	async fn trashed_book(
		&self,
		rel: &str,
		days_ago: i64,
		with_file: bool,
	) -> media::Model {
		let path = self.library_path.join(rel);
		if with_file {
			fs::create_dir_all(path.parent().unwrap()).unwrap();
			fs::write(&path, format!("the bytes of {rel}")).unwrap();
		}
		let book = fake_data::Media {
			series_id: self.series_id.clone(),
			path: Some(path.to_string_lossy().into_owned()),
			..Default::default()
		}
		.insert(&self.conn)
		.await;
		media_metadata::ActiveModel {
			media_id: Set(Some(book.id.clone())),
			title: Set(Some(format!("Title of {rel}"))),
			writers: Set(Some("Umberto Eco, Someone Else".to_string())),
			..Default::default()
		}
		.insert(&self.conn)
		.await
		.unwrap();
		let mut active = book.into_active_model();
		active.deleted_at = Set(Some((Utc::now() - Duration::days(days_ago)).into()));
		active.deleted_by = Set(Some(self.user_id.clone()));
		active.update(&self.conn).await.unwrap()
	}

	async fn book(&self, id: &str) -> Option<media::Model> {
		media::Entity::find_by_id(id).one(&self.conn).await.unwrap()
	}

	/// A book in its library (not removed), with its file
	async fn live_book(&self, rel: &str, bytes: &[u8]) -> media::Model {
		let path = self.library_path.join(rel);
		fs::create_dir_all(path.parent().unwrap()).unwrap();
		fs::write(&path, bytes).unwrap();
		fake_data::Media {
			series_id: self.series_id.clone(),
			path: Some(path.to_string_lossy().into_owned()),
			size: Some(bytes.len() as i64),
			..Default::default()
		}
		.insert(&self.conn)
		.await
	}

	/// What post-processing does to a book: its original held in the trash folder `days_ago`,
	/// the processed bytes in the library, a task saying so
	async fn processed(
		&self,
		book: &media::Model,
		processed: &[u8],
		days_ago: i64,
	) -> held_file::Model {
		let library_path = self.library_path.clone();
		let (absolute, relative) = held::destination(
			&self.config,
			&library_path,
			Path::new(&book.path),
			Utc::now(),
		);
		let bytes = files::copy_new(Path::new(&book.path), &absolute).unwrap();
		fs::write(&book.path, processed).unwrap();
		post_processing_task::ActiveModel {
			id: Set(format!("task-{}", book.id)),
			library_id: Set(book.library_id.clone()),
			media_id: Set(Some(book.id.clone())),
			kind: Set(PostProcessKind::Grey16),
			status: Set(PostProcessStatus::Done),
			path: Set(book.path.clone()),
			bytes_before: Set(Some(bytes as i64)),
			bytes_after: Set(Some(processed.len() as i64)),
			..Default::default()
		}
		.insert(&self.conn)
		.await
		.unwrap();
		let row = held::record(
			&self.conn,
			book,
			"A manga".to_string(),
			&relative,
			bytes,
			held::HeldFileInfo {
				reason: HeldFileReason::Grey16,
				steps: None,
				task_id: Some(format!("task-{}", book.id)),
				held_by: None,
			},
		)
		.await
		.unwrap();
		let mut active = row.into_active_model();
		active.held_at = Set(Utc::now() - Duration::days(days_ago));
		active.update(&self.conn).await.unwrap()
	}

	fn ctx(&self) -> Ctx {
		let mut ctx = Ctx::for_testing(self.conn.clone());
		ctx.config = Arc::new(self.config.clone());
		ctx
	}
}

/// Every file under `dir`, hidden ones included
fn files_under(dir: &Path) -> Vec<PathBuf> {
	let mut out = vec![];
	for entry in walkdir::WalkDir::new(dir)
		.into_iter()
		.filter_map(Result::ok)
	{
		if entry.file_type().is_file() {
			out.push(entry.path().to_path_buf());
		}
	}
	out.sort();
	out
}

fn ids(books: &[&media::Model]) -> Vec<String> {
	books.iter().map(|b| b.id.clone()).collect()
}

#[tokio::test]
async fn test_purge_deletes_the_file_and_the_data() {
	let setup = setup().await;
	let book = setup
		.trashed_book("Umberto_Eco/El_nombre.epub", 3, true)
		.await;
	let conn = &setup.conn;

	// what a reader leaves behind: progress, a bookmark, a tag
	fake_data::ReadingSession {
		media_id: book.id.clone(),
		user_id: setup.user_id.clone(),
		end_percentage: 0.4,
		..Default::default()
	}
	.insert(conn)
	.await;
	bookmark::ActiveModel {
		id: Set("bookmark".to_string()),
		page: Set(Some(3)),
		media_id: Set(book.id.clone()),
		user_id: Set(setup.user_id.clone()),
		created_at: Set(Utc::now()),
		..Default::default()
	}
	.insert(conn)
	.await
	.unwrap();
	let tag = tag::ActiveModel {
		name: Set("Kept".to_string()),
		..Default::default()
	}
	.insert(conn)
	.await
	.unwrap();
	media_tag::ActiveModel {
		media_id: Set(book.id.clone()),
		tag_id: Set(tag.id),
		..Default::default()
	}
	.insert(conn)
	.await
	.unwrap();
	// its thumbnail and a no-cover marker
	let thumbnail = setup
		.config
		.thumbnails_directory()
		.join(format!("{}.webp", book.id));
	fs::write(&thumbnail, b"webp").unwrap();
	let marker = setup
		.config
		.thumbnails_directory()
		.join(".no-cover")
		.join(&book.id);
	fs::create_dir_all(marker.parent().unwrap()).unwrap();
	fs::write(&marker, b"1 2").unwrap();

	let report = purge_books(conn, &setup.config, &ids(&[&book]), Some(&setup.user_id))
		.await
		.unwrap();

	let bytes = "the bytes of Umberto_Eco/El_nombre.epub".len() as u64;
	assert_eq!(report.items[0].outcome, PurgeOutcome::Deleted { bytes });
	assert_eq!((report.purged(), report.kept()), (1, 0));
	assert_eq!(report.freed_bytes(), bytes);
	// the file is gone for good: no hidden copy left next to it, nothing anywhere else
	assert!(files_under(&setup.library_path).is_empty());
	assert!(!setup.config.trash_directory().exists());

	// the data is gone, cascading to what depended on the book
	assert!(setup.book(&book.id).await.is_none());
	for count in [
		reading_session::Entity::find()
			.filter(reading_session::Column::MediaId.eq(&book.id))
			.count(conn)
			.await
			.unwrap(),
		bookmark::Entity::find().count(conn).await.unwrap(),
		media_tag::Entity::find().count(conn).await.unwrap(),
		media_metadata::Entity::find()
			.filter(media_metadata::Column::MediaId.eq(&book.id))
			.count(conn)
			.await
			.unwrap(),
	] {
		assert_eq!(count, 0);
	}
	assert!(!thumbnail.exists());
	assert!(!marker.exists());

	// and logged
	let purge = media_purge::Entity::find()
		.one(conn)
		.await
		.unwrap()
		.unwrap();
	assert_eq!(purge.media_id, book.id);
	assert_eq!(purge.title, "Title of Umberto_Eco/El_nombre.epub");
	assert_eq!(purge.quarantine_path, None);
	assert_eq!(purge.bytes, Some(bytes as i64));
	assert_eq!(purge.original_path, book.path);
	assert_eq!(purge.deleted_by.as_deref(), Some(setup.user_id.as_str()));
	assert_eq!(purge.purged_by.as_deref(), Some(setup.user_id.as_str()));
}

/// When the book's data can't be deleted, its file goes back where it was and the book stays
#[tokio::test]
async fn test_purge_keeps_the_book_and_its_file_when_the_data_cant_go() {
	let setup = setup().await;
	let book = setup.trashed_book("A/Book.pdf", 30, true).await;
	// the deletion log can't be written: the transaction fails
	setup
		.conn
		.execute_unprepared("DROP TABLE media_purges")
		.await
		.unwrap();

	let report = purge_books(&setup.conn, &setup.config, &ids(&[&book]), None)
		.await
		.unwrap();
	let PurgeOutcome::Kept(reason) = &report.items[0].outcome else {
		panic!("the book should have been kept: {report:?}");
	};
	assert!(reason.contains("data couldn't be deleted"), "{reason}");
	assert_eq!(report.errors().len(), 1);
	assert_eq!(
		fs::read_to_string(setup.library_path.join("A/Book.pdf")).unwrap(),
		"the bytes of A/Book.pdf"
	);
	assert_eq!(
		files_under(&setup.library_path).len(),
		1,
		"no hidden copy left"
	);
	let kept = setup.book(&book.id).await.expect("the book should be kept");
	assert!(kept.deleted_at.is_some());
}

#[tokio::test]
async fn test_purge_of_a_missing_file_deletes_only_the_data() {
	let setup = setup().await;
	let book = setup.trashed_book("Gone/Book.epub", 30, false).await;

	let report = purge_books(&setup.conn, &setup.config, &ids(&[&book]), None)
		.await
		.unwrap();

	assert_eq!(report.items[0].outcome, PurgeOutcome::FileMissing);
	assert_eq!(report.missing_files(), 1);
	assert!(setup.book(&book.id).await.is_none());
	let purge = media_purge::Entity::find()
		.one(&setup.conn)
		.await
		.unwrap()
		.unwrap();
	assert_eq!(purge.bytes, None);
	assert_eq!(purge.purged_by, None, "the automatic purge has no user");
}

#[tokio::test]
async fn test_purge_only_takes_books_in_the_trash() {
	let setup = setup().await;
	let live = fake_data::Media {
		series_id: setup.series_id.clone(),
		path: Some(
			setup
				.library_path
				.join("Live.epub")
				.to_string_lossy()
				.into_owned(),
		),
		..Default::default()
	}
	.insert(&setup.conn)
	.await;
	fs::write(setup.library_path.join("Live.epub"), b"live").unwrap();

	let report = purge_books(
		&setup.conn,
		&setup.config,
		&[live.id.clone(), "unknown".to_string()],
		None,
	)
	.await
	.unwrap();

	assert_eq!(report.kept(), 2);
	assert!(report.items.iter().all(
		|item| item.outcome == PurgeOutcome::Kept("It isn't in Deleted books".into())
	));
	assert!(setup.book(&live.id).await.is_some());
	assert!(setup.library_path.join("Live.epub").exists());
}

/// A post-processed book's original is held; putting it back swaps the files (the processed
/// version is held in turn) and marks the task reverted; deleting a held file deletes it
#[tokio::test]
async fn test_held_original_put_back_and_deleted() {
	let setup = setup().await;
	let book = setup
		.live_book("_Manga/S/S v01.cbz", b"original bytes")
		.await;
	let original = setup.processed(&book, b"16 greys", 0).await;
	assert_eq!(original.reason, HeldFileReason::Grey16);
	assert_eq!(original.bytes, 14);
	let held_path = held::held_file_path(&setup.config, &original);
	assert!(
		held_path.ends_with("_Manga/S/S v01.cbz"),
		"{}",
		held_path.display()
	);
	assert!(held_path.starts_with(setup.config.trash_directory()));
	assert_eq!(fs::read(&held_path).unwrap(), b"original bytes");

	let ctx = setup.ctx();
	let put = held::put_back(&ctx, original.id, &setup.user_id)
		.await
		.unwrap();
	assert_eq!(fs::read(&book.path).unwrap(), b"original bytes");
	assert_eq!(put.book.size, 14, "the book follows its file");
	// the processed version is held now, the original's row and copy are gone
	let replaced = put.replaced.expect("the processed version should be held");
	assert_eq!(replaced.reason, HeldFileReason::PutBack);
	assert_eq!(replaced.held_by.as_deref(), Some(setup.user_id.as_str()));
	assert_eq!(
		fs::read(held::held_file_path(&setup.config, &replaced)).unwrap(),
		b"16 greys"
	);
	assert!(held_file::Entity::find_by_id(original.id)
		.one(&setup.conn)
		.await
		.unwrap()
		.is_none());
	assert!(!held_path.exists());
	let task = post_processing_task::Entity::find_by_id(format!("task-{}", book.id))
		.one(&setup.conn)
		.await
		.unwrap()
		.unwrap();
	assert_eq!(task.status, PostProcessStatus::Reverted);
	// no hidden leftovers next to the book
	assert_eq!(files_under(&setup.library_path).len(), 1);

	// putting the processed version back holds the original again, as an original
	let again = held::put_back(&ctx, replaced.id, &setup.user_id)
		.await
		.unwrap();
	assert_eq!(fs::read(&book.path).unwrap(), b"16 greys");
	let original_again = again.replaced.expect("the original should be held again");
	assert_eq!(original_again.reason, HeldFileReason::Grey16);
	assert_eq!(
		fs::read(held::held_file_path(&setup.config, &original_again)).unwrap(),
		b"original bytes"
	);
	let task = post_processing_task::Entity::find_by_id(format!("task-{}", book.id))
		.one(&setup.conn)
		.await
		.unwrap()
		.unwrap();
	assert_eq!(task.status, PostProcessStatus::Done);
	// and back to the original, so the processed version is the one held below
	let replaced = held::put_back(&ctx, original_again.id, &setup.user_id)
		.await
		.unwrap()
		.replaced
		.unwrap();
	assert_eq!(fs::read(&book.path).unwrap(), b"original bytes");

	// the held processed version is deleted by hand: gone from disk and from the list
	let purge = held::purge_held(&setup.conn, &setup.config, &[replaced.id])
		.await
		.unwrap();
	assert_eq!((purge.deleted, purge.bytes), (1, 8));
	assert!(files_under(&setup.config.trash_directory()).is_empty());
	assert_eq!(
		held_file::Entity::find().count(&setup.conn).await.unwrap(),
		0
	);
	assert_eq!(fs::read(&book.path).unwrap(), b"original bytes");
}

/// A book deleted permanently takes its held originals with it
#[tokio::test]
async fn test_purging_a_book_deletes_its_held_files() {
	let setup = setup().await;
	let book = setup.live_book("_Manga/S/S v02.cbz", b"original").await;
	let original = setup.processed(&book, b"grey", 0).await;
	let mut active = book.clone().into_active_model();
	active.deleted_at = Set(Some((Utc::now() - Duration::days(30)).into()));
	active.update(&setup.conn).await.unwrap();

	let report = purge_books(&setup.conn, &setup.config, &ids(&[&book]), None)
		.await
		.unwrap();
	assert_eq!(report.purged(), 1);
	assert!(held_file::Entity::find_by_id(original.id)
		.one(&setup.conn)
		.await
		.unwrap()
		.is_none());
	assert!(files_under(&setup.config.trash_directory()).is_empty());
	assert!(files_under(&setup.library_path).is_empty());
}

#[test]
fn test_expiry_and_the_daily_schedule() {
	let deleted_at = "2026-11-03T16:06:19Z".parse::<DateTime<Utc>>().unwrap();
	assert_eq!(
		expires_at(deleted_at, 14),
		Some("2026-11-17T16:06:19Z".parse().unwrap())
	);
	assert_eq!(
		expires_at(deleted_at, 0),
		None,
		"0 keeps books until deleted by hand"
	);

	let now = "2026-11-04T20:00:00Z".parse::<DateTime<Utc>>().unwrap();
	assert!(!is_expired(deleted_at, 14, now));
	assert!(is_expired(deleted_at, 1, now));
	assert!(!is_expired(deleted_at, 0, now));

	// the first daily run once its time is over
	assert_eq!(
		purge_at(deleted_at, 14, now),
		Some("2026-11-18T04:00:00Z".parse().unwrap())
	);
	// overdue: the next run
	assert_eq!(
		purge_at(deleted_at, 1, now),
		Some("2026-11-05T04:00:00Z".parse().unwrap())
	);
	assert_eq!(purge_at(deleted_at, 0, now), None);
	assert_eq!(
		next_purge_run(now, false),
		Some("2026-11-05T04:00:00Z".parse().unwrap())
	);
}

#[tokio::test]
async fn test_retention_and_expired_books() {
	let setup = setup().await;
	assert_eq!(retention_days(&setup.conn).await.unwrap(), 14);
	let old = setup.trashed_book("Old.epub", 20, true).await;
	let recent = setup.trashed_book("Recent.epub", 1, true).await;

	let now = Utc::now();
	assert_eq!(
		expired_book_ids(&setup.conn, 14, now).await.unwrap(),
		vec![old.id.clone()]
	);
	let mut both = expired_book_ids(&setup.conn, 1, now).await.unwrap();
	both.sort();
	let mut expected = vec![old.id.clone(), recent.id.clone()];
	expected.sort();
	assert_eq!(both, expected);
	assert!(expired_book_ids(&setup.conn, 0, now)
		.await
		.unwrap()
		.is_empty());

	assert_eq!(set_retention_days(&setup.conn, 0).await.unwrap(), 0);
	assert_eq!(retention_days(&setup.conn).await.unwrap(), 0);
	assert!(set_retention_days(&setup.conn, -1).await.is_err());
	assert!(set_retention_days(&setup.conn, 100_000).await.is_err());
}

/// The scheduled purge as the scheduler starts it: overdue on a new server, a note when nothing
/// expired, the job deleting the expired books and recording what it did
#[tokio::test]
async fn test_automatic_purge_job() {
	let setup = setup().await;
	// the job's own tables, which the shared test database leaves out
	let schema = sea_orm::Schema::new(sea_orm::DbBackend::Sqlite);
	for table in [
		schema.create_table_from_entity(models::entity::job::Entity),
		schema.create_table_from_entity(models::entity::log::Entity),
	] {
		setup.conn.execute(&table).await.unwrap();
	}
	let conn = Arc::new(setup.conn.clone());
	let now = Utc::now();
	assert!(purge_is_overdue(conn.as_ref(), now).await.unwrap());

	// nothing expired: no job, only a note
	let recent = setup.trashed_book("Recent.epub", 1, true).await;
	let ctx = Ctx::for_testing(setup.conn.clone());
	start_automatic_purge(&ctx).await.unwrap();
	let config = server_config::Entity::find()
		.one(conn.as_ref())
		.await
		.unwrap()
		.unwrap();
	let summary = config.trash_last_purge.clone().unwrap();
	assert_eq!((summary.purged, summary.job_id), (0, None));
	assert!(!purge_is_overdue(conn.as_ref(), Utc::now()).await.unwrap());
	assert!(
		purge_is_overdue(conn.as_ref(), Utc::now() + Duration::days(1))
			.await
			.unwrap()
	);

	// one expired with its file, one expired without a file, one kept by its retention, and
	// held originals: one expired, one recent
	let expired = setup.trashed_book("A/Expired.epub", 15, true).await;
	let missing = setup.trashed_book("A/Missing.epub", 15, false).await;
	let old_manga = setup.live_book("_Manga/S/S v01.cbz", b"old original").await;
	let old_original = setup.processed(&old_manga, b"grey", 15).await;
	let new_manga = setup.live_book("_Manga/S/S v02.cbz", b"new original").await;
	let new_original = setup.processed(&new_manga, b"grey", 1).await;
	assert_eq!(
		held::expired_held_ids(conn.as_ref(), 14, Utc::now())
			.await
			.unwrap(),
		vec![old_original.id]
	);
	let state = Arc::new(ApalisWorkerState::new(
		conn.clone(),
		Arc::new(setup.config.clone()),
		tokio::sync::broadcast::channel(64).0,
		MemoryStorage::new(),
	));
	dispatch_job(StumpJob::TrashPurge, Data::new(state))
		.await
		.expect("the job should run");

	assert!(setup.book(&expired.id).await.is_none());
	assert!(setup.book(&missing.id).await.is_none());
	assert!(setup.book(&recent.id).await.is_some(), "not expired yet");
	assert!(!setup.library_path.join("A/Expired.epub").exists());
	assert!(setup.library_path.join("Recent.epub").exists());
	assert!(!held::held_file_path(&setup.config, &old_original).exists());
	assert!(held::held_file_path(&setup.config, &new_original).exists());
	assert_eq!(
		held_file::Entity::find()
			.all(conn.as_ref())
			.await
			.unwrap()
			.into_iter()
			.map(|row| row.id)
			.collect::<Vec<_>>(),
		vec![new_original.id]
	);
	let config = server_config::Entity::find()
		.one(conn.as_ref())
		.await
		.unwrap()
		.unwrap();
	let TrashPurgeSummary {
		purged,
		missing_files,
		failed,
		originals,
		job_id,
		..
	} = config.trash_last_purge.unwrap();
	assert_eq!((purged, missing_files, failed, originals), (1, 1, 0, 1));
	assert!(job_id.is_some());
	let job = models::entity::job::Entity::find()
		.one(conn.as_ref())
		.await
		.unwrap()
		.unwrap();
	assert_eq!(job.name, "trash_purge");
	assert_eq!(job.status, crate::job::JobStatus::Completed);
}

#[tokio::test]
async fn test_load_ignores_books_outside_the_trash() {
	let setup = setup().await;
	let book = setup.trashed_book("Kept.epub", 2, true).await;
	let mut active = book.clone().into_active_model();
	active.deleted_at = Set(None);
	active.update(&setup.conn).await.unwrap();
	assert!(load_trashed(&setup.conn, std::slice::from_ref(&book.id))
		.await
		.unwrap()
		.is_empty());
	assert!(Path::new(&book.path).exists());
}
