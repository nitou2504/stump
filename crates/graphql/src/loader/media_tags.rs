use async_graphql::dataloader::Loader;
use models::entity::{media_tag, tag};
use sea_orm::{
	prelude::*, ColumnTrait, DatabaseConnection, JoinType, QueryFilter, QueryOrder,
	QuerySelect,
};
use std::{collections::HashMap, sync::Arc};

/// Loads the tags of many media items in one query, so listing a large smart list's books
/// with their tags doesn't issue one query per book.
pub struct MediaTagsLoader {
	conn: Arc<DatabaseConnection>,
}

impl MediaTagsLoader {
	pub fn new(conn: Arc<DatabaseConnection>) -> Self {
		Self { conn }
	}
}

/// The media ID
pub type MediaTagsLoaderKey = String;

impl Loader<MediaTagsLoaderKey> for MediaTagsLoader {
	type Value = Vec<tag::Model>;
	type Error = Arc<sea_orm::error::DbErr>;

	async fn load(
		&self,
		keys: &[MediaTagsLoaderKey],
	) -> Result<HashMap<MediaTagsLoaderKey, Self::Value>, Self::Error> {
		let rows = tag::Entity::find()
			.join(JoinType::InnerJoin, tag::Relation::MediaTags.def())
			.filter(media_tag::Column::MediaId.is_in(keys.to_vec()))
			.column_as(media_tag::Column::MediaId, "media_id")
			.order_by_asc(tag::Column::Name)
			.into_tuple::<(i32, String, String)>()
			.all(self.conn.as_ref())
			.await?;

		let mut result: HashMap<MediaTagsLoaderKey, Self::Value> = HashMap::new();
		for (id, name, media_id) in rows {
			result
				.entry(media_id)
				.or_default()
				.push(tag::Model { id, name });
		}

		Ok(result)
	}
}
