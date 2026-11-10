use crate::{
	data::{AuthContext, CoreContext},
	object::tag::Tag,
};
use async_graphql::{Context, Object, Result};
use models::entity::{
	library, library_tag, media, media_tag, series, series_tag, tag, user::AuthUser,
};
use sea_orm::{
	prelude::*,
	sea_query::{Query, SelectStatement},
	Condition, QuerySelect, QueryTrait,
};

#[derive(Default)]
pub struct TagQuery;

/// The ids of the tags on books, optionally only on the books `visible` selects
fn media_tag_ids(visible: Option<SelectStatement>) -> SelectStatement {
	let mut select = Query::select()
		.column(media_tag::Column::TagId)
		.from(media_tag::Entity)
		.to_owned();
	if let Some(visible) = visible {
		select.and_where(media_tag::Column::MediaId.in_subquery(visible));
	}
	select
}

/// The ids of the tags on series, optionally only on the series `visible` selects
fn series_tag_ids(visible: Option<SelectStatement>) -> SelectStatement {
	let mut select = Query::select()
		.column(series_tag::Column::TagId)
		.from(series_tag::Entity)
		.to_owned();
	if let Some(visible) = visible {
		select.and_where(series_tag::Column::SeriesId.in_subquery(visible));
	}
	select
}

/// The ids of the tags on libraries, optionally only on the libraries `visible` selects
fn library_tag_ids(visible: Option<SelectStatement>) -> SelectStatement {
	let mut select = Query::select()
		.column(library_tag::Column::TagId)
		.from(library_tag::Entity)
		.to_owned();
	if let Some(visible) = visible {
		select.and_where(library_tag::Column::LibraryId.in_subquery(visible));
	}
	select
}

/// The tags a user is shown: the server owner sees them all. Everyone else sees those on at least
/// one book, series or library they can see, and those on nothing yet (so they can pick one for
/// something they edit). A tag only on what is hidden from them would give that away by its name.
async fn get_tags(conn: &DatabaseConnection, user: &AuthUser) -> Result<Vec<Tag>> {
	let mut query = tag::Entity::find();

	if !user.is_server_owner {
		let visible_media = media::Entity::find_for_user(user)
			.select_only()
			.column(media::Column::Id)
			.into_query();
		let visible_series = series::Entity::find_for_user(user)
			.select_only()
			.column(series::Column::Id)
			.into_query();
		let visible_libraries = library::Entity::find_for_user(user)
			.select_only()
			.column(library::Column::Id)
			.into_query();

		query = query.filter(
			Condition::any()
				.add(tag::Column::Id.in_subquery(media_tag_ids(Some(visible_media))))
				.add(tag::Column::Id.in_subquery(series_tag_ids(Some(visible_series))))
				.add(
					tag::Column::Id.in_subquery(library_tag_ids(Some(visible_libraries))),
				)
				.add(
					Condition::all()
						.add(tag::Column::Id.not_in_subquery(media_tag_ids(None)))
						.add(tag::Column::Id.not_in_subquery(series_tag_ids(None)))
						.add(tag::Column::Id.not_in_subquery(library_tag_ids(None))),
				),
		);
	}

	let models = query.into_model::<tag::Model>().all(conn).await?;

	Ok(models.into_iter().map(Tag::from).collect())
}

#[Object]
impl TagQuery {
	/// Returns the tags the user can see (see [`get_tags`]).
	async fn tags(&self, ctx: &Context<'_>) -> Result<Vec<Tag>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		get_tags(conn, user).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::common::get_default_user;
	use sea_orm::MockDatabase;

	#[tokio::test]
	async fn test_find_tags() {
		let mock_db = MockDatabase::new(sea_orm::DatabaseBackend::Sqlite)
			.append_query_results(vec![vec![tag::Model {
				id: 123,
				name: "hello".to_string(),
			}]])
			.into_connection();

		let tags = get_tags(&mock_db, &get_default_user()).await.unwrap();

		assert_eq!(tags.len(), 1);
		assert_eq!(tags[0].model.id, 123);
		assert_eq!(tags[0].model.name, "hello");
	}

	#[tokio::test]
	async fn test_find_tags_no_tags() {
		let mock_db = MockDatabase::new(sea_orm::DatabaseBackend::Sqlite)
			.append_query_results::<tag::Model, Vec<_>, Vec<Vec<_>>>(vec![vec![]])
			.into_connection();

		let tags = get_tags(&mock_db, &get_default_user()).await.unwrap();

		assert_eq!(tags.len(), 0);
	}
}
