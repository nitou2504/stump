use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use models::entity::{media, reading_list, reading_list_item};
use sea_orm::{prelude::*, JoinType, QueryOrder, QuerySelect, Select};

use crate::{
	data::{AuthContext, CoreContext},
	object::media::Media,
};

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
	/// The list's books the user can see, in the list's order
	async fn books(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] limit: Option<u64>,
	) -> Result<Vec<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let models = in_reading_list(
			media::ModelWithMetadata::find_for_user(user),
			&self.model.id,
		)
		.order_by_asc(reading_list_item::Column::DisplayOrder)
		.order_by_asc(reading_list_item::Column::Id)
		.limit(limit)
		.into_model::<media::ModelWithMetadata>()
		.all(conn)
		.await?;

		Ok(models.into_iter().map(Media::from).collect())
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
