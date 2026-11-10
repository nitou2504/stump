//! The *My reading* page: what a person has finished and is reading now, for themselves and, when
//! they allow it, for everyone else on the server.
//!
//! Who is asking is the *viewer*, whose history it is is the *reader* (the same person on their
//! own page). Everything here is the reader's history **seen by the viewer**: the books are the
//! ones the viewer may read (their hidden libraries, age restriction and removed books apply to
//! what is listed, counted, searched and dated), and a person is only shown as a [`ReaderProfile`]
//! (a name and an avatar). It never carries the time spent, the devices, a locator, a journal
//! entry, a bookmark or an annotation, and it never returns a `User` or a `ReadingSession`.
//! See `models::services::reading_history` for the rules of what counts as finished.

use std::collections::HashMap;

use async_graphql::{
	ComplexObject, Context, CustomValidator, Enum, InputValueError, Result, SimpleObject,
};
use chrono::Datelike;
use models::{
	entity::{media, user::AuthUser},
	services::reading_history::{self, FinishedRead, ReadingNowRow, SessionRow},
};
use num_traits::ToPrimitive;
use sea_orm::{prelude::*, QuerySelect, QueryTrait};

use crate::{
	data::{AuthContext, CoreContext},
	filter::search::media_search_condition,
	object::{media::Media, reader_profile::ReaderProfile},
	pagination::{list_limit, OffsetPagination, OffsetPaginationInfo},
};

/// What a user gets for a reader that doesn't exist, never shared, stopped sharing, or was deleted
/// or locked: the same, so that the answer doesn't tell which of them it is
pub const READER_NOT_FOUND: &str = "Reader not found";

/// The longest search a history takes; more is cut
const MAX_SEARCH_CHARS: usize = 200;

/// What a person lets the other people on the server see of their reading
#[derive(Debug, Clone, Copy, PartialEq, Eq, SimpleObject)]
pub struct ReadingPrivacy {
	/// Whether everyone on the server can see what the person is reading now and has finished.
	/// It is off until they turn it on
	pub share_reading_history: bool,
}

/// How many books were finished in a year
#[derive(Debug, Clone, Copy, PartialEq, Eq, SimpleObject)]
pub struct ReadingYear {
	pub year: i32,
	pub count: u64,
}

/// A book the reader is reading now
#[derive(Debug, Clone, SimpleObject)]
pub struct ReadingNow {
	pub media: Media,
	/// How far along the reader is, 0 to 100, when it can be told. It is the reader's, unlike the
	/// progress of the book itself (`Media.readProgress`), which is always the viewer's
	pub percentage: Option<i32>,
	/// The logical day the reader last read it
	pub last_read_on: Date,
	/// Which read of the book it is, 1 for the first
	pub readthrough_number: i32,
}

/// A book the reader has finished: one read of it (a book read twice is listed twice)
#[derive(Debug, Clone, SimpleObject)]
pub struct FinishedReading {
	pub media: Media,
	/// Which read of the book it is, 1 for the first
	pub readthrough_number: i32,
	/// The logical day the reader started this read
	pub started_on: Date,
	/// The logical day the reader finished this read
	pub finished_on: Date,
}

/// A page of the books a reader has finished
#[derive(Debug, SimpleObject)]
pub struct FinishedReadingPage {
	pub nodes: Vec<FinishedReading>,
	pub page_info: OffsetPaginationInfo,
}

/// The order of the books a reader has finished
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Enum)]
pub enum ReadingHistoryOrder {
	/// The last one finished first
	#[default]
	NewestFirst,
	/// The first one finished first
	OldestFirst,
}

/// A page is at least the first and holds at least one book
#[derive(Default)]
pub struct FinishedPaginationValidator;

impl CustomValidator<OffsetPagination> for FinishedPaginationValidator {
	fn check(
		&self,
		input: &OffsetPagination,
	) -> Result<(), InputValueError<OffsetPagination>> {
		if !input.zero_based.unwrap_or(false) && input.page == 0 {
			Err(InputValueError::custom("Page must be greater than 0"))
		} else if input.page_size == Some(0) {
			Err(InputValueError::custom("Page size must be greater than 0"))
		} else {
			Ok(())
		}
	}
}

/// The reading history of one person (the reader), as the viewer may see it
#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct ReadingHistory {
	/// Whose history it is
	pub reader: ReaderProfile,
	/// Whether it is the history of the person asking
	pub is_self: bool,
	#[graphql(skip)]
	reader_id: String,
}

impl ReadingHistory {
	pub fn new(reader: ReaderProfile, is_self: bool) -> Self {
		Self {
			reader_id: reader.id.clone(),
			reader,
			is_self,
		}
	}

	/// The reader's sessions for the books the viewer may read (narrowed by the search, if any)
	async fn sessions(
		&self,
		ctx: &Context<'_>,
		search: Option<&str>,
	) -> Result<Vec<SessionRow>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let mut visible = media::Entity::find_for_user(user);
		if let Some(search) = search
			.map(|search| search.chars().take(MAX_SEARCH_CHARS).collect::<String>())
			.filter(|search| !search.trim().is_empty())
		{
			visible = visible.filter(media_search_condition(&search));
		}
		let visible = visible.select_only().column(media::Column::Id).into_query();

		Ok(reading_history::load_sessions(conn, &self.reader_id, visible).await?)
	}
}

/// The books with these ids that the viewer may read, by id
async fn load_books(
	conn: &DbConn,
	viewer: &AuthUser,
	ids: Vec<String>,
) -> Result<HashMap<String, media::ModelWithMetadata>> {
	if ids.is_empty() {
		return Ok(HashMap::new());
	}

	let books = media::ModelWithMetadata::find_for_user(viewer)
		.filter(media::Column::Id.is_in(ids))
		.into_model::<media::ModelWithMetadata>()
		.all(conn)
		.await?;
	Ok(books
		.into_iter()
		.map(|book| (book.media.id.clone(), book))
		.collect())
}

/// How far along a book is, 0 to 100: the percentage the reader saved (an ebook), or else the page
/// they are on of the book's pages (a comic or a PDF). `None` when neither is known
fn progress_percent(
	end_percentage: Option<Decimal>,
	end_page: Option<i32>,
	pages: i32,
) -> Option<i32> {
	let percent = match (end_percentage, end_page) {
		// checked: a percentage that is out of range must not panic (that aborts the server)
		(Some(fraction), _) => {
			fraction.checked_mul(Decimal::from(100))?.to_f64()?.round()
		},
		(None, Some(page)) if page > 0 && pages > 0 => {
			(f64::from(page) / f64::from(pages) * 100.0).round()
		},
		_ => return None,
	};
	Some(percent.clamp(0.0, 100.0) as i32)
}

#[ComplexObject]
impl ReadingHistory {
	/// How many books the reader finished in each year, the latest year first. A search counts
	/// only the books it finds
	#[graphql(complexity = "crate::complexity::SMALL_LIST * child_complexity")]
	async fn years(
		&self,
		ctx: &Context<'_>,
		search: Option<String>,
	) -> Result<Vec<ReadingYear>> {
		let sessions = self.sessions(ctx, search.as_deref()).await?;

		Ok(
			reading_history::years(&reading_history::finished_reads(&sessions))
				.into_iter()
				.map(|(year, count)| ReadingYear { year, count })
				.collect(),
		)
	}

	/// The books the reader is reading now, the one read last first: at most 200 (`limit` for
	/// fewer)
	#[graphql(complexity = "crate::complexity::list_cost(limit, child_complexity)")]
	async fn reading(
		&self,
		ctx: &Context<'_>,
		limit: Option<u64>,
	) -> Result<Vec<ReadingNow>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let sessions = self.sessions(ctx, None).await?;
		let mut now: Vec<ReadingNowRow> = reading_history::reading_now(&sessions);
		now.truncate(list_limit(limit) as usize);

		let books = load_books(
			conn,
			user,
			now.iter().map(|row| row.media_id.clone()).collect(),
		)
		.await?;
		Ok(now
			.into_iter()
			.filter_map(|row| {
				let book = books.get(&row.media_id)?;
				Some(ReadingNow {
					percentage: progress_percent(
						row.end_percentage,
						row.end_page,
						book.media.pages,
					),
					media: Media::from(book.clone()),
					last_read_on: row.last_read_on,
					readthrough_number: row.readthrough_number,
				})
			})
			.collect())
	}

	/// The books the reader has finished, a page at a time (at most 200 a page), the last one
	/// finished first. `search` finds books by title, author, series and the like (every word
	/// must match), `year` keeps those finished in a year. A book read twice is listed twice
	#[graphql(
		complexity = "crate::complexity::list_cost(pagination.page_size, child_complexity)"
	)]
	async fn finished(
		&self,
		ctx: &Context<'_>,
		search: Option<String>,
		year: Option<i32>,
		#[graphql(default)] order: ReadingHistoryOrder,
		#[graphql(default, validator(custom = "FinishedPaginationValidator"))]
		pagination: OffsetPagination,
	) -> Result<FinishedReadingPage> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let sessions = self.sessions(ctx, search.as_deref()).await?;
		let mut finished = reading_history::finished_reads(&sessions);
		if let Some(year) = year {
			finished.retain(|read| read.finished_on.year() == year);
		}
		if order == ReadingHistoryOrder::OldestFirst {
			finished.reverse();
		}

		let total = finished.len() as u64;
		let page: Vec<FinishedRead> = finished
			.into_iter()
			.skip(usize::try_from(pagination.offset()).unwrap_or(usize::MAX))
			.take(pagination.limit() as usize)
			.collect();

		let books = load_books(
			conn,
			user,
			page.iter().map(|read| read.media_id.clone()).collect(),
		)
		.await?;
		let nodes = page
			.into_iter()
			.filter_map(|read| {
				Some(FinishedReading {
					media: Media::from(books.get(&read.media_id)?.clone()),
					readthrough_number: read.readthrough_number,
					started_on: read.started_on,
					finished_on: read.finished_on,
				})
			})
			.collect();

		Ok(FinishedReadingPage {
			nodes,
			page_info: OffsetPaginationInfo::new(pagination, total),
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn decimal(value: &str) -> Option<Decimal> {
		Some(value.parse::<Decimal>().expect("a decimal"))
	}

	#[test]
	fn progress_is_the_saved_percentage_or_the_page() {
		assert_eq!(progress_percent(decimal("0.425"), None, 0), Some(43));
		assert_eq!(progress_percent(decimal("0.4"), Some(3), 100), Some(40));
		assert_eq!(progress_percent(None, Some(25), 100), Some(25));
		assert_eq!(progress_percent(None, Some(1), 3), Some(33));
	}

	#[test]
	fn progress_stays_in_range() {
		assert_eq!(progress_percent(decimal("1.5"), None, 0), Some(100));
		assert_eq!(progress_percent(decimal("-0.5"), None, 0), Some(0));
		assert_eq!(progress_percent(None, Some(500), 100), Some(100));
	}

	#[test]
	fn progress_without_anything_to_tell_it_by_is_none() {
		assert_eq!(progress_percent(None, None, 100), None);
		assert_eq!(progress_percent(None, Some(0), 100), None);
		assert_eq!(progress_percent(None, Some(5), 0), None);
	}

	/// The field names of a type of the schema, in order (the descriptions and arguments left out)
	fn fields_of(sdl: &str, type_name: &str) -> Vec<String> {
		let body = sdl
			.split(&format!("type {type_name} {{"))
			.nth(1)
			.and_then(|rest| rest.split("\n}").next())
			.unwrap_or_else(|| panic!("{type_name} is in the schema"));
		let mut fields = Vec::new();
		let mut in_description = false;
		for line in body.lines().map(str::trim) {
			if line == r#"""""# {
				in_description = !in_description;
			} else if !in_description && !line.is_empty() && !line.starts_with('"') {
				let name = line
					.split(|c| c == ':' || c == '(')
					.next()
					.expect("a field")
					.trim();
				fields.push(name.to_string());
			}
		}
		fields
	}

	/// What a friend may see of a history is these fields and not one more: never the time spent,
	/// the devices, a locator, a journal entry, a bookmark or an annotation, and never a `User`
	/// or a `ReadingSession`. Asking for anything else is an error, not an empty answer, so a field
	/// added to one of these types later has to be added here on purpose
	#[test]
	fn nothing_but_the_book_the_day_and_how_far_can_be_asked_of_a_history() {
		let sdl = crate::schema::build_schema_bare().sdl();

		assert_eq!(
			fields_of(&sdl, "ReadingHistory"),
			["reader", "isSelf", "years", "reading", "finished"]
		);
		assert_eq!(fields_of(&sdl, "ReadingYear"), ["year", "count"]);
		assert_eq!(
			fields_of(&sdl, "ReadingNow"),
			["media", "percentage", "lastReadOn", "readthroughNumber"]
		);
		assert_eq!(
			fields_of(&sdl, "FinishedReading"),
			["media", "readthroughNumber", "startedOn", "finishedOn"]
		);
		assert_eq!(
			fields_of(&sdl, "FinishedReadingPage"),
			["nodes", "pageInfo"]
		);
		assert_eq!(fields_of(&sdl, "ReadingPrivacy"), ["shareReadingHistory"]);
		// the person is a name and an avatar (see `ReaderProfile`)
		assert_eq!(
			fields_of(&sdl, "ReaderProfile"),
			["id", "username", "avatarUrl"]
		);
	}

	/// A percentage from a client is not trusted to be small: multiplying it must not panic (a
	/// panic aborts the server)
	#[test]
	fn a_percentage_too_big_to_multiply_is_none_and_not_a_panic() {
		assert_eq!(progress_percent(Some(Decimal::MAX), None, 100), None);
		assert_eq!(progress_percent(Some(Decimal::MIN), None, 100), None);
	}
}
