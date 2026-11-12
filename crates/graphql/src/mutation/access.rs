use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use models::{
	entity::user,
	services::{
		access::{self as service, AccessError},
		access_people::{self, PersonAccessChange},
		access_warnings,
		signup::{self, NewSignupLink},
		user::revoke_user_sessions,
	},
	shared::enums::UserPermission,
};
use sea_orm::{prelude::*, sea_query::Expr, TransactionTrait};
use stump_core::access::{pass, state as edge_state};

use crate::{
	data::{AuthContext, CoreContext},
	guard::{NotViaPublicEdgeGuard, SameOriginGuard, ServerOwnerGuard},
	object::{
		access::{
			AccessInvite, AccessPersonRow, CreateAccessInviteInput, CreatedAccessInvite,
			INVITE_HOURS,
		},
		access_signup::{
			AccessSignupLink, CreateSignupLinkInput, CreatedSignupLink, PersonAccessInput,
		},
	},
};

/// What the admin is told when a change is made but the edge could not be: the app refuses the
/// device at once itself, and the edge follows once the state file can be written again
const EDGE_NOT_TOLD: &str = "Done, and Stump refuses it at once, but the edge could not be told (its state file was not written). Check the state folder, then try again";

/// The shortest password the owner can set for somebody
const MIN_PASSWORD_CHARS: usize = 8;
/// The longest (bcrypt only reads the first 72 bytes of one anyway)
const MAX_PASSWORD_CHARS: usize = 128;

/// The most warning ids one call may name (there are never that many warnings)
const MAX_WARNING_IDS: usize = 200;

/// The ids of a call, as text, and refused when there are far more than there can be warnings
fn warning_ids(ids: Option<Vec<ID>>) -> Result<Option<Vec<String>>> {
	let Some(ids) = ids else {
		return Ok(None);
	};
	if ids.len() > MAX_WARNING_IDS {
		return Err(async_graphql::Error::new("Too many warnings in one call"));
	}
	Ok(Some(ids.into_iter().map(|id| id.to_string()).collect()))
}

/// What an audit row of a warning call points at: the id when there is exactly one, else `all`
fn audit_target_of(ids: Option<&[String]>) -> String {
	match ids {
		Some([one]) => one.clone(),
		_ => "all".to_string(),
	}
}

/// The mutations of the *Public access* page. Every one is for the server owner only, refused on
/// a request that came through the public edge, and refused when another site may have made the
/// browser send it (a forged form)
#[derive(Default)]
pub struct AccessMutation;

pub(crate) fn access_error(error: AccessError) -> async_graphql::Error {
	match error {
		AccessError::Db(error) => {
			tracing::error!(?error, "Public access: a database error");
			"Something went wrong".into()
		},
		other => other.to_string().into(),
	}
}

/// Writes the state file so the edge follows a change, and says so if it can't
pub(crate) async fn tell_the_edge(ctx: &Context<'_>) -> Result<()> {
	let core_ctx = ctx.data::<CoreContext>()?;
	match edge_state::publish_if_configured(
		core_ctx.conn.as_ref(),
		core_ctx.config.as_ref(),
	)
	.await
	{
		Ok(_) => Ok(()),
		Err(error) => {
			tracing::error!(?error, "Public access: the edge could not be told");
			Err(EDGE_NOT_TOLD.into())
		},
	}
}

#[Object]
impl AccessMutation {
	/// Makes an invite link for a person and a device. The link is in the response and nowhere
	/// else: Stump keeps a hash of the token, not the token
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn create_access_invite(
		&self,
		ctx: &Context<'_>,
		input: CreateAccessInviteInput,
	) -> Result<CreatedAccessInvite> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core_ctx = ctx.data::<CoreContext>()?;
		let conn = core_ctx.conn.as_ref();

		let edge = core_ctx.config.edge.as_ref();
		let Some(base_url) = edge.and_then(|edge| edge.public_base_url()) else {
			return Err("The public address is not set (STUMP_EDGE_PUBLIC_URL)".into());
		};
		if edge.and_then(|edge| edge.usable_key()).is_none() {
			return Err("There is no key to sign passes with (STUMP_EDGE_KEY)".into());
		}
		if !INVITE_HOURS.contains(&input.valid_for_hours) {
			return Err("That validity is not allowed".into());
		}

		let (token, token_hash) = pass::new_invite_token().map_err(|error| {
			tracing::error!(error, "Public access: could not make a token");
			async_graphql::Error::new("Something went wrong")
		})?;

		let now = Utc::now();
		let invite = service::create_invite(
			conn,
			now,
			service::NewInvite {
				person_id: input.person_id,
				label: input.label,
				created_by: user.username.clone(),
				token_hash,
				valid_hours: i64::from(input.valid_for_hours),
				max_uses: input.max_uses,
			},
		)
		.await
		.map_err(access_error)?;

		Ok(CreatedAccessInvite {
			link: format!("{base_url}/api/_i/{token}"),
			invite: AccessInvite::new(invite, now),
		})
	}

	/// Makes a link that lets somebody make their own account: for one person or for several
	/// (a budget of accounts), valid for some hours, with the preset the accounts get. Refused
	/// when the preset holds a permission a friend may not have (Upload files and Scan library
	/// included). The link is in the response and nowhere else
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn create_access_signup_link(
		&self,
		ctx: &Context<'_>,
		input: CreateSignupLinkInput,
	) -> Result<CreatedSignupLink> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core_ctx = ctx.data::<CoreContext>()?;
		let conn = core_ctx.conn.as_ref();

		let edge = core_ctx.config.edge.as_ref();
		let Some(base_url) = edge.and_then(|edge| edge.public_base_url()) else {
			return Err("The public address is not set (STUMP_EDGE_PUBLIC_URL)".into());
		};
		if edge.and_then(|edge| edge.usable_key()).is_none() {
			return Err("There is no key to sign passes with (STUMP_EDGE_KEY)".into());
		}
		if !edge.is_some_and(|edge| edge.public_signin) {
			return Err(
				"Public sign-in is switched off (STUMP_EDGE_PUBLIC_SIGNIN)".into()
			);
		}

		let (token, token_hash) = pass::new_invite_token().map_err(|error| {
			tracing::error!(error, "Public access: could not make a token");
			async_graphql::Error::new("Something went wrong")
		})?;
		let now = Utc::now();
		let link = signup::create_signup_link(
			conn,
			now,
			NewSignupLink {
				label: input.label,
				kind: input.kind.into(),
				created_by: user.username.clone(),
				token_hash,
				valid_hours: i64::from(input.valid_for_hours),
				max_uses: input.max_uses,
				preset: input.preset.into_preset(),
			},
		)
		.await
		.map_err(access_error)?;

		let libraries = signup::preset_of(&link)
			.map(|preset| preset.library_ids)
			.unwrap_or_default();
		let names: std::collections::HashMap<String, String> =
			models::entity::library::Entity::find()
				.filter(models::entity::library::Column::Id.is_in(libraries.clone()))
				.all(conn)
				.await?
				.into_iter()
				.map(|library| (library.id, library.name))
				.collect();
		let libraries = libraries
			.into_iter()
			.filter_map(|id| {
				names
					.get(&id)
					.map(|name| crate::object::access_signup::PresetLibrary {
						id,
						name: name.clone(),
					})
			})
			.collect();
		Ok(CreatedSignupLink {
			url: format!("{base_url}/api/_i/{token}"),
			link: AccessSignupLink::new(link, now, libraries, vec![]),
		})
	}

	/// Revokes a signup link: nobody can make an account with it any more. With
	/// `disableAccounts` the accounts it already made are turned off too (their public sign-in
	/// goes off, their devices are revoked, their sessions end; the accounts stay). Returns how
	/// many accounts were turned off
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn revoke_access_signup_link(
		&self,
		ctx: &Context<'_>,
		id: ID,
		#[graphql(default = false)] disable_accounts: bool,
	) -> Result<u64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let revoked = signup::revoke_signup_link(
			conn,
			Utc::now(),
			id.as_str(),
			&user.username,
			disable_accounts,
		)
		.await?;
		tell_the_edge(ctx).await?;
		Ok(revoked.accounts_disabled)
	}

	/// Changes what a person can do from the internet: whether they may sign in, how many devices
	/// they may have, whether libraries made later are hidden from them, and (with
	/// `onlyLibraryIds`) exactly which libraries they see. Refused for the server owner, and for an
	/// account that holds permissions a friend may not have: the error names them
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn set_person_access(
		&self,
		ctx: &Context<'_>,
		input: PersonAccessInput,
	) -> Result<AccessPersonRow> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let now = Utc::now();

		access_people::set_person_access(
			conn,
			now,
			&user.username,
			input.person_id.as_str(),
			PersonAccessChange {
				public_signin: input.public_signin,
				max_devices: input.max_devices,
				new_libraries_hidden: input.new_libraries_hidden,
				only_library_ids: input
					.only_library_ids
					.map(|ids| ids.into_iter().map(|id| id.to_string()).collect()),
			},
		)
		.await
		.map_err(access_error)?;

		let person = service::people(conn)
			.await?
			.into_iter()
			.find(|person| person.user.id == input.person_id.as_str())
			.ok_or("User not found")?;
		let summary = access_people::person_summaries(conn, now, &[person.user.clone()])
			.await?
			.remove(&person.user.id);
		Ok(AccessPersonRow::new(person, summary))
	}

	/// Marks warnings as seen: they leave *Needs your attention* and the badge until something new
	/// or worse happens. Without `ids`: every warning shown now (*Clear all*). Nothing is deleted:
	/// the sign-ins and the audit log stay. Returns how many it marked
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn dismiss_access_warnings(
		&self,
		ctx: &Context<'_>,
		ids: Option<Vec<ID>>,
	) -> Result<u32> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let ids = warning_ids(ids)?;

		let now = Utc::now();
		let changed = access_warnings::dismiss(conn, now, &user.id, ids.as_deref()).await?;
		if changed > 0 {
			service::audit(
				conn,
				now,
				&user.username,
				"warning.dismiss",
				&audit_target_of(ids.as_deref()),
				&format!("{changed} items"),
			)
			.await?;
		}
		Ok(changed as u32)
	}

	/// Shows warnings that were marked as seen again (*Undo*). Returns how many were shown again
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn restore_access_warnings(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
	) -> Result<u32> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let ids = warning_ids(Some(ids))?.unwrap_or_default();

		let now = Utc::now();
		let changed = access_warnings::restore(conn, &ids).await?;
		if changed > 0 {
			service::audit(
				conn,
				now,
				&user.username,
				"warning.restore",
				&audit_target_of(Some(&ids)),
				&format!("{changed} items"),
			)
			.await?;
		}
		Ok(changed as u32)
	}

	/// Revokes an invite: it can't be opened any more. The devices that were made from it keep
	/// their pass (revoke those one by one)
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn revoke_access_invite(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		Ok(service::revoke_invite(conn, Utc::now(), id.as_str(), &user.username).await?)
	}

	/// Revokes a device: its pass stops working, at once for the app and within a couple of
	/// seconds at the edge. The invite it came from is revoked with it. Returns whether it
	/// changed
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn revoke_access_device(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let changed =
			service::revoke_device(conn, Utc::now(), id.as_str(), &user.username).await?;
		tell_the_edge(ctx).await?;
		Ok(changed)
	}

	/// Brings a revoked device back. Its invite stays revoked. Returns whether it changed
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn restore_access_device(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let changed =
			service::restore_device(conn, Utc::now(), id.as_str(), &user.username)
				.await?;
		tell_the_edge(ctx).await?;
		Ok(changed)
	}

	/// Cuts a person off: every device of theirs is revoked, so is every open invite of theirs,
	/// and their sessions and refresh tokens are removed. Returns how many devices were revoked
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn revoke_access_devices_of_person(
		&self,
		ctx: &Context<'_>,
		person_id: ID,
	) -> Result<u64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let revoked = service::revoke_devices_of_person(
			conn,
			Utc::now(),
			person_id.as_str(),
			&user.username,
		)
		.await?;
		tell_the_edge(ctx).await?;
		Ok(revoked.devices)
	}

	/// Gives a device a fresh daily sign-in budget at the edge. Returns whether the device exists
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn reset_access_device_budget(
		&self,
		ctx: &Context<'_>,
		id: ID,
	) -> Result<bool> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let found =
			service::reset_device_budget(conn, Utc::now(), id.as_str(), &user.username)
				.await?;
		tell_the_edge(ctx).await?;
		Ok(found)
	}

	/// Unlocks an account and forgives the failed sign-ins of the last 24 hours, so that the next
	/// typo doesn't lock it again. Returns whether the account exists
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn unblock_user(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		Ok(service::unblock_user(conn, Utc::now(), id.as_str(), &user.username).await?)
	}

	/// Sets somebody's password. With `requireChange` (the default) they must choose their own
	/// before they can use anything else: until then they can only change the password and sign
	/// out. It ends their sessions. The server owner's password is never set here.
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)")]
	async fn set_user_password(
		&self,
		ctx: &Context<'_>,
		id: ID,
		password: String,
		#[graphql(default = true)] require_change: bool,
	) -> Result<bool> {
		let AuthContext { user: admin, .. } = ctx.data::<AuthContext>()?;
		let core_ctx = ctx.data::<CoreContext>()?;
		let conn = core_ctx.conn.as_ref();

		let length = password.chars().count();
		if !(MIN_PASSWORD_CHARS..=MAX_PASSWORD_CHARS).contains(&length) {
			return Err(format!(
				"The password must have {MIN_PASSWORD_CHARS} to {MAX_PASSWORD_CHARS} characters"
			)
			.into());
		}

		let target = user::Entity::find_by_id(id.to_string())
			.filter(user::Column::DeletedAt.is_null())
			.one(conn)
			.await?
			.ok_or("User not found")?;
		if target.is_server_owner {
			return Err("The server owner's password is not set from here".into());
		}
		if require_change {
			// somebody who may not change their password could never replace the one they were
			// given
			let permissions = models::shared::permission_set::PermissionSet::from(
				target.permissions.clone().unwrap_or_default(),
			)
			.resolve_into_vec();
			if !permissions.contains(&UserPermission::ChangePassword) {
				return Err("That person may not change their password, so they could not replace this one. Allow it for them first, or set it without the requirement".into());
			}
		}

		let hashed = bcrypt::hash(password, core_ctx.config.password_hash_cost)?;
		let now = Utc::now();
		let txn = conn.begin().await?;
		user::Entity::update_many()
			.col_expr(user::Column::HashedPassword, Expr::value(hashed))
			.filter(user::Column::Id.eq(target.id.clone()))
			.exec(&txn)
			.await?;
		revoke_user_sessions(&txn, &target.id, None).await?;
		if require_change {
			service::require_password_change(&txn, now, &target.id, &admin.username)
				.await?;
		} else {
			service::clear_password_change(&txn, &target.id).await?;
		}
		service::audit(
			&txn,
			now,
			&admin.username,
			"user.password_set",
			&target.id,
			&format!(
				"{} ({})",
				target.username,
				if require_change {
					"must choose their own"
				} else {
					"no requirement"
				}
			),
		)
		.await?;
		txn.commit().await?;

		Ok(true)
	}
}
