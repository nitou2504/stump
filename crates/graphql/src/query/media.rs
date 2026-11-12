use std::collections::{HashMap, HashSet};

use async_graphql::{Context, Object, Result, ID};
use models::{
	entity::{
		media, media_metadata, reading_session, user::AuthUser, user_discover_rules,
	},
	shared::{
		discover::DiscoverRules,
		enums::{ReadingStatus, UserPermission},
	},
};
use sea_orm::{
	prelude::*,
	sea_query::{ExprTrait, Query},
	Condition, FromQueryResult, JoinType, Order, QueryOrder, QuerySelect, QueryTrait,
};

use crate::{
	data::{AuthContext, CoreContext},
	filter::{media::MediaFilterInput, search::media_relevance, IntoFilter},
	guard::{PermissionGuard, ServerOwnerGuard},
	object::{
		media::Media,
		reading_session::{ReadingSession, ReadingSessionConflictResolutionView},
	},
	order::{order_spec, MediaOrderBy},
	pagination::{
		list_limit, CursorPaginationInfo, OffsetPaginationInfo, PaginatedResponse,
		Pagination, PaginationValidator,
	},
	utils::{db_statement, letter_counts},
};

/// The most characters of a `relevanceTo` text that are used
const MAX_RELEVANCE_CHARS: usize = 200;

#[derive(Default)]
pub struct MediaQuery;

/// The user's saved picks rules, or the defaults
pub async fn saved_discover_rules(
	conn: &DatabaseConnection,
	user_id: &str,
) -> Result<DiscoverRules> {
	let saved = user_discover_rules::Entity::find_by_id(user_id.to_string())
		.one(conn)
		.await?;
	Ok(DiscoverRules::from_json(
		saved.as_ref().map(|row| row.rules.as_str()),
	))
}

/// Seeded picks for `randomMedia`: each ID gets a stable pseudo-random key from the seed, and
/// the lowest keys win, so a seed always picks the same books (in the same order) as long as
/// they still match, whatever order the database returns them in
mod random_pick {
	use models::{
		entity::{media, media_metadata},
		shared::discover::DiscoverRules,
	};
	use sea_orm::{prelude::*, sea_query::Expr, Condition, Value};

	use crate::filter::{list_has_element, list_has_element_starting_with};

	/// `outer NOT IN` the ids tagged with any of `tags` in a `*_tags` junction table
	fn not_tagged(table: &str, column: &str, outer: &str, tags: &[String]) -> Expr {
		let placeholders = vec!["?"; tags.len()].join(", ");
		Expr::cust_with_values(
			format!(
				r#"{outer} NOT IN (SELECT "{table}"."{column}" FROM "{table}" JOIN "tags" ON "tags"."id" = "{table}"."tag_id" WHERE LOWER("tags"."name") IN ({placeholders}))"#
			),
			// SQLite's LOWER folds ASCII only, so the values are folded the same way
			tags.iter()
				.map(|tag| Value::from(tag.to_ascii_lowercase()))
				.collect::<Vec<_>>(),
		)
	}

	/// The books a user's picks may come from (see `DiscoverRules`). Needs the media
	/// metadata join (`find_for_user` has it)
	pub fn rules_condition(user_id: &str, rules: &DiscoverRules) -> Condition {
		let mut condition = Condition::all();
		if rules.unread_only {
			condition = condition.add(Expr::cust_with_values(
				r#"NOT EXISTS (SELECT 1 FROM "reading_sessions" WHERE "reading_sessions"."media_id" = "media"."id" AND "reading_sessions"."user_id" = ?)"#,
				[Value::from(user_id.to_string())],
			));
		}
		let tags = &rules.excluded_tags;
		if !tags.is_empty() {
			condition = condition
				.add(not_tagged(
					"media_tags",
					"media_id",
					r#""media"."id""#,
					tags,
				))
				.add(not_tagged(
					"library_tags",
					"library_id",
					r#""media"."library_id""#,
					tags,
				))
				.add(Condition::any().add(media::Column::SeriesId.is_null()).add(
					not_tagged(
						"series_tags",
						"series_id",
						r#""media"."series_id""#,
						tags,
					),
				));
		}
		if !rules.excluded_genres.is_empty() {
			let genres = media_metadata::Column::Genres;
			let any =
				rules
					.excluded_genres
					.iter()
					.fold(Condition::any(), |any, genre| {
						match genre.strip_suffix('*') {
							Some(prefix) => {
								any.add(list_has_element_starting_with(genres, prefix))
							},
							None => any.add(list_has_element(genres, genre)),
						}
					});
			condition =
				condition.add(Condition::any().add(genres.is_null()).add(any.not()));
		}
		if !rules.excluded_library_ids.is_empty() {
			condition = condition.add(
				media::Column::LibraryId.is_not_in(rules.excluded_library_ids.clone()),
			);
		}
		if let Some(rating) = rules.min_rating {
			condition = condition.add(
				Condition::any()
					.add(media_metadata::Column::Rating.is_null())
					.add(media_metadata::Column::Rating.gte(rating)),
			);
		}
		condition
	}

	/// FNV-1a over `value`, continuing from `state`
	pub fn hash_str(state: u64, value: &str) -> u64 {
		let mut hash = state ^ 0xcbf2_9ce4_8422_2325;
		for byte in value.bytes() {
			hash ^= u64::from(byte);
			hash = hash.wrapping_mul(0x0100_0000_01b3);
		}
		hash
	}

	/// splitmix64's finalizer: spreads FNV's output over all bits
	fn mix(mut z: u64) -> u64 {
		z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
		z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
		z ^ (z >> 31)
	}

	/// The `count` IDs with the lowest keys for `seed`, lowest first (linear time)
	pub fn pick(ids: Vec<String>, seed: u64, count: usize) -> Vec<String> {
		let mut keyed: Vec<(u64, String)> = ids
			.into_iter()
			.map(|id| (mix(hash_str(seed, &id)), id))
			.collect();
		if keyed.len() > count {
			keyed.select_nth_unstable(count);
			keyed.truncate(count);
		}
		keyed.sort_unstable();
		keyed.into_iter().map(|(_, id)| id).collect()
	}

	#[cfg(test)]
	mod tests {
		use super::*;

		fn ids(n: usize) -> Vec<String> {
			(0..n).map(|i| format!("book-{i}")).collect()
		}

		#[test]
		fn same_seed_same_picks_whatever_the_order() {
			let mut reversed = ids(500);
			reversed.reverse();
			assert_eq!(pick(ids(500), 7, 12), pick(reversed, 7, 12));
			assert_eq!(pick(ids(500), 7, 12).len(), 12);
		}

		#[test]
		fn another_seed_other_picks() {
			assert_ne!(pick(ids(500), 7, 12), pick(ids(500), 8, 12));
		}

		#[test]
		fn fewer_books_than_asked_returns_them_all() {
			let mut picked = pick(ids(3), 1, 12);
			picked.sort();
			assert_eq!(picked, ids(3));
			assert!(pick(vec![], 1, 12).is_empty());
		}

		#[test]
		fn picks_are_spread_out() {
			// every book has a fair chance: over many seeds no book wins far too often
			let mut wins = std::collections::HashMap::new();
			for seed in 0..2000 {
				for id in pick(ids(100), seed, 1) {
					*wins.entry(id).or_insert(0) += 1;
				}
			}
			assert!(wins.len() > 90);
			assert!(wins.values().all(|&n| n < 60));
		}
	}
}

pub fn should_add_sessions_join_for_filter(filter: &MediaFilterInput) -> bool {
	filter.reading_status.is_some()
		|| [&filter._and, &filter._or, &filter._not]
			.iter()
			.filter_map(|opt| opt.as_ref())
			.any(|filters| filters.iter().any(|f| f.reading_status.is_some()))
}

/// Loads the metadata of `rows` (media + their reading session) in one query.
///
/// The keep-reading query used `select_also_fake(media_metadata::Entity)` after a
/// `select_also(reading_session)`, which since the SeaORM 2 upgrade always yields `None`,
/// so continue-reading cards showed file names instead of titles.
async fn attach_metadata<T>(
	conn: &DatabaseConnection,
	rows: Vec<(media::Model, T)>,
) -> Result<Vec<Media>> {
	let ids = rows.iter().map(|(m, _)| m.id.clone()).collect::<Vec<_>>();
	let mut metadata = media_metadata::Entity::find()
		.filter(media_metadata::Column::MediaId.is_in(ids))
		.all(conn)
		.await?
		.into_iter()
		.filter_map(|m| m.media_id.clone().map(|id| (id, m)))
		.collect::<std::collections::HashMap<_, _>>();
	Ok(rows
		.into_iter()
		.map(|(media, _)| {
			let metadata = metadata.remove(&media.id);
			Media::from(media::ModelWithMetadata { media, metadata })
		})
		.collect())
}

pub fn add_sessions_join_for_filter(
	user: &AuthUser,
	filter: &MediaFilterInput,
	query: Select<media::Entity>,
) -> Select<media::Entity> {
	let should_join_sessions = should_add_sessions_join_for_filter(filter);

	if should_join_sessions {
		let user_id = user.id.clone();
		query
			.join_rev(
				JoinType::LeftJoin,
				reading_session::Entity::belongs_to(media::Entity)
					.from(reading_session::Column::MediaId)
					.to(media::Column::Id)
					.on_condition(move |_left, _right| {
						Condition::all()
							.add(reading_session::Column::UserId.eq(user_id.clone()))
					})
					.into(),
			)
			.group_by(media::Column::Id)
	} else {
		query
	}
}

#[Object]
impl MediaQuery {
	async fn media_count(&self, ctx: &Context<'_>) -> Result<i64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let count = media::Entity::find_for_user(user)
			.filter(media::Column::DeletedAt.is_null())
			.count(conn)
			.await?;

		Ok(count as i64)
	}

	// TODO: Add variant to only fetch your own sessions and remove guard
	#[graphql(
		guard = "PermissionGuard::one(UserPermission::ReadUsers).or(ServerOwnerGuard)"
	)]
	async fn finished_reading_session_count(&self, ctx: &Context<'_>) -> Result<i64> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let count = reading_session::Entity::find()
			.filter(reading_session::Column::Status.eq(ReadingStatus::Finished))
			.count(conn)
			.await?;

		Ok(count as i64)
	}

	// TODO: Add variant to only fetch your own sessions and remove guard
	#[graphql(
		guard = "PermissionGuard::one(UserPermission::ReadUsers).or(ServerOwnerGuard)"
	)]
	async fn active_reading_session_count(&self, ctx: &Context<'_>) -> Result<i64> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let newer_exists = reading_session::Entity::newer_session_exists_subquery();
		let count = reading_session::Entity::find()
			.filter(reading_session::Column::Status.eq(ReadingStatus::Reading))
			.filter(Expr::expr(Expr::exists(newer_exists)).not())
			.count(conn)
			.await?;

		Ok(count as i64)
	}

	/// The disk space of the books the user can see
	async fn media_disk_usage(&self, ctx: &Context<'_>) -> Result<i64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let total_size = media::Entity::find_for_user(user)
			.select_only()
			.expr(Expr::cust("CAST(COALESCE(SUM(media.size), 0) AS BIGINT)"))
			.into_tuple::<i64>()
			.one(conn)
			.await?
			.ok_or_else(|| async_graphql::Error::new("Failed to get disk usage"))?;

		Ok(total_size)
	}

	#[graphql(complexity = "crate::complexity::pagination_cost(&pagination, child_complexity)")]
	async fn media(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] filter: MediaFilterInput,
		#[graphql(default_with = "MediaOrderBy::default_vec()")] order_by: Vec<
			MediaOrderBy,
		>,
		#[graphql(
			desc = "Put the books whose title best answers this text first, before `orderBy`: the title is exactly the text, then titles that start with it, then titles with a word that starts with it, then the rest (books found through their author, series or tags). Case and accents don't count. Not for cursor pagination"
		)]
		relevance_to: Option<String>,
		#[graphql(default, validator(custom = "PaginationValidator"))]
		pagination: Pagination,
	) -> Result<PaginatedResponse<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let mut spec = MediaOrderBy::spec(&order_by);
		if let Some(key) = relevance_to.as_deref().and_then(|text| {
			// a title is never this long: the cap keeps a huge text out of the query
			media_relevance(&text.chars().take(MAX_RELEVANCE_CHARS).collect::<String>())
		}) {
			spec = spec.led_by(key);
		}
		let query = spec.apply(
			add_sessions_join_for_filter(
				user,
				&filter,
				media::ModelWithMetadata::find_for_user(user),
			)
			.filter(filter.into_filter())
			.filter(media::Column::DeletedAt.is_null()),
		);

		match pagination.resolve() {
			Pagination::Cursor(info) => {
				if spec.is_led() {
					return Err("relevanceTo can't be used with cursor pagination".into());
				}
				let mut query = query;
				if let Some(ref id) = info.after {
					let cursor =
						media::ModelWithMetadata::find_by_id_for_user(id.clone(), user)
							.filter(media::Column::DeletedAt.is_null())
							.into_model::<media::ModelWithMetadata>()
							.one(conn)
							.await?
							.ok_or("Cursor not found")?;
					query =
						query.filter(spec.sorts_after(&MediaOrderBy::cursor_values(
							&order_by, &cursor,
						)));
				}

				let models = query
					.limit(info.limit)
					.into_model::<media::ModelWithMetadata>()
					.all(conn)
					.await?;
				let current_cursor = info
					.after
					.or_else(|| models.first().map(|m| m.media.id.clone()));
				let next_cursor = match models.last().map(|m| m.media.id.clone()) {
					Some(id) if models.len() == info.limit as usize => Some(id),
					_ => None,
				};

				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Media::from).collect(),
					page_info: CursorPaginationInfo {
						current_cursor,
						next_cursor,
						limit: info.limit,
					}
					.into(),
				})
			},
			Pagination::Offset(info) => {
				let count = query.clone().count(conn).await?;

				let models = query
					.offset(info.offset())
					.limit(info.limit())
					.into_model::<media::ModelWithMetadata>()
					.all(conn)
					.await?;

				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Media::from).collect(),
					page_info: OffsetPaginationInfo::new(info, count).into(),
				})
			},
			Pagination::None(_) => {
				let models = query
					.into_model::<media::ModelWithMetadata>()
					.all(conn)
					.await?;
				let count = models.len().try_into()?;

				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Media::from).collect(),
					page_info: OffsetPaginationInfo::unpaged(count).into(),
				})
			},
		}
	}

	async fn media_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let model = media::ModelWithMetadata::find_by_id_for_user(id.to_string(), user)
			.filter(media::Column::DeletedAt.is_null())
			.into_model::<media::ModelWithMetadata>()
			.one(conn)
			.await?;

		Ok(model.map(Media::from))
	}

	/// Books removed from their library (soft-deleted; the files are still on disk), most
	/// recently removed first. They can be brought back with `restoreMedia`.
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageLibrary)")]
	async fn removed_media(
		&self,
		ctx: &Context<'_>,
		library_id: Option<ID>,
	) -> Result<Vec<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let visible_libraries = models::entity::library::Entity::find_for_user(user)
			.select_only()
			.column(models::entity::library::Column::Id)
			.into_query();
		let mut query = media::ModelWithMetadata::find()
			.filter(media::Column::DeletedAt.is_not_null())
			.filter(media::Column::LibraryId.in_subquery(visible_libraries));
		if let Some(library_id) = library_id {
			query = query.filter(media::Column::LibraryId.eq(library_id.to_string()));
		}
		let models = query
			.order_by_desc(media::Column::DeletedAt)
			.into_model::<media::ModelWithMetadata>()
			.all(conn)
			.await?;

		Ok(models.into_iter().map(Media::from).collect())
	}

	async fn media_by_path(
		&self,
		ctx: &Context<'_>,
		path: String,
	) -> Result<Option<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let model = media::ModelWithMetadata::find_for_user(user)
			.filter(
				media::Column::Path
					.eq(path)
					.and(media::Column::DeletedAt.is_null()),
			)
			.into_model::<media::ModelWithMetadata>()
			.one(conn)
			.await?;

		Ok(model.map(Media::from))
	}

	/// The books at these paths (as Stump stores them), in the order asked: `media` is null
	/// for a path with no book the user can see. For scripts that know their files by path,
	/// e.g. before `editMediaMetadataBatch`. At most 500 paths per call
	// One match, with whatever is selected inside it, for every path asked (the same book again
	// for a path asked twice): the cost is per path, or 500 aliases of it would ask for a book
	// 250,000 times
	#[graphql(complexity = "paths.len().saturating_mul(child_complexity)")]
	async fn media_by_paths(
		&self,
		ctx: &Context<'_>,
		paths: Vec<String>,
	) -> Result<Vec<MediaPathMatch>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		if paths.len() > 500 {
			return Err("At most 500 paths per call".into());
		}
		let mut found: HashMap<String, Media> = HashMap::new();
		for chunk in paths.chunks(400) {
			let models = media::ModelWithMetadata::find_for_user(user)
				.filter(
					media::Column::Path
						.is_in(chunk.to_vec())
						.and(media::Column::DeletedAt.is_null()),
				)
				.into_model::<media::ModelWithMetadata>()
				.all(conn)
				.await?;
			for model in models {
				found.insert(model.media.path.clone(), Media::from(model));
			}
		}
		Ok(paths
			.into_iter()
			.map(|path| MediaPathMatch {
				media: found.get(&path).cloned(),
				path,
			})
			.collect())
	}

	/// Returns the available alphabet for the media the user can see
	async fn media_alphabet(&self, ctx: &Context<'_>) -> Result<HashMap<String, i64>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		letter_counts(
			conn,
			media::Entity::find_for_user(user),
			"COALESCE(media_metadata.title, media.name)",
			"media.id",
		)
		.await
	}

	#[graphql(complexity = "crate::complexity::pagination_cost(&pagination, child_complexity)")]
	pub(crate) async fn keep_reading(
		&self,
		ctx: &Context<'_>,
		#[graphql(default, validator(custom = "PaginationValidator"))]
		pagination: Pagination,
	) -> Result<PaginatedResponse<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let user_id = user.id.clone();
		let newer_exists = reading_session::Entity::newer_session_exists_subquery();

		let spec = order_spec![
			(reading_session::Column::UpdatedAt, Order::Desc),
			(reading_session::Column::Id, Order::Desc),
		];

		let query = spec.apply(
			media::Entity::apply_for_user(user, media::Entity::find())
				.select_also(reading_session::Entity)
				.filter(media::Column::DeletedAt.is_null())
				.join_rev(
					JoinType::InnerJoin,
					reading_session::Entity::belongs_to(media::Entity)
						.from(reading_session::Column::MediaId)
						.to(media::Column::Id)
						.on_condition(move |_left, _right| {
							Condition::all()
								.add(reading_session::Column::UserId.eq(user_id.clone()))
								.add(
									reading_session::Column::Status
										.eq(ReadingStatus::Reading),
								)
								// for each session row, ensure there does not exist a newer session for the same user+media
								.add(Expr::expr(Expr::exists(newer_exists.clone())).not())
						})
						.into(),
				),
		);

		match pagination.resolve() {
			Pagination::Cursor(info) => {
				let mut query = query;
				if let Some(ref id) = info.after {
					let (_, session) = query
						.clone()
						.filter(media::Column::Id.eq(id.clone()))
						.one(conn)
						.await?
						.ok_or("Cursor not found")?;
					let session = session.ok_or("Cursor not found")?;
					query = query.filter(spec.sorts_after(&[
						session.get(reading_session::Column::UpdatedAt),
						session.get(reading_session::Column::Id),
					]));
				}

				let rows = query.limit(info.limit).all(conn).await?;
				let nodes = attach_metadata(conn, rows).await?;

				let current_cursor = info
					.after
					.or_else(|| nodes.first().map(|m| m.model.id.clone()));
				let next_cursor = match nodes.last().map(|m| m.model.id.clone()) {
					Some(id) if nodes.len() == info.limit as usize => Some(id),
					_ => None,
				};

				Ok(PaginatedResponse {
					nodes,
					page_info: CursorPaginationInfo {
						current_cursor,
						next_cursor,
						limit: info.limit,
					}
					.into(),
				})
			},
			Pagination::Offset(info) => {
				let count = query.clone().count(conn).await?;
				let rows = query
					.offset(info.offset())
					.limit(info.limit())
					.all(conn)
					.await?;
				let nodes = attach_metadata(conn, rows).await?;

				Ok(PaginatedResponse {
					nodes,
					page_info: OffsetPaginationInfo::new(info, count).into(),
				})
			},
			Pagination::None(_) => {
				let rows = query.all(conn).await?;
				let nodes = attach_metadata(conn, rows).await?;

				let count = nodes.len().try_into()?;

				Ok(PaginatedResponse {
					nodes,
					page_info: OffsetPaginationInfo::unpaged(count).into(),
				})
			},
		}
	}

	#[graphql(complexity = "crate::complexity::pagination_cost(&pagination, child_complexity)")]
	async fn on_deck(
		&self,
		ctx: &Context<'_>,
		#[graphql(default, validator(custom = "PaginationValidator"))]
		pagination: Pagination,
	) -> Result<PaginatedResponse<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let offset_info = match pagination.resolve() {
			Pagination::Offset(info) => info,
			_ => return Err("Only offset pagination is supported for onDeck".into()),
		};

		let user_id = user.id.clone();
		let limit = offset_info.limit();
		let offset = offset_info.offset();

		#[derive(Debug, FromQueryResult)]
		struct OnDeckMediaId {
			id: String,
		}

		let on_deck_media_ids = OnDeckMediaId::find_by_statement(db_statement(
			conn,
			r#"
				WITH
				-- Find all series where the user has read at least one book
				user_read_series AS (
					SELECT DISTINCT m.series_id
					FROM media m
					JOIN reading_sessions rs ON rs.media_id = m.id
					WHERE rs.user_id = $1
					AND rs.status = 'FINISHED'
					AND m.series_id IS NOT NULL
				),

				-- Find all media IDs that user has read
				user_read_media AS (
					SELECT DISTINCT media_id
					FROM reading_sessions
					WHERE user_id = $1
					AND status = 'FINISHED'
				),

				-- We do not want books from series with active reading sessions
				user_active_series AS (
					SELECT DISTINCT m.series_id
					FROM media m
					JOIN reading_sessions rs ON rs.media_id = m.id
					WHERE rs.user_id = $1
					AND m.series_id IS NOT NULL
					AND rs.status = 'READING'
					AND NOT EXISTS (
						SELECT 1
						FROM reading_sessions rs2
						WHERE rs2.user_id = rs.user_id
						AND rs2.media_id = rs.media_id
						AND (
							rs2.updated_at > rs.updated_at
							OR (
								rs2.updated_at = rs.updated_at
								AND rs2.created_at > rs.created_at
							)
							OR (
								rs2.updated_at = rs.updated_at
								AND rs2.created_at = rs.created_at
								AND rs2.id > rs.id
							)
						)
					)
				),

				-- For each series, get last read date for sorting priority
				series_last_read AS (
					SELECT
						m.series_id,
						MAX(COALESCE(rs.updated_at, rs.created_at)) as last_read_date
					FROM reading_sessions rs
					JOIN media m ON m.id = rs.media_id
					WHERE rs.user_id = $1
					AND rs.status = 'FINISHED'
					AND m.series_id IN (SELECT series_id FROM user_read_series)
					GROUP BY m.series_id
				),

				-- Find the first unread book for each series
				next_in_series AS (
					SELECT
						m.id,
						m.name,
						m.series_id,
						ROW_NUMBER() OVER(
							PARTITION BY m.series_id
							ORDER BY CASE WHEN s.kind = 'VIRTUAL' THEN mm.number END NULLS LAST, m.name, m.id
						) as book_rank,
						COALESCE(srl.last_read_date, '1970-01-01') as series_last_read_date
					FROM
						media m
                    LEFT JOIN media_metadata mm ON mm.media_id = m.id
                    LEFT JOIN series s ON s.id = m.series_id
					LEFT JOIN
						series_last_read srl ON srl.series_id = m.series_id
					WHERE
						m.series_id IN (SELECT series_id FROM user_read_series)
						AND m.series_id NOT IN (SELECT series_id FROM user_active_series)
						-- Exclude media that user has read or is currently reading
						AND m.id NOT IN (SELECT media_id FROM user_read_media)
						AND m.deleted_at IS NULL
				)

				-- Get only the first book for each series
				SELECT
					id
				FROM
					next_in_series
				WHERE
					book_rank = 1
				ORDER BY
					-- Most recently read series first
					series_last_read_date DESC
				"#,
			[user_id.into()],
		))
		.all(conn)
		.await?;

		// One book per series (a few hundred at most). The next book of a series may be one this
		// user may not read (their libraries and age limits): that series is left out, and the
		// total counts what they can get, so the pages and the total agree
		let candidate_ids: Vec<String> =
			on_deck_media_ids.into_iter().map(|row| row.id).collect();
		let mut readable: HashSet<String> = HashSet::with_capacity(candidate_ids.len());
		for chunk in candidate_ids.chunks(400) {
			readable.extend(
				media::Entity::find_for_user(user)
					.select_only()
					.column(media::Column::Id)
					.filter(media::Column::Id.is_in(chunk.to_vec()))
					.into_tuple::<String>()
					.all(conn)
					.await?,
			);
		}
		let readable_ids: Vec<String> = candidate_ids
			.into_iter()
			.filter(|id| readable.contains(id))
			.collect();
		let total_count = readable_ids.len() as u64;

		let media_ids: Vec<String> = readable_ids
			.into_iter()
			.skip(usize::try_from(offset)?)
			.take(usize::try_from(limit)?)
			.collect();

		if media_ids.is_empty() {
			return Ok(PaginatedResponse {
				nodes: vec![],
				page_info: OffsetPaginationInfo::new(offset_info, total_count).into(),
			});
		}

		let mut media_map: HashMap<String, media::ModelWithMetadata> = HashMap::new();

		let models = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::Id.is_in(media_ids.clone()))
			.into_model::<media::ModelWithMetadata>()
			.all(conn)
			.await?;

		for model in models {
			media_map.insert(model.media.id.clone(), model);
		}

		// Note: The requery likely lost original order, so manually reorder
		let ordered_results: Vec<Media> = media_ids
			.into_iter()
			.filter_map(|id| media_map.remove(&id))
			.map(Media::from)
			.collect();

		Ok(PaginatedResponse {
			nodes: ordered_results,
			page_info: OffsetPaginationInfo::new(offset_info, total_count).into(),
		})
	}

	/// The signed-in user's rules for the home's picks and Surprise me (the defaults until
	/// saved)
	async fn my_discover_rules(&self, ctx: &Context<'_>) -> Result<DiscoverRules> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		saved_discover_rules(conn, &user.id).await
	}

	/// How many books the picks can come from under `rules` (the saved rules if omitted),
	/// e.g. to preview a change before saving it
	async fn discover_pool_size(
		&self,
		ctx: &Context<'_>,
		rules: Option<DiscoverRules>,
	) -> Result<u64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let rules = match rules {
			Some(rules) => rules.normalized(),
			None => saved_discover_rules(conn, &user.id).await?,
		};
		Ok(media::Entity::find_for_user(user)
			.filter(media::Column::DeletedAt.is_null())
			.filter(random_pick::rules_condition(&user.id, &rules))
			.count(conn)
			.await?)
	}

	/// A few books picked at random among those `filter` matches (e.g. the unread ones).
	/// The same `seed` gives the same books for the same user (the home's "Picks of the
	/// week" seeds with the week); without one, every call picks anew ("Surprise me").
	/// Only books the user's picks rules allow (`myDiscoverRules`: unread only, and no
	/// `No picks` tag on the book, its series or library, by default)
	#[graphql(complexity = "count * child_complexity")]
	async fn random_media(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] filter: MediaFilterInput,
		#[graphql(default = 12, validator(minimum = 1, maximum = 50))] count: usize,
		seed: Option<String>,
	) -> Result<Vec<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let rules = saved_discover_rules(conn, &user.id).await?;

		// only the IDs of the candidates: a few hundred KB for a large library
		let ids = add_sessions_join_for_filter(
			user,
			&filter,
			media::Entity::find_for_user(user),
		)
		.filter(filter.into_filter())
		.filter(media::Column::DeletedAt.is_null())
		.filter(random_pick::rules_condition(&user.id, &rules))
		.select_only()
		.column(media::Column::Id)
		.into_tuple::<String>()
		.all(conn)
		.await?;

		let seed = match seed {
			Some(seed) => {
				random_pick::hash_str(random_pick::hash_str(0, &user.id), &seed)
			},
			None => std::hash::BuildHasher::hash_one(
				&std::collections::hash_map::RandomState::new(),
				&user.id,
			),
		};
		let picked = random_pick::pick(ids, seed, count);
		if picked.is_empty() {
			return Ok(vec![]);
		}

		let mut models = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::Id.is_in(picked.clone()))
			.into_model::<media::ModelWithMetadata>()
			.all(conn)
			.await?;
		let order: HashMap<&str, usize> = picked
			.iter()
			.enumerate()
			.map(|(index, id)| (id.as_str(), index))
			.collect();
		models.sort_by_key(|m| order.get(m.media.id.as_str()).copied());
		Ok(models.into_iter().map(Media::from).collect())
	}

	#[graphql(complexity = "crate::complexity::pagination_cost(&pagination, child_complexity)")]
	async fn recently_added_media(
		&self,
		ctx: &Context<'_>,
		#[graphql(default, validator(custom = "PaginationValidator"))]
		pagination: Pagination,
	) -> Result<PaginatedResponse<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let query = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::DeletedAt.is_null());

		match pagination.resolve() {
			Pagination::Cursor(info) => {
				let mut cursor = query.cursor_by(media::Column::CreatedAt);
				if let Some(ref id) = info.after {
					let media = media::Entity::find_for_user(user)
						.select_only()
						.column(media::Column::CreatedAt)
						.filter(
							media::Column::Id
								.eq(id.clone())
								.and(media::Column::DeletedAt.is_null()),
						)
						.into_model::<media::MediaCreatedAtCmpSelect>()
						.one(conn)
						.await?
						.ok_or("Cursor not found")?;
					cursor.after(media.created_at);
				}
				cursor.first(info.limit).desc();

				let models = cursor
					.into_model::<media::ModelWithMetadata>()
					.all(conn)
					.await?;
				let current_cursor = info
					.after
					.or_else(|| models.first().map(|m| m.media.id.clone()));
				let next_cursor = match models.last().map(|m| m.media.id.clone()) {
					Some(id) if models.len() == info.limit as usize => Some(id),
					_ => None,
				};

				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Media::from).collect(),
					page_info: CursorPaginationInfo {
						current_cursor,
						next_cursor,
						limit: info.limit,
					}
					.into(),
				})
			},
			Pagination::Offset(info) => {
				let count = query.clone().count(conn).await?;

				let models = query
					.order_by_desc(media::Column::CreatedAt)
					.offset(info.offset())
					.limit(info.limit())
					.into_model::<media::ModelWithMetadata>()
					.all(conn)
					.await?;

				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Media::from).collect(),
					page_info: OffsetPaginationInfo::new(info, count).into(),
				})
			},
			Pagination::None(_) => {
				let models = query
					.order_by_desc(media::Column::CreatedAt)
					.into_model::<media::ModelWithMetadata>()
					.all(conn)
					.await?;
				let count = models.len().try_into()?;

				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Media::from).collect(),
					page_info: OffsetPaginationInfo::unpaged(count).into(),
				})
			},
		}
	}

	/// The books that share their file with another book, at most 200 (the first by hash)
	#[graphql(complexity = "crate::complexity::list_cost(None, child_complexity)")]
	async fn duplicate_media(&self, ctx: &Context<'_>) -> Result<Vec<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let models = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::DeletedAt.is_null())
			.filter(
				media::Column::Hash.in_subquery(
					Query::select()
						.column(media::Column::Hash)
						.from(media::Entity)
						.add_group_by([Expr::col(media::Column::Hash)])
						.and_having(Expr::col(media::Column::Id).count().gt(1))
						.to_owned(),
				),
			)
			.order_by_asc(media::Column::Hash)
			.order_by_asc(media::Column::Id)
			.limit(list_limit(None))
			.into_model::<media::ModelWithMetadata>()
			.all(conn)
			.await?;

		Ok(models.into_iter().map(Media::from).collect())
	}

	async fn reading_session_conflict_view(
		&self,
		ctx: &Context<'_>,
		media_id: ID,
		branched_session_id: Option<i32>,
	) -> Result<ReadingSessionConflictResolutionView> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let media_id_str = media_id.as_str();

		let ancestor_session = match branched_session_id {
			Some(session_id) => {
				reading_session::Entity::find_by_id(session_id)
					.filter(reading_session::Column::UserId.eq(&user.id))
					.filter(reading_session::Column::MediaId.eq(media_id_str))
					.one(conn)
					.await?
			},
			None => None,
		};

		let remote_sessions =
			reading_session::Entity::find_for_user_and_media(user, media_id_str)
				// find_for_user_and_media, you might guess, filters by user and media already. so we only need to filter by ancestor session
				// when it exists, otherwise its just all sessions for the user and media
				.apply_if(ancestor_session.clone(), |query, ancestor| {
					query.filter(
						Condition::any()
							// any new sessions created after the ancestor
							.add(
								reading_session::Column::CreatedAt
									.gt(ancestor.created_at),
							)
							// the ancestor itself was updated after its creation, should be included but it is a bit awkward
							.add(
								Condition::all()
									.add(reading_session::Column::Id.eq(ancestor.id))
									.add(
										reading_session::Column::UpdatedAt
											.gt(ancestor.created_at),
									),
							),
					)
				})
				.order_by_asc(reading_session::Column::CreatedAt)
				.order_by_asc(reading_session::Column::Id)
				.all(conn)
				.await?;

		Ok(ReadingSessionConflictResolutionView {
			ancestor_session: ancestor_session.map(ReadingSession::from),
			remote_sessions: remote_sessions
				.into_iter()
				.map(ReadingSession::from)
				.collect(),
		})
	}
}

/// A path asked for by `mediaByPaths`, and the book there
#[derive(Debug, Clone, async_graphql::SimpleObject)]
pub struct MediaPathMatch {
	pub path: String,
	pub media: Option<Media>,
}
