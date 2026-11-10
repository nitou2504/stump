use std::collections::HashSet;

use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
	input::smart_lists::{list_visibility, SaveSmartListInput, SmartListLimits},
	object::smart_lists::SmartList,
};
use async_graphql::{Context, Object, Result, ID};
use models::{
	entity::smart_list,
	shared::enums::{EntityVisibility, UserPermission},
};
use sea_orm::{prelude::*, ActiveModelTrait, Set, TransactionTrait};

/// What a user gets for a list they can't change, whether it is somebody else's or doesn't exist:
/// the same, so that the answer doesn't tell which private lists there are
const NOT_FOUND: &str = "Smart list not found";

/// How many lists `setSmartListsVisibility` changes at most in one go
const MAX_VISIBILITY_BATCH: usize = 1_000;

#[derive(Default)]
pub struct SmartListMutation;

#[Object]
impl SmartListMutation {
	/// Creates a smart list owned by the current user. Everyone but the server owner has at most
	/// 20 lists, each with rules of at most 128 KB, a name of at most 200 characters and a
	/// description of at most 2,000.
	#[graphql(guard = "PermissionGuard::one(UserPermission::AccessSmartList)")]
	async fn create_smart_list(
		&self,
		ctx: &Context<'_>,
		input: SaveSmartListInput,
	) -> Result<SmartList> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let limits = SmartListLimits::for_user(user);

		// an oversized list is refused before anything is looked up
		let active_model = input.into_active_model(&user.id, &limits)?;

		let txn = conn.begin().await?;
		if let Some(max) = limits.lists {
			let lists = smart_list::Entity::find()
				.filter(smart_list::Column::CreatorId.eq(user.id.clone()))
				.count(&txn)
				.await?;
			if lists >= max {
				return Err(format!("You can have at most {max} smart lists").into());
			}
		}
		let inserted_smart_list = smart_list::Entity::insert(active_model)
			.exec_with_returning(&txn)
			.await?;
		txn.commit().await?;

		Ok(SmartList::from(inserted_smart_list))
	}

	#[graphql(guard = "PermissionGuard::one(UserPermission::AccessSmartList)")]
	async fn update_smart_list(
		&self,
		ctx: &Context<'_>,
		id: ID,
		input: SaveSmartListInput,
	) -> Result<SmartList> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let txn = conn.begin().await?;

		// Only its creator (or the server owner) may change a list, even a PUBLIC one. The
		// same answer as for a list they can't see, so this doesn't tell what exists
		let existing = smart_list::Entity::find_by_id_for_edit(user, id.clone())
			.one(&txn)
			.await?
			.ok_or(NOT_FOUND.to_string())?;

		// The list stays its creator's, also when the server owner edits it. The limits are the
		// editor's: the owner moderating a friend's list has none
		let mut active_model = input
			.into_active_model(&existing.creator_id, &SmartListLimits::for_user(user))?;
		active_model.id = Set(id.to_string());
		let updated = active_model.update(&txn).await?;
		txn.commit().await?;

		Ok(updated.into())
	}

	/// Makes some of the current user's smart lists private or public in one go ("share all my
	/// lists"). A list is changed only by its creator, so any ID that isn't one of the user's own
	/// lists (somebody else's, or none at all) fails the whole request with the same answer and
	/// changes nothing. `SHARED` is not offered.
	#[graphql(guard = "PermissionGuard::one(UserPermission::AccessSmartList)")]
	async fn set_smart_lists_visibility(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
		visibility: EntityVisibility,
	) -> Result<Vec<SmartList>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let visibility = list_visibility(visibility)?;

		let mut seen = HashSet::new();
		let ids = ids
			.into_iter()
			.map(|id| id.to_string())
			.filter(|id| seen.insert(id.clone()))
			.collect::<Vec<_>>();
		if ids.len() > MAX_VISIBILITY_BATCH {
			return Err(format!(
				"At most {MAX_VISIBILITY_BATCH} smart lists can be changed at once"
			)
			.into());
		}
		if ids.is_empty() {
			return Ok(vec![]);
		}

		let txn = conn.begin().await?;
		let mut own = smart_list::Entity::find()
			.filter(smart_list::Column::CreatorId.eq(user.id.clone()))
			.filter(smart_list::Column::Id.is_in(ids.clone()))
			.all(&txn)
			.await?;
		if own.len() != ids.len() {
			return Err(NOT_FOUND.into());
		}

		let mut changed = Vec::with_capacity(own.len());
		for list in own.drain(..) {
			changed.push(if list.visibility == visibility {
				list
			} else {
				let mut active_model: smart_list::ActiveModel = list.into();
				active_model.visibility = Set(visibility);
				active_model.update(&txn).await?
			});
		}
		txn.commit().await?;

		// in the order they were asked for
		changed.sort_by_key(|list| ids.iter().position(|id| *id == list.id));
		Ok(changed.into_iter().map(SmartList::from).collect())
	}

	#[graphql(guard = "PermissionGuard::one(UserPermission::AccessSmartList)")]
	async fn delete_smart_list(&self, ctx: &Context<'_>, id: ID) -> Result<SmartList> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let txn = conn.begin().await?;

		let smart_list = smart_list::Entity::find_by_id_for_edit(user, id.clone())
			.one(&txn)
			.await?
			.ok_or("Smart list not found".to_string())?;

		smart_list.clone().delete(&txn).await?;
		txn.commit().await?;

		Ok(SmartList::from(smart_list))
	}
}
