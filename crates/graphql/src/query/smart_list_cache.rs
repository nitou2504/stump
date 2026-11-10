//! The books of a smart list, kept instead of worked out on every visit (package SL-3).
//!
//! A smart list is a set of rules. Working them out means going over every book of the catalog
//! (a list made of hundreds of paths is one OR of hundreds of comparisons for each of 9,000
//! books), and a screen of lists asks for each list five or more times. So the books a list's
//! rules match are worked out once and kept (`smart_list_members`). What the list holds for
//! somebody is those members **narrowed to what they may read**, every time, by the same query
//! that narrows any other list of books ([`crate::query::smart_lists_builder::smart_list_books`]):
//! the cache holds what the list *is*, and never what any reader gets of it. It has no ID, count
//! or title of a book that somebody may not see, and no reader's view of a list is ever kept for
//! another.
//!
//! The members were worked out at a revision of the catalog, which a trigger bumps whenever
//! something a rule can look at changes ([`models::catalog_revision`]). A list is up to date for
//! a reader when its revision is the catalog's, **or** it was worked out less than [`FRESH_FOR`]
//! ago: while a scan is adding books the revision moves all the time, and working every list out
//! again for every request would be the cost this cache is here to save. So a change shows in a
//! list within ten seconds, and the warmer ([`run_warmer`]) usually has it there sooner.
//!
//! What can't be kept: a list whose rules use a reader's own progress (`readingStatus`) or a
//! book's `updatedAt` (it moves with every thumbnail and metadata write) is different for each
//! reader or all the time, so it is worked out whenever it is read (`Source::Live`), as is every
//! list on a database that isn't SQLite.
//!
//! A cache that goes wrong never makes a list wrong: when working a list out fails, the members
//! of the last time stay (and the failure is logged once), and a list that has none is worked
//! out live.

use std::{
	collections::{HashMap, HashSet},
	future::Future,
	sync::{Arc, LazyLock, Mutex as StdMutex},
	time::Duration,
};

use chrono::{DateTime, SecondsFormat, Utc};
use models::{catalog_revision as schema, entity::smart_list};
use sea_orm::{
	prelude::*,
	sea_query::{Alias, Expr, Query, SelectStatement},
	ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, ExprTrait, Statement,
	TransactionTrait, Value,
};
use tokio::sync::Mutex;

use crate::{
	filter::{media::MediaFilterInput, series::SeriesFilterInput},
	input::smart_lists::{SmartListFilterGroupInput, SmartListFilterInput},
	query::smart_lists_builder::catalog_members_query,
};

/// How long the members of a list stay good for a reader after they were worked out, whatever
/// has changed in the catalog since
pub const FRESH_FOR_SECS: i64 = 10;

/// How many rows one INSERT of members holds (two values each: far below SQLite's limit)
const INSERT_CHUNK: usize = 400;

/// Where the books of a smart list come from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
	/// The members that were kept, which are good for a reader
	Cached,
	/// The list's rules, worked out now
	Live,
}

/// How up to date a list's members must be
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
	/// For a reader: the revision of the catalog, or less than [`FRESH_FOR_SECS`] old
	Reader,
	/// For the warmer: the revision of the catalog
	Current,
	/// Worked out again whatever the revision says (the owner asks for it)
	Always,
}

/// What [`ensure_fresh`] did
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
	/// The members were good as they were
	Fresh,
	/// The members were worked out again: this many books
	Refreshed(usize),
	/// The list's rules were saved while it was being worked out, so what was worked out was
	/// thrown away (the new rules have their own members, which the next read works out)
	Superseded,
}

// ---------------------------------------------------------------------------------------------
// What can be kept
// ---------------------------------------------------------------------------------------------

/// Whether the rules of a list can be answered from kept members. They can't when one of them
/// depends on who reads the list (a reading status) or moves with every write (`updatedAt`),
/// at any depth. See [`models::catalog_revision::LIVE`].
pub fn is_cacheable(filters: &[SmartListFilterGroupInput]) -> bool {
	!filters
		.iter()
		.flat_map(|group| &group.groups)
		.any(|filter| match filter {
			SmartListFilterInput::Media(media) => media_filter_is_live(media),
			SmartListFilterInput::Series(series) => series_filter_is_live(series),
			_ => false,
		})
}

fn media_filter_is_live(filter: &MediaFilterInput) -> bool {
	filter.reading_status.is_some()
		|| filter.updated_at.is_some()
		|| filter.series.as_ref().is_some_and(series_filter_is_live)
		|| [&filter._and, &filter._or, &filter._not]
			.into_iter()
			.flatten()
			.flatten()
			.any(media_filter_is_live)
}

fn series_filter_is_live(filter: &SeriesFilterInput) -> bool {
	filter.reading_status.is_some()
		|| [&filter._and, &filter._or, &filter._not]
			.into_iter()
			.flatten()
			.flatten()
			.any(series_filter_is_live)
}

// ---------------------------------------------------------------------------------------------
// The state of a list's members
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Materialized {
	revision: i64,
	computed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy)]
struct State {
	/// The revision of the catalog now
	head: i64,
	materialized: Option<Materialized>,
}

impl State {
	fn is_fresh(&self, need: Need, now: DateTime<Utc>) -> bool {
		let Some(materialized) = self.materialized else {
			return false;
		};
		match need {
			Need::Always => false,
			Need::Current => materialized.revision == self.head,
			Need::Reader => {
				materialized.revision == self.head || {
					let age = now.signed_duration_since(materialized.computed_at);
					// a clock that went back is no reason to trust old members
					age >= chrono::Duration::zero()
						&& age < chrono::Duration::seconds(FRESH_FOR_SECS)
				}
			},
		}
	}
}

async fn read_state<C: ConnectionTrait>(conn: &C, list_id: &str) -> Result<State, DbErr> {
	let row = conn
		.query_one_raw(Statement::from_sql_and_values(
			DatabaseBackend::Sqlite,
			format!(
				"SELECT c.revision AS head_revision, m.revision AS revision, m.computed_at AS computed_at
				 FROM {} c LEFT JOIN {} m ON m.smart_list_id = ? WHERE c.id = 1",
				schema::REVISION_TABLE,
				schema::MATERIALIZATIONS_TABLE
			),
			[list_id.into()],
		))
		.await?
		.ok_or_else(|| DbErr::RecordNotFound("the catalog's revision".to_string()))?;

	let head: i64 = row.try_get("", "head_revision")?;
	let revision: Option<i64> = row.try_get("", "revision")?;
	let computed_at: Option<String> = row.try_get("", "computed_at")?;
	let materialized = match (revision, computed_at) {
		(Some(revision), Some(computed_at)) => Some(Materialized {
			revision,
			computed_at: DateTime::parse_from_rfc3339(&computed_at)
				.map_err(|e| DbErr::Custom(format!("bad computed_at: {e}")))?
				.with_timezone(&Utc),
		}),
		_ => None,
	};
	Ok(State { head, materialized })
}

/// The revision of the catalog now
pub async fn current_revision<C: ConnectionTrait>(conn: &C) -> Result<i64, DbErr> {
	let row = conn
		.query_one_raw(Statement::from_string(
			DatabaseBackend::Sqlite,
			format!(
				"SELECT revision FROM {} WHERE id = 1",
				schema::REVISION_TABLE
			),
		))
		.await?
		.ok_or_else(|| DbErr::RecordNotFound("the catalog's revision".to_string()))?;
	row.try_get("", "revision")
}

/// Whether a list has members that were worked out at some time
async fn has_members<C: ConnectionTrait>(conn: &C, list_id: &str) -> bool {
	conn.query_one_raw(Statement::from_sql_and_values(
		DatabaseBackend::Sqlite,
		format!(
			"SELECT 1 AS found FROM {} WHERE smart_list_id = ?",
			schema::MATERIALIZATIONS_TABLE
		),
		[list_id.into()],
	))
	.await
	.ok()
	.flatten()
	.is_some()
}

// ---------------------------------------------------------------------------------------------
// Working a list out
// ---------------------------------------------------------------------------------------------

/// How many times each list was worked out (the tests assert on it: they run side by side, so
/// it is counted for each list, whose ID no other test has)
#[cfg(test)]
static REFRESHES: LazyLock<StdMutex<HashMap<String, u64>>> =
	LazyLock::new(Default::default);

#[cfg(test)]
fn refreshes_of(list_id: &str) -> u64 {
	*REFRESHES
		.lock()
		.expect("the counts are never poisoned")
		.get(list_id)
		.unwrap_or(&0)
}

/// One refresh at a time for each list: a screen of lists asks for every one of them several
/// times at once, and the first to find a list out of date works it out while the others wait
/// for the result, instead of all of them doing it
static FLIGHTS: LazyLock<StdMutex<HashMap<String, Arc<Mutex<()>>>>> =
	LazyLock::new(Default::default);

fn flight(list_id: &str) -> Arc<Mutex<()>> {
	let mut flights = FLIGHTS.lock().expect("the flights are never poisoned");
	if flights.len() > 512 {
		// the ones nobody is waiting for
		flights.retain(|_, flight| Arc::strong_count(flight) > 1);
	}
	flights.entry(list_id.to_string()).or_default().clone()
}

/// Works a list out: the books its rules match over the whole catalog, kept as its members with
/// the revision they were worked out at. `None` when the list's rules were saved (or the list was
/// deleted) in the meantime: the members worked out for the old rules are not written.
///
/// The rules are read first (a snapshot of its own, so the revision belongs to the books), and the
/// members are written after, in one short transaction: the server isn't held up for as long as
/// the rules take. A write to the catalog in between leaves the revision behind, so the next
/// read finds the list out of date and works it out again; and the write checks, under the
/// database's write lock, that the rules are still the ones the members were worked out for.
/// `between` runs after the read and before the write (the tests make that write).
async fn refresh_with_hook<F, Fut>(
	conn: &DatabaseConnection,
	list: &smart_list::Model,
	filters: &[SmartListFilterGroupInput],
	between: F,
) -> Result<Option<usize>, DbErr>
where
	F: FnOnce() -> Fut,
	Fut: Future<Output = ()>,
{
	let read = conn.begin().await?;
	let revision = current_revision(&read).await?;
	let mut ids: Vec<String> = catalog_members_query(list.joiner, filters)
		.into_tuple::<String>()
		.all(&read)
		.await?;
	read.commit().await?;
	// a book once, whatever the joins gave
	ids.sort_unstable();
	ids.dedup();

	between().await;

	let write = conn.begin().await?;
	// the first statement is a write, so the transaction holds the write lock from here on: what
	// it reads next is the database as it is, and no rules can be saved before it commits
	write
		.execute_raw(Statement::from_sql_and_values(
			DatabaseBackend::Sqlite,
			format!(
				"DELETE FROM {} WHERE smart_list_id = ?",
				schema::MEMBERS_TABLE
			),
			[list.id.clone().into()],
		))
		.await?;
	let same_rules = write
		.query_one_raw(Statement::from_sql_and_values(
			DatabaseBackend::Sqlite,
			"SELECT 1 AS found FROM smart_lists WHERE id = ? AND filters = ? AND joiner = ?",
			[
				list.id.clone().into(),
				list.filters.clone().into(),
				list.joiner.to_string().into(),
			],
		))
		.await?
		.is_some();
	if !same_rules {
		write.rollback().await?;
		return Ok(None);
	}

	for chunk in ids.chunks(INSERT_CHUNK) {
		let rows = vec!["(?, ?)"; chunk.len()].join(", ");
		let mut values: Vec<Value> = Vec::with_capacity(chunk.len() * 2);
		for id in chunk {
			values.push(list.id.clone().into());
			values.push(id.clone().into());
		}
		write
			.execute_raw(Statement::from_sql_and_values(
				DatabaseBackend::Sqlite,
				format!(
					"INSERT INTO {} (smart_list_id, media_id) VALUES {rows}",
					schema::MEMBERS_TABLE
				),
				values,
			))
			.await?;
	}
	write
		.execute_raw(Statement::from_sql_and_values(
			DatabaseBackend::Sqlite,
			format!(
				"INSERT OR REPLACE INTO {} (smart_list_id, revision, member_count, computed_at) VALUES (?, ?, ?, ?)",
				schema::MATERIALIZATIONS_TABLE
			),
			[
				list.id.clone().into(),
				revision.into(),
				(ids.len() as i64).into(),
				Utc::now()
					.to_rfc3339_opts(SecondsFormat::Millis, true)
					.into(),
			],
		))
		.await?;
	write.commit().await?;

	#[cfg(test)]
	{
		*REFRESHES
			.lock()
			.expect("the counts are never poisoned")
			.entry(list.id.clone())
			.or_default() += 1;
	}
	Ok(Some(ids.len()))
}

/// Makes sure the members of a list are as up to date as `need` says, working the list out if
/// they aren't. Concurrent calls for the same list work it out once.
pub async fn ensure_fresh(
	conn: &DatabaseConnection,
	list: &smart_list::Model,
	filters: &[SmartListFilterGroupInput],
	need: Need,
) -> Result<Outcome, DbErr> {
	if read_state(conn, &list.id).await?.is_fresh(need, Utc::now()) {
		return Ok(Outcome::Fresh);
	}

	let flight = flight(&list.id);
	let _turn = flight.lock().await;
	// whoever had the turn before may have worked the list out
	if need != Need::Always
		&& read_state(conn, &list.id).await?.is_fresh(need, Utc::now())
	{
		return Ok(Outcome::Fresh);
	}

	Ok(
		match refresh_with_hook(conn, list, filters, || async {}).await? {
			Some(members) => Outcome::Refreshed(members),
			None => Outcome::Superseded,
		},
	)
}

/// The lists whose last refresh failed: the failure is logged once, until it works again
static FAILING: LazyLock<StdMutex<HashSet<String>>> = LazyLock::new(Default::default);

fn failed(list_id: &str, error: &DbErr) {
	let first = FAILING
		.lock()
		.expect("the failures are never poisoned")
		.insert(list_id.to_string());
	if first {
		tracing::error!(
			list_id,
			%error,
			"Failed to work out the books of a smart list: its last members stay (or the list is worked out live) until it works again"
		);
	}
}

fn worked(list_id: &str) {
	FAILING
		.lock()
		.expect("the failures are never poisoned")
		.remove(list_id);
}

/// Where the books of a list come from now, working the cache out first when a reader needs it.
/// It never fails: a cache that can't be used makes the list live, so a reader still gets the
/// right books.
pub async fn source(
	conn: &DatabaseConnection,
	list: &smart_list::Model,
	filters: &[SmartListFilterGroupInput],
) -> Source {
	if conn.get_database_backend() != DatabaseBackend::Sqlite || !is_cacheable(filters) {
		return Source::Live;
	}

	match ensure_fresh(conn, list, filters, Need::Reader).await {
		// the rules were saved while the list was worked out: this read has the rules it
		// started with, which no members are for any more
		Ok(Outcome::Superseded) => Source::Live,
		Ok(_) => {
			worked(&list.id);
			Source::Cached
		},
		Err(error) => {
			failed(&list.id, &error);
			if has_members(conn, &list.id).await {
				Source::Cached
			} else {
				Source::Live
			}
		},
	}
}

/// The IDs of the members of a list, as a subquery. Never read books from it for a user without
/// narrowing them to what the user may read.
pub fn members_subquery(list_id: &str) -> SelectStatement {
	Query::select()
		.column(Alias::new("media_id"))
		.from(Alias::new(schema::MEMBERS_TABLE))
		.and_where(Expr::col(Alias::new("smart_list_id")).eq(list_id))
		.to_owned()
}

// ---------------------------------------------------------------------------------------------
// The warmer
// ---------------------------------------------------------------------------------------------

/// Works out the lists that need it (or the ones given). A list is skipped when it is as up to
/// date as `need` says or can't be kept. Returns how many lists were worked out.
pub async fn refresh_lists(
	conn: &DatabaseConnection,
	ids: Option<&[String]>,
	need: Need,
) -> usize {
	if conn.get_database_backend() != DatabaseBackend::Sqlite {
		return 0;
	}
	let mut query = smart_list::Entity::find();
	if let Some(ids) = ids {
		query = query.filter(smart_list::Column::Id.is_in(ids.to_vec()));
	}
	let lists = match query.all(conn).await {
		Ok(lists) => lists,
		Err(error) => {
			tracing::error!(%error, "Failed to look up the smart lists to work out");
			return 0;
		},
	};

	let mut refreshed = 0;
	for list in lists {
		let filters: Vec<SmartListFilterGroupInput> =
			match serde_json::from_slice(&list.filters) {
				Ok(filters) => filters,
				Err(error) => {
					tracing::warn!(list_id = %list.id, %error, "A smart list's rules can't be read");
					continue;
				},
			};
		if !is_cacheable(&filters) {
			continue;
		}
		match ensure_fresh(conn, &list, &filters, need).await {
			Ok(Outcome::Refreshed(_)) => {
				worked(&list.id);
				refreshed += 1;
				// the lists are worked out one after the other, easy on the rest of the server
				tokio::time::sleep(Duration::from_millis(25)).await;
			},
			Ok(Outcome::Fresh | Outcome::Superseded) => worked(&list.id),
			Err(error) => failed(&list.id, &error),
		}
	}
	refreshed
}

/// How often the warmer looks at the catalog
#[derive(Debug, Clone, Copy)]
pub struct WarmerConfig {
	/// How often it reads the revision
	pub poll: Duration,
	/// How long the revision has to stay as it is before the lists are worked out: a scan keeps
	/// changing it, and one pass after it ends is all it takes
	pub quiet: Duration,
	/// Every list is worked out again this often, whatever the revision says
	pub backstop: Duration,
}

impl Default for WarmerConfig {
	fn default() -> Self {
		// a scan that adds books is caught up with 4 to 6 seconds after it ends, and working
		// every list out takes about 3 seconds on a library of 9,000 books: within 10 seconds
		Self {
			poll: Duration::from_secs(2),
			quiet: Duration::from_secs(4),
			backstop: Duration::from_secs(24 * 60 * 60),
		}
	}
}

/// Keeps the smart lists' members up to date in the background, so a reader mostly finds them
/// ready. It works out every list that needs it at start-up, then looks at the catalog's
/// revision: when it moves and stays still for a while, the lists that are behind are worked out;
/// and once a day all of them are, in case something changed a rule's inputs behind the triggers'
/// back. It runs until the server stops.
pub async fn run_warmer(conn: Arc<DatabaseConnection>, config: WarmerConfig) {
	if conn.get_database_backend() != DatabaseBackend::Sqlite {
		return;
	}

	let started = refresh_lists(&conn, None, Need::Current).await;
	tracing::info!(lists = started, "Smart lists worked out at start-up");

	let mut warmed = current_revision(conn.as_ref()).await.ok();
	let mut seen = warmed;
	let mut changed_at: Option<tokio::time::Instant> = None;
	let mut last_backstop = tokio::time::Instant::now();

	loop {
		tokio::time::sleep(config.poll).await;

		if let Ok(revision) = current_revision(conn.as_ref()).await {
			if Some(revision) != seen {
				// still changing
				seen = Some(revision);
				changed_at = Some(tokio::time::Instant::now());
			} else if seen != warmed
				&& changed_at.is_some_and(|at| at.elapsed() >= config.quiet)
			{
				warmed = seen;
				changed_at = None;
				let refreshed = refresh_lists(&conn, None, Need::Current).await;
				tracing::debug!(
					lists = refreshed,
					"Smart lists worked out after a change"
				);
			}
		}

		if last_backstop.elapsed() >= config.backstop {
			last_backstop = tokio::time::Instant::now();
			let refreshed = refresh_lists(&conn, None, Need::Always).await;
			tracing::info!(lists = refreshed, "Smart lists worked out again (daily)");
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::query::smart_lists_builder::{build_books_query, build_filters};
	use ::tests::{db::test_database, fake_data};
	use models::{
		entity::{
			age_restriction, library, library_exclusion, media, media_metadata,
			media_tag, reading_session, series_metadata,
			smart_list::{SmartListGrouping, SmartListJoiner},
			tag,
			user::AuthUser,
		},
		shared::enums::EntityVisibility,
	};
	use pretty_assertions::assert_eq;
	use sea_orm::{
		sea_query::SqliteQueryBuilder, ActiveModelTrait, ActiveValue::Set, QuerySelect,
	};
	use serde_json::{json, Value as Json};

	// -----------------------------------------------------------------------------------------
	// helpers
	// -----------------------------------------------------------------------------------------

	fn rules(value: Json) -> Vec<SmartListFilterGroupInput> {
		serde_json::from_value(value).expect("the rules should parse")
	}

	/// One group of rules, as they are stored (`And`, `Or` or `Not`)
	fn group(joiner: &str, filters: Vec<Json>) -> Json {
		json!([{ "joiner": joiner, "groups": filters }])
	}

	async fn make_list(
		db: &DatabaseConnection,
		creator: &AuthUser,
		joiner: SmartListJoiner,
		filters: &[SmartListFilterGroupInput],
	) -> smart_list::Model {
		smart_list::ActiveModel {
			id: Set(Uuid::new_v4().to_string()),
			name: Set("A list".to_string()),
			filters: Set(serde_json::to_vec(filters).expect("rules serialize")),
			joiner: Set(joiner),
			default_grouping: Set(SmartListGrouping::ByBooks),
			visibility: Set(EntityVisibility::Private),
			creator_id: Set(creator.id.clone()),
			..Default::default()
		}
		.insert(db)
		.await
		.expect("should insert the list")
	}

	async fn revision(db: &DatabaseConnection) -> i64 {
		current_revision(db)
			.await
			.expect("should read the revision")
	}

	async fn member_ids(
		db: &DatabaseConnection,
		list: &smart_list::Model,
	) -> Vec<String> {
		let mut ids: Vec<String> = db
			.query_all_raw(Statement::from_sql_and_values(
				DatabaseBackend::Sqlite,
				format!(
					"SELECT media_id FROM {} WHERE smart_list_id = ?",
					schema::MEMBERS_TABLE
				),
				[list.id.clone().into()],
			))
			.await
			.expect("should read the members")
			.iter()
			.map(|row| row.try_get::<String>("", "media_id").expect("an id"))
			.collect();
		ids.sort();
		ids
	}

	/// What a reader gets: the members, narrowed to what `user` may read
	async fn cached_ids(
		db: &DatabaseConnection,
		user: &AuthUser,
		list: &smart_list::Model,
	) -> Vec<String> {
		let filters: Vec<SmartListFilterGroupInput> =
			serde_json::from_slice(&list.filters).unwrap();
		assert_eq!(source(db, list, &filters).await, Source::Cached);
		let mut ids = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::Id.in_subquery(members_subquery(&list.id)))
			.select_only()
			.column(media::Column::Id)
			.into_tuple::<String>()
			.all(db)
			.await
			.expect("should read the cached books");
		ids.sort();
		ids
	}

	/// What the rules give when worked out on the spot (the code the cache replaces)
	async fn live_ids(
		db: &DatabaseConnection,
		user: &AuthUser,
		list: &smart_list::Model,
	) -> Vec<String> {
		let filters: Vec<SmartListFilterGroupInput> =
			serde_json::from_slice(&list.filters).unwrap();
		let mut ids = build_books_query(user, list.joiner, &filters, None)
			.select_only()
			.column(media::Column::Id)
			.into_tuple::<String>()
			.all(db)
			.await
			.expect("should read the live books");
		ids.sort();
		ids
	}

	/// Pretends the members were worked out long ago, so that a reader looks at the revision
	async fn age(db: &DatabaseConnection, list: &smart_list::Model) {
		db.execute_unprepared(&format!(
			"UPDATE {} SET computed_at = '2020-01-01T00:00:00.000Z' WHERE smart_list_id = '{}'",
			schema::MATERIALIZATIONS_TABLE,
			list.id
		))
		.await
		.expect("should age the members");
	}

	struct World {
		db: DatabaseConnection,
		owner: AuthUser,
		/// ana: the *hidden* library is hidden from her, and her age restriction (16) applies to
		/// unrated books too
		ana: AuthUser,
		open: library::Model,
		hidden: library::Model,
		books: Vec<media::Model>,
	}

	/// Two libraries, three series and twelve books with genres, writers, ratings and paths to
	/// match on. The *Science* series (4 books) has the genre `Science`, the other eight
	/// `Fantasy`; the fourth book of each series is rated 18, the others 0.
	async fn world() -> World {
		let db = test_database().await;
		let owner = fake_data::User::new("owner").auth_user(&db).await;
		let mut ana = fake_data::User::new("ana").auth_user(&db).await;
		ana.is_server_owner = false;

		let open = fake_data::Library {
			name: Some("Open".to_string()),
			..Default::default()
		}
		.insert(&db)
		.await;
		let hidden = fake_data::Library {
			name: Some("Hidden".to_string()),
			..Default::default()
		}
		.insert(&db)
		.await;
		library_exclusion::ActiveModel {
			user_id: Set(ana.id.clone()),
			library_id: Set(hidden.id.clone()),
			..Default::default()
		}
		.insert(&db)
		.await
		.expect("should hide the library");
		let restriction = age_restriction::ActiveModel {
			user_id: Set(ana.id.clone()),
			age: Set(16),
			restrict_on_unset: Set(true),
			..Default::default()
		}
		.insert(&db)
		.await
		.expect("should restrict ana");
		ana.age_restriction = Some(restriction);

		let mut books = Vec::new();
		for (library, series_name, titles) in [
			(
				&open,
				"Fantasy saga",
				["Dragons", "Elves", "Orcs", "Wizards"],
			),
			(&open, "Science", ["Atoms", "Stars", "Cells", "Genes"]),
			(
				&hidden,
				"Secret",
				["Hidden one", "Hidden two", "Hidden three", "Hidden four"],
			),
		] {
			let series = fake_data::Series {
				name: Some(series_name.to_string()),
				library_id: Some(library.id.clone()),
				..Default::default()
			}
			.insert(&db)
			.await;
			for (index, title) in titles.into_iter().enumerate() {
				let book = fake_data::Media {
					series_id: series.id.clone(),
					name: Some(title.to_string()),
					path: Some(format!("/shelf/{series_name}/{title}.epub")),
					size: Some(1000 + index as i64),
					..Default::default()
				}
				.insert(&db)
				.await;
				let genre = if series_name == "Science" {
					"Science"
				} else {
					"Fantasy"
				};
				media_metadata::ActiveModel {
					media_id: Set(Some(book.id.clone())),
					genres: Set(Some(genre.to_string())),
					writers: Set(Some(format!("Writer {}", index % 2))),
					age_rating: Set(Some(if index == 3 { 18 } else { 0 })),
					..Default::default()
				}
				.insert(&db)
				.await
				.expect("should insert the metadata");
				books.push(book);
			}
		}

		World {
			db,
			owner,
			ana,
			open,
			hidden,
			books,
		}
	}

	fn science_rules() -> Vec<SmartListFilterGroupInput> {
		rules(group(
			"And",
			vec![json!({ "mediaMetadata": { "genres": { "eq": "Science" } } })],
		))
	}

	// -----------------------------------------------------------------------------------------
	// what can be kept
	// -----------------------------------------------------------------------------------------

	#[test]
	fn a_reading_status_or_updated_at_at_any_depth_keeps_a_list_live() {
		let plain = rules(group(
			"Or",
			vec![
				json!({ "mediaMetadata": { "genres": { "anyOf": ["Fantasy"] } } }),
				json!({ "media": { "path": { "anyOf": ["/a", "/b"] } } }),
				json!({ "series": { "name": { "contains": "saga" } } }),
				json!({ "library": { "name": { "eq": "Open" } } }),
			],
		));
		assert!(is_cacheable(&plain));
		assert!(is_cacheable(&[]));

		for live in [
			json!({ "media": { "readingStatus": { "is": "READING" } } }),
			json!({ "media": { "updatedAt": { "gt": "2026-01-01T00:00:00Z" } } }),
			// nested: one level and two (the rules are stored with serde's names: `or`, not `_or`)
			json!({ "media": { "or": [{ "readingStatus": { "is": "FINISHED" } }] } }),
			json!({ "media": { "not": [{ "and": [{ "updatedAt": { "gt": "2026-01-01T00:00:00Z" } }] }] } }),
			// inside the series filter of a book, and a series' own
			json!({ "media": { "series": { "readingStatus": { "is": "READING" } } } }),
			json!({ "series": { "or": [{ "readingStatus": { "is": "NOT_STARTED" } }] } }),
		] {
			let alone = rules(group("And", vec![live.clone()]));
			assert!(!is_cacheable(&alone), "{live}");
			// next to rules that could be kept, it is the list that is live
			let mixed = rules(group(
				"And",
				vec![
					json!({ "mediaMetadata": { "genres": { "eq": "x" } } }),
					live,
				],
			));
			assert!(!is_cacheable(&mixed));
		}
	}

	// -----------------------------------------------------------------------------------------
	// the cache is the live list, narrowed to the reader
	// -----------------------------------------------------------------------------------------

	/// The books a reader gets from the members equal the books the rules give worked out live,
	/// for every shape of rule, for the owner and for a user with limits
	#[tokio::test]
	async fn the_cache_gives_what_the_rules_give() {
		let world = world().await;
		let db = &world.db;
		let favourite = tag::ActiveModel {
			name: Set("Favourite".to_string()),
			..Default::default()
		}
		.insert(db)
		.await
		.expect("should insert a tag");
		for book in world.books.iter().step_by(3) {
			media_tag::ActiveModel {
				media_id: Set(book.id.clone()),
				tag_id: Set(favourite.id),
				..Default::default()
			}
			.insert(db)
			.await
			.expect("should tag the book");
		}
		series_metadata::ActiveModel {
			series_id: Set(world.books[0].series_id.clone().expect("a series")),
			title: Set(Some("The Fantasy Saga".to_string())),
			..Default::default()
		}
		.insert(db)
		.await
		.expect("should insert series metadata");

		// a few real paths among many that are gone, as in the real lists
		let paths: Vec<String> = world
			.books
			.iter()
			.take(10)
			.map(|book| book.path.clone())
			.chain((0..300).map(|n| format!("/gone/{n}.epub")))
			.collect();

		let shapes: Vec<(&str, SmartListJoiner, Json)> = vec![
			// the real lists: an OR of genres and tags, and a long run of paths
			(
				"genres or tags or paths",
				SmartListJoiner::Or,
				json!([
					{ "joiner": "Or", "groups": [
						{ "mediaMetadata": { "genres": { "hasAnyOf": ["Science"] } } },
						{ "media": { "tags": { "anyOf": ["Favourite"] } } },
					]},
					{ "joiner": "Or", "groups": [{ "media": { "path": { "anyOf": paths } } }] },
				]),
			),
			(
				"and of two groups",
				SmartListJoiner::And,
				json!([
					{ "joiner": "And", "groups": [{ "mediaMetadata": { "genres": { "eq": "Fantasy" } } }] },
					{ "joiner": "And", "groups": [{ "mediaMetadata": { "writers": { "eq": "Writer 1" } } }] },
				]),
			),
			(
				"or of two groups",
				SmartListJoiner::Or,
				json!([
					{ "joiner": "And", "groups": [{ "media": { "name": { "eq": "Atoms" } } }] },
					{ "joiner": "And", "groups": [{ "media": { "name": { "eq": "Elves" } } }] },
				]),
			),
			(
				"not",
				SmartListJoiner::And,
				json!([{ "joiner": "Not", "groups": [
					{ "mediaMetadata": { "genres": { "eq": "Science" } } },
					{ "media": { "name": { "eq": "Orcs" } } },
				]}]),
			),
			(
				"a library",
				SmartListJoiner::And,
				json!([{ "joiner": "And", "groups": [{ "library": { "name": { "eq": "Hidden" } } }] }]),
			),
			(
				"a series and its metadata",
				SmartListJoiner::Or,
				json!([{ "joiner": "Or", "groups": [
					{ "series": { "name": { "contains": "Science" } } },
					{ "seriesMetadata": { "title": { "contains": "Fantasy" } } },
				]}]),
			),
			(
				"a book's series",
				SmartListJoiner::And,
				json!([{ "joiner": "And", "groups": [{ "media": { "series": { "name": { "eq": "Fantasy saga" } } } }] }]),
			),
			(
				"a search",
				SmartListJoiner::And,
				json!([{ "joiner": "And", "groups": [{ "media": { "search": "hidden" } }] }]),
			),
			(
				"a library type",
				SmartListJoiner::And,
				json!([{ "joiner": "And", "groups": [{ "media": { "libraryType": { "isAnyOf": ["MIXED"] } } }] }]),
			),
			("no rules", SmartListJoiner::And, json!([])),
		];

		for (name, joiner, shape) in shapes {
			let filters = rules(shape);
			let list = make_list(db, &world.owner, joiner, &filters).await;
			for (who, user) in [("the owner", &world.owner), ("ana", &world.ana)] {
				assert_eq!(
					cached_ids(db, user, &list).await,
					live_ids(db, user, &list).await,
					"{name}, for {who}"
				);
			}
		}

		// a list is not empty for the wrong reason
		let all = make_list(db, &world.owner, SmartListJoiner::And, &[]).await;
		assert_eq!(cached_ids(db, &world.owner, &all).await.len(), 12);
		// ana: nothing from the hidden library, nothing rated above 16
		let ana_books = cached_ids(db, &world.ana, &all).await;
		assert_eq!(ana_books.len(), 6, "{ana_books:?}");
		for hidden in world
			.books
			.iter()
			.filter(|book| book.library_id == world.hidden.id)
		{
			assert!(!ana_books.contains(&hidden.id));
		}
	}

	/// The kept members are what the rules match over the whole catalog, so they hold the books a
	/// reader may not read; the reader's query is what leaves them out
	#[tokio::test]
	async fn the_members_are_the_whole_catalog_and_a_reader_gets_their_share() {
		let world = world().await;
		let db = &world.db;
		let filters = rules(json!([]));
		let list = make_list(db, &world.owner, SmartListJoiner::And, &filters).await;
		assert_eq!(source(db, &list, &filters).await, Source::Cached);

		assert_eq!(member_ids(db, &list).await.len(), 12);
		assert_eq!(cached_ids(db, &world.ana, &list).await.len(), 6);
		assert_eq!(cached_ids(db, &world.owner, &list).await.len(), 12);

		// a removed book is a member too: whoever reads the list leaves it out, and bringing it
		// back needs no new work
		let removed = world.books[0].id.clone();
		db.execute_unprepared(&format!(
			"UPDATE media SET deleted_at = CURRENT_TIMESTAMP WHERE id = '{removed}'"
		))
		.await
		.unwrap();
		assert!(member_ids(db, &list).await.contains(&removed));
		assert!(!cached_ids(db, &world.owner, &list).await.contains(&removed));
		db.execute_unprepared(&format!(
			"UPDATE media SET deleted_at = NULL WHERE id = '{removed}'"
		))
		.await
		.unwrap();
		assert!(cached_ids(db, &world.owner, &list).await.contains(&removed));
		assert_eq!(refreshes_of(&list.id), 1);
	}

	// -----------------------------------------------------------------------------------------
	// when the revision moves
	// -----------------------------------------------------------------------------------------

	async fn moves(db: &DatabaseConnection, what: &str, sql: String) {
		let before = revision(db).await;
		db.execute_unprepared(&sql)
			.await
			.unwrap_or_else(|e| panic!("{what}: {e}"));
		assert!(
			revision(db).await > before,
			"{what} should bump the revision"
		);
	}

	async fn stays(db: &DatabaseConnection, what: &str, sql: String) {
		let before = revision(db).await;
		db.execute_unprepared(&sql)
			.await
			.unwrap_or_else(|e| panic!("{what}: {e}"));
		assert_eq!(
			revision(db).await,
			before,
			"{what} should not bump the revision"
		);
	}

	/// What changes a rule's input bumps the revision; what doesn't, doesn't
	#[tokio::test]
	async fn the_revision_moves_for_what_a_rule_can_see_and_not_for_the_rest() {
		let world = world().await;
		let db = &world.db;
		let book = world.books[0].clone();
		let series = book.series_id.clone().expect("a series");

		// a new book (and its metadata)
		let before = revision(db).await;
		let extra = fake_data::Media {
			series_id: series.clone(),
			..Default::default()
		}
		.insert(db)
		.await;
		assert!(revision(db).await > before, "a new book");
		moves(
			db,
			"a new metadata row",
			format!(
				"INSERT INTO media_metadata (media_id, genres) VALUES ('{}', 'x')",
				extra.id
			),
		)
		.await;

		moves(
			db,
			"a genre edit",
			format!(
				"UPDATE media_metadata SET genres = 'Horror' WHERE media_id = '{}'",
				book.id
			),
		)
		.await;
		moves(
			db,
			"a metadata title",
			format!(
				"UPDATE media_metadata SET title = 'T' WHERE media_id = '{}'",
				book.id
			),
		)
		.await;
		moves(
			db,
			"a rename of a book",
			format!("UPDATE media SET name = 'Renamed' WHERE id = '{}'", book.id),
		)
		.await;
		moves(
			db,
			"a move of a book",
			format!(
				"UPDATE media SET path = '/moved.epub' WHERE id = '{}'",
				book.id
			),
		)
		.await;
		moves(
			db,
			"a status",
			format!(
				"UPDATE media SET status = 'MISSING' WHERE id = '{}'",
				book.id
			),
		)
		.await;
		moves(
			db,
			"a rename of a series",
			format!("UPDATE series SET name = 'Renamed' WHERE id = '{series}'"),
		)
		.await;
		moves(
			db,
			"a series' metadata",
			format!(
				"INSERT INTO series_metadata (series_id, title) VALUES ('{series}', 'x')"
			),
		)
		.await;
		moves(
			db,
			"a rename of a library",
			format!(
				"UPDATE libraries SET name = 'Renamed' WHERE id = '{}'",
				world.open.id
			),
		)
		.await;
		moves(db, "a library's type", format!("UPDATE library_configs SET library_type = 'MANGA' WHERE library_id = '{}'", world.open.id)).await;
		moves(
			db,
			"a new tag",
			"INSERT INTO tags (name) VALUES ('new')".to_string(),
		)
		.await;
		moves(
			db,
			"a tag's name",
			"UPDATE tags SET name = 'newer' WHERE name = 'new'".to_string(),
		)
		.await;
		moves(
			db,
			"a tagged book",
			format!("INSERT INTO media_tags (media_id, tag_id) VALUES ('{}', (SELECT id FROM tags LIMIT 1))", book.id),
		)
		.await;
		moves(
			db,
			"an untagged book",
			format!("DELETE FROM media_tags WHERE media_id = '{}'", book.id),
		)
		.await;
		// a permanent delete
		moves(
			db,
			"a delete",
			format!("DELETE FROM media WHERE id = '{}'", extra.id),
		)
		.await;

		stays(db, "a thumbnail", format!("UPDATE media SET thumbnail_path = '/t.webp', thumbnail_meta = NULL WHERE id = '{}'", book.id)).await;
		stays(db, "a hash", format!("UPDATE media SET hash = 'abc', koreader_hash = 'def', modified_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = '{}'", book.id)).await;
		stays(db, "a removal", format!("UPDATE media SET deleted_at = CURRENT_TIMESTAMP, deleted_by = NULL WHERE id = '{}'", book.id)).await;
		stays(
			db,
			"a restore",
			format!(
				"UPDATE media SET deleted_at = NULL WHERE id = '{}'",
				book.id
			),
		)
		.await;
		stays(db, "a library's scan", format!("UPDATE libraries SET last_scanned_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = '{}'", world.open.id)).await;
		stays(db, "a series' thumbnail", format!("UPDATE series SET thumbnail_path = '/s.webp', updated_at = CURRENT_TIMESTAMP WHERE id = '{series}'")).await;

		// reading is nobody's rule that can be kept
		let before = revision(db).await;
		fake_data::ReadingSession::completed(&book.id, &world.owner.id)
			.insert(db)
			.await;
		assert_eq!(revision(db).await, before, "reading");
		reading_session::Entity::delete_many()
			.exec(db)
			.await
			.expect("should clear the sessions");
		assert_eq!(revision(db).await, before, "forgetting what was read");
	}

	/// New rules drop the members at once, and a list that goes takes them with it
	#[tokio::test]
	async fn new_rules_drop_the_members() {
		let world = world().await;
		let db = &world.db;
		let filters = science_rules();
		let list = make_list(db, &world.owner, SmartListJoiner::And, &filters).await;
		assert_eq!(source(db, &list, &filters).await, Source::Cached);
		assert_eq!(member_ids(db, &list).await.len(), 4);

		// the rules are saved: the members go with the old rules, whatever their age
		let other = rules(group(
			"And",
			vec![json!({ "mediaMetadata": { "genres": { "eq": "Fantasy" } } })],
		));
		let mut model: smart_list::ActiveModel = list.clone().into();
		model.filters = Set(serde_json::to_vec(&other).unwrap());
		let list = model.update(db).await.expect("should save the rules");
		assert!(member_ids(db, &list).await.is_empty());
		assert!(!has_members(db, &list.id).await);

		// and the next read works the list out with the new ones
		assert_eq!(source(db, &list, &other).await, Source::Cached);
		assert_eq!(member_ids(db, &list).await.len(), 8);

		// the list goes: so do its members
		smart_list::Entity::delete_by_id(list.id.clone())
			.exec(db)
			.await
			.expect("should delete the list");
		assert!(member_ids(db, &list).await.is_empty());
		assert!(!has_members(db, &list.id).await);
	}

	/// A book that is deleted takes its memberships with it
	#[tokio::test]
	async fn a_deleted_book_leaves_every_list() {
		let world = world().await;
		let db = &world.db;
		let filters = rules(json!([]));
		let list = make_list(db, &world.owner, SmartListJoiner::And, &filters).await;
		assert_eq!(source(db, &list, &filters).await, Source::Cached);

		let gone = world.books[0].clone();
		media::Entity::delete_by_id(gone.id.clone())
			.exec(db)
			.await
			.expect("should delete");
		let members = member_ids(db, &list).await;
		assert_eq!(members.len(), 11);
		assert!(!members.contains(&gone.id));
	}

	// -----------------------------------------------------------------------------------------
	// freshness
	// -----------------------------------------------------------------------------------------

	/// A change shows once the members are older than ten seconds, and not before
	#[tokio::test]
	async fn a_reader_sees_a_change_when_the_members_are_old_enough() {
		let world = world().await;
		let db = &world.db;
		let list =
			make_list(db, &world.owner, SmartListJoiner::And, &science_rules()).await;
		assert_eq!(cached_ids(db, &world.owner, &list).await.len(), 4);
		assert_eq!(refreshes_of(&list.id), 1);

		// a genre edit: the members are young, so the list isn't worked out again yet
		db.execute_unprepared(&format!(
			"UPDATE media_metadata SET genres = 'Science' WHERE media_id = '{}'",
			world.books[0].id
		))
		.await
		.unwrap();
		assert_eq!(cached_ids(db, &world.owner, &list).await.len(), 4);
		assert_eq!(refreshes_of(&list.id), 1);

		// the members get old: the next read works the list out
		age(db, &list).await;
		assert_eq!(cached_ids(db, &world.owner, &list).await.len(), 5);
		assert_eq!(refreshes_of(&list.id), 2);

		// and then it is current: nothing is worked out, however old the members get
		assert_eq!(cached_ids(db, &world.owner, &list).await.len(), 5);
		age(db, &list).await;
		assert_eq!(cached_ids(db, &world.owner, &list).await.len(), 5);
		assert_eq!(refreshes_of(&list.id), 2, "nothing changed since");
	}

	/// Concurrent readers of a list that is out of date work it out once
	#[tokio::test]
	async fn concurrent_reads_work_a_list_out_once() {
		let world = world().await;
		let db = &world.db;
		let filters = science_rules();
		let list = make_list(db, &world.owner, SmartListJoiner::And, &filters).await;

		let reads = (0..8).map(|_| source(db, &list, &filters));
		let sources = futures_util::future::join_all(reads).await;
		assert!(sources.iter().all(|s| *s == Source::Cached));
		assert_eq!(
			refreshes_of(&list.id),
			1,
			"one worked it out, the others waited for it"
		);

		// and the same when it is out of date again
		db.execute_unprepared(&format!(
			"UPDATE media SET name = 'x' WHERE id = '{}'",
			world.books[0].id
		))
		.await
		.unwrap();
		age(db, &list).await;
		let reads = (0..8).map(|_| source(db, &list, &filters));
		futures_util::future::join_all(reads).await;
		assert_eq!(refreshes_of(&list.id), 2);
	}

	/// A write while a list is being worked out leaves it out of date, and the next read works it
	/// out again, with the change
	#[tokio::test]
	async fn a_write_during_a_refresh_leaves_the_list_out_of_date() {
		let world = world().await;
		let db = &world.db;
		let filters = science_rules();
		let list = make_list(db, &world.owner, SmartListJoiner::And, &filters).await;

		let book = world.books[0].id.clone();
		let members = refresh_with_hook(db, &list, &filters, || async {
			// after the rules were read, before the members are written
			db.execute_unprepared(&format!(
				"UPDATE media_metadata SET genres = 'Science' WHERE media_id = '{book}'"
			))
			.await
			.expect("the write should work");
		})
		.await
		.expect("the refresh should work");
		assert_eq!(
			members,
			Some(4),
			"the books as they were when the rules were read"
		);

		// the members carry the revision they were read at: behind the catalog now
		let state = read_state(db, &list.id).await.unwrap();
		assert!(state.materialized.unwrap().revision < state.head);
		assert!(!state.is_fresh(Need::Current, Utc::now()));
		// a reader still gets them while they are young
		assert!(state.is_fresh(Need::Reader, Utc::now()));

		age(db, &list).await;
		assert_eq!(cached_ids(db, &world.owner, &list).await.len(), 5);
		let state = read_state(db, &list.id).await.unwrap();
		assert!(state.is_fresh(Need::Current, Utc::now()));
	}

	/// New rules saved while a list is being worked out are not overwritten by members worked out
	/// for the old ones: they are thrown away, and the next read works the list out again
	#[tokio::test]
	async fn rules_saved_during_a_refresh_are_not_overwritten() {
		let world = world().await;
		let db = &world.db;
		let filters = science_rules();
		let list = make_list(db, &world.owner, SmartListJoiner::And, &filters).await;

		let other = rules(group(
			"And",
			vec![json!({ "mediaMetadata": { "genres": { "eq": "Fantasy" } } })],
		));
		let new_rules = serde_json::to_vec(&other).unwrap();
		let members = refresh_with_hook(db, &list, &filters, || async {
			// the rules are saved after the old ones were read, before the members are written
			let mut model: smart_list::ActiveModel = list.clone().into();
			model.filters = Set(new_rules);
			model.update(db).await.expect("should save the rules");
		})
		.await
		.expect("the refresh should work");
		assert_eq!(
			members, None,
			"what was worked out for the old rules is dropped"
		);
		assert!(member_ids(db, &list).await.is_empty());
		assert!(!has_members(db, &list.id).await);

		// a read that started with the old rules is answered with them, live
		assert_eq!(source(db, &list, &filters).await, Source::Live);
		// and the next one, with the saved rules, is kept
		let saved = smart_list::Entity::find()
			.filter(smart_list::Column::Id.eq(list.id.clone()))
			.one(db)
			.await
			.unwrap()
			.expect("the list is there");
		assert_eq!(source(db, &saved, &other).await, Source::Cached);
		assert_eq!(member_ids(db, &saved).await.len(), 8);

		// a list deleted during a refresh is dropped the same way, and is no failure
		let gone = make_list(db, &world.owner, SmartListJoiner::And, &filters).await;
		let members = refresh_with_hook(db, &gone, &filters, || async {
			smart_list::Entity::delete_by_id(gone.id.clone())
				.exec(db)
				.await
				.expect("should delete the list");
		})
		.await
		.expect("the refresh should work");
		assert_eq!(members, None);
		assert!(!FAILING.lock().unwrap().contains(&gone.id));
	}

	/// The warmer's need is stricter than a reader's, and the owner's forces
	#[tokio::test]
	async fn what_each_need_asks_for() {
		let world = world().await;
		let db = &world.db;
		let filters = rules(json!([]));
		let list = make_list(db, &world.owner, SmartListJoiner::And, &filters).await;

		let work = |need| ensure_fresh(db, &list, &filters, need);
		assert_eq!(work(Need::Reader).await.unwrap(), Outcome::Refreshed(12));
		assert_eq!(work(Need::Reader).await.unwrap(), Outcome::Fresh);
		assert_eq!(work(Need::Current).await.unwrap(), Outcome::Fresh);

		db.execute_unprepared(&format!(
			"UPDATE media SET name = 'x' WHERE id = '{}'",
			world.books[0].id
		))
		.await
		.unwrap();
		// young members are good for a reader, but not for the warmer
		assert_eq!(work(Need::Reader).await.unwrap(), Outcome::Fresh);
		assert_eq!(work(Need::Current).await.unwrap(), Outcome::Refreshed(12));
		assert_eq!(work(Need::Current).await.unwrap(), Outcome::Fresh);
		// the owner forces
		assert_eq!(work(Need::Always).await.unwrap(), Outcome::Refreshed(12));
	}

	// -----------------------------------------------------------------------------------------
	// a cache that goes wrong
	// -----------------------------------------------------------------------------------------

	const NO_MEMBERS: &str = "CREATE TRIGGER test_no_members BEFORE INSERT ON smart_list_members BEGIN SELECT RAISE(ABORT, 'no'); END";

	/// When working a list out fails, the members of the last time stay, and what a reader gets of
	/// them is still only what they may read
	#[tokio::test]
	async fn a_failed_refresh_keeps_the_members_it_had() {
		let world = world().await;
		let db = &world.db;
		let list =
			make_list(db, &world.owner, SmartListJoiner::And, &science_rules()).await;
		assert_eq!(cached_ids(db, &world.owner, &list).await.len(), 4);

		// the write of the members fails from now on
		db.execute_unprepared(NO_MEMBERS).await.unwrap();
		db.execute_unprepared(&format!(
			"UPDATE media_metadata SET genres = 'Science' WHERE media_id = '{}'",
			world.books[0].id
		))
		.await
		.unwrap();
		age(db, &list).await;

		// the old members stay (the transaction that failed was rolled back whole)
		assert_eq!(cached_ids(db, &world.owner, &list).await.len(), 4);
		assert_eq!(member_ids(db, &list).await.len(), 4);
		assert!(FAILING.lock().unwrap().contains(&list.id));

		// it works again: the list is worked out, and the failure is forgotten
		db.execute_unprepared("DROP TRIGGER test_no_members")
			.await
			.unwrap();
		assert_eq!(cached_ids(db, &world.owner, &list).await.len(), 5);
		assert!(!FAILING.lock().unwrap().contains(&list.id));
	}

	/// A list that was never worked out and can't be is worked out live
	#[tokio::test]
	async fn a_list_with_no_members_and_no_cache_is_live() {
		let world = world().await;
		let db = &world.db;
		let filters = science_rules();
		let list = make_list(db, &world.owner, SmartListJoiner::And, &filters).await;

		db.execute_unprepared(NO_MEMBERS).await.unwrap();
		assert_eq!(source(db, &list, &filters).await, Source::Live);
		assert!(!has_members(db, &list.id).await);

		// and the table gone (a database the migration didn't reach) is live too
		db.execute_unprepared("DROP TRIGGER test_no_members")
			.await
			.unwrap();
		db.execute_unprepared("DROP TABLE smart_list_members")
			.await
			.unwrap();
		assert_eq!(source(db, &list, &filters).await, Source::Live);
	}

	/// A list with a rule that is different for each reader is never kept
	#[tokio::test]
	async fn a_list_that_depends_on_the_reader_is_live() {
		let world = world().await;
		let db = &world.db;
		let filters = rules(group(
			"And",
			vec![json!({ "media": { "readingStatus": { "is": "NOT_STARTED" } } })],
		));
		let list = make_list(db, &world.owner, SmartListJoiner::And, &filters).await;
		assert_eq!(source(db, &list, &filters).await, Source::Live);
		assert!(!has_members(db, &list.id).await);
		assert_eq!(refreshes_of(&list.id), 0);
		// and the warmer leaves it alone
		assert_eq!(refresh_lists(db, None, Need::Always).await, 0);
		assert_eq!(refreshes_of(&list.id), 0);
	}

	// -----------------------------------------------------------------------------------------
	// the warmer
	// -----------------------------------------------------------------------------------------

	async fn wait_for<F, Fut>(mut condition: F)
	where
		F: FnMut() -> Fut,
		Fut: Future<Output = bool>,
	{
		for _ in 0..200 {
			if condition().await {
				return;
			}
			tokio::time::sleep(Duration::from_millis(50)).await;
		}
		panic!("it did not happen within 10 seconds");
	}

	/// The warmer has every list ready after start-up, and catches up with a change (a scan that
	/// adds 50 books) once the catalog is quiet, with nobody reading the list
	#[tokio::test]
	async fn the_warmer_catches_up_with_a_scan() {
		let world = world().await;
		let db = Arc::new(world.db.clone());
		let list =
			make_list(&db, &world.owner, SmartListJoiner::And, &science_rules()).await;
		assert!(!has_members(db.as_ref(), &list.id).await);

		let warmer = tokio::spawn(run_warmer(
			db.clone(),
			WarmerConfig {
				poll: Duration::from_millis(40),
				quiet: Duration::from_millis(120),
				backstop: Duration::from_secs(3600),
			},
		));

		// ready after start-up
		wait_for(|| async { has_members(db.as_ref(), &list.id).await }).await;
		assert_eq!(member_ids(&db, &list).await.len(), 4);

		// a scan adds 50 books to the science series
		let series = world.books[4].series_id.clone().expect("a series");
		for _ in 0..50 {
			let book = fake_data::Media {
				series_id: series.clone(),
				..Default::default()
			}
			.insert(db.as_ref())
			.await;
			media_metadata::ActiveModel {
				media_id: Set(Some(book.id.clone())),
				genres: Set(Some("Science".to_string())),
				..Default::default()
			}
			.insert(db.as_ref())
			.await
			.expect("should insert the metadata");
		}

		// the members catch up by themselves
		wait_for(|| async { member_ids(&db, &list).await.len() == 54 }).await;
		let state = read_state(db.as_ref(), &list.id).await.unwrap();
		assert!(state.is_fresh(Need::Current, Utc::now()));
		warmer.abort();
	}

	// -----------------------------------------------------------------------------------------
	// every column a rule can look at is watched
	// -----------------------------------------------------------------------------------------

	/// The `"table"."column"` references and the tables (`FROM "t"`, `JOIN "t"`) of a piece of SQL
	fn references(sql: &str) -> (HashSet<(String, String)>, HashSet<String>) {
		let chars: Vec<char> = sql.chars().collect();
		// every "quoted" name: where it opens, where it closes, and what it says
		let mut quoted: Vec<(usize, usize, String)> = Vec::new();
		let mut i = 0;
		while i < chars.len() {
			if chars[i] == '"' {
				let mut j = i + 1;
				while j < chars.len() && chars[j] != '"' {
					j += 1;
				}
				quoted.push((i, j, chars[i + 1..j].iter().collect()));
				i = j + 1;
			} else {
				i += 1;
			}
		}

		let mut columns = HashSet::new();
		for pair in quoted.windows(2) {
			// "table"."column"
			if pair[1].0 == pair[0].1 + 2 && chars[pair[0].1 + 1] == '.' {
				columns.insert((pair[0].2.clone(), pair[1].2.clone()));
			}
		}
		let mut tables = HashSet::new();
		for (start, _, name) in &quoted {
			let before: String = chars[..*start].iter().collect();
			let before = before.trim_end().to_uppercase();
			if before.ends_with("FROM") || before.ends_with("JOIN") {
				tables.insert(name.clone());
			}
		}
		(columns, tables)
	}

	/// What a filter field is in this test
	enum Sample {
		/// Not a field that reads a column of its own: a logical operator, or a nested filter
		/// that is checked as a filter of its own
		Skip,
		/// A value for the field, that a cached list can answer
		Kept(Json),
		/// A value for a field that keeps a list live
		Live(Json),
	}

	/// One value for each filter field (what the web app sends), by the kind and name of the
	/// field. A field that is added later has no sample: the test below fails until it has one,
	/// and its column is in [`schema::WATCHED`] (or [`schema::LIVE`]).
	fn sample(kind: &str, field: &str) -> Option<Sample> {
		use Sample::{Kept, Live, Skip};
		let text = || Kept(json!({ "eq": "x" }));
		let number = || Kept(json!({ "eq": 1 }));
		Some(match (kind, field) {
			(_, "_and" | "_or" | "_not") => Skip,
			("media", "metadata" | "series") | ("series", "metadata" | "library") => Skip,
			(
				"media",
				"id" | "name" | "extension" | "seriesId" | "libraryId" | "path" | "tags",
			) => text(),
			("media", "size" | "pages") => number(),
			("media", "createdAt") => Kept(json!({ "eq": "2026-01-01T00:00:00Z" })),
			("media", "updatedAt") => Live(json!({ "eq": "2026-01-01T00:00:00Z" })),
			("media", "status") => Kept(json!({ "eq": "READY" })),
			("media" | "series", "libraryType") => Kept(json!({ "isAnyOf": ["BOOK"] })),
			("media" | "series", "readingStatus") => Live(json!({ "is": "READING" })),
			("media" | "series", "search") => Kept(json!("word")),
			("series", "name" | "path" | "libraryId") => text(),
			("series", "isOneshot") => Kept(json!(true)),
			("library", "id" | "name" | "path") => text(),
			("mediaMetadata", "ageRating" | "rating" | "year" | "month" | "day") => {
				number()
			},
			("mediaMetadata", "adult") => Kept(json!(true)),
			("mediaMetadata", _) => text(),
			("seriesMetadata", "ageRating" | "comicid" | "volume" | "year") => number(),
			("seriesMetadata", _) => text(),
			_ => return None,
		})
	}

	/// The names of the fields of a GraphQL input type, from the schema
	async fn input_fields(input: &str) -> Vec<String> {
		let schema = crate::schema::build_schema_bare();
		let response = schema
			.execute(format!(
				r#"{{ __type(name: "{input}") {{ inputFields {{ name }} }} }}"#
			))
			.await;
		let data = response.data.into_json().expect("the data is JSON");
		data["__type"]["inputFields"]
			.as_array()
			.unwrap_or_else(|| panic!("no input {input} in the schema: {data}"))
			.iter()
			.map(|field| field["name"].as_str().expect("a name").to_string())
			.collect()
	}

	/// For every field of every filter a rule can use: the columns its SQL reads are watched by a
	/// trigger, or the field keeps a list live. The fields come from the schema, so a field that
	/// is added later fails this test until it is covered.
	#[tokio::test]
	async fn every_column_a_rule_can_read_is_watched_or_live() {
		let watched = |table: &str, column: &str| {
			schema::WATCHED
				.iter()
				.any(|w| w.table == table && w.columns.contains(&column))
		};
		let live = |table: &str, column: &str| {
			schema::LIVE
				.iter()
				.any(|(t, c)| *t == table && (*c == "*" || *c == column))
		};

		let mut checked = 0;
		for (input, kind) in [
			("MediaFilterInput", "media"),
			("MediaMetadataFilterInput", "mediaMetadata"),
			("SeriesFilterInput", "series"),
			("SeriesMetadataFilterInput", "seriesMetadata"),
			("LibraryFilterInput", "library"),
		] {
			let fields = input_fields(input).await;
			assert!(!fields.is_empty(), "no fields found for {input}");
			for field in fields {
				let (value, is_live) = match sample(kind, &field) {
					None => panic!(
						"{input}.{field} is a new filter field: give it a sample in this test, and \
						 make sure the column it reads is in models::catalog_revision::WATCHED (or LIVE)"
					),
					Some(Sample::Skip) => continue,
					Some(Sample::Kept(value)) => (value, false),
					Some(Sample::Live(value)) => (value, true),
				};
				let mut inner = serde_json::Map::new();
				inner.insert(field.clone(), value);
				let mut outer = serde_json::Map::new();
				outer.insert(kind.to_string(), Json::Object(inner));
				let filters: Vec<SmartListFilterGroupInput> = serde_json::from_value(
					json!([{ "joiner": "And", "groups": [Json::Object(outer)] }]),
				)
				.unwrap_or_else(|e| panic!("{input}.{field} should parse: {e}"));

				if is_live {
					assert!(
						!is_cacheable(&filters),
						"{input}.{field} should keep a list live"
					);
					checked += 1;
					continue;
				}
				assert!(
					is_cacheable(&filters),
					"{input}.{field} should not keep a list live"
				);

				let sql = Query::select()
					.cond_where(build_filters(SmartListJoiner::And, &filters))
					.to_string(SqliteQueryBuilder);
				let (columns, tables) = references(&sql);
				// a field the filter ignores reads nothing (the library filter has an `id` that
				// does nothing): it can't change what a rule matches
				let ignored = [("library", "id")].contains(&(kind, field.as_str()));
				assert!(
					ignored || !columns.is_empty() || !tables.is_empty(),
					"{input}.{field} reads nothing?\n{sql}"
				);
				for (table, column) in &columns {
					assert!(
						watched(table, column) || live(table, column),
						"{input}.{field} reads {table}.{column}, which no trigger watches\n{sql}"
					);
				}
				for table in &tables {
					assert!(
						schema::WATCHED.iter().any(|w| w.table == table) || live(table, "*"),
						"{input}.{field} reads from {table}, which no trigger watches\n{sql}"
					);
				}
				checked += 1;
			}
		}
		assert!(checked > 50, "only {checked} fields were checked");
	}
}
