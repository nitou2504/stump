use std::collections::HashMap;

use async_graphql::{Context, Object, Result, ID};
use models::entity::series;
use sea_orm::{prelude::*, QueryOrder, QuerySelect};

use crate::{
	data::{AuthContext, CoreContext},
	filter::series::SeriesFilterInput,
	object::series::Series,
	order::SeriesOrderBy,
	pagination::{
		CursorPaginationInfo, OffsetPaginationInfo, PaginatedResponse, Pagination,
		PaginationValidator,
	},
	utils::letter_counts,
};

#[derive(Default)]
pub struct SeriesQuery;

#[Object]
impl SeriesQuery {
	#[graphql(complexity = "crate::complexity::pagination_cost(&pagination, child_complexity)")]
	async fn series(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] filter: SeriesFilterInput,
		#[graphql(default_with = "SeriesOrderBy::default_vec()")] order_by: Vec<
			SeriesOrderBy,
		>,
		#[graphql(default, validator(custom = "PaginationValidator"))]
		pagination: Pagination,
	) -> Result<PaginatedResponse<Series>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let conditions = filter.into_filter_with_user(&user.id);
		let spec = SeriesOrderBy::spec(&order_by);
		let query =
			spec.apply(series::ModelWithMetadata::find_for_user(user).filter(conditions));

		match pagination.resolve() {
			Pagination::Cursor(info) => {
				let mut query = query;
				if let Some(ref id) = info.after {
					let cursor =
						series::ModelWithMetadata::find_by_id_for_user(id.clone(), user)
							.into_model::<series::ModelWithMetadata>()
							.one(conn)
							.await?
							.ok_or("Cursor not found")?;
					query =
						query.filter(spec.sorts_after(&SeriesOrderBy::cursor_values(
							&order_by, &cursor,
						)));
				}

				let models = query
					.limit(info.limit)
					.into_model::<series::ModelWithMetadata>()
					.all(conn)
					.await?;
				let current_cursor = info
					.after
					.or_else(|| models.first().map(|result| result.series.id.clone()));
				let next_cursor =
					match models.last().map(|result| result.series.id.clone()) {
						Some(id) if models.len() == info.limit as usize => Some(id),
						_ => None,
					};

				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Series::from).collect(),
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
					.into_model::<series::ModelWithMetadata>()
					.all(conn)
					.await?;

				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Series::from).collect(),
					page_info: OffsetPaginationInfo::new(info, count).into(),
				})
			},
			Pagination::None(_) => {
				let models = query
					.into_model::<series::ModelWithMetadata>()
					.all(conn)
					.await?;
				let count = models.len().try_into()?;
				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Series::from).collect(),
					page_info: OffsetPaginationInfo::unpaged(count).into(),
				})
			},
		}
	}

	async fn series_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Series>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let model = series::ModelWithMetadata::find_for_user(user)
			.filter(series::Column::Id.eq(id.to_string()))
			.into_model::<series::ModelWithMetadata>()
			.one(conn)
			.await?;

		Ok(model.map(Series::from))
	}

	/// Returns the available alphabet for the series the user can see
	async fn series_alphabet(&self, ctx: &Context<'_>) -> Result<HashMap<String, i64>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		letter_counts(
			conn,
			series::Entity::with_metadata_for_user(
				series::Entity::find_for_user(user),
				user,
			),
			"COALESCE(series_metadata.title, series.name)",
			"series.id",
		)
		.await
	}

	async fn number_of_series(&self, ctx: &Context<'_>) -> Result<u64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let count = series::Entity::find_for_user(user).count(conn).await?;
		Ok(count)
	}

	#[graphql(complexity = "crate::complexity::pagination_cost(&pagination, child_complexity)")]
	async fn recently_added_series(
		&self,
		ctx: &Context<'_>,
		#[graphql(default, validator(custom = "PaginationValidator"))]
		pagination: Pagination,
	) -> Result<PaginatedResponse<Series>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let query = series::ModelWithMetadata::find_for_user(user);

		match pagination.resolve() {
			Pagination::Cursor(info) => {
				let mut cursor = query.cursor_by(series::Column::CreatedAt);
				if let Some(ref id) = info.after {
					let series = series::Entity::find_for_user(user)
						.select_only()
						.column(series::Column::CreatedAt)
						.filter(series::Column::Id.eq(id.clone()))
						.into_model::<series::SeriesCreatedAtCmpSelect>()
						.one(conn)
						.await?
						.ok_or("Cursor not found")?;
					cursor.after(series.created_at);
				}
				cursor.first(info.limit).desc();

				let models = cursor
					.into_model::<series::ModelWithMetadata>()
					.all(conn)
					.await?;
				let current_cursor = info
					.after
					.or_else(|| models.first().map(|m| m.series.id.clone()));
				let next_cursor =
					match models.last().map(|result| result.series.id.clone()) {
						Some(id) if models.len() == info.limit as usize => Some(id),
						_ => None,
					};

				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Series::from).collect(),
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
					.order_by_desc(series::Column::CreatedAt)
					.offset(info.offset())
					.limit(info.limit())
					.into_model::<series::ModelWithMetadata>()
					.all(conn)
					.await?;

				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Series::from).collect(),
					page_info: OffsetPaginationInfo::new(info, count).into(),
				})
			},
			Pagination::None(_) => {
				let models = query
					.order_by_desc(series::Column::CreatedAt)
					.into_model::<series::ModelWithMetadata>()
					.all(conn)
					.await?;
				let count = models.len().try_into()?;

				Ok(PaginatedResponse {
					nodes: models.into_iter().map(Series::from).collect(),
					page_info: OffsetPaginationInfo::unpaged(count).into(),
				})
			},
		}
	}
}
