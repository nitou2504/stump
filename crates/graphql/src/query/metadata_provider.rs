use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
	input::metadata_provider::MetadataFetchRecordId,
	object::metadata_fetch_record::MetadataFetchRecord,
};
use async_graphql::{Context, Object, Result};
use models::{
	entity::{
		media, metadata_fetch_record, metadata_provider_config, series, user::AuthUser,
	},
	shared::enums::{MetadataFetchStatus, UserPermission},
};
use sea_orm::{prelude::*, Condition, QuerySelect, QueryTrait};

/// The ids of the books the user may see. A fetch record holds what an outside search found
/// for a book's title and author, so it is for those who can see the book
fn visible_media_ids(user: &AuthUser) -> sea_orm::sea_query::SelectStatement {
	media::Entity::find_for_user(user)
		.select_only()
		.column(media::Column::Id)
		.into_query()
}

/// The ids of the series the user may see (see [`visible_media_ids`])
fn visible_series_ids(user: &AuthUser) -> sea_orm::sea_query::SelectStatement {
	series::Entity::find_for_user(user)
		.select_only()
		.column(series::Column::Id)
		.into_query()
}

#[derive(Default)]
pub struct MetadataProviderQuery;

#[Object]
impl MetadataProviderQuery {
	#[graphql(guard = "PermissionGuard::one(UserPermission::MetadataProviderRead)")]
	async fn metadata_provider_configs(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<metadata_provider_config::Model>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let configs = metadata_provider_config::Entity::find().all(conn).await?;
		Ok(configs)
	}

	#[graphql(guard = "PermissionGuard::one(UserPermission::MetadataProviderRead)")]
	async fn metadata_provider_config_by_id(
		&self,
		ctx: &Context<'_>,
		id: i32,
	) -> Result<Option<metadata_provider_config::Model>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let config = metadata_provider_config::Entity::find_by_id(id)
			.one(conn)
			.await?;
		Ok(config)
	}

	#[graphql(guard = "PermissionGuard::one(UserPermission::MetadataFetchRecordRead)")]
	async fn metadata_fetch_record(
		&self,
		ctx: &Context<'_>,
		id: MetadataFetchRecordId,
	) -> Result<Option<MetadataFetchRecord>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let (col, value, visible) = match id {
			MetadataFetchRecordId::Media(media_id) => (
				metadata_fetch_record::Column::MediaId,
				media_id,
				visible_media_ids(user),
			),
			MetadataFetchRecordId::Series(series_id) => (
				metadata_fetch_record::Column::SeriesId,
				series_id,
				visible_series_ids(user),
			),
		};

		let record = metadata_fetch_record::Entity::find()
			.filter(col.eq(value))
			.filter(col.in_subquery(visible))
			.one(conn)
			.await?;

		Ok(record.map(MetadataFetchRecord::from))
	}

	/// Return all metadata fetch records that are awaiting user review.
	#[graphql(guard = "PermissionGuard::one(UserPermission::MetadataFetchRecordRead)")]
	async fn pending_metadata_matches(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<MetadataFetchRecord>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let records = metadata_fetch_record::Entity::find()
			.filter(
				metadata_fetch_record::Column::Status
					.eq(MetadataFetchStatus::AwaitingReview),
			)
			.filter(
				Condition::any()
					.add(
						metadata_fetch_record::Column::MediaId
							.in_subquery(visible_media_ids(user)),
					)
					.add(
						metadata_fetch_record::Column::SeriesId
							.in_subquery(visible_series_ids(user)),
					),
			)
			.all(conn)
			.await?;

		Ok(records.into_iter().map(MetadataFetchRecord::from).collect())
	}
}
