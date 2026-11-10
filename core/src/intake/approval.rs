//! Uploads that wait for somebody to say yes.
//!
//! Anyone with "Add books" can upload, review and tag their books, but only an **approver** (the
//! server owner, or anyone who manages libraries) files them into a library. What somebody else
//! uploads waits in the approval queue, where the approver reads it through the same card, fixes
//! what is wrong, and approves it or sends it back with a note.
//!
//! ```text
//! ANALYSING → READY | NEEDS_REVIEW → PENDING_APPROVAL → FILING → FILED
//!                          ↑               │ ↘ REJECTED (with a note) → edited → READY…
//!                          └── withdrawn ──┘
//! ```
//!
//! Every move is one statement with the state it starts from in its `WHERE`, so of two requests
//! (two approvers, an approver and a withdrawal) only one wins and nothing is filed twice. Filing
//! itself is a task that outlives the request; an item left in `FILING` or `ANALYSING` by a
//! restart is put back by [`recover_interrupted`].

use std::{collections::HashSet, sync::Arc};

use chrono::Utc;
use models::{
	entity::{intake_item, library, library_exclusion, user, user::AuthUser},
	shared::{enums::UserPermission, intake::IntakeState, permission_set::PermissionSet},
};
use sea_orm::{prelude::*, ActiveValue::Set, QueryOrder};

use super::{
	compute_destination, decision_problems, file_claimed, load_library, rules, scope,
	settings_of,
};
use crate::{CoreError, CoreResult, Ctx};

/// Whether a user may file books into a library without anyone's approval: the server owner, or
/// anyone who manages libraries
pub fn can_file_directly(user: &AuthUser) -> bool {
	user.is_server_owner
		|| (!user.is_locked && user.permissions.contains(&UserPermission::ManageLibrary))
}

/// The same for a user in the database (an item's uploader, whose permissions may have changed
/// since they uploaded)
pub async fn uploader_can_file_directly(
	conn: &DatabaseConnection,
	user_id: &str,
) -> CoreResult<bool> {
	let Some(uploader) = user::Entity::find_by_id(user_id.to_string())
		.one(conn)
		.await?
	else {
		return Ok(false);
	};
	if uploader.is_server_owner {
		return Ok(true);
	}
	let permissions =
		PermissionSet::from(uploader.permissions.unwrap_or_default()).resolve_into_vec();
	Ok(uploader.deleted_at.is_none()
		&& !uploader.is_locked
		&& permissions.contains(&UserPermission::ManageLibrary))
}

/// A user as the server sees them when they make a request: their permissions, their library
/// exclusions and age restriction are what decide what they may see
pub async fn load_auth_user(
	conn: &DatabaseConnection,
	user_id: &str,
) -> CoreResult<Option<AuthUser>> {
	Ok(user::LoginUser::find_by_id(user_id.to_string())
		.filter(user::Column::DeletedAt.is_null())
		.into_model::<user::LoginUser>()
		.one(conn)
		.await?
		.map(AuthUser::from))
}

fn not_found() -> CoreError {
	CoreError::NotFound("Inbox item not found".to_string())
}

/// An item of the approval queue (or an item of the viewer's own): `None` when it doesn't exist
/// or its library is hidden from the approver
pub async fn item_for_approver(
	conn: &DatabaseConnection,
	approver: &AuthUser,
	id: &str,
) -> CoreResult<intake_item::Model> {
	let item = intake_item::Entity::find_by_id(id.to_string())
		.one(conn)
		.await?
		.ok_or_else(not_found)?;
	library_visible(conn, approver, &item).await?;
	Ok(item)
}

async fn library_visible(
	conn: &DatabaseConnection,
	user: &AuthUser,
	item: &intake_item::Model,
) -> CoreResult<()> {
	library::Entity::find_for_user(user)
		.filter(library::Column::Id.eq(item.library_id.clone()))
		.one(conn)
		.await?
		.map(|_| ())
		.ok_or_else(not_found)
}

fn now() -> DateTimeWithTimeZone {
	Utc::now().into()
}

/// Moves an item from one state to another in one statement, with what else changes with it.
/// `false`: it was not in the state `from` (somebody else moved it first)
async fn transition(
	conn: &DatabaseConnection,
	id: &str,
	from: &[IntakeState],
	change: intake_item::ActiveModel,
	only_user: Option<&str>,
) -> CoreResult<bool> {
	let mut change = change;
	// an update of many rows doesn't run the model's `before_save`
	change.updated_at = Set(now());
	let mut update = intake_item::Entity::update_many()
		.set(change)
		.filter(intake_item::Column::Id.eq(id.to_string()))
		.filter(intake_item::Column::State.is_in(from.iter().copied()));
	if let Some(user_id) = only_user {
		update = update.filter(intake_item::Column::UserId.eq(user_id.to_string()));
	}
	Ok(update.exec(conn).await?.rows_affected > 0)
}

async fn reload(conn: &DatabaseConnection, id: &str) -> CoreResult<intake_item::Model> {
	intake_item::Entity::find_by_id(id.to_string())
		.one(conn)
		.await?
		.ok_or_else(not_found)
}

/// The state an item goes back to when it returns to its uploader: ready when nothing they can
/// fix is left
async fn state_for_return(
	ctx: &Ctx,
	item: &intake_item::Model,
	uploader_is_approver: bool,
) -> CoreResult<IntakeState> {
	let conn = ctx.conn.as_ref();
	let (library, config) = load_library(conn, &item.library_id).await?;
	let settings =
		settings_of(conn, &library.id, super::library_type_in(config.as_ref())).await?;
	let (Some(decision), files) = (item.decision.clone(), item.files.0.clone()) else {
		return Ok(IntakeState::NeedsReview);
	};
	let destination = compute_destination(
		std::path::Path::new(&library.path),
		&settings,
		&files,
		&decision,
	);
	let mut problems = decision_problems(&decision, &destination);
	if !uploader_is_approver {
		problems = scope::problems_for_uploader(problems);
	}
	Ok(if problems.is_empty() {
		IntakeState::Ready
	} else {
		IntakeState::NeedsReview
	})
}

/// Sends the uploader's own items for approval. Each must be ready for it (nothing they can fix
/// is left), in a library they can still use, and not the same file as a book they can already
/// read. Of a file the same as a book they can't read nothing is said: that the library holds
/// it is for the approver to see.
pub async fn submit(
	ctx: &Ctx,
	user: &AuthUser,
	ids: &[String],
) -> CoreResult<Vec<intake_item::Model>> {
	if can_file_directly(user) {
		return Err(CoreError::BadRequest(
			"You can file books yourself: there is nobody to ask".to_string(),
		));
	}
	let conn = ctx.conn.as_ref();
	let mut items = vec![];
	for id in ids {
		let item = intake_item::Entity::find_for_user(user)
			.filter(intake_item::Column::Id.eq(id.clone()))
			.one(conn)
			.await?
			.ok_or_else(not_found)?;
		if !matches!(item.state, IntakeState::Ready | IntakeState::NeedsReview) {
			return Err(CoreError::BadRequest(format!(
				"'{}' can't be sent for approval while it is {}",
				item.name, item.state
			)));
		}
		library_visible(conn, user, &item).await?;
		let (Some(decision), Some(destination), Some(proposal)) = (
			item.decision.clone(),
			item.destination.clone(),
			item.proposal.clone(),
		) else {
			return Err(CoreError::BadRequest(format!(
				"'{}' hasn't been analysed yet",
				item.name
			)));
		};
		let problems =
			scope::problems_for_uploader(decision_problems(&decision, &destination));
		if !problems.is_empty() {
			return Err(CoreError::BadRequest(problems.join("; ")));
		}
		let same_file: Vec<String> = proposal
			.duplicates
			.iter()
			.filter(|d| d.reason == "SAME_FILE")
			.map(|d| d.media_id.clone())
			.collect();
		if !scope::visible_media_ids(conn, user, &same_file)
			.await?
			.is_empty()
		{
			return Err(CoreError::BadRequest(format!(
				"'{}' is already in the library",
				item.name
			)));
		}
		items.push(item);
	}

	let mut out = vec![];
	for item in items {
		let change = intake_item::ActiveModel {
			state: Set(IntakeState::PendingApproval),
			submitted_at: Set(Some(now())),
			review_note: Set(None),
			error: Set(None),
			..Default::default()
		};
		let moved = transition(
			conn,
			&item.id,
			&[IntakeState::Ready, IntakeState::NeedsReview],
			change,
			Some(&user.id),
		)
		.await?;
		if !moved {
			return Err(CoreError::BadRequest(format!(
				"'{}' changed while it was being sent: look at it again",
				item.name
			)));
		}
		out.push(reload(conn, &item.id).await?);
	}
	Ok(out)
}

/// Takes the uploader's own items back from the approval queue, to change them
pub async fn withdraw(
	ctx: &Ctx,
	user: &AuthUser,
	ids: &[String],
) -> CoreResult<Vec<intake_item::Model>> {
	let conn = ctx.conn.as_ref();
	let mut out = vec![];
	for id in ids {
		let item = intake_item::Entity::find_for_user(user)
			.filter(intake_item::Column::Id.eq(id.clone()))
			.one(conn)
			.await?
			.ok_or_else(not_found)?;
		if item.state != IntakeState::PendingApproval {
			return Err(CoreError::BadRequest(format!(
				"'{}' is not waiting for approval",
				item.name
			)));
		}
		let back = state_for_return(ctx, &item, false).await?;
		let change = intake_item::ActiveModel {
			state: Set(back),
			submitted_at: Set(None),
			..Default::default()
		};
		if !transition(
			conn,
			&item.id,
			&[IntakeState::PendingApproval],
			change,
			Some(&user.id),
		)
		.await?
		{
			return Err(CoreError::BadRequest(format!(
				"'{}' is being filed: it can't be taken back now",
				item.name
			)));
		}
		out.push(reload(conn, &item.id).await?);
	}
	Ok(out)
}

/// The items waiting for approval in the libraries the approver can use, oldest first
pub async fn pending(
	conn: &DatabaseConnection,
	approver: &AuthUser,
	library_id: Option<&str>,
	offset: u64,
	limit: u64,
) -> CoreResult<(Vec<intake_item::Model>, u64)> {
	let mut query = intake_item::Entity::find()
		.filter(intake_item::Column::State.eq(IntakeState::PendingApproval))
		.filter(intake_item::Column::LibraryId.not_in_subquery(
			library_exclusion::Entity::library_hidden_to_user_query(approver),
		));
	if let Some(library_id) = library_id {
		query = query.filter(intake_item::Column::LibraryId.eq(library_id.to_string()));
	}
	let total = query.clone().count(conn).await?;
	let items = sea_orm::QuerySelect::limit(
		sea_orm::QuerySelect::offset(
			query.order_by_asc(intake_item::Column::SubmittedAt),
			offset,
		),
		limit,
	)
	.all(conn)
	.await?;
	Ok((items, total))
}

/// How many items wait for approval in the libraries the approver can use
pub async fn pending_count(
	conn: &DatabaseConnection,
	approver: &AuthUser,
) -> CoreResult<u64> {
	Ok(pending(conn, approver, None, 0, 1).await?.1)
}

/// Approves items: each is claimed with one statement and filed by a task that outlives the
/// request. The items come back as they are now (`FILING`); an item that was already being filed
/// or filed comes back as it is, so approving twice does no harm.
pub async fn approve(
	ctx: &Arc<Ctx>,
	approver: &AuthUser,
	ids: &[String],
) -> CoreResult<Vec<intake_item::Model>> {
	require_approver(approver)?;
	let conn = ctx.conn.as_ref();
	let mut seen = HashSet::new();
	let mut out = vec![];
	for id in ids.iter().filter(|id| seen.insert(id.as_str())) {
		let item = item_for_approver(conn, approver, id).await?;
		match item.state {
			IntakeState::PendingApproval => {},
			IntakeState::Filing | IntakeState::Filed => {
				out.push(item);
				continue;
			},
			_ => {
				return Err(CoreError::BadRequest(format!(
					"'{}' is not waiting for approval",
					item.name
				)))
			},
		}
		if !claim_for_filing(conn, &item.id, IntakeState::PendingApproval, &approver.id)
			.await?
		{
			// somebody approved it, or the uploader took it back, in the meantime
			out.push(reload(conn, &item.id).await?);
			continue;
		}
		let claimed = reload(conn, &item.id).await?;
		let task_ctx = ctx.clone();
		let task_item = claimed.clone();
		tokio::spawn(async move {
			if let Err(error) =
				file_claimed(&task_ctx, task_item, IntakeState::PendingApproval).await
			{
				tracing::warn!(?error, "Filing an approved book failed");
			}
		});
		out.push(claimed);
	}
	Ok(out)
}

/// Sends an item of the approval queue back to its uploader with a note
pub async fn reject(
	ctx: &Ctx,
	approver: &AuthUser,
	id: &str,
	note: &str,
) -> CoreResult<intake_item::Model> {
	require_approver(approver)?;
	let note = rules::plain_text(note);
	if note.is_empty() {
		return Err(CoreError::BadRequest(
			"Say what is wrong, so the uploader knows what to change".to_string(),
		));
	}
	if note.chars().count() > rules::MAX_NOTE_CHARS {
		return Err(CoreError::BadRequest(format!(
			"The note can't be longer than {} characters",
			rules::MAX_NOTE_CHARS
		)));
	}
	let conn = ctx.conn.as_ref();
	let item = item_for_approver(conn, approver, id).await?;
	if item.state != IntakeState::PendingApproval {
		return Err(CoreError::BadRequest(format!(
			"'{}' is not waiting for approval",
			item.name
		)));
	}
	let change = intake_item::ActiveModel {
		state: Set(IntakeState::Rejected),
		submitted_at: Set(None),
		reviewed_at: Set(Some(now())),
		reviewed_by: Set(Some(approver.id.clone())),
		review_note: Set(Some(note)),
		error: Set(None),
		..Default::default()
	};
	if !transition(
		conn,
		&item.id,
		&[IntakeState::PendingApproval],
		change,
		None,
	)
	.await?
	{
		return Err(CoreError::BadRequest(format!(
			"'{}' was decided on while you were looking at it",
			item.name
		)));
	}
	reload(conn, &item.id).await
}

fn require_approver(user: &AuthUser) -> CoreResult<()> {
	if can_file_directly(user) {
		Ok(())
	} else {
		Err(CoreError::BadRequest(
			"Only whoever approves books can do this".to_string(),
		))
	}
}

/// Claims an item for filing: one statement, so of two requests only one wins. `false`: the
/// item was not in the state `from` any more
pub async fn claim_for_filing(
	conn: &DatabaseConnection,
	id: &str,
	from: IntakeState,
	reviewer_id: &str,
) -> CoreResult<bool> {
	let change = intake_item::ActiveModel {
		state: Set(IntakeState::Filing),
		reviewed_at: Set(Some(now())),
		reviewed_by: Set(Some(reviewer_id.to_string())),
		error: Set(None),
		..Default::default()
	};
	transition(conn, id, &[from], change, None).await
}

/// Files an approver's own ready item, now
pub async fn accept(
	ctx: &Ctx,
	approver: &AuthUser,
	item: intake_item::Model,
) -> CoreResult<intake_item::Model> {
	require_approver(approver)?;
	if item.user_id != approver.id {
		return Err(not_found());
	}
	if item.state != IntakeState::Ready
		|| !claim_for_filing(
			ctx.conn.as_ref(),
			&item.id,
			IntakeState::Ready,
			&approver.id,
		)
		.await?
	{
		return Err(CoreError::BadRequest(format!(
			"'{}' is not ready to be filed",
			item.name
		)));
	}
	let claimed = reload(ctx.conn.as_ref(), &item.id).await?;
	file_claimed(ctx, claimed, IntakeState::Ready).await
}

/// Puts back what a restart interrupted. An item that was being filed waits for approval again
/// (what was already placed in the library is remembered, so approving it again doesn't copy
/// it twice); an item that was being analysed failed, and its uploader can analyse it again (it
/// is not started again by itself: a file that stops the analysis would stop it again). The
/// folders of uploads that were still arriving are removed.
pub async fn recover_interrupted(ctx: &Ctx) -> CoreResult<()> {
	let conn = ctx.conn.as_ref();
	for item in intake_item::Entity::find()
		.filter(intake_item::Column::State.eq(IntakeState::Filing))
		.all(conn)
		.await?
	{
		let back = if item.submitted_at.is_some() {
			IntakeState::PendingApproval
		} else {
			IntakeState::Ready
		};
		let change = intake_item::ActiveModel {
			state: Set(back),
			error: Set(Some(
				"The server restarted while this was being filed: approve it again"
					.to_string(),
			)),
			..Default::default()
		};
		transition(conn, &item.id, &[IntakeState::Filing], change, None).await?;
	}
	for item in intake_item::Entity::find()
		.filter(intake_item::Column::State.eq(IntakeState::Analysing))
		.all(conn)
		.await?
	{
		let change = intake_item::ActiveModel {
			state: Set(IntakeState::Failed),
			error: Set(Some(
				"The server restarted while this was being analysed: analyse it again"
					.to_string(),
			)),
			..Default::default()
		};
		transition(conn, &item.id, &[IntakeState::Analysing], change, None).await?;
	}
	super::upload::remove_incoming(ctx).await;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn user(owner: bool, permissions: &[UserPermission], locked: bool) -> AuthUser {
		AuthUser {
			id: "u".into(),
			is_server_owner: owner,
			is_locked: locked,
			permissions: permissions.to_vec(),
			..Default::default()
		}
	}

	#[test]
	fn the_owner_and_library_managers_file_directly() {
		assert!(can_file_directly(&user(true, &[], false)));
		assert!(can_file_directly(&user(
			false,
			&[UserPermission::ManageLibrary],
			false
		)));
		// what lets somebody add books doesn't let them file without asking
		assert!(!can_file_directly(&user(
			false,
			&[UserPermission::AddBooks, UserPermission::EditLibrary],
			false
		)));
		assert!(!can_file_directly(&user(false, &[], false)));
		// a locked account that manages libraries doesn't; the owner always can
		assert!(!can_file_directly(&user(
			false,
			&[UserPermission::ManageLibrary],
			true
		)));
	}
}
