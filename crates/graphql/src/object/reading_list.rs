use async_graphql::{
	dataloader::DataLoader, ComplexObject, Context, Result, SimpleObject,
};
use models::entity::{media, reading_list, reading_list_item};
use sea_orm::{prelude::*, JoinType, QueryOrder, QuerySelect, Select};

use crate::{
	data::{AuthContext, CoreContext},
	loader::reader_profile::ReaderProfileLoader,
	object::{media::Media, reader_profile::ReaderProfile},
	pagination::{list_limit, list_offset},
};

/// What a user gets for a list that doesn't exist and for one they may not see: the same, so that
/// the answer doesn't tell which private lists there are
pub const NOT_FOUND: &str = "Reading list not found";

/// What a user gets for changing a list they may see but not change
pub const CANT_CHANGE: &str = "You can't change this list";

#[derive(Debug, SimpleObject)]
#[graphql(complex)]
pub struct ReadingList {
	#[graphql(flatten)]
	pub model: reading_list::Model,
}

impl From<reading_list::Model> for ReadingList {
	fn from(entity: reading_list::Model) -> Self {
		Self { model: entity }
	}
}

/// Narrows a selection of the books a user can see (`find_for_user`: hidden libraries, age
/// restrictions and removed books are left out) to the books of a reading list
fn in_reading_list(
	select: Select<media::Entity>,
	reading_list_id: &str,
) -> Select<media::Entity> {
	select
		.join_rev(
			JoinType::InnerJoin,
			reading_list_item::Entity::belongs_to(media::Entity)
				.from(reading_list_item::Column::MediaId)
				.to(media::Column::Id)
				.into(),
		)
		.filter(reading_list_item::Column::ReadingListId.eq(reading_list_id))
}

#[ComplexObject]
impl ReadingList {
	/// The list's books the user can see, in the list's order: at most 200 at a time, `offset`
	/// books in
	#[graphql(complexity = "crate::complexity::list_cost(limit, child_complexity)")]
	async fn books(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] limit: Option<u64>,
		#[graphql(default)] offset: Option<u64>,
	) -> Result<Vec<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let models = in_reading_list(
			media::ModelWithMetadata::find_for_user(user),
			&self.model.id,
		)
		.order_by_asc(reading_list_item::Column::DisplayOrder)
		.order_by_asc(reading_list_item::Column::Id)
		.limit(list_limit(limit))
		.offset(list_offset(offset))
		.into_model::<media::ModelWithMetadata>()
		.all(conn)
		.await?;

		Ok(models.into_iter().map(Media::from).collect())
	}

	/// Whether the user made the list, and so may rename it, change what is in it and who sees
	/// it. Anyone else sees it read-only (the server owner can only take a public list of
	/// someone else back to private, or delete it)
	async fn can_edit(&self, ctx: &Context<'_>) -> Result<bool> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		Ok(self.model.creating_user_id == user.id)
	}

	/// Who made the list: their name and avatar. None when they can't be shown any more (a
	/// deleted or locked account's lists aren't listed at all)
	async fn creator(&self, ctx: &Context<'_>) -> Result<Option<ReaderProfile>> {
		let loader = ctx.data::<DataLoader<ReaderProfileLoader>>()?;
		Ok(loader.load_one(self.model.creating_user_id.clone()).await?)
	}

	/// How many of the list's books the user can see
	async fn book_count(&self, ctx: &Context<'_>) -> Result<u64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		Ok(
			in_reading_list(media::Entity::find_for_user(user), &self.model.id)
				.count(conn)
				.await?,
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::common::get_default_user;
	use sea_orm::{DbBackend, QueryTrait};

	#[test]
	fn test_in_reading_list_query() {
		let user = get_default_user();
		let sql = in_reading_list(media::Entity::find_for_user(&user), "list-1")
			.build(DbBackend::Sqlite)
			.to_string();

		assert!(sql.contains(
			r#"INNER JOIN "reading_list_items" ON "reading_list_items"."media_id" = "media"."id""#
		));
		assert!(sql.contains(r#""reading_list_items"."reading_list_id" = 'list-1'"#));
		// the usual rules for what a user can see still apply
		assert!(sql.contains(r#""media"."deleted_at" IS NULL"#));
		assert!(sql.contains(r#""library_exclusions""#));
	}
}
