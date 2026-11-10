use crate::{
	filter::IntoFilter,
	input::smart_lists::{
		SmartListFilterGroupInput, SmartListFilterInput, SmartListGroupJoiner,
	},
	object::{
		media::Media,
		smart_list_item::{
			SmartListGrouped, SmartListGroupedItem, SmartListItemEntity, SmartListItems,
			SmartListUngrouped,
		},
		smart_lists::SmartListMeta,
	},
	query::{
		media::{add_sessions_join_for_filter, should_add_sessions_join_for_filter},
		smart_list_cache::{self, Source},
	},
};
use async_graphql::Result;
use models::entity::{
	library, media, series,
	smart_list::{self, SmartListGrouping},
	user::AuthUser,
};
use sea_orm::{
	prelude::*,
	sea_query::{Alias, Expr, Query, SelectStatement},
	Condition, ConnectionTrait, DatabaseConnection, DatabaseTransaction, FromQueryResult,
	QuerySelect, QueryTrait, Select,
};

use std::collections::HashMap;

/// The books of a smart list that `user` may read, as a query to build on (order, limit, filters).
///
/// A list is answered from its cached members when it can be (see [`smart_list_cache`]): the
/// members are the books the list's rules match over the whole catalog, and the query narrows
/// them to what `user` may read, the same way every other list of books does. A list whose rules
/// use a reader's own progress or `updatedAt` is worked out when it is read. Either way the
/// result is the same books, so the caller can't tell which it got.
///
/// It looks at the cache before it returns, and so before the caller opens a transaction of its
/// own: a cache that is out of date is worked out again in a write of its own, which a
/// transaction that is already reading wouldn't see.
pub async fn smart_list_books(
	conn: &DatabaseConnection,
	user: &AuthUser,
	list: &smart_list::Model,
) -> Result<Select<media::Entity>> {
	let filters: Vec<SmartListFilterGroupInput> = serde_json::from_slice(&list.filters)?;
	Ok(match smart_list_cache::source(conn, list, &filters).await {
		Source::Cached => media::ModelWithMetadata::find_for_user(user).filter(
			media::Column::Id.in_subquery(smart_list_cache::members_subquery(&list.id)),
		),
		Source::Live => build_books_query(user, list.joiner, &filters, None),
	})
}

/// Resolves a smart list into the subquery of the IDs of the media it matches for `user`
pub async fn smart_list_media_ids(
	conn: &DatabaseConnection,
	user: &AuthUser,
	list: &smart_list::Model,
) -> Result<SelectStatement> {
	Ok(smart_list_books(conn, user, list)
		.await?
		.select_only()
		.column(media::Column::Id)
		.into_query())
}

#[derive(FromQueryResult)]
struct MetaCounts {
	books: i64,
	series: i64,
	libraries: i64,
}

/// How many books, series and libraries a smart list's books (see [`smart_list_books`]) come
/// from, counted by the database in one query.
///
/// The books are counted as a subquery: a list that is worked out live may group its books by ID
/// (it joins the reader's progress), which would make one count of every book.
pub async fn smart_list_meta<C: ConnectionTrait>(
	conn: &C,
	books: Select<media::Entity>,
) -> Result<SmartListMeta> {
	let matched = books
		.select_only()
		.column(media::Column::Id)
		.column(media::Column::SeriesId)
		.column(media::Column::LibraryId)
		.into_query();
	let counts = Query::select()
		.expr_as(Expr::cust("COUNT(*)"), Alias::new("books"))
		.expr_as(
			Expr::cust("COUNT(DISTINCT series_id)"),
			Alias::new("series"),
		)
		.expr_as(
			Expr::cust("COUNT(DISTINCT library_id)"),
			Alias::new("libraries"),
		)
		.from_subquery(matched, Alias::new("matched"))
		.to_owned();

	let counts =
		MetaCounts::find_by_statement(conn.get_database_backend().build(&counts))
			.one(conn)
			.await?;

	Ok(match counts {
		Some(counts) => SmartListMeta {
			matched_books: counts.books,
			matched_series: counts.series,
			matched_libraries: counts.libraries,
		},
		None => SmartListMeta {
			matched_books: 0,
			matched_series: 0,
			matched_libraries: 0,
		},
	})
}

/// The IDs of the books a smart list's rules match over the whole catalog: the books of every
/// library, removed ones too, whoever reads the list. These are the list's members, which the
/// cache keeps (see [`smart_list_cache`]); what a reader gets of them is a different query
/// ([`smart_list_books`]).
pub fn catalog_members_query(
	joiner: smart_list::SmartListJoiner,
	filters: &[SmartListFilterGroupInput],
) -> Select<media::Entity> {
	add_library_join(media::Entity::find_catalog(), filters)
		.filter(build_filters(joiner, filters))
		.select_only()
		.column(media::Column::Id)
}

/// The IDs of the media visible to `user` which match an ad-hoc [MediaFilterInput], as a
/// subquery. Used to narrow down a smart list without fighting its own joins.
pub fn filtered_media_ids(
	user: &AuthUser,
	filter: crate::filter::media::MediaFilterInput,
) -> SelectStatement {
	add_sessions_join_for_filter(user, &filter, media::Entity::find_for_user(user))
		.filter(filter.into_filter())
		.select_only()
		.column(media::Column::Id)
		.into_query()
}

pub async fn build_smart_list_items(
	user: &AuthUser,
	grouping: SmartListGrouping,
	books: Vec<Media>,
	txn: &DatabaseTransaction,
) -> Result<SmartListItems> {
	match grouping {
		SmartListGrouping::ByBooks => {
			Ok(SmartListItems::Ungrouped(SmartListUngrouped { books }))
		},
		SmartListGrouping::BySeries => group_by_series(user, books, txn).await,
		SmartListGrouping::ByLibrary => group_by_library(user, books, txn).await,
	}
}

async fn group_by_series(
	user: &AuthUser,
	books: Vec<Media>,
	txn: &DatabaseTransaction,
) -> Result<SmartListItems> {
	let mut series_map: HashMap<Option<String>, Vec<Media>> = HashMap::new();
	for book in books {
		series_map
			.entry(book.model.series_id.clone())
			.or_default()
			.push(book);
	}

	// get all series for the books
	let series_models = series::ModelWithMetadata::find_for_user(user)
		.filter(series::Column::Id.is_in(series_map.keys().flatten()))
		.into_model::<series::ModelWithMetadata>()
		.all(txn)
		.await?;

	let mut items: Vec<SmartListGroupedItem> = series_models
		.into_iter()
		.map(|series_model| {
			let books = series_map
				.remove(&Some(series_model.series.id.clone()))
				.unwrap_or_default();
			SmartListGroupedItem {
				entity: Some(SmartListItemEntity::Series(Box::new(series_model.into()))),
				books,
			}
		})
		.collect();
	if let Some(books) = series_map.remove(&None) {
		items.push(SmartListGroupedItem {
			entity: None,
			books,
		});
	}

	Ok(SmartListItems::Grouped(SmartListGrouped { items }))
}

async fn group_by_library(
	user: &AuthUser,
	books: Vec<Media>,
	txn: &DatabaseTransaction,
) -> Result<SmartListItems> {
	let mut library_books: HashMap<String, Vec<Media>> = HashMap::new();
	for book in books {
		library_books
			.entry(book.model.library_id.clone())
			.or_default()
			.push(book);
	}
	let library_models = library::Entity::find_for_user(user)
		.filter(library::Column::Id.is_in(library_books.keys()))
		.all(txn)
		.await?;
	let items = library_models
		.into_iter()
		.map(|library| {
			let books = library_books.remove(&library.id).unwrap_or_default();
			SmartListGroupedItem {
				entity: Some(SmartListItemEntity::Library(Box::new(library.into()))),
				books,
			}
		})
		.collect();

	Ok(SmartListItems::Grouped(SmartListGrouped { items }))
}

pub fn build_filters(
	joiner: smart_list::SmartListJoiner,
	filters: &[SmartListFilterGroupInput],
) -> Condition {
	let start_condition = if joiner == smart_list::SmartListJoiner::Or {
		Condition::any()
	} else {
		Condition::all()
	};

	// accumulate conditions based on filters
	filters.iter().fold(start_condition, |acc, filter_group| {
		let mut condition = match filter_group.joiner {
			SmartListGroupJoiner::And => Condition::all(),
			SmartListGroupJoiner::Or => Condition::any(),
			// semantically this is meant to be "none in group are true" and therefore
			// uses `any` instead of `all`, otherwise it is a simple negate
			SmartListGroupJoiner::Not => Condition::any().not(),
		};
		for filter in &filter_group.groups {
			condition = match filter {
				SmartListFilterInput::Media(media_filter) => {
					condition.add(media_filter.clone().into_filter())
				},
				SmartListFilterInput::MediaMetadata(media_metadata_filter) => {
					condition.add(media_metadata_filter.clone().into_filter())
				},
				SmartListFilterInput::Series(series_filter) => {
					condition.add(series_filter.clone().into_filter())
				},
				SmartListFilterInput::SeriesMetadata(series_metadata_filter) => {
					condition.add(series_metadata_filter.clone().into_filter())
				},
				SmartListFilterInput::Library(library_filter) => {
					condition.add(library_filter.clone().into_filter())
				},
			};
		}

		acc.add(condition)
	})
}

pub fn build_books_query(
	user: &AuthUser,
	joiner: smart_list::SmartListJoiner,
	filters: &[SmartListFilterGroupInput],
	limit: Option<u64>,
) -> Select<media::Entity> {
	let conditions = build_filters(joiner, filters);
	let query =
		add_sessions_join(user, media::ModelWithMetadata::find_for_user(user), filters)
			.apply_if(limit, |query, limit| query.limit(limit));
	let query = add_library_join(query, filters);

	query.filter(conditions)
}

fn add_library_join(
	query: Select<media::Entity>,
	filters: &[SmartListFilterGroupInput],
) -> Select<media::Entity> {
	let is_using_library = filters.iter().any(|filter_group| {
		for filter in &filter_group.groups {
			if let SmartListFilterInput::Library(_) = filter {
				return true;
			}
		}

		false
	});

	if is_using_library {
		query.inner_join(library::Entity)
	} else {
		query
	}
}

fn add_sessions_join(
	user: &AuthUser,
	query: Select<media::Entity>,
	filters: &[SmartListFilterGroupInput],
) -> Select<media::Entity> {
	let filter_using_session = filters.iter().find(|filter_group| {
		for filter in &filter_group.groups {
			if let SmartListFilterInput::Media(media_filter) = filter {
				if should_add_sessions_join_for_filter(media_filter) {
					return true;
				}
			}
		}

		false
	});

	if let Some(filter_group) = filter_using_session {
		for filter in &filter_group.groups {
			if let SmartListFilterInput::Media(media_filter) = filter {
				return add_sessions_join_for_filter(user, media_filter, query);
			}
		}
	}

	query
}

// TODO(tests): i hate these sql string tests, thanks to the newer fake_data test crate i think
// we should just instead actually kick off queries against real data and assert a few fundamentals

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		filter::{
			library::LibraryFilterInput, media::MediaFilterInput, NumericFilter,
			StringLikeFilter,
		},
		tests::common::get_default_user,
	};
	use ::tests::{db::test_database, fake_data};
	use pretty_assertions::assert_eq;
	use sea_orm::{
		sea_query::{Query, SqliteQueryBuilder},
		QueryTrait,
	};

	pub fn condition_to_string(condition: &Condition) -> String {
		Query::select()
			.cond_where(condition.clone())
			.to_string(SqliteQueryBuilder)
	}

	#[tokio::test]
	async fn test_media_group_filter_not_is_none_are_true() {
		let db = test_database().await;
		let auth_user = fake_data::User::default().auth_user(&db).await;

		let filters: Vec<SmartListFilterGroupInput> = vec![SmartListFilterGroupInput {
			joiner: SmartListGroupJoiner::Not,
			groups: vec![
				SmartListFilterInput::Media(MediaFilterInput {
					name: Some(StringLikeFilter::Eq("stinky book".to_string())),
					..Default::default()
				}),
				SmartListFilterInput::Media(MediaFilterInput {
					name: Some(StringLikeFilter::Eq("another stinky book".to_string())),
					..Default::default()
				}),
				SmartListFilterInput::Media(MediaFilterInput {
					size: Some(NumericFilter::Eq(100)),
					..Default::default()
				}),
			],
		}];

		let series = fake_data::Series::default().insert(&db).await;

		let stinky_book = fake_data::Media {
			name: Some("stinky book".to_string()),
			series_id: series.id.clone(),
			..Default::default()
		}
		.insert(&db)
		.await;

		let another_stinky_book = fake_data::Media {
			name: Some("another stinky book".to_string()),
			series_id: series.id.clone(),
			..Default::default()
		}
		.insert(&db)
		.await;

		let okay_name_but_stinky_size = fake_data::Media {
			name: Some("okay name but stinky size".to_string()),
			size: Some(100),
			series_id: series.id.clone(),
			..Default::default()
		}
		.insert(&db)
		.await;

		let totally_okay_will_return_book = fake_data::Media {
			name: Some("totally okay will return book".to_string()),
			series_id: series.id.clone(),
			..Default::default()
		}
		.insert(&db)
		.await;

		let should_exclude_ids = vec![
			stinky_book.id,
			another_stinky_book.id,
			okay_name_but_stinky_size.id,
		];

		let query = build_books_query(
			&auth_user,
			smart_list::SmartListJoiner::And,
			&filters,
			None,
		);

		let results = query
			.into_model::<media::ModelWithMetadata>()
			.all(&db)
			.await
			.expect("should exec query");

		assert_eq!(results.len(), 1);
		assert!(!results
			.iter()
			.any(|book| should_exclude_ids.contains(&book.media.id)));
		assert_eq!(results[0].media.id, totally_okay_will_return_book.id);
	}

	#[test]
	fn test_build_filters_empty() {
		let filters: Vec<SmartListFilterGroupInput> = vec![];
		let condition = build_filters(smart_list::SmartListJoiner::And, &filters);
		let sql = condition_to_string(&condition);
		assert_eq!(sql, "SELECT  WHERE TRUE");
	}

	#[test]
	fn test_build_filters_simple() {
		let filters: Vec<SmartListFilterGroupInput> = vec![SmartListFilterGroupInput {
			joiner: SmartListGroupJoiner::And,
			groups: vec![SmartListFilterInput::Media(MediaFilterInput {
				name: Some(StringLikeFilter::Eq("Test".to_string())),
				..Default::default()
			})],
		}];
		let condition = build_filters(smart_list::SmartListJoiner::And, &filters);
		let sql = condition_to_string(&condition);
		assert_eq!(sql, r#"SELECT  WHERE "media"."name" = 'Test'"#);
	}

	#[test]
	fn test_build_filters_two() {
		let filters: Vec<SmartListFilterGroupInput> = vec![
			SmartListFilterGroupInput {
				joiner: SmartListGroupJoiner::Not,
				groups: vec![SmartListFilterInput::Media(MediaFilterInput {
					name: Some(StringLikeFilter::Eq("Book".to_string())),
					..Default::default()
				})],
			},
			SmartListFilterGroupInput {
				joiner: SmartListGroupJoiner::Or,
				groups: vec![SmartListFilterInput::Library(LibraryFilterInput {
					name: Some(StringLikeFilter::Eq("Test".to_string())),
					..Default::default()
				})],
			},
		];
		let condition = build_filters(smart_list::SmartListJoiner::Or, &filters);
		let sql = condition_to_string(&condition);
		assert_eq!(
			sql,
			r#"SELECT  WHERE (NOT "media"."name" = 'Book') OR "libraries"."name" = 'Test'"#
		);
	}

	#[test]
	fn test_build_books_query() {
		let filters: Vec<SmartListFilterGroupInput> = vec![
			SmartListFilterGroupInput {
				joiner: SmartListGroupJoiner::Or,
				groups: vec![SmartListFilterInput::Media(MediaFilterInput {
					name: Some(StringLikeFilter::Eq("Book".to_string())),
					..Default::default()
				})],
			},
			SmartListFilterGroupInput {
				joiner: SmartListGroupJoiner::Or,
				groups: vec![SmartListFilterInput::Library(LibraryFilterInput {
					name: Some(StringLikeFilter::Eq("Test".to_string())),
					..Default::default()
				})],
			},
		];
		let user = get_default_user();
		let query =
			build_books_query(&user, smart_list::SmartListJoiner::Or, &filters, None);

		let sql = query
			.select_only()
			.into_query()
			.to_string(SqliteQueryBuilder);
		assert_eq!(
			sql,
			r#"SELECT  FROM "media" LEFT JOIN "media_metadata" ON "media"."id" = "media_metadata"."media_id" LEFT JOIN "series" ON "media"."series_id" = "series"."id" LEFT JOIN "series_metadata" ON "series_metadata"."series_id" = "series"."id" INNER JOIN "libraries" ON "media"."library_id" = "libraries"."id" WHERE "media"."library_id" NOT IN (SELECT "library_id" FROM "library_exclusions" WHERE "library_exclusions"."user_id" = '42') AND "media"."deleted_at" IS NULL AND ("media"."name" = 'Book' OR "libraries"."name" = 'Test')"#
		);
	}
}
