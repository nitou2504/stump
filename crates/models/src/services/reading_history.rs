//! What a person has finished reading and is reading now: the data behind the *My reading* page.
//!
//! It comes from the person's reading sessions, and only from what the *viewer* may see of it.
//! Whose history it is and who is looking are two different users: a person's own page is their
//! history seen by themselves, a friend's page is the friend's history seen by someone else.
//! Either way the sessions are narrowed to the books the viewer may read (their hidden libraries,
//! age restriction and removed books apply), so a book the viewer can't see is never listed,
//! counted, searched or dated.
//!
//! What is read from a session is the book, the logical day, the status and how far along it
//! got. Never the time spent, the devices, a locator, a journal entry, a bookmark or an
//! annotation ([`SessionRow`] holds nothing else).
//!
//! The grouping is done here, in plain functions, so it can be tested without a database: a
//! person's sessions are a few thousand rows at most.

use std::{cmp::Reverse, collections::HashMap};

use chrono::{Datelike, Utc};
use sea_orm::{
	prelude::*,
	sea_query::{OnConflict, SelectStatement},
	ActiveValue::Set,
	FromQueryResult, QueryOrder, QuerySelect, QueryTrait,
};

use crate::{
	entity::{
		reading_session,
		user::{self, AuthUser},
		user_sharing_settings,
	},
	shared::enums::ReadingStatus,
};

/// What a session row says that the history needs, and nothing more
#[derive(Debug, Clone, FromQueryResult)]
pub struct SessionRow {
	pub id: i32,
	pub media_id: String,
	pub readthrough_number: i32,
	/// The logical day of the session (the user's *day reset offset* applies)
	pub session_date: Date,
	pub status: ReadingStatus,
	pub end_percentage: Option<Decimal>,
	pub end_page: Option<i32>,
	pub created_at: DateTimeWithTimeZone,
	pub updated_at: Option<DateTimeWithTimeZone>,
}

/// How recent a session is, the same order `reading_session::Entity::newer_session_exists_subquery`
/// uses: the last update, then the creation, then the id
type Recency = (Option<DateTimeWithTimeZone>, DateTimeWithTimeZone, i32);

impl SessionRow {
	fn recency(&self) -> Recency {
		(self.updated_at, self.created_at, self.id)
	}
}

/// One finished read of a book: a readthrough that has a FINISHED session
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedRead {
	pub media_id: String,
	/// Which read of the book it is, 1 for the first
	pub readthrough_number: i32,
	/// The logical day of the first session of the read
	pub started_on: Date,
	/// The logical day of the last session that finished it
	pub finished_on: Date,
	/// How recent the finishing session is, to order the reads finished on the same day
	recency: Recency,
}

/// A book the person is reading now: the newest session of the book is still READING
#[derive(Debug, Clone, PartialEq)]
pub struct ReadingNowRow {
	pub media_id: String,
	pub readthrough_number: i32,
	/// The logical day of the newest session
	pub last_read_on: Date,
	pub end_percentage: Option<Decimal>,
	pub end_page: Option<i32>,
	recency: Recency,
}

/// The finished reads in `rows`, the latest first: by the day they were finished, then by how
/// recently, then by book.
///
/// A read is a readthrough of a book, and it is finished when one of its sessions is FINISHED
/// (the day is the last of them). A read that was given up (ABANDONED) is not listed, and neither
/// is one that was finished and then reopened within the grace period (the session went back to
/// READING: it is being read, not finished).
pub fn finished_reads(rows: &[SessionRow]) -> Vec<FinishedRead> {
	struct Read<'a> {
		started_on: Date,
		finished: Option<(Date, Recency)>,
		media_id: &'a str,
	}

	let mut reads: HashMap<(&str, i32), Read> = HashMap::new();
	for row in rows {
		let read = reads
			.entry((row.media_id.as_str(), row.readthrough_number))
			.or_insert(Read {
				started_on: row.session_date,
				finished: None,
				media_id: row.media_id.as_str(),
			});
		read.started_on = read.started_on.min(row.session_date);
		if row.status == ReadingStatus::Finished {
			let finished = (row.session_date, row.recency());
			if read.finished.is_none_or(|current| finished > current) {
				read.finished = Some(finished);
			}
		}
	}

	let mut finished: Vec<FinishedRead> = reads
		.into_iter()
		.filter_map(|((_, readthrough_number), read)| {
			read.finished.map(|(finished_on, recency)| FinishedRead {
				media_id: read.media_id.to_string(),
				readthrough_number,
				started_on: read.started_on,
				finished_on,
				recency,
			})
		})
		.collect();
	finished.sort_by(|a, b| {
		(Reverse(a.finished_on), Reverse(a.recency), &a.media_id, a.readthrough_number)
			.cmp(&(
				Reverse(b.finished_on),
				Reverse(b.recency),
				&b.media_id,
				b.readthrough_number,
			))
	});
	finished
}

/// How many reads were finished in each year, the latest year first
pub fn years(finished: &[FinishedRead]) -> Vec<(i32, u64)> {
	let mut counts: HashMap<i32, u64> = HashMap::new();
	for read in finished {
		*counts.entry(read.finished_on.year()).or_default() += 1;
	}
	let mut years: Vec<(i32, u64)> = counts.into_iter().collect();
	years.sort_by_key(|(year, _)| Reverse(*year));
	years
}

/// The books being read now, the most recently read first: those whose newest session is READING
/// (the same rule as the home's *Continue reading*)
pub fn reading_now(rows: &[SessionRow]) -> Vec<ReadingNowRow> {
	let mut newest: HashMap<&str, &SessionRow> = HashMap::new();
	for row in rows {
		let entry = newest.entry(row.media_id.as_str()).or_insert(row);
		if row.recency() > entry.recency() {
			*entry = row;
		}
	}

	let mut reading: Vec<ReadingNowRow> = newest
		.into_values()
		.filter(|row| row.status == ReadingStatus::Reading)
		.map(|row| ReadingNowRow {
			media_id: row.media_id.clone(),
			readthrough_number: row.readthrough_number,
			last_read_on: row.session_date,
			end_percentage: row.end_percentage,
			end_page: row.end_page,
			recency: row.recency(),
		})
		.collect();
	reading.sort_by(|a, b| {
		(Reverse(a.recency), &a.media_id).cmp(&(Reverse(b.recency), &b.media_id))
	});
	reading
}

/// The sessions of `reader_id` for the books in `visible_books` (a selection of book ids: the
/// ones the *viewer* may read, narrowed by a search if there is one)
pub async fn load_sessions(
	conn: &impl ConnectionTrait,
	reader_id: &str,
	visible_books: SelectStatement,
) -> Result<Vec<SessionRow>, DbErr> {
	reading_session::Entity::find()
		.filter(reading_session::Column::UserId.eq(reader_id))
		.filter(reading_session::Column::MediaId.in_subquery(visible_books))
		.select_only()
		.columns([
			reading_session::Column::Id,
			reading_session::Column::MediaId,
			reading_session::Column::ReadthroughNumber,
			reading_session::Column::SessionDate,
			reading_session::Column::Status,
			reading_session::Column::EndPercentage,
			reading_session::Column::EndPage,
			reading_session::Column::CreatedAt,
			reading_session::Column::UpdatedAt,
		])
		.into_model::<SessionRow>()
		.all(conn)
		.await
}

/// The ids of the users who let everyone see their reading history
fn sharing_users() -> SelectStatement {
	user_sharing_settings::Entity::sharing_reading_history()
		.select_only()
		.column(user_sharing_settings::Column::UserId)
		.into_query()
}

/// The user whose reading `viewer` may see, or `None`.
///
/// That is the viewer themselves, or somebody who let everyone see their history and is still
/// around (not deleted, not locked). Somebody who doesn't exist, who never shared, who stopped
/// sharing, or who was deleted or locked all give `None`: nothing tells them apart. There is no
/// way around it for anyone, the server owner included.
pub async fn viewable_reader(
	conn: &impl ConnectionTrait,
	viewer: &AuthUser,
	reader_id: &str,
) -> Result<Option<user::Model>, DbErr> {
	let reader = user::Entity::find_by_id(reader_id.to_string())
		.filter(user::Column::DeletedAt.is_null());
	if reader_id == viewer.id {
		return reader.one(conn).await;
	}
	reader
		.filter(user::Column::IsLocked.eq(false))
		.filter(user::Column::Id.in_subquery(sharing_users()))
		.one(conn)
		.await
}

/// The people other than `viewer` who let everyone see their reading history, by name. At most
/// `limit` of them
pub async fn readers_sharing_history(
	conn: &impl ConnectionTrait,
	viewer: &AuthUser,
	limit: u64,
) -> Result<Vec<user::Model>, DbErr> {
	user::Entity::find()
		.filter(user::Column::Id.in_subquery(sharing_users()))
		.filter(user::Column::Id.ne(viewer.id.clone()))
		.filter(user::Column::DeletedAt.is_null())
		.filter(user::Column::IsLocked.eq(false))
		.order_by_asc(user::Column::Username)
		.order_by_asc(user::Column::Id)
		.limit(limit)
		.all(conn)
		.await
}

/// Whether the user lets everyone see their reading history (they don't, until they say so)
pub async fn is_sharing_history(
	conn: &impl ConnectionTrait,
	user_id: &str,
) -> Result<bool, DbErr> {
	Ok(user_sharing_settings::Entity::find_by_id(user_id.to_string())
		.one(conn)
		.await?
		.is_some_and(|settings| settings.share_reading_history))
}

/// Lets everyone see the user's reading history, or stops it (at once: the next request of
/// anyone else gets nothing)
pub async fn set_sharing_history(
	conn: &impl ConnectionTrait,
	user_id: &str,
	share: bool,
) -> Result<user_sharing_settings::Model, DbErr> {
	user_sharing_settings::Entity::insert(user_sharing_settings::ActiveModel {
		user_id: Set(user_id.to_string()),
		share_reading_history: Set(share),
		updated_at: Set(Utc::now()),
	})
	.on_conflict(
		OnConflict::column(user_sharing_settings::Column::UserId)
			.update_columns([
				user_sharing_settings::Column::ShareReadingHistory,
				user_sharing_settings::Column::UpdatedAt,
			])
			.to_owned(),
	)
	.exec_with_returning(conn)
	.await
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::{Duration, NaiveDate, TimeZone};

	fn day(y: i32, m: u32, d: u32) -> NaiveDate {
		NaiveDate::from_ymd_opt(y, m, d).expect("a valid date")
	}

	/// A session row of `media` read-through `readthrough`, on `date`, whose rows are ordered by
	/// `id` (the later the id, the more recently it was written)
	fn row(
		id: i32,
		media: &str,
		readthrough: i32,
		date: NaiveDate,
		status: ReadingStatus,
	) -> SessionRow {
		let at = chrono::FixedOffset::east_opt(0)
			.unwrap()
			.with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
			.unwrap()
			+ Duration::minutes(id as i64);
		SessionRow {
			id,
			media_id: media.to_string(),
			readthrough_number: readthrough,
			session_date: date,
			status,
			end_percentage: None,
			end_page: None,
			created_at: at,
			updated_at: Some(at),
		}
	}

	use ReadingStatus::{Abandoned, Finished, Reading};

	fn ids(finished: &[FinishedRead]) -> Vec<(&str, i32)> {
		finished
			.iter()
			.map(|read| (read.media_id.as_str(), read.readthrough_number))
			.collect()
	}

	#[test]
	fn a_finished_book_is_listed_with_the_day_it_was_finished() {
		let rows = [
			row(1, "dune", 1, day(2026, 3, 1), Reading),
			row(2, "dune", 1, day(2026, 3, 2), Reading),
			row(3, "dune", 1, day(2026, 3, 5), Finished),
		];

		let finished = finished_reads(&rows);

		assert_eq!(finished.len(), 1);
		assert_eq!(finished[0].media_id, "dune");
		assert_eq!(finished[0].started_on, day(2026, 3, 1));
		assert_eq!(finished[0].finished_on, day(2026, 3, 5));
		assert_eq!(finished[0].readthrough_number, 1);
	}

	#[test]
	fn a_book_read_twice_is_two_finished_reads() {
		let rows = [
			row(1, "dune", 1, day(2025, 12, 30), Finished),
			row(2, "dune", 2, day(2026, 3, 1), Reading),
			row(3, "dune", 2, day(2026, 3, 9), Finished),
		];

		let finished = finished_reads(&rows);

		// the 2nd read first (it is the latest), then the 1st
		assert_eq!(ids(&finished), vec![("dune", 2), ("dune", 1)]);
		assert_eq!(finished[0].started_on, day(2026, 3, 1));
		assert_eq!(finished[0].finished_on, day(2026, 3, 9));
		assert_eq!(finished[1].finished_on, day(2025, 12, 30));
	}

	#[test]
	fn a_read_that_was_given_up_is_not_listed() {
		let rows = [
			row(1, "dune", 1, day(2026, 3, 1), Reading),
			row(2, "dune", 1, day(2026, 3, 2), Abandoned),
			row(3, "emma", 1, day(2026, 3, 3), Finished),
		];

		assert_eq!(ids(&finished_reads(&rows)), vec![("emma", 1)]);
	}

	/// Opening a finished book again within the grace period puts its session back to READING
	#[test]
	fn a_finished_book_that_went_back_to_reading_is_not_finished() {
		let rows = [row(1, "dune", 1, day(2026, 3, 5), Reading)];

		assert!(finished_reads(&rows).is_empty());
		assert_eq!(reading_now(&rows).len(), 1);
	}

	#[test]
	fn the_latest_finished_come_first_and_the_same_day_by_recency_then_book() {
		let rows = [
			row(1, "old", 1, day(2025, 1, 1), Finished),
			row(2, "new", 1, day(2026, 2, 1), Finished),
			// finished on the same day: the one written last is the later
			row(3, "a", 1, day(2026, 1, 15), Finished),
			row(4, "b", 1, day(2026, 1, 15), Finished),
		];

		assert_eq!(
			ids(&finished_reads(&rows)),
			vec![("new", 1), ("b", 1), ("a", 1), ("old", 1)]
		);
	}

	#[test]
	fn years_count_the_finished_reads_of_each_year_latest_first() {
		let rows = [
			row(1, "a", 1, day(2024, 12, 31), Finished),
			row(2, "b", 1, day(2025, 1, 1), Finished),
			row(3, "c", 1, day(2025, 6, 1), Finished),
			row(4, "d", 1, day(2026, 2, 1), Finished),
			row(5, "e", 1, day(2026, 3, 1), Reading),
		];

		assert_eq!(
			years(&finished_reads(&rows)),
			vec![(2026, 1), (2025, 2), (2024, 1)]
		);
	}

	#[test]
	fn a_book_is_being_read_when_its_newest_session_is_reading() {
		let rows = [
			// a first read, finished, then a second one in progress: being read
			row(1, "dune", 1, day(2025, 5, 1), Finished),
			row(2, "dune", 2, day(2026, 3, 1), Reading),
			// finished: not being read
			row(3, "emma", 1, day(2026, 3, 2), Finished),
			// given up: not being read
			row(4, "ulysses", 1, day(2026, 3, 3), Abandoned),
			// reading, with two sessions: once
			row(5, "kim", 1, day(2026, 3, 4), Reading),
			row(6, "kim", 1, day(2026, 3, 6), Reading),
		];

		let now = reading_now(&rows);

		// the most recently read first
		assert_eq!(
			now.iter()
				.map(|r| (r.media_id.as_str(), r.readthrough_number))
				.collect::<Vec<_>>(),
			vec![("kim", 1), ("dune", 2)]
		);
		assert_eq!(now[0].last_read_on, day(2026, 3, 6));
		// the 1st read of dune is in the finished list too
		assert_eq!(ids(&finished_reads(&rows)), vec![("emma", 1), ("dune", 1)]);
	}

	#[test]
	fn a_book_read_again_after_giving_up_is_being_read() {
		let rows = [
			row(1, "dune", 1, day(2026, 1, 1), Abandoned),
			row(2, "dune", 2, day(2026, 2, 1), Reading),
		];

		assert_eq!(reading_now(&rows).len(), 1);
		assert!(finished_reads(&rows).is_empty());
	}

	#[test]
	fn nothing_read_is_nothing_listed() {
		assert!(finished_reads(&[]).is_empty());
		assert!(reading_now(&[]).is_empty());
		assert!(years(&[]).is_empty());
	}
}
