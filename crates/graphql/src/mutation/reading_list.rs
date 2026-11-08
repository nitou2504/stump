use std::collections::HashSet;

use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use models::entity::{
	media, reading_list, reading_list_item, reading_list_rule, user::AuthUser,
};
use sea_orm::{prelude::*, ActiveValue::Set, QuerySelect, TransactionTrait};

use crate::{
	data::{AuthContext, CoreContext},
	input::reading_list::{ReadingListInput, UpdateReadingListInput},
	object::reading_list::ReadingList,
};

#[derive(Default)]
pub struct ReadingListMutation;

#[Object]
impl ReadingListMutation {
	/// Creates a reading list owned by the current user, with the given books (in order; books
	/// the user can't see are left out). The ID is generated when omitted.
	///
	/// # Returns
	///
	/// A result containing the newly created reading list, or an error if creation failed.
	async fn create_reading_list(
		&self,
		ctx: &Context<'_>,
		input: ReadingListInput,
	) -> Result<ReadingList> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let txn = conn.begin().await?;
		let media_ids = visible_media_ids(user, &input.media_ids, &txn).await?;
		let reading_list = input.into_active_model(&user.id)?.insert(&txn).await?;
		append_reading_list_items(&reading_list.id, media_ids, &txn).await?;
		txn.commit().await?;

		Ok(ReadingList::from(reading_list))
	}

	/// Renames one of the current user's reading lists, or changes its description or
	/// visibility.
	///
	/// # Returns
	///
	/// A result containing the updated reading list, or an error if update failed.
	async fn update_reading_list(
		&self,
		ctx: &Context<'_>,
		id: ID,
		input: UpdateReadingListInput,
	) -> Result<ReadingList> {
		let user_id = ctx.data::<AuthContext>()?.id();
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let txn = conn.begin().await?;
		let reading_list = get_for_owner(&id, &txn, user_id).await?;
		let updated = input.apply(reading_list)?.update(&txn).await?;
		txn.commit().await?;

		Ok(ReadingList::from(updated))
	}

	/// Adds books to the end of one of the current user's reading lists. Books already in it
	/// and books the user can't see are left out.
	async fn add_books_to_reading_list(
		&self,
		ctx: &Context<'_>,
		id: ID,
		media_ids: Vec<ID>,
	) -> Result<ReadingList> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let media_ids = media_ids
			.into_iter()
			.map(|id| id.to_string())
			.collect::<Vec<_>>();

		let txn = conn.begin().await?;
		let reading_list = get_for_owner(&id, &txn, user.id.clone()).await?;
		let media_ids = visible_media_ids(user, &media_ids, &txn).await?;
		let added = append_reading_list_items(&reading_list.id, media_ids, &txn).await?;
		let reading_list = if added > 0 {
			touch(reading_list, &txn).await?
		} else {
			reading_list
		};
		txn.commit().await?;

		Ok(ReadingList::from(reading_list))
	}

	/// Removes books from one of the current user's reading lists
	async fn remove_books_from_reading_list(
		&self,
		ctx: &Context<'_>,
		id: ID,
		media_ids: Vec<ID>,
	) -> Result<ReadingList> {
		let user_id = ctx.data::<AuthContext>()?.id();
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let media_ids = media_ids
			.into_iter()
			.map(|id| id.to_string())
			.collect::<Vec<_>>();

		let txn = conn.begin().await?;
		let reading_list = get_for_owner(&id, &txn, user_id).await?;
		let removed = reading_list_item::Entity::delete_many()
			.filter(reading_list_item::Column::ReadingListId.eq(reading_list.id.clone()))
			.filter(reading_list_item::Column::MediaId.is_in(media_ids))
			.exec(&txn)
			.await?
			.rows_affected;
		let reading_list = if removed > 0 {
			touch(reading_list, &txn).await?
		} else {
			reading_list
		};
		txn.commit().await?;

		Ok(ReadingList::from(reading_list))
	}

	/// Deletes one of the current user's reading lists (not its books).
	///
	/// # Returns
	///
	/// A result containing the deleted reading list, or an error if deletion failed.
	async fn delete_reading_list(
		&self,
		ctx: &Context<'_>,
		id: String,
	) -> Result<ReadingList> {
		let user_id = ctx.data::<AuthContext>()?.id();
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let txn = conn.begin().await?;
		let reading_list = get_for_owner(&id, &txn, user_id).await?;
		reading_list_item::Entity::delete_many()
			.filter(reading_list_item::Column::ReadingListId.eq(reading_list.id.clone()))
			.exec(&txn)
			.await?;
		reading_list_rule::Entity::delete_many()
			.filter(reading_list_rule::Column::ReadingListId.eq(reading_list.id.clone()))
			.exec(&txn)
			.await?;
		reading_list.clone().delete(&txn).await?;
		txn.commit().await?;

		Ok(ReadingList::from(reading_list))
	}
}

/// The given books the user can see (the same rules as every book list), in the given order
/// and each once
async fn visible_media_ids<C: ConnectionTrait>(
	user: &AuthUser,
	media_ids: &[String],
	conn: &C,
) -> Result<Vec<String>, DbErr> {
	if media_ids.is_empty() {
		return Ok(vec![]);
	}

	let visible = media::Entity::find_for_user(user)
		.select_only()
		.column(media::Column::Id)
		.filter(media::Column::Id.is_in(media_ids.iter().cloned()))
		.into_tuple::<String>()
		.all(conn)
		.await?
		.into_iter()
		.collect::<HashSet<_>>();

	let mut seen = HashSet::new();
	Ok(media_ids
		.iter()
		.filter(|id| visible.contains(*id) && seen.insert(*id))
		.cloned()
		.collect())
}

/// Adds the books to the end of the list, in order, leaving out the ones already in it.
/// Returns how many were added
async fn append_reading_list_items<C: ConnectionTrait>(
	reading_list_id: &str,
	media_ids: Vec<String>,
	conn: &C,
) -> Result<usize, DbErr> {
	let existing = reading_list_item::Entity::find()
		.select_only()
		.column(reading_list_item::Column::MediaId)
		.column(reading_list_item::Column::DisplayOrder)
		.filter(reading_list_item::Column::ReadingListId.eq(reading_list_id))
		.into_tuple::<(String, i32)>()
		.all(conn)
		.await?;

	let mut next_order = existing
		.iter()
		.map(|(_, order)| order + 1)
		.max()
		.unwrap_or(0);
	let mut in_list = existing
		.into_iter()
		.map(|(media_id, _)| media_id)
		.collect::<HashSet<_>>();

	let items = media_ids
		.into_iter()
		.filter(|media_id| in_list.insert(media_id.clone()))
		.map(|media_id| {
			let display_order = next_order;
			next_order += 1;
			reading_list_item::ActiveModel {
				display_order: Set(display_order),
				media_id: Set(media_id),
				reading_list_id: Set(reading_list_id.to_string()),
				..Default::default()
			}
		})
		.collect::<Vec<_>>();
	let added = items.len();

	reading_list_item::Entity::insert_many(items)
		.exec_without_returning(conn)
		.await?;

	Ok(added)
}

/// Marks the list as changed now
async fn touch<C: ConnectionTrait>(
	reading_list: reading_list::Model,
	conn: &C,
) -> Result<reading_list::Model, DbErr> {
	let mut active_model: reading_list::ActiveModel = reading_list.into();
	active_model.updated_at = Set(Utc::now().into());
	active_model.update(conn).await
}

async fn get_for_owner<C: ConnectionTrait>(
	id: &str,
	conn: &C,
	user_id: String,
) -> Result<reading_list::Model> {
	// Check if reading list exists
	let reading_list = reading_list::Entity::find()
		.filter(reading_list::Column::Id.eq(id.to_owned()))
		.one(conn)
		.await?
		.ok_or("Reading list not found")?;

	if reading_list.creating_user_id != user_id {
		// TODO: log bad access attempt to DB
		return Err("You do not have permission to access this resource."
			.to_string()
			.into());
	}

	Ok(reading_list)
}

#[cfg(test)]
mod tests {
	use super::*;
	use ::tests::{db::test_database, fake_data};
	use models::entity::library_exclusion;
	use sea_orm::{MockDatabase, QueryOrder};

	fn get_reading_list_test_object() -> reading_list::Model {
		reading_list::Model {
			id: "123".to_string(),
			name: "hello".to_string(),
			visibility: "PUBLIC".to_string(),
			creating_user_id: "42".to_string(),
			description: None,
			updated_at: "2021-08-01T00:00:00Z".parse().unwrap(),
			ordering: "MANUAL".to_string(),
		}
	}

	#[tokio::test]
	async fn test_get_for_owner() {
		let mock_db = MockDatabase::new(sea_orm::DatabaseBackend::Sqlite)
			.append_query_results(vec![vec![get_reading_list_test_object()]])
			.into_connection();

		let reading_list = get_for_owner("123", &mock_db, "42".to_string())
			.await
			.unwrap();

		assert_eq!(reading_list, get_reading_list_test_object());
	}

	#[tokio::test]
	async fn test_get_for_not_owner() {
		let mut test_model = get_reading_list_test_object();
		test_model.creating_user_id = "123".to_string();
		let mock_db = MockDatabase::new(sea_orm::DatabaseBackend::Sqlite)
			.append_query_results(vec![vec![test_model]])
			.into_connection();

		let result = get_for_owner("123", &mock_db, "user".to_string()).await;

		assert!(result.is_err());
	}

	/// The books of a list, in the list's order
	async fn list_media_ids(
		db: &DatabaseConnection,
		reading_list_id: &str,
	) -> Vec<String> {
		reading_list_item::Entity::find()
			.select_only()
			.column(reading_list_item::Column::MediaId)
			.filter(reading_list_item::Column::ReadingListId.eq(reading_list_id))
			.order_by_asc(reading_list_item::Column::DisplayOrder)
			.into_tuple::<String>()
			.all(db)
			.await
			.unwrap()
	}

	async fn insert_list(db: &DatabaseConnection, user_id: &str) -> reading_list::Model {
		ReadingListInput {
			id: None,
			name: "Favourites".to_string(),
			description: None,
			visibility: None,
			media_ids: vec![],
		}
		.into_active_model(user_id)
		.unwrap()
		.insert(db)
		.await
		.unwrap()
	}

	#[tokio::test]
	async fn test_append_reading_list_items() {
		let db = test_database().await;
		let user = fake_data::User::default().insert(&db).await;
		let series = fake_data::Series::default().insert(&db).await;
		let mut books = Vec::new();
		for _ in 0..3 {
			let book = fake_data::Media {
				series_id: series.id.clone(),
				..Default::default()
			}
			.insert(&db)
			.await;
			books.push(book.id);
		}
		let reading_list = insert_list(&db, &user.id).await;

		let added = append_reading_list_items(
			&reading_list.id,
			vec![books[1].clone(), books[0].clone()],
			&db,
		)
		.await
		.unwrap();
		assert_eq!(added, 2);

		// appended after the books already in the list, each book once
		let added = append_reading_list_items(
			&reading_list.id,
			vec![books[0].clone(), books[2].clone(), books[2].clone()],
			&db,
		)
		.await
		.unwrap();
		assert_eq!(added, 1);
		assert_eq!(
			list_media_ids(&db, &reading_list.id).await,
			vec![books[1].clone(), books[0].clone(), books[2].clone()]
		);

		// nothing new: nothing to insert
		let added = append_reading_list_items(&reading_list.id, vec![], &db)
			.await
			.unwrap();
		assert_eq!(added, 0);
	}

	#[tokio::test]
	async fn test_visible_media_ids() {
		let db = test_database().await;
		let user = fake_data::User::default().auth_user(&db).await;
		let series = fake_data::Series::default().insert(&db).await;
		let visible = fake_data::Media {
			series_id: series.id.clone(),
			..Default::default()
		}
		.insert(&db)
		.await;
		let removed = fake_data::Media {
			series_id: series.id.clone(),
			deleted_at: Some(Utc::now().into()),
			..Default::default()
		}
		.insert(&db)
		.await;

		let ids = visible_media_ids(
			&user,
			&[
				removed.id.clone(),
				visible.id.clone(),
				"missing".to_string(),
				visible.id.clone(),
			],
			&db,
		)
		.await
		.unwrap();
		assert_eq!(ids, vec![visible.id.clone()]);

		// a library hidden from the user hides its books
		library_exclusion::ActiveModel {
			user_id: Set(user.id.clone()),
			library_id: Set(visible.library_id.clone()),
			..Default::default()
		}
		.insert(&db)
		.await
		.unwrap();
		let ids = visible_media_ids(&user, &[visible.id.clone()], &db)
			.await
			.unwrap();
		assert!(ids.is_empty());
	}

	#[tokio::test]
	async fn test_deleting_a_book_or_a_list_takes_its_entries_along() {
		let db = test_database().await;
		let user = fake_data::User::default().insert(&db).await;
		let series = fake_data::Series::default().insert(&db).await;
		let book = fake_data::Media {
			series_id: series.id.clone(),
			..Default::default()
		}
		.insert(&db)
		.await;
		let reading_list = insert_list(&db, &user.id).await;
		append_reading_list_items(&reading_list.id, vec![book.id.clone()], &db)
			.await
			.unwrap();

		media::Entity::delete_by_id(book.id.clone())
			.exec(&db)
			.await
			.unwrap();
		assert!(list_media_ids(&db, &reading_list.id).await.is_empty());

		let reading_list = touch(reading_list, &db).await.unwrap();
		reading_list.delete(&db).await.unwrap();
		assert_eq!(reading_list::Entity::find().count(&db).await.unwrap(), 0);
	}
}
