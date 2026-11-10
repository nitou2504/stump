//! What the smart lists' cache saves (package SL-3), measured.
//!
//! Every read of a smart list used to work its rules out over the whole catalog: the books of a
//! screen of lists (`meta`, `books`, a page of items) each cost one evaluation of the rules, and a
//! real list is a few rules about genres and tags plus up to a thousand paths. With the cache a
//! read is a lookup in the list's members, narrowed to what the reader may see.
//!
//! [`a_kept_list_is_cheaper_than_one_worked_out_on_every_visit`] builds such a catalog and a list
//! of that shape and times both (it asserts a conservative ratio, and prints the numbers).
//! [`on_a_copy_of_the_real_database`] times every list of a copy of the real database: run it with
//! `STUMP_SL3_PERF_DB=/path/to/a/copy cargo test -p graphql on_a_copy_of_the_real_database -- --ignored --nocapture`.

use std::time::{Duration, Instant};

use ::tests::{db::test_database, fake_data};
use models::entity::{
	media,
	smart_list::{self, SmartListGrouping, SmartListJoiner},
	user::AuthUser,
};
use sea_orm::{
	prelude::*, ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseConnection,
	QueryOrder, QuerySelect,
};
use serde_json::json;

use crate::{
	input::smart_lists::SmartListFilterGroupInput,
	object::smart_lists::SmartListMeta,
	query::{
		smart_list_cache,
		smart_lists_builder::{build_books_query, smart_list_books, smart_list_meta},
	},
};

const BOOKS: usize = 6_000;
const PATHS: usize = 900;

/// What a read used to do for `meta`: every matching book's series and library, counted here
async fn live_meta(
	conn: &DatabaseConnection,
	user: &AuthUser,
	list: &smart_list::Model,
) -> i64 {
	let filters: Vec<SmartListFilterGroupInput> =
		serde_json::from_slice(&list.filters).unwrap();
	let rows: Vec<(Option<String>, String)> =
		build_books_query(user, list.joiner, &filters, None)
			.select_only()
			.column(media::Column::SeriesId)
			.column(media::Column::LibraryId)
			.into_tuple()
			.all(conn)
			.await
			.unwrap();
	rows.len() as i64
}

/// What a read used to do for a page of items
async fn live_page(
	conn: &DatabaseConnection,
	user: &AuthUser,
	list: &smart_list::Model,
	limit: u64,
) -> usize {
	let filters: Vec<SmartListFilterGroupInput> =
		serde_json::from_slice(&list.filters).unwrap();
	build_books_query(user, list.joiner, &filters, None)
		.order_by_asc(media::Column::Id)
		.limit(limit)
		.into_model::<media::ModelWithMetadata>()
		.all(conn)
		.await
		.unwrap()
		.len()
}

async fn cached_meta(
	conn: &DatabaseConnection,
	user: &AuthUser,
	list: &smart_list::Model,
) -> SmartListMeta {
	let books = smart_list_books(conn, user, list).await.unwrap();
	smart_list_meta(conn, books).await.unwrap()
}

async fn cached_page(
	conn: &DatabaseConnection,
	user: &AuthUser,
	list: &smart_list::Model,
	limit: u64,
) -> usize {
	smart_list_books(conn, user, list)
		.await
		.unwrap()
		.order_by_asc(media::Column::Id)
		.limit(limit)
		.into_model::<media::ModelWithMetadata>()
		.all(conn)
		.await
		.unwrap()
		.len()
}

/// The time of `runs` calls of `f`, on average
async fn average<F, Fut, R>(runs: u32, mut f: F) -> Duration
where
	F: FnMut() -> Fut,
	Fut: std::future::Future<Output = R>,
{
	let started = Instant::now();
	for _ in 0..runs {
		f().await;
	}
	started.elapsed() / runs
}

fn ms(duration: Duration) -> f64 {
	duration.as_secs_f64() * 1000.0
}

/// A catalog of [`BOOKS`] books with genres, tags and paths, and a list of the shape the real
/// ones have: an OR of a genre and a tag, and an OR of [`PATHS`] paths
async fn catalog_and_list() -> (DatabaseConnection, AuthUser, smart_list::Model) {
	let db = test_database().await;
	let owner = fake_data::User::new("owner").auth_user(&db).await;
	let library = fake_data::Library::default().insert(&db).await;
	let series = fake_data::Series {
		library_id: Some(library.id.clone()),
		..Default::default()
	}
	.insert(&db)
	.await;

	db.execute_unprepared(&format!(
		"WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < {BOOKS})
		 INSERT INTO media (id, name, size, extension, pages, is_oneshot, path, status, library_id, series_id, created_at)
		 SELECT 'perf-' || n, 'Book ' || n, 1000, 'epub', 100, 0,
		        '/shelf/' || (n % 400) || '/book-' || n || '.epub', 'READY', '{}', '{}', '2026-01-01T00:00:00+00:00'
		 FROM seq",
		library.id, series.id
	))
	.await
	.expect("should insert the books");
	db.execute_unprepared(
		"INSERT INTO media_metadata (media_id, genres, writers, age_rating)
		 SELECT id,
		        CASE (CAST(substr(id, 6) AS INTEGER) % 9)
		          WHEN 0 THEN 'Fantasy, Adventure' WHEN 1 THEN 'Science' WHEN 2 THEN 'History, Science'
		          ELSE 'Other' END,
		        'Writer ' || (CAST(substr(id, 6) AS INTEGER) % 70), 0
		 FROM media WHERE id LIKE 'perf-%'",
	)
	.await
	.expect("should insert the metadata");
	db.execute_unprepared("INSERT INTO tags (name) VALUES ('Adventure'), ('Classic')")
		.await
		.expect("should insert tags");
	db.execute_unprepared(
		"INSERT INTO media_tags (media_id, tag_id)
		 SELECT id, 1 FROM media WHERE id LIKE 'perf-%' AND CAST(substr(id, 6) AS INTEGER) % 11 = 0",
	)
	.await
	.expect("should tag the books");

	let paths: Vec<String> = (1..=PATHS)
		.map(|i| {
			let n = i * (BOOKS / PATHS);
			format!("/shelf/{}/book-{n}.epub", n % 400)
		})
		.collect();
	let filters = json!([
		{ "joiner": "Or", "groups": [
			{ "mediaMetadata": { "genres": { "hasAnyOf": ["Adventure"] } } },
			{ "media": { "tags": { "anyOf": ["Adventure"] } } },
		]},
		{ "joiner": "Or", "groups": [{ "media": { "path": { "anyOf": paths } } }] },
	]);
	let filters: Vec<SmartListFilterGroupInput> =
		serde_json::from_value(filters).unwrap();
	let list = smart_list::ActiveModel {
		id: Set(Uuid::new_v4().to_string()),
		name: Set("A shelf".to_string()),
		filters: Set(serde_json::to_vec(&filters).unwrap()),
		joiner: Set(SmartListJoiner::Or),
		default_grouping: Set(SmartListGrouping::ByBooks),
		visibility: Set(models::shared::enums::EntityVisibility::Public),
		creator_id: Set(owner.id.clone()),
		..Default::default()
	}
	.insert(&db)
	.await
	.expect("should insert the list");
	(db, owner, list)
}

/// A list matching every 70th book: a page of it can't stop early, the rules go over the catalog
async fn sparse_list(db: &DatabaseConnection, owner: &AuthUser) -> smart_list::Model {
	let filters: Vec<SmartListFilterGroupInput> = serde_json::from_value(json!([
		{ "joiner": "Or", "groups": [{ "mediaMetadata": { "writers": { "hasAnyOf": ["Writer 7"] } } }] }
	]))
	.unwrap();
	smart_list::ActiveModel {
		id: Set(Uuid::new_v4().to_string()),
		name: Set("A writer".to_string()),
		filters: Set(serde_json::to_vec(&filters).unwrap()),
		joiner: Set(SmartListJoiner::Or),
		default_grouping: Set(SmartListGrouping::ByBooks),
		visibility: Set(models::shared::enums::EntityVisibility::Public),
		creator_id: Set(owner.id.clone()),
		..Default::default()
	}
	.insert(db)
	.await
	.expect("should insert the list")
}

/// A list of the shape the real ones have, over thousands of books: the cached list gives the same
/// books and counts, and answers the reads of a screen faster than the rules do. A page of a long
/// list stops as soon as it has its books, so it was cheap already (the cache must not make it
/// dearer); what the cache saves is the reads that go over the whole catalog: the counts, and the
/// page of a list that is short.
#[tokio::test]
async fn a_kept_list_is_cheaper_than_one_worked_out_on_every_visit() {
	let (db, owner, list) = catalog_and_list().await;
	let sparse = sparse_list(&db, &owner).await;

	// the same answer
	let live = live_meta(&db, &owner, &list).await;
	let first_read = Instant::now();
	let kept = cached_meta(&db, &owner, &list).await;
	let first_read = first_read.elapsed();
	assert_eq!(kept.matched_books, live);
	assert!(live > 500, "the list should be a long one: {live}");
	assert_eq!(live_page(&db, &owner, &list, 100).await, 100);
	assert_eq!(cached_page(&db, &owner, &list, 100).await, 100);
	let sparse_books = live_meta(&db, &owner, &sparse).await;
	assert_eq!(
		cached_meta(&db, &owner, &sparse).await.matched_books,
		sparse_books
	);
	assert!(sparse_books > 50 && sparse_books < 100, "{sparse_books}");
	assert_eq!(
		live_page(&db, &owner, &sparse, 100).await,
		sparse_books as usize
	);
	assert_eq!(
		cached_page(&db, &owner, &sparse, 100).await,
		sparse_books as usize
	);

	// the cost of a read, before and after
	let runs = 5;
	let before_meta = average(runs, || live_meta(&db, &owner, &list)).await;
	let before_page = average(runs, || live_page(&db, &owner, &list, 100)).await;
	let before_sparse = average(runs, || live_page(&db, &owner, &sparse, 100)).await;
	let after_meta = average(runs, || cached_meta(&db, &owner, &list)).await;
	let after_page = average(runs, || cached_page(&db, &owner, &list, 100)).await;
	let after_sparse = average(runs, || cached_page(&db, &owner, &sparse, 100)).await;
	eprintln!(
		"{BOOKS} books, a list of {live} ({PATHS} paths): meta {:.1} ms -> {:.1} ms, page of 100 {:.1} ms -> {:.1} ms; \
		 a list of {sparse_books}: page {:.1} ms -> {:.1} ms; first read, which works the list out: {:.1} ms",
		ms(before_meta),
		ms(after_meta),
		ms(before_page),
		ms(after_page),
		ms(before_sparse),
		ms(after_sparse),
		ms(first_read),
	);

	// generous margins: this runs beside the rest of the tests, and what it guards is that the
	// cache is not the dearer way (the numbers above are the measurement)
	assert!(
		after_meta * 2 < before_meta,
		"meta: {:.1} ms kept, {:.1} ms worked out",
		ms(after_meta),
		ms(before_meta)
	);
	assert!(
		after_sparse < before_sparse,
		"the page of a short list: {:.1} ms kept, {:.1} ms worked out",
		ms(after_sparse),
		ms(before_sparse)
	);
	assert!(
		after_page < before_page * 3,
		"the page of a long list: {:.1} ms kept, {:.1} ms worked out",
		ms(after_page),
		ms(before_page)
	);
	assert!(
		smart_list_cache::refresh_lists(&db, None, smart_list_cache::Need::Always).await
			>= 1
	);
}

/// Every list of a copy of the real database, as the owner reads it: `meta` and the first page of
/// each, worked out on the spot and from the cache (once the cache is warm)
#[tokio::test]
#[ignore = "needs a copy of the real database: STUMP_SL3_PERF_DB=/path/to/a/copy"]
async fn on_a_copy_of_the_real_database() {
	use models::entity::user;

	let path = std::env::var("STUMP_SL3_PERF_DB")
		.expect("STUMP_SL3_PERF_DB should be the path of a COPY of the database");
	let conn = stump_core::database::connect_at(&format!("sqlite://{path}?mode=rw"))
		.await
		.expect("should open and migrate the copy");

	let owner_row = user::Entity::find()
		.filter(user::Column::IsServerOwner.eq(true))
		.one(&conn)
		.await
		.unwrap()
		.expect("an owner");
	let owner = AuthUser::from(
		user::LoginUser::find_by_id(owner_row.id)
			.into_model::<user::LoginUser>()
			.one(&conn)
			.await
			.unwrap()
			.expect("the owner"),
	);

	let lists = smart_list::Entity::find().all(&conn).await.unwrap();
	let books = media::Entity::find().count(&conn).await.unwrap();
	eprintln!("{} books, {} smart lists", books, lists.len());

	// before: every list's meta and first page, worked out on the spot
	let mut rows = Vec::new();
	let mut before_meta = Duration::ZERO;
	let mut before_page = Duration::ZERO;
	for list in &lists {
		let started = Instant::now();
		let books = live_meta(&conn, &owner, list).await;
		let meta = started.elapsed();
		let started = Instant::now();
		live_page(&conn, &owner, list, 100).await;
		let page = started.elapsed();
		before_meta += meta;
		before_page += page;
		rows.push((list.name.clone(), books, meta, page));
	}

	// the first read of each list works it out
	let started = Instant::now();
	for list in &lists {
		cached_meta(&conn, &owner, list).await;
	}
	let first_reads = started.elapsed();

	// after: the same, from the cache
	let mut after_meta = Duration::ZERO;
	let mut after_page = Duration::ZERO;
	let mut biggest: Option<(String, i64, Duration, Duration, Duration)> = None;
	for (list, (name, books, _live_meta, live_p)) in lists.iter().zip(&rows) {
		let started = Instant::now();
		let meta = cached_meta(&conn, &owner, list).await;
		let m = started.elapsed();
		assert_eq!(meta.matched_books, *books, "{name}");
		let started = Instant::now();
		cached_page(&conn, &owner, list, 100).await;
		let p = started.elapsed();
		after_meta += m;
		after_page += p;
		if biggest.as_ref().map_or(true, |(_, b, ..)| books > b) {
			biggest = Some((name.clone(), *books, *live_p, p, m));
		}
	}

	eprintln!(
		"meta of all {} lists: {:.0} ms worked out, {:.0} ms kept",
		lists.len(),
		ms(before_meta),
		ms(after_meta)
	);
	eprintln!(
		"first page of all {} lists: {:.0} ms worked out, {:.0} ms kept",
		lists.len(),
		ms(before_page),
		ms(after_page)
	);
	eprintln!(
		"working all {} lists out once (what the warmer does at start-up): {:.0} ms",
		lists.len(),
		ms(first_reads)
	);
	// where the time of working a list out goes: reading the rules' books, and writing them
	let mut read_total = Duration::ZERO;
	let mut refresh_total = Duration::ZERO;
	for list in &lists {
		let filters: Vec<SmartListFilterGroupInput> =
			serde_json::from_slice(&list.filters).unwrap();
		if !smart_list_cache::is_cacheable(&filters) {
			continue;
		}
		let started = Instant::now();
		let ids: Vec<String> = crate::query::smart_lists_builder::catalog_members_query(
			list.joiner,
			&filters,
		)
		.into_tuple::<String>()
		.all(&conn)
		.await
		.unwrap();
		read_total += started.elapsed();
		let started = Instant::now();
		smart_list_cache::ensure_fresh(
			&conn,
			list,
			&filters,
			smart_list_cache::Need::Always,
		)
		.await
		.unwrap();
		refresh_total += started.elapsed();
		let _ = ids;
	}
	eprintln!(
		"working a list out, for all of them: {:.0} ms to read the books of the rules, {:.0} ms for a whole refresh (which reads them again and writes)",
		ms(read_total),
		ms(refresh_total)
	);

	if let Some((name, books, live_p, p, m)) = biggest {
		eprintln!(
			"the biggest list, {name} ({books} books): page {:.1} ms worked out, {:.1} ms kept; meta {:.1} ms kept",
			ms(live_p),
			ms(p),
			ms(m)
		);
	}
}
