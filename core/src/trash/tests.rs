use std::{fs, path::Path, sync::Arc};

use ::tests::{db::test_database, fake_data};
use apalis::prelude::{Data, MemoryStorage};
use chrono::{DateTime, Duration, Utc};
use models::{
	entity::{
		bookmark, media, media_metadata, media_purge, media_tag, reading_session,
		server_config, tag,
	},
	shared::trash::TrashPurgeSummary,
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
	quarantine: PathBuf,
	user_id: String,
	series_id: String,
}

/// A library folder, a quarantine folder and Stump's config folder in a temporary folder,
/// and a database with a library, a series, a user and the server config
async fn setup() -> Setup {
	let dir = tempfile::tempdir().unwrap();
	let library_path = dir.path().join("lib");
	let quarantine = dir.path().join("quarantine");
	fs::create_dir_all(&library_path).unwrap();
	fs::create_dir_all(&quarantine).unwrap();
	let config = StumpConfig {
		config_dir: dir.path().join("config").to_string_lossy().into_owned(),
		quarantine_dir: Some(quarantine.to_string_lossy().into_owned()),
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
		quarantine,
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

	fn today_dir(&self) -> PathBuf {
		quarantine::day_dir(&self.quarantine, Utc::now())
	}
}

fn ids(books: &[&media::Model]) -> Vec<String> {
	books.iter().map(|b| b.id.clone()).collect()
}

#[tokio::test]
async fn test_purge_moves_the_file_to_quarantine_and_deletes_the_data() {
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

	let quarantined = setup.today_dir().join("Umberto_Eco/El_nombre.epub");
	assert_eq!(
		report.items[0].outcome,
		PurgeOutcome::Quarantined {
			path: quarantined.clone(),
			bytes: "the bytes of Umberto_Eco/El_nombre.epub".len() as u64,
		}
	);
	assert_eq!((report.purged(), report.kept()), (1, 0));
	// the file moved, nothing was destroyed
	assert!(!setup
		.library_path
		.join("Umberto_Eco/El_nombre.epub")
		.exists());
	assert_eq!(
		fs::read_to_string(&quarantined).unwrap(),
		"the bytes of Umberto_Eco/El_nombre.epub"
	);
	let log = fs::read_to_string(setup.today_dir().join("restore_log.csv")).unwrap();
	let lines: Vec<_> = log.lines().collect();
	assert_eq!(lines[0], quarantine::RESTORE_LOG_HEADER);
	assert!(lines[1].starts_with(&format!(
		"{},{},{},remover,",
		book.path,
		quarantined.display(),
		"the bytes of Umberto_Eco/El_nombre.epub".len()
	)));
	assert!(lines[1]
		.ends_with(",Title of Umberto_Eco/El_nombre.epub,\"Umberto Eco, Someone Else\""));

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
	assert_eq!(
		purge.quarantine_path.as_deref(),
		Some(quarantined.to_string_lossy().as_ref())
	);
	assert_eq!(purge.deleted_by.as_deref(), Some(setup.user_id.as_str()));
	assert_eq!(purge.purged_by.as_deref(), Some(setup.user_id.as_str()));
}

#[tokio::test]
async fn test_purge_without_a_quarantine_folder_keeps_the_book() {
	let setup = setup().await;
	let book = setup.trashed_book("A/Book.pdf", 30, true).await;
	for config in [
		StumpConfig {
			quarantine_dir: None,
			..setup.config.clone()
		},
		// set, but not mounted
		StumpConfig {
			quarantine_dir: Some(
				setup
					.quarantine
					.join("missing")
					.to_string_lossy()
					.into_owned(),
			),
			..setup.config.clone()
		},
	] {
		let report = purge_books(&setup.conn, &config, &ids(&[&book]), None)
			.await
			.unwrap();
		let PurgeOutcome::Kept(reason) = &report.items[0].outcome else {
			panic!("the book should have been kept: {report:?}");
		};
		assert!(
			reason.contains("quarantine folder"),
			"the reason should name the quarantine folder: {reason}"
		);
		assert_eq!(report.errors().len(), 1);
		assert!(setup.library_path.join("A/Book.pdf").exists());
		let kept = setup.book(&book.id).await.expect("the book should be kept");
		assert!(kept.deleted_at.is_some());
	}
	assert_eq!(
		media_purge::Entity::find()
			.count(&setup.conn)
			.await
			.unwrap(),
		0
	);
}

#[tokio::test]
async fn test_purge_of_a_missing_file_deletes_only_the_data() {
	let setup = setup().await;
	let book = setup.trashed_book("Gone/Book.epub", 30, false).await;
	// no quarantine folder is needed when there is no file
	let config = StumpConfig {
		quarantine_dir: None,
		..setup.config.clone()
	};

	let report = purge_books(&setup.conn, &config, &ids(&[&book]), None)
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
	assert_eq!(purge.quarantine_path, None);
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

#[tokio::test]
async fn test_purge_never_overwrites_a_quarantined_file() {
	let setup = setup().await;
	let first = setup.trashed_book("A/Book.epub", 30, true).await;
	purge_books(&setup.conn, &setup.config, &ids(&[&first]), None)
		.await
		.unwrap();
	// the same path again, deleted the same day
	let second = setup.trashed_book("A/Book.epub", 30, true).await;
	let report = purge_books(&setup.conn, &setup.config, &ids(&[&second]), None)
		.await
		.unwrap();

	assert_eq!(
		report.items[0].outcome,
		PurgeOutcome::Quarantined {
			path: setup.today_dir().join("A/Book.prev1.epub"),
			bytes: "the bytes of A/Book.epub".len() as u64,
		}
	);
	assert!(setup.today_dir().join("A/Book.epub").exists());
	let log = fs::read_to_string(setup.today_dir().join("restore_log.csv")).unwrap();
	assert_eq!(log.lines().count(), 3, "one header and a row per book");
}

#[tokio::test]
async fn test_quarantine_inside_the_library_is_refused() {
	let setup = setup().await;
	let book = setup.trashed_book("A/Book.epub", 30, true).await;
	let inside = setup.library_path.join(".quarantine");
	fs::create_dir_all(&inside).unwrap();
	let config = StumpConfig {
		quarantine_dir: Some(inside.to_string_lossy().into_owned()),
		..setup.config.clone()
	};

	let report = purge_books(&setup.conn, &config, &ids(&[&book]), None)
		.await
		.unwrap();

	assert!(
		matches!(&report.items[0].outcome, PurgeOutcome::Kept(reason) if reason.contains("inside a library"))
	);
	assert!(setup.library_path.join("A/Book.epub").exists());
	assert!(setup.book(&book.id).await.is_some());
}

#[tokio::test]
async fn test_restore_from_quarantine_puts_the_file_back() {
	let setup = setup().await;
	let book = setup.trashed_book("A/Book.epub", 30, true).await;
	purge_books(&setup.conn, &setup.config, &ids(&[&book]), None)
		.await
		.unwrap();
	let purge = media_purge::Entity::find()
		.one(&setup.conn)
		.await
		.unwrap()
		.unwrap();
	let ctx = Ctx::for_testing(setup.conn.clone());

	let restored = restore_purged(&ctx, purge.id, &setup.user_id)
		.await
		.unwrap();

	assert!(restored.restored_at.is_some());
	assert_eq!(
		restored.restored_by.as_deref(),
		Some(setup.user_id.as_str())
	);
	assert_eq!(
		fs::read_to_string(setup.library_path.join("A/Book.epub")).unwrap(),
		"the bytes of A/Book.epub"
	);
	assert!(!setup.today_dir().join("A/Book.epub").exists());
	// once only
	assert!(restore_purged(&ctx, purge.id, &setup.user_id)
		.await
		.is_err());
}

#[tokio::test]
async fn test_restore_from_quarantine_never_replaces_a_file() {
	let setup = setup().await;
	let book = setup.trashed_book("A/Book.epub", 30, true).await;
	purge_books(&setup.conn, &setup.config, &ids(&[&book]), None)
		.await
		.unwrap();
	let purge = media_purge::Entity::find()
		.one(&setup.conn)
		.await
		.unwrap()
		.unwrap();
	// a new file took its place
	fs::write(setup.library_path.join("A/Book.epub"), b"newer").unwrap();
	let ctx = Ctx::for_testing(setup.conn.clone());

	let error = restore_purged(&ctx, purge.id, &setup.user_id)
		.await
		.unwrap_err();

	assert!(error.to_string().contains("already"), "{error}");
	assert_eq!(
		fs::read(setup.library_path.join("A/Book.epub")).unwrap(),
		b"newer"
	);
	assert!(setup.today_dir().join("A/Book.epub").exists());
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

	// one expired with its file, one expired without a file, one kept by its retention
	let expired = setup.trashed_book("A/Expired.epub", 15, true).await;
	let missing = setup.trashed_book("A/Missing.epub", 15, false).await;
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
	assert!(setup.today_dir().join("A/Expired.epub").exists());
	let config = server_config::Entity::find()
		.one(conn.as_ref())
		.await
		.unwrap()
		.unwrap();
	let TrashPurgeSummary {
		purged,
		missing_files,
		failed,
		job_id,
		..
	} = config.trash_last_purge.unwrap();
	assert_eq!((purged, missing_files, failed), (1, 1, 0));
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
