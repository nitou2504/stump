use crate::common::{
	book::{fudge_session_time, update_progress},
	series::setup_single_series_with_n_books,
	TestApp,
};

use graphql::input::media::{EpubProgressInput, MediaProgressInput, PagedProgressInput};
use models::{
	entity::{media, reading_session},
	shared::{enums::ReadingStatus, readium::ReadiumLocator},
};
use sea_orm::{prelude::*, ExprTrait, QueryOrder};
use serde_json::json;
use tests::fake_data;

async fn setup() -> (TestApp, media::Model) {
	let app = TestApp::new_with_default_user().await;

	let (_, books) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			id: Some("black_science".to_string()),
			name: Some("Black Science".to_string()),
			..Default::default()
		},
		1,
	)
	.await;

	let book = books
		.into_iter()
		.next()
		.expect("should have created a book");

	(app, book)
}

/// if a session does not exist for a user+book pair, a new session should be created
#[tokio::test]
async fn test_start_reading_session() {
	let (app, book) = setup().await;

	let conn = app.conn();

	// no sessions for this book should exist
	let sessions_count = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.count(conn)
		.await
		.expect("could not query reading sessions");
	assert_eq!(sessions_count, 0);

	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 10,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	// a session for this book should now exist
	let sessions_count = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.count(conn)
		.await
		.expect("could not query reading sessions");
	assert_eq!(sessions_count, 1);

	let session = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.one(conn)
		.await
		.expect("could not query reading sessions")
		.expect("no session found");
	assert_eq!(session.end_page, Some(10));
}

/// if a session already exists and the grace period has not elapsed, the existing
/// session should be extended instead of creating a new session
#[tokio::test]
async fn test_extend_existing_reading_session() {
	let (app, book) = setup().await;

	let conn = app.conn();

	// start the session
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 10,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	// flip to next page
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 11,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	let session = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.one(conn)
		.await
		.expect("could not query reading sessions")
		.expect("no session found");
	assert_eq!(session.end_page, Some(11));
	// the elapsed seconds should have been added together, not overwritten
	assert_eq!(session.elapsed_seconds, Some(600));
}

/// if the grace period has elapsed, a new session should be created instead of extending
/// the existing session
#[tokio::test]
async fn test_new_session_on_elapsed_grace_period() {
	let (app, book) = setup().await;

	let conn = app.conn();

	// start the session
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 10,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	// fudge the created_at to be outside the grace period, since manipulating time in rust is way
	// more annoying than js :(
	let session = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.one(conn)
		.await
		.expect("could not query reading sessions")
		.expect("no session found");
	let fudge_time = session.created_at - chrono::Duration::minutes(40);
	// cannot use active_model here since it auto-updates the timestamps
	reading_session::Entity::update_many()
		.filter(reading_session::Column::Id.eq(session.id))
		.col_expr(reading_session::Column::UpdatedAt, Expr::value(fudge_time))
		.exec(conn)
		.await
		.expect("could not update session timestamp");

	// flip to next page, which should create a new session since the grace period has elapsed
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 11,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	let sessions = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.order_by_asc(reading_session::Column::CreatedAt)
		.all(conn)
		.await
		.expect("could not query reading sessions");
	assert_eq!(sessions.len(), 2);

	let latest_session = sessions.last().expect("no session found");
	assert_eq!(latest_session.end_page, Some(11));
	// it doesn't accumulate elapsed seconds since it's a new session
	assert_eq!(latest_session.elapsed_seconds, Some(300));
}

/// if the user finishes the book, the session should be marked as such
#[tokio::test]
async fn test_detect_finishing_session() {
	let (app, book) = setup().await;

	let conn = app.conn();

	// start the session
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 10,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	// flip to last page
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 100,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	let session = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.one(conn)
		.await
		.expect("could not query reading sessions")
		.expect("no session found");
	assert_eq!(session.end_page, Some(100));
	assert!(session.is_finalized());
}

/// a finished book stays finished: opening it again later (to look at a note) is not a second read,
/// so no new session and no second readthrough is made, and it stays finished
#[tokio::test]
async fn test_opening_a_finished_book_again_is_not_a_second_read() {
	let (app, book) = setup().await;

	let conn = app.conn();

	// start the session
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 10,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	// flip to last page
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 100,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	let session = reading_session::Entity::find()
		.filter(
			reading_session::Column::MediaId
				.eq(book.id.clone())
				.and(reading_session::Column::Status.eq(ReadingStatus::Finished)),
		)
		.one(conn)
		.await
		.expect("could not query reading sessions")
		.expect("no session found");
	assert_eq!(session.readthrough_number, 1); // start at 1

	// we have to fudge the updated_at to be outside the guard period where it attempts to block creating a new session
	// after completion as a form of deduplication etc
	let fudge_time = session.updated_at.expect("session missing updated_at")
		- chrono::Duration::minutes(40);
	reading_session::Entity::update_many()
		.filter(reading_session::Column::Id.eq(session.id))
		.col_expr(reading_session::Column::UpdatedAt, Expr::value(fudge_time))
		.exec(conn)
		.await
		.expect("could not update session timestamp");

	// opened again later, at a note on page 10: not a new read
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 10,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	let all_sessions = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.all(conn)
		.await
		.expect("could not query reading sessions");
	assert_eq!(all_sessions.len(), 1, "no second session");
	assert_eq!(all_sessions[0].readthrough_number, 1);
	assert_eq!(all_sessions[0].status, ReadingStatus::Finished);
	assert_eq!(
		all_sessions[0].end_page,
		Some(100),
		"the finished read is untouched"
	);
}

/// going back a page right after finishing (within grace) does not re-open the read: it stays
/// finished, one session, one readthrough
#[tokio::test]
async fn test_a_finished_book_stays_finished_within_grace() {
	let (app, book) = setup().await;

	let conn = app.conn();

	// first readthrough start
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 10,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	// first readthrough finish
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 100,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	// regress progress
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 99,
			elapsed_seconds_delta: Some(120),
			..Default::default()
		}),
	)
	.await;

	let latest_before_finish = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.order_by_desc(reading_session::Column::CreatedAt)
		.one(conn)
		.await
		.expect("could not query reading sessions")
		.expect("no session found");
	assert_eq!(latest_before_finish.readthrough_number, 1);
	assert_eq!(latest_before_finish.status, ReadingStatus::Finished);

	let sessions_after_reopen = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.all(conn)
		.await
		.expect("could not query reading sessions");
	assert_eq!(sessions_after_reopen.len(), 1);

	// finish again
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 100,
			elapsed_seconds_delta: Some(180),
			..Default::default()
		}),
	)
	.await;

	let all_sessions = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.order_by_asc(reading_session::Column::CreatedAt)
		.all(conn)
		.await
		.expect("could not query reading sessions");
	assert_eq!(all_sessions.len(), 1);

	let latest_after_finish = all_sessions.last().expect("no session found");
	assert_eq!(latest_after_finish.readthrough_number, 1);
	assert_eq!(latest_after_finish.status, ReadingStatus::Finished);

	let finished_count = all_sessions
		.iter()
		.filter(|s| s.status == ReadingStatus::Finished)
		.count();
	assert_eq!(finished_count, 1);
}

/// if the user finishes an ebook, the session should be marked as such
#[tokio::test]
async fn test_detect_finishing_session_for_ebook() {
	let app = TestApp::new_with_default_user().await;
	let db = app.conn();

	let series = fake_data::Series {
		name: Some("The Wayfarers".to_string()),
		..Default::default()
	}
	.insert(db)
	.await;

	let book = fake_data::Media {
		series_id: series.id.clone(),
		id: Some("book-1".to_string()),
		name: Some("A Long Way to a Small Angry Planet".to_string()),
		created_at: Some("1605-01-16T00:00:00Z".parse().unwrap()),
		pages: Some(432),
		..Default::default()
	}
	.insert(db)
	.await;

	let conn = app.conn();

	fn locator(chapter: usize) -> ReadiumLocator {
		ReadiumLocator {
			chapter_title: format!("Chapter {chapter}"),
			href: format!("chapter{chapter}.xhtml"),
			r#type: "application/xhtml+xml".to_string(),
			..Default::default()
		}
	}

	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Epub(Box::new(EpubProgressInput {
			locator: locator(1),
			percentage: Some(Decimal::new(2, 1)),
			is_complete: Some(false),
			elapsed_seconds_delta: Some(300),
			device_id: None,
			reset_elapsed_seconds: Some(false),
		})),
	)
	.await;

	// session not complete
	let session = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.one(conn)
		.await
		.expect("could not query reading sessions")
		.expect("no session found");
	assert!(!session.is_finalized());

	// flip to last chapter
	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Epub(Box::new(EpubProgressInput {
			locator: locator(23),
			percentage: Some(Decimal::new(100, 0)),
			is_complete: Some(true),
			elapsed_seconds_delta: Some(300),
			device_id: None,
			reset_elapsed_seconds: Some(false),
		})),
	)
	.await;

	// now session should be marked as complete
	let session = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book.id.clone()))
		.one(conn)
		.await
		.expect("could not query reading sessions")
		.expect("no session found");
	assert!(session.is_finalized());
}

async fn is_latest(app: &TestApp, id: i32) -> bool {
	let result = app
		.execute_gql(
			r#"
		query SessionById($sessionId: Int) {
			readingSessionById(id: $sessionId) {
				isLatest
			}
		}
		"#,
			Some(json!({"sessionId": id})),
		)
		.await;

	let session_selection = result
		.get("data")
		.and_then(|data| data.get("readingSessionById"))
		.expect("session should exist");

	session_selection
		.get("isLatest")
		.expect("isLatest is missing??")
		.as_bool()
		.expect("should parse as a bool")
}

/// the latest active session should have isLatest of true
#[tokio::test]
async fn test_latest_reading_session_is_latest() {
	let app = TestApp::new_with_default_user().await;
	let db = app.conn();

	let series = fake_data::Series {
		name: Some("The Wayfarers".to_string()),
		..Default::default()
	}
	.insert(db)
	.await;

	let book = fake_data::Media {
		series_id: series.id.clone(),
		id: Some("book-1".to_string()),
		name: Some("A Long Way to a Small Angry Planet".to_string()),
		created_at: Some("1605-01-16T00:00:00Z".parse().unwrap()),
		pages: Some(432),
		..Default::default()
	}
	.insert(db)
	.await;

	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 50,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	let first_session = reading_session::Entity::find()
		.one(db)
		.await
		.expect("should have queried correctly")
		.expect("session should have existed");
	// latest while active
	assert!(
		is_latest(&app, first_session.id).await,
		"should have been latest"
	);
	fudge_session_time(&first_session, db).await;
	// latest after lapsed
	assert!(
		is_latest(&app, first_session.id).await,
		"should have still been latest"
	);
	// ^ mostly sanity checks, lapsed or not should not matter

	update_progress(
		&app,
		&book.id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 100,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;
	// ^ the subsequent session supplants the first

	let second_session = reading_session::Entity::find()
		.filter(reading_session::Column::Id.ne(first_session.id))
		.one(db)
		.await
		.expect("should have queried correctly")
		.expect("should have another session");

	assert!(
		is_latest(&app, second_session.id).await,
		"second session should have been latest now"
	);
	assert!(
		!is_latest(&app, first_session.id).await,
		"first session should have no longer been latest"
	);
}
