use std::collections::HashSet;

use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use models::entity::{
	media, reading_list, reading_list_item, reading_list_rule, user::AuthUser,
};
use sea_orm::{prelude::*, ActiveValue::Set, QuerySelect, SqlErr, TransactionTrait};

use crate::{
	data::{AuthContext, CoreContext},
	input::reading_list::{ReadingListInput, ReadingListLimits, UpdateReadingListInput},
	object::reading_list::{ReadingList, CANT_CHANGE, NOT_FOUND},
};

/// What a user gets when a list can't be created with the ID they chose. It's the same whether
/// the ID is taken by a list they can see or by one they can't
const CANT_CREATE: &str = "Couldn't create the reading list";

#[derive(Default)]
pub struct ReadingListMutation;

#[Object]
impl ReadingListMutation {
	/// Creates a reading list owned by the current user, with the given books (in order; books
	/// the user can't see are left out). The ID is generated when omitted. Everyone but the
	/// server owner has at most 100 lists of at most 2,000 books each.
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
		let limits = ReadingListLimits::for_user(user);

		// an oversized request is refused before any of it is looked at
		if limits
			.books_per_list
			.is_some_and(|max| input.media_ids.len() as u64 > max)
		{
			return Err(too_many_books(&limits));
		}

		let txn = conn.begin().await?;
		if let Some(max) = limits.lists {
			let lists = reading_list::Entity::find()
				.filter(reading_list::Column::CreatingUserId.eq(user.id.clone()))
				.count(&txn)
				.await?;
			if lists >= max {
				return Err(format!("You can have at most {max} reading lists").into());
			}
		}

		let media_ids = visible_media_ids(user, &input.media_ids, &txn).await?;
		let active_model = input.into_active_model(&user.id, &limits)?;
		if let Set(id) = &active_model.id {
			if reading_list::Entity::find_by_id(id.clone())
				.count(&txn)
				.await? > 0
			{
				return Err(CANT_CREATE.into());
			}
		}
		let reading_list = match active_model.insert(&txn).await {
			Ok(reading_list) => reading_list,
			// two requests with the same ID at once
			Err(error)
				if matches!(
					error.sql_err(),
					Some(SqlErr::UniqueConstraintViolation(_))
				) =>
			{
				return Err(CANT_CREATE.into());
			},
			Err(error) => return Err(error.into()),
		};
		append_reading_list_items(
			&reading_list.id,
			media_ids,
			limits.books_per_list,
			&txn,
		)
		.await?;
		txn.commit().await?;

		Ok(ReadingList::from(reading_list))
	}

	/// Renames one of the current user's reading lists, or changes its description or
	/// visibility. The server owner can take someone else's public list back to private (and
	/// nothing else).
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
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let txn = conn.begin().await?;
		let (reading_list, rights) = get_editable(user, &id, &txn).await?;
		let active_model = match rights {
			Rights::Creator => {
				input.apply(reading_list, &ReadingListLimits::for_user(user))?
			},
			Rights::Moderator => input.apply_as_moderator(reading_list)?,
		};
		let updated = active_model.update(&txn).await?;
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
		let limits = ReadingListLimits::for_user(user);
		let media_ids = media_ids
			.into_iter()
			.map(|id| id.to_string())
			.collect::<Vec<_>>();

		let txn = conn.begin().await?;
		let reading_list = get_owned(user, &id, &txn).await?;
		// an oversized request is refused before any of it is looked at
		if limits
			.books_per_list
			.is_some_and(|max| media_ids.len() as u64 > max)
		{
			return Err(too_many_books(&limits));
		}
		let media_ids = visible_media_ids(user, &media_ids, &txn).await?;
		let added = append_reading_list_items(
			&reading_list.id,
			media_ids,
			limits.books_per_list,
			&txn,
		)
		.await?;
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
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let media_ids = media_ids
			.into_iter()
			.map(|id| id.to_string())
			.collect::<Vec<_>>();

		let txn = conn.begin().await?;
		let reading_list = get_owned(user, &id, &txn).await?;
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

	/// Deletes one of the current user's reading lists (not its books). The server owner can
	/// delete someone else's public list too.
	///
	/// # Returns
	///
	/// A result containing the deleted reading list, or an error if deletion failed.
	async fn delete_reading_list(
		&self,
		ctx: &Context<'_>,
		id: String,
	) -> Result<ReadingList> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let txn = conn.begin().await?;
		let (reading_list, _) = get_editable(user, &id, &txn).await?;
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

fn too_many_books(limits: &ReadingListLimits) -> async_graphql::Error {
	format!(
		"A reading list can hold at most {} books",
		limits.books_per_list.unwrap_or_default()
	)
	.into()
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
/// Returns how many were added. A list that would hold more than `max_books` after it is
/// refused, and nothing is added
async fn append_reading_list_items<C: ConnectionTrait>(
	reading_list_id: &str,
	media_ids: Vec<String>,
	max_books: Option<u64>,
	conn: &C,
) -> Result<usize> {
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
	let held = existing.len() as u64;
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

	if let Some(max) = max_books {
		if held + added as u64 > max {
			return Err(format!("A reading list can hold at most {max} books").into());
		}
	}
	if added == 0 {
		return Ok(0);
	}

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

/// What a user may do with a list they can see
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rights {
	/// They made it: they change it, its books and who sees it, and delete it
	Creator,
	/// They are the server owner and it's someone else's public list: they may take it back to
	/// private or delete it, nothing else
	Moderator,
}

/// A list the user sees, and what they may do with it. A list that doesn't exist and one they
/// may not see are the same ("not found"); one they see but aren't allowed to change says so
async fn get_editable<C: ConnectionTrait>(
	user: &AuthUser,
	id: &str,
	conn: &C,
) -> Result<(reading_list::Model, Rights)> {
	let reading_list = reading_list::Entity::find_for_user_and_id(user, id)
		.one(conn)
		.await?
		.ok_or(NOT_FOUND)?;

	if reading_list.creating_user_id == user.id {
		Ok((reading_list, Rights::Creator))
	} else if user.is_server_owner {
		Ok((reading_list, Rights::Moderator))
	} else {
		Err(CANT_CHANGE.into())
	}
}

/// A list the user made. The server owner doesn't change what is in someone else's list
async fn get_owned<C: ConnectionTrait>(
	user: &AuthUser,
	id: &str,
	conn: &C,
) -> Result<reading_list::Model> {
	match get_editable(user, id, conn).await? {
		(reading_list, Rights::Creator) => Ok(reading_list),
		(_, Rights::Moderator) => Err(CANT_CHANGE.into()),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use ::tests::{db::test_database, fake_data};
	use models::entity::library_exclusion;
	use models::entity::user::{self, LoginUser};
	use sea_orm::QueryOrder;

	/// A user who isn't the server owner (the fake users all are)
	async fn friend(db: &DatabaseConnection, name: &str) -> AuthUser {
		let model = fake_data::User::new(name).insert(db).await;
		user::ActiveModel {
			is_server_owner: Set(false),
			..model.clone().into()
		}
		.update(db)
		.await
		.unwrap();
		AuthUser::from(
			LoginUser::find_by_id(model.id)
				.into_model::<LoginUser>()
				.one(db)
				.await
				.unwrap()
				.unwrap(),
		)
	}

	/// A list of the user's, public or private
	async fn list_of(
		db: &DatabaseConnection,
		user: &AuthUser,
		visibility: &str,
	) -> reading_list::Model {
		reading_list::ActiveModel {
			visibility: Set(visibility.to_string()),
			..ReadingListInput {
				id: None,
				name: "Favourites".to_string(),
				description: None,
				visibility: None,
				media_ids: vec![],
			}
			.into_active_model(&user.id, &ReadingListLimits::UNLIMITED)
			.unwrap()
		}
		.insert(db)
		.await
		.unwrap()
	}

	async fn error_of<T>(result: Result<T>) -> String {
		match result {
			Ok(_) => panic!("should have been refused"),
			Err(error) => error.message,
		}
	}

	#[tokio::test]
	async fn the_creator_changes_their_list() {
		let db = test_database().await;
		let ana = friend(&db, "ana").await;
		let private = list_of(&db, &ana, "PRIVATE").await;
		let public = list_of(&db, &ana, "PUBLIC").await;

		for list in [&private, &public] {
			let (found, rights) = get_editable(&ana, &list.id, &db).await.unwrap();
			assert_eq!(found, *list);
			assert_eq!(rights, Rights::Creator);
			assert_eq!(get_owned(&ana, &list.id, &db).await.unwrap(), *list);
		}
	}

	#[tokio::test]
	async fn a_private_list_is_the_same_as_one_that_doesnt_exist() {
		let db = test_database().await;
		let ana = friend(&db, "ana").await;
		let ben = friend(&db, "ben").await;
		let private = list_of(&db, &ana, "PRIVATE").await;

		let unknown = error_of(get_editable(&ben, "no-such-list", &db).await).await;
		let hidden = error_of(get_editable(&ben, &private.id, &db).await).await;
		assert_eq!(unknown, NOT_FOUND);
		assert_eq!(hidden, unknown);

		// not even the server owner sees (or moderates) a private list of someone else
		let owner = fake_data::User::default().auth_user(&db).await;
		assert!(owner.is_server_owner);
		assert_eq!(
			error_of(get_editable(&owner, &private.id, &db).await).await,
			NOT_FOUND
		);
	}

	#[tokio::test]
	async fn a_friend_sees_a_public_list_but_may_not_change_it() {
		let db = test_database().await;
		let ana = friend(&db, "ana").await;
		let ben = friend(&db, "ben").await;
		let public = list_of(&db, &ana, "PUBLIC").await;

		assert_eq!(
			error_of(get_editable(&ben, &public.id, &db).await).await,
			CANT_CHANGE
		);
		assert_eq!(
			error_of(get_owned(&ben, &public.id, &db).await).await,
			CANT_CHANGE
		);
	}

	#[tokio::test]
	async fn the_server_owner_moderates_public_lists_but_doesnt_edit_their_books() {
		let db = test_database().await;
		let ana = friend(&db, "ana").await;
		let owner = fake_data::User::new("owner").auth_user(&db).await;
		let public = list_of(&db, &ana, "PUBLIC").await;

		let (found, rights) = get_editable(&owner, &public.id, &db).await.unwrap();
		assert_eq!(found, public);
		assert_eq!(rights, Rights::Moderator);
		assert_eq!(
			error_of(get_owned(&owner, &public.id, &db).await).await,
			CANT_CHANGE
		);
	}

	#[tokio::test]
	async fn the_lists_of_people_who_are_gone_are_not_there() {
		let db = test_database().await;
		let ana = friend(&db, "ana").await;
		let zoe = friend(&db, "zoe").await;
		let kim = friend(&db, "kim").await;
		let zoes = list_of(&db, &zoe, "PUBLIC").await;
		let kims = list_of(&db, &kim, "PUBLIC").await;
		for (who, deleted) in [(&zoe, true), (&kim, false)] {
			let model = user::Entity::find_by_id(who.id.clone())
				.one(&db)
				.await
				.unwrap()
				.unwrap();
			let mut active: user::ActiveModel = model.into();
			if deleted {
				active.deleted_at = Set(Some(Utc::now().into()));
			} else {
				active.is_locked = Set(true);
			}
			active.update(&db).await.unwrap();
		}

		for list in [&zoes, &kims] {
			assert_eq!(
				error_of(get_editable(&ana, &list.id, &db).await).await,
				NOT_FOUND
			);
		}
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
		.into_active_model(user_id, &ReadingListLimits::UNLIMITED)
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
			None,
			&db,
		)
		.await
		.unwrap();
		assert_eq!(added, 2);

		// appended after the books already in the list, each book once
		let added = append_reading_list_items(
			&reading_list.id,
			vec![books[0].clone(), books[2].clone(), books[2].clone()],
			None,
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
		let added = append_reading_list_items(&reading_list.id, vec![], None, &db)
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
		append_reading_list_items(&reading_list.id, vec![book.id.clone()], None, &db)
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

	#[tokio::test]
	async fn a_list_holds_at_most_the_limit_and_a_refused_add_adds_nothing() {
		let db = test_database().await;
		let user = fake_data::User::default().insert(&db).await;
		let series = fake_data::Series::default().insert(&db).await;
		let mut books = Vec::new();
		for _ in 0..4 {
			let book = fake_data::Media {
				series_id: series.id.clone(),
				..Default::default()
			}
			.insert(&db)
			.await;
			books.push(book.id);
		}
		let reading_list = insert_list(&db, &user.id).await;

		// exactly the limit fits
		let added = append_reading_list_items(
			&reading_list.id,
			books[..3].to_vec(),
			Some(3),
			&db,
		)
		.await
		.unwrap();
		assert_eq!(added, 3);

		// one more doesn't, and the list is as it was; a book that is in it already isn't more
		let refused = append_reading_list_items(
			&reading_list.id,
			vec![books[3].clone()],
			Some(3),
			&db,
		)
		.await;
		assert_eq!(
			error_of(refused).await,
			"A reading list can hold at most 3 books"
		);
		assert_eq!(
			list_media_ids(&db, &reading_list.id).await,
			books[..3].to_vec()
		);
		let again = append_reading_list_items(
			&reading_list.id,
			vec![books[0].clone()],
			Some(3),
			&db,
		)
		.await
		.unwrap();
		assert_eq!(again, 0);
	}
}
