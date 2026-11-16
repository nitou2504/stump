use async_graphql::InputType;
use chrono::{DateTime, Duration, Utc};
use graphql::input::media::{MediaProgressInput, PagedProgressInput};
use models::{entity::reading_session, shared::enums::ReadingStatus};
use sea_orm::{prelude::*, QueryOrder, Set};

use crate::common::TestApp;

pub async fn update_progress(app: &TestApp, book_id: &str, input: MediaProgressInput) {
	let json_input = input
		.to_value()
		.into_json()
		.expect("failed to convert to json");
	let result = app
		.execute_gql(
			r#"
        mutation UpdateMediaProgress($id: String!, $input: MediaProgressInput!) {
            updateMediaProgress(id: $id, input: $input) {
                __typename
            }
        }
        "#,
			Some(serde_json::json!({
				"id": book_id,
				"input": json_input,
			})),
		)
		.await;
	assert!(
		result.get("data").is_some_and(|data| !data.is_null()),
		"expected data in response, got: {result:?}"
	); // i.e. it worked
}

/// fudge the session updated_at to be outside the guard period where the server
/// attempts to block creating a new session after completion as a form of deduplication etc
pub async fn fudge_session_time(
	session: &reading_session::Model,
	conn: &sea_orm::DatabaseConnection,
) {
	let fudge_time =
		session.updated_at.expect("session missing updated_at") - Duration::minutes(40);
	reading_session::Entity::update_many()
		.filter(reading_session::Column::Id.eq(session.id))
		.col_expr(reading_session::Column::UpdatedAt, Expr::value(fudge_time))
		.exec(conn)
		.await
		.expect("could not update session timestamp");
}

/// fudge the session updated_at to a specific timestamp
pub async fn fudge_session_time_with_timestamp(
	session: &reading_session::Model,
	conn: &sea_orm::DatabaseConnection,
	updated_at: DateTime<Utc>,
) {
	reading_session::Entity::update_many()
		.filter(reading_session::Column::Id.eq(session.id))
		.col_expr(reading_session::Column::UpdatedAt, Expr::value(updated_at))
		.exec(conn)
		.await
		.expect("could not update session timestamp");
}

/// this will create n completed readthroughs and then fudge the timestamps to be old enough that
/// a follow-up session can be created.
///
/// Opening a finished book again is no longer a second read (a book is read or not), so the API
/// can't make readthroughs 2..n any more: the first is made through the API as a reader would, and
/// the others are written as earlier versions of Stump stored them (history that still exists)
pub async fn create_nth_readthrough(app: &TestApp, book_id: &str, n: i32) {
	let conn = app.conn();

	// start the session
	update_progress(
		app,
		book_id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 10,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	// flip to last page
	update_progress(
		app,
		book_id,
		MediaProgressInput::Paged(PagedProgressInput {
			page: 100,
			elapsed_seconds_delta: Some(300),
			..Default::default()
		}),
	)
	.await;

	let first = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book_id))
		.filter(reading_session::Column::Status.eq(ReadingStatus::Finished))
		.one(conn)
		.await
		.expect("db error")
		.expect("session should exist");
	fudge_session_time(&first, conn).await;

	let old = Utc::now() - Duration::minutes(40);
	for number in 2..=n {
		let session = reading_session::ActiveModel {
			session_date: Set(first.session_date),
			start_page: Set(Some(10)),
			end_page: Set(Some(100)),
			start_percentage: Set(first.start_percentage),
			end_percentage: Set(first.end_percentage),
			elapsed_seconds: Set(first.elapsed_seconds),
			readthrough_number: Set(number),
			status: Set(ReadingStatus::Finished),
			media_id: Set(book_id.to_string()),
			user_id: Set(first.user_id.clone()),
			created_at: Set((first.created_at + Duration::seconds(i64::from(number))).into()),
			updated_at: Set(Some(old.into())),
			..Default::default()
		};
		session.insert(conn).await.expect("could not write an earlier readthrough");
	}
}

pub async fn active_session_for_book(
	app: &TestApp,
	book_id: &str,
) -> reading_session::Model {
	let conn = app.conn();

	reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book_id))
		.filter(reading_session::Column::Status.eq(ReadingStatus::Reading))
		.order_by_desc(reading_session::Column::UpdatedAt)
		.one(conn)
		.await
		.expect("db error")
		.expect("active session should exist")
}

pub async fn latest_finished_session_for_book(
	app: &TestApp,
	book_id: &str,
) -> reading_session::Model {
	reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book_id))
		.filter(reading_session::Column::Status.eq(ReadingStatus::Finished))
		.order_by_desc(reading_session::Column::UpdatedAt)
		.one(app.conn())
		.await
		.expect("db error")
		.expect("finished session should exist")
}

/// Starts reading a finished book again as earlier versions of Stump stored it: a new session in
/// progress, numbered one past the book's latest readthrough. The API no longer does this (a
/// finished book stays finished), but such history exists and the features over it are tested
pub async fn start_reread_from_history(app: &TestApp, book_id: &str, page: i32) {
	let conn = app.conn();
	let latest = reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book_id))
		.order_by_desc(reading_session::Column::CreatedAt)
		.one(conn)
		.await
		.expect("db error")
		.expect("a finished readthrough should exist");
	let now = Utc::now();
	reading_session::ActiveModel {
		session_date: Set(now.date_naive()),
		start_page: Set(Some(page)),
		end_page: Set(Some(page)),
		elapsed_seconds: Set(Some(300)),
		readthrough_number: Set(latest.readthrough_number + 1),
		status: Set(ReadingStatus::Reading),
		media_id: Set(book_id.to_string()),
		user_id: Set(latest.user_id.clone()),
		created_at: Set(now.into()),
		updated_at: Set(Some(now.into())),
		..Default::default()
	}
	.insert(conn)
	.await
	.expect("could not write a reread");
}
