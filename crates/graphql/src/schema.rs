use crate::{
	complexity::MAX_QUERY_COMPLEXITY,
	data::CoreContext,
	loader::{
		author::{AuthorMediaLoader, MetadataSeriesMediaLoader},
		favorite::FavoritesLoader,
		library::LibraryLoader,
		library_config::LibraryConfigLoader,
		log::JobAssociatedLogLoader,
		media::MediaLoader,
		media_analysis::MediaAnalysisLoader,
		media_tags::MediaTagsLoader,
		reader_profile::ReaderProfileLoader,
		reading_session::ReadingSessionLoader,
		series::SeriesLoader,
		series_count::SeriesCountLoader,
		series_finished_count::SeriesFinishedCountLoader,
		series_thumbnail_media::SeriesThumbnailMediaLoader,
	},
	mutation::Mutation,
	query::Query,
	subscription::Subscription,
};
use async_graphql::{dataloader::DataLoader, ObjectType, Schema, SchemaBuilder};
use models::shared::enums::AccessRole;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub type AppSchema = Schema<Query, Mutation, Subscription>;

/// The limits on the queries a client can send. Without a complexity limit a single query could
/// ask for the books of the series of the books of a series, and build over half a million
/// of them: see [crate::complexity] for what a query costs.
fn limit_queries<
	QueryType: ObjectType + 'static,
	MutationType: ObjectType + 'static,
	SubscriptionType: 'static,
>(
	schema: SchemaBuilder<QueryType, MutationType, SubscriptionType>,
) -> SchemaBuilder<QueryType, MutationType, SubscriptionType> {
	schema
		.limit_depth(15)
		.limit_complexity(MAX_QUERY_COMPLEXITY)
}

pub async fn build_schema(ctx: CoreContext) -> AppSchema {
	let conn = ctx.conn.clone();
	let schema_builder = limit_queries(
		Schema::build(
			Query::default(),
			Mutation::default(),
			Subscription::default(),
		)
		// AccessRole is used in a serialized json for SmartList so we need to register it manually
		.register_output_type::<AccessRole>(),
	)
	.data(ctx);

	add_data_loaders(schema_builder, conn).finish()
}

pub fn add_data_loaders<
	QueryType: ObjectType + 'static,
	MutationType: ObjectType + 'static,
	SubscriptionType: 'static,
>(
	schema: SchemaBuilder<QueryType, MutationType, SubscriptionType>,
	conn: Arc<DatabaseConnection>,
) -> SchemaBuilder<QueryType, MutationType, SubscriptionType> {
	schema
		.data(DataLoader::new(
			AuthorMediaLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			MetadataSeriesMediaLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			JobAssociatedLogLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			ReadingSessionLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			MediaLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			LibraryLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			LibraryConfigLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			SeriesLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			SeriesCountLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			MediaTagsLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			SeriesFinishedCountLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			SeriesThumbnailMediaLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			FavoritesLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			MediaAnalysisLoader::new(conn.clone()),
			tokio::spawn,
		))
		.data(DataLoader::new(
			ReaderProfileLoader::new(conn.clone()),
			tokio::spawn,
		))
}

pub fn build_schema_bare() -> AppSchema {
	limit_queries(
		Schema::build(
			Query::default(),
			Mutation::default(),
			Subscription::default(),
		)
		// AccessRole is used in a serialized json for SmartList so we need to register it manually
		.register_output_type::<AccessRole>(),
	)
	.finish()
}
