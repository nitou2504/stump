//! Public access: what the owner manages from *Settings → Public access* instead of a terminal.
//!
//! Everything that touches the database is here; the pass itself (the signed cookie and the
//! invite tokens) and the file the edge follows are in `stump_core::access`.
//!
//! - invites: one link for one person, hashed at rest, spent by a single conditional update;
//! - devices: the browsers that hold a pass and the reading-app keys, each counted against their
//!   person's limit by one conditional insert, revoked and restored from the page;
//! - the sign-in log, which is `user_login_activity` plus [`access_signin_mark`] (the pass a
//!   sign-in came with, why it failed, whether the lock rule counts it);
//! - the lock rule's count and the *unblock* that really clears a lock;
//! - the audit log of all of it;
//! - the snapshot the edge's state file is made of, and the retention of old rows.
//!
//! Timestamps are `DateTime<Utc>` bound through the entities everywhere, so every comparison is
//! between values written the same way.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Duration, Utc};
use sea_orm::{
	prelude::*,
	sea_query::{Expr, Query},
	ActiveValue::{Set, Unchanged},
	ColumnTrait, Condition, ConnectionTrait, DbBackend, DbErr, EntityTrait, ExprTrait,
	PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Statement, TransactionSession,
	TransactionTrait, Value,
};
use thiserror::Error;
use uuid::Uuid;

use crate::{
	entity::{
		access_audit, access_device, access_invite, access_password_reset,
		access_person, access_signin_mark, access_signup_link, user, user_login_activity,
	},
	services::{
		access_policy::{self, DORMANT_AFTER_DAYS},
		user::revoke_user_sessions,
	},
};

/// The most characters of a device name
pub const MAX_LABEL_CHARS: usize = 40;
/// The most characters kept of a browser's user agent
pub const MAX_AGENT_CHARS: usize = 120;
/// The most uses one invite can be given
pub const MAX_INVITE_USES: i32 = 10;
/// The longest an invite can be valid for, in hours (30 days)
pub const MAX_INVITE_HOURS: i64 = 720;
/// A pass is valid for 180 days; a revocation is remembered for longer than that
pub const REVOKED_DEVICE_KEEP_DAYS: i64 = 200;
/// How long the audit log is kept
pub const AUDIT_KEEP_DAYS: i64 = 400;
/// How long an expired or revoked invite is kept (unless a live device came from it)
pub const INVITE_KEEP_DAYS: i64 = 30;
/// The window the lock rule looks at, in hours
pub const LOCK_WINDOW_HOURS: i64 = 24;

/// Why a sign-in failed, as stored in [`access_signin_mark`]
pub mod reason {
	pub const WRONG_PASSWORD: &str = "wrong_password";
	pub const LOCKED: &str = "locked";
	pub const OWNER_VIA_EDGE: &str = "owner_via_edge";
	pub const WRONG_PERSON: &str = "wrong_person";
	/// The right password of an account that is not allowed in from the internet
	pub const NOT_ALLOWED_PUBLICLY: &str = "not_allowed_publicly";
	/// The right password, but the person already uses all their devices
	pub const DEVICE_LIMIT: &str = "device_limit";
}

/// The two kinds of device
pub mod device_kind {
	/// A pass in a browser's cookie
	pub const BROWSER: &str = "browser";
	/// A reading-app key (`<id>.<mac>` in an address)
	pub const OPDS: &str = "opds";
}

#[derive(Debug, Error)]
pub enum AccessError {
	#[error(transparent)]
	Db(#[from] DbErr),
	/// The person doesn't exist, was removed, or is the server owner (who signs in from home
	/// only)
	#[error("That person can't be given an invite link")]
	PersonNotAllowed,
	#[error("{0}")]
	Invalid(&'static str),
	/// A refusal that names something (the permissions an account holds beyond the friend set)
	#[error("{0}")]
	Message(String),
}

// -------------------------------------------------------------------------------------------
// text and ids
// -------------------------------------------------------------------------------------------

/// A character that has no place in a name shown to the owner: a control character, a zero-width
/// one, or one that changes the direction of the text around it (it could make a name read as
/// another)
fn is_unsafe_char(c: char) -> bool {
	c.is_control()
		|| matches!(
			c,
			'\u{200b}'..='\u{200f}'
				| '\u{202a}'..='\u{202e}'
				| '\u{2066}'..='\u{2069}'
				| '\u{feff}'
		)
}

/// Text from outside, made safe to store and show: control, zero-width and direction-changing
/// characters are dropped, the ends are trimmed and it is cut to `max_chars` characters
pub fn clean_text(text: &str, max_chars: usize) -> String {
	let without_controls: String = text.chars().filter(|c| !is_unsafe_char(*c)).collect();
	let cut: String = without_controls.trim().chars().take(max_chars).collect();
	cut.trim_end().to_string()
}

/// The readable part of a device id: lowercase letters and digits of the name, anything else
/// as a single hyphen, at most 17 characters (`Ana's phone` is `ana-s-phone`)
pub fn device_slug(label: &str) -> String {
	let mut slug = String::new();
	for c in label.chars() {
		let c = c.to_ascii_lowercase();
		if c.is_ascii_lowercase() || c.is_ascii_digit() {
			slug.push(c);
		} else if !slug.ends_with('-') {
			slug.push('-');
		}
	}
	let cut: String = slug.trim_matches('-').chars().take(17).collect();
	let cut = cut.trim_matches('-');
	if cut.is_empty() {
		"device".to_string()
	} else {
		cut.to_string()
	}
}

const ID_ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// A new device id: the slug of the name and five random characters (`ana-phone-k3f9x`), at
/// most 23 of the 24 a pass name can have
pub fn new_device_id(label: &str) -> String {
	let random = Uuid::new_v4();
	let suffix: String = random
		.as_bytes()
		.iter()
		.take(5)
		.map(|byte| ID_ALPHABET[(*byte & 31) as usize] as char)
		.collect();
	format!("{}-{suffix}", device_slug(label))
}

/// Whether `id` is shaped like the name of a pass: `[a-z0-9-]{1,24}`
pub fn is_valid_device_id(id: &str) -> bool {
	(1..=24).contains(&id.len())
		&& id
			.bytes()
			.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

// -------------------------------------------------------------------------------------------
// audit
// -------------------------------------------------------------------------------------------

/// Writes a row of the audit log. `detail` is for ids and names: never a token, a link or a
/// password
pub async fn audit<C>(
	conn: &C,
	now: DateTime<Utc>,
	actor: &str,
	action: &str,
	target: &str,
	detail: &str,
) -> Result<(), DbErr>
where
	C: ConnectionTrait,
{
	access_audit::ActiveModel {
		at: Set(now),
		actor: Set(clean_text(actor, 64)),
		action: Set(clean_text(action, 64)),
		target: Set(clean_text(target, 80)),
		detail: Set(clean_text(detail, 300)),
		..Default::default()
	}
	.insert(conn)
	.await?;
	Ok(())
}

/// A page of the audit log, the latest first, and how many rows there are
pub async fn list_audit<C>(
	conn: &C,
	page: u64,
	page_size: u64,
) -> Result<(Vec<access_audit::Model>, u64), DbErr>
where
	C: ConnectionTrait,
{
	let paginator = access_audit::Entity::find()
		.order_by_desc(access_audit::Column::Id)
		.paginate(conn, std::cmp::max(page_size, 1));
	let total = paginator.num_items().await?;
	let rows = paginator.fetch_page(page.saturating_sub(1)).await?;
	Ok((rows, total))
}

// -------------------------------------------------------------------------------------------
// invites
// -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteStatus {
	/// Not used yet, still valid
	Unused,
	/// Used at least once, uses left
	Used,
	/// Every use is spent
	UsedUp,
	Expired,
	Revoked,
}

/// What an invite is at `now`
pub fn invite_status(invite: &access_invite::Model, now: DateTime<Utc>) -> InviteStatus {
	if invite.revoked_at.is_some() {
		InviteStatus::Revoked
	} else if invite.expires_at <= now {
		InviteStatus::Expired
	} else if invite.uses >= invite.max_uses {
		InviteStatus::UsedUp
	} else if invite.uses > 0 {
		InviteStatus::Used
	} else {
		InviteStatus::Unused
	}
}

pub struct NewInvite {
	pub person_id: String,
	/// The device name the owner typed
	pub label: String,
	/// The admin's username
	pub created_by: String,
	/// The SHA-256 of the token (hex); the token itself is never given to this module
	pub token_hash: String,
	pub valid_hours: i64,
	pub max_uses: i32,
}

/// Makes an invite for one person: a user of this server who isn't the owner. The audit row is
/// written with it
pub async fn create_invite<C>(
	conn: &C,
	now: DateTime<Utc>,
	new: NewInvite,
) -> Result<access_invite::Model, AccessError>
where
	C: TransactionTrait,
{
	let label = clean_text(&new.label, MAX_LABEL_CHARS);
	if label.is_empty() {
		return Err(AccessError::Invalid("Give the device a name"));
	}
	if !(1..=MAX_INVITE_HOURS).contains(&new.valid_hours) {
		return Err(AccessError::Invalid("That validity is not allowed"));
	}
	if !(1..=MAX_INVITE_USES).contains(&new.max_uses) {
		return Err(AccessError::Invalid("That number of uses is not allowed"));
	}

	let txn = conn.begin().await?;

	let person = user::Entity::find_by_id(new.person_id.clone())
		.filter(user::Column::DeletedAt.is_null())
		.one(&txn)
		.await?
		.filter(|person| !person.is_server_owner)
		.ok_or(AccessError::PersonNotAllowed)?;

	// a device link for an existing account also lets a friend-class account in from the internet
	// (the owner's decision, made by making the link); an account that holds more than the friend
	// set can't be, and the People table says why
	if access_policy::is_friend_class(person.permissions.as_deref())
		&& !person.hashed_password.is_empty()
	{
		crate::services::access_people::enable_public_signin(&txn, now, &person.id).await?;
	}

	let invite = access_invite::ActiveModel {
		id: Set(Uuid::new_v4().to_string()),
		token_hash: Set(new.token_hash),
		person_id: Set(person.id.clone()),
		label: Set(label.clone()),
		created_at: Set(now),
		created_by: Set(clean_text(&new.created_by, 64)),
		expires_at: Set(now + Duration::hours(new.valid_hours)),
		max_uses: Set(new.max_uses),
		uses: Set(0),
		revoked_at: Set(None),
	}
	.insert(&txn)
	.await?;

	audit(
		&txn,
		now,
		&invite.created_by,
		"invite.create",
		&invite.id,
		&format!("for {} ({label})", person.username),
	)
	.await?;

	txn.commit().await?;
	Ok(invite)
}

/// A page of invites, the latest first, and how many there are
pub async fn list_invites<C>(
	conn: &C,
	page: u64,
	page_size: u64,
) -> Result<(Vec<access_invite::Model>, u64), DbErr>
where
	C: ConnectionTrait,
{
	let paginator = access_invite::Entity::find()
		.order_by_desc(access_invite::Column::CreatedAt)
		.paginate(conn, std::cmp::max(page_size, 1));
	let total = paginator.num_items().await?;
	let rows = paginator.fetch_page(page.saturating_sub(1)).await?;
	Ok((rows, total))
}

/// How many invites can still be redeemed at `now`
pub async fn count_open_invites<C>(conn: &C, now: DateTime<Utc>) -> Result<u64, DbErr>
where
	C: ConnectionTrait,
{
	let candidates = access_invite::Entity::find()
		.filter(access_invite::Column::RevokedAt.is_null())
		.filter(access_invite::Column::ExpiresAt.gt(now))
		.all(conn)
		.await?;
	Ok(candidates
		.iter()
		.filter(|invite| invite.uses < invite.max_uses)
		.count() as u64)
}

/// Revokes an invite that is not revoked yet. Returns whether it changed
pub async fn revoke_invite<C>(
	conn: &C,
	now: DateTime<Utc>,
	id: &str,
	actor: &str,
) -> Result<bool, DbErr>
where
	C: TransactionTrait,
{
	let txn = conn.begin().await?;
	let changed = access_invite::Entity::update_many()
		.col_expr(access_invite::Column::RevokedAt, Expr::value(now))
		.filter(access_invite::Column::Id.eq(id))
		.filter(access_invite::Column::RevokedAt.is_null())
		.exec(&txn)
		.await?
		.rows_affected
		> 0;
	if changed {
		audit(&txn, now, actor, "invite.revoke", id, "").await?;
	}
	txn.commit().await?;
	Ok(changed)
}

#[derive(Debug)]
pub enum RedeemError {
	/// Unknown, expired, used up, revoked, or its person is gone, locked or the owner: one
	/// answer for all, so nothing can be learnt from it
	Invalid,
	Db(DbErr),
}

impl From<DbErr> for RedeemError {
	fn from(error: DbErr) -> Self {
		Self::Db(error)
	}
}

pub struct Redeemed {
	pub device: access_device::Model,
	/// `true` when this browser already held the pass of this invite and nothing was spent
	pub reused: bool,
}

/// Redeems an invite: checks the person, spends one use (a single conditional update, so ten
/// parallel redemptions of a three-use invite give exactly three) and makes the device, in one
/// transaction.
///
/// `current_device` is the id of the pass this browser already holds, when it holds a valid one: a
/// second tap with the pass that this very invite made gives the same device back and spends
/// nothing. A revoked device is no device here: a new invite replaces it
pub async fn redeem_invite<C>(
	conn: &C,
	now: DateTime<Utc>,
	token_hash: &str,
	current_device: Option<&str>,
	ip: &str,
	agent: &str,
) -> Result<Redeemed, RedeemError>
where
	C: TransactionTrait,
{
	let txn = conn.begin().await?;

	let invite = access_invite::Entity::find()
		.filter(access_invite::Column::TokenHash.eq(token_hash))
		.one(&txn)
		.await?
		.ok_or(RedeemError::Invalid)?;

	if let Some(current_device) = current_device {
		let same_device = access_device::Entity::find_by_id(current_device.to_string())
			.one(&txn)
			.await?
			.filter(|device| {
				device.invite_id.as_deref() == Some(invite.id.as_str())
					&& device.revoked_at.is_none()
			});
		if let (Some(device), None) = (same_device, invite.revoked_at) {
			return Ok(Redeemed {
				device,
				reused: true,
			});
		}
	}

	// the person first: a use is never spent on somebody who can't sign in anyway
	let person = user::Entity::find_by_id(invite.person_id.clone())
		.one(&txn)
		.await?
		.filter(|person| {
			person.deleted_at.is_none() && !person.is_server_owner && !person.is_locked
		})
		.ok_or(RedeemError::Invalid)?;

	let spent = access_invite::Entity::update_many()
		.col_expr(
			access_invite::Column::Uses,
			Expr::col(access_invite::Column::Uses).add(1),
		)
		.filter(access_invite::Column::Id.eq(invite.id.clone()))
		.filter(access_invite::Column::RevokedAt.is_null())
		.filter(access_invite::Column::ExpiresAt.gt(now))
		.filter(
			Expr::col(access_invite::Column::Uses)
				.lt(Expr::col(access_invite::Column::MaxUses)),
		)
		.exec(&txn)
		.await?
		.rows_affected;
	if spent != 1 {
		return Err(RedeemError::Invalid);
	}

	let mut device_id = new_device_id(&invite.label);
	for _ in 0..8 {
		if access_device::Entity::find_by_id(device_id.clone())
			.one(&txn)
			.await?
			.is_none()
		{
			break;
		}
		device_id = new_device_id(&invite.label);
	}

	let device = access_device::ActiveModel {
		id: Set(device_id),
		invite_id: Set(Some(invite.id.clone())),
		person_id: Set(Some(person.id.clone())),
		label: Set(invite.label.clone()),
		created_at: Set(now),
		first_seen_at: Set(Some(now)),
		last_seen_at: Set(Some(now)),
		last_ip: Set(clean_text(ip, 64)),
		last_agent: Set(clean_text(agent, MAX_AGENT_CHARS)),
		revoked_at: Set(None),
		revoked_by: Set(None),
		budget_reset_at: Set(0),
		kind: Set(device_kind::BROWSER.to_string()),
	}
	.insert(&txn)
	.await?;

	audit(
		&txn,
		now,
		"public",
		"invite.redeemed",
		&device.id,
		&format!("invite {} for {}", invite.id, person.username),
	)
	.await?;

	txn.commit().await?;
	Ok(Redeemed {
		device,
		reused: false,
	})
}

// -------------------------------------------------------------------------------------------
// devices
// -------------------------------------------------------------------------------------------

pub async fn find_device<C>(
	conn: &C,
	id: &str,
) -> Result<Option<access_device::Model>, DbErr>
where
	C: ConnectionTrait,
{
	access_device::Entity::find_by_id(id.to_string())
		.one(conn)
		.await
}

/// Registers a pass the app has never seen (one the terminal script made, or one the old edge
/// minted before this feature): a *legacy* device, bound to no one, named after its id
pub async fn register_legacy_device<C>(
	conn: &C,
	now: DateTime<Utc>,
	id: &str,
) -> Result<access_device::Model, DbErr>
where
	C: ConnectionTrait,
{
	if let Some(existing) = find_device(conn, id).await? {
		return Ok(existing);
	}

	let inserted = access_device::ActiveModel {
		id: Set(id.to_string()),
		invite_id: Set(None),
		person_id: Set(None),
		label: Set(id.to_string()),
		created_at: Set(now),
		first_seen_at: Set(Some(now)),
		last_seen_at: Set(Some(now)),
		last_ip: Set(String::new()),
		last_agent: Set(String::new()),
		revoked_at: Set(None),
		revoked_by: Set(None),
		budget_reset_at: Set(0),
		kind: Set(device_kind::BROWSER.to_string()),
	}
	.insert(conn)
	.await;

	match inserted {
		Ok(device) => {
			audit(conn, now, "public", "device.registered_legacy", id, "").await?;
			Ok(device)
		},
		// two requests with the same new pass at once: the other one won
		Err(error) => find_device(conn, id).await?.ok_or(error),
	}
}

/// Notes that a device was seen: when, from where, with what. It changes nothing else (a revoke
/// that lands at the same moment is kept)
pub async fn touch_device<C>(
	conn: &C,
	now: DateTime<Utc>,
	id: &str,
	ip: &str,
	agent: &str,
) -> Result<(), DbErr>
where
	C: ConnectionTrait,
{
	let Some(device) = find_device(conn, id).await? else {
		return Ok(());
	};
	access_device::ActiveModel {
		id: Unchanged(id.to_string()),
		first_seen_at: Set(device.first_seen_at.or(Some(now))),
		last_seen_at: Set(Some(now)),
		last_ip: Set(clean_text(ip, 64)),
		last_agent: Set(clean_text(agent, MAX_AGENT_CHARS)),
		..Default::default()
	}
	.update(conn)
	.await?;
	Ok(())
}

/// A page of devices, the latest made first, and how many there are
pub async fn list_devices<C>(
	conn: &C,
	page: u64,
	page_size: u64,
) -> Result<(Vec<access_device::Model>, u64), DbErr>
where
	C: ConnectionTrait,
{
	let paginator = access_device::Entity::find()
		.order_by_desc(access_device::Column::CreatedAt)
		.paginate(conn, std::cmp::max(page_size, 1));
	let total = paginator.num_items().await?;
	let rows = paginator.fetch_page(page.saturating_sub(1)).await?;
	Ok((rows, total))
}

pub async fn count_active_devices<C>(conn: &C) -> Result<u64, DbErr>
where
	C: ConnectionTrait,
{
	access_device::Entity::find()
		.filter(access_device::Column::RevokedAt.is_null())
		.count(conn)
		.await
}

/// Revokes a device that is not revoked, and the invite it came from while that is still open (a
/// stolen phone must not come back through its own link). Returns whether it changed
pub async fn revoke_device<C>(
	conn: &C,
	now: DateTime<Utc>,
	id: &str,
	actor: &str,
) -> Result<bool, DbErr>
where
	C: TransactionTrait,
{
	revoke_device_as(conn, now, id, actor, "device.revoke").await
}

async fn revoke_device_as<C>(
	conn: &C,
	now: DateTime<Utc>,
	id: &str,
	actor: &str,
	action: &str,
) -> Result<bool, DbErr>
where
	C: TransactionTrait,
{
	let txn = conn.begin().await?;
	let Some(device) = find_device(&txn, id).await? else {
		return Ok(false);
	};
	let changed = access_device::Entity::update_many()
		.col_expr(access_device::Column::RevokedAt, Expr::value(now))
		.col_expr(
			access_device::Column::RevokedBy,
			Expr::value(clean_text(actor, 64)),
		)
		.filter(access_device::Column::Id.eq(id))
		.filter(access_device::Column::RevokedAt.is_null())
		.exec(&txn)
		.await?
		.rows_affected
		> 0;
	if changed {
		if let Some(invite_id) = device.invite_id {
			access_invite::Entity::update_many()
				.col_expr(access_invite::Column::RevokedAt, Expr::value(now))
				.filter(access_invite::Column::Id.eq(invite_id))
				.filter(access_invite::Column::RevokedAt.is_null())
				.exec(&txn)
				.await?;
		}
		audit(&txn, now, actor, action, id, &device.label).await?;
	}
	txn.commit().await?;
	Ok(changed)
}

/// Brings a revoked device back. Its invite stays revoked. Returns whether it changed
pub async fn restore_device<C>(
	conn: &C,
	now: DateTime<Utc>,
	id: &str,
	actor: &str,
) -> Result<bool, DbErr>
where
	C: TransactionTrait,
{
	let txn = conn.begin().await?;
	let changed = access_device::Entity::update_many()
		.col_expr(
			access_device::Column::RevokedAt,
			Expr::value(Option::<DateTime<Utc>>::None),
		)
		.col_expr(
			access_device::Column::RevokedBy,
			Expr::value(Option::<String>::None),
		)
		.filter(access_device::Column::Id.eq(id))
		.filter(access_device::Column::RevokedAt.is_not_null())
		.exec(&txn)
		.await?
		.rows_affected
		> 0;
	if changed {
		audit(&txn, now, actor, "device.restore", id, "").await?;
	}
	txn.commit().await?;
	Ok(changed)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RevokedAll {
	pub devices: u64,
	pub invites: u64,
	pub sessions: u64,
}

/// Cuts a person off: every device of theirs is revoked, every open invite of theirs too, and
/// their sessions and refresh tokens are removed
pub async fn revoke_devices_of_person<C>(
	conn: &C,
	now: DateTime<Utc>,
	person_id: &str,
	actor: &str,
) -> Result<RevokedAll, DbErr>
where
	C: TransactionTrait,
{
	let txn = conn.begin().await?;
	let devices = access_device::Entity::update_many()
		.col_expr(access_device::Column::RevokedAt, Expr::value(now))
		.col_expr(
			access_device::Column::RevokedBy,
			Expr::value(clean_text(actor, 64)),
		)
		.filter(access_device::Column::PersonId.eq(person_id))
		.filter(access_device::Column::RevokedAt.is_null())
		.exec(&txn)
		.await?
		.rows_affected;
	let invites = access_invite::Entity::update_many()
		.col_expr(access_invite::Column::RevokedAt, Expr::value(now))
		.filter(access_invite::Column::PersonId.eq(person_id))
		.filter(access_invite::Column::RevokedAt.is_null())
		.exec(&txn)
		.await?
		.rows_affected;
	let sessions = revoke_user_sessions(&txn, person_id, None).await?.len() as u64;
	audit(
		&txn,
		now,
		actor,
		"person.revoke_all",
		person_id,
		&format!("{devices} devices, {invites} invites, {sessions} sessions"),
	)
	.await?;
	txn.commit().await?;
	Ok(RevokedAll {
		devices,
		invites,
		sessions,
	})
}

/// Gives a device a fresh daily sign-in budget at the edge. The epoch only ever grows, so two
/// resets in one second still make a new count. Returns whether the device exists
pub async fn reset_device_budget<C>(
	conn: &C,
	now: DateTime<Utc>,
	id: &str,
	actor: &str,
) -> Result<bool, DbErr>
where
	C: TransactionTrait,
{
	let txn = conn.begin().await?;
	let Some(device) = find_device(&txn, id).await? else {
		return Ok(false);
	};
	let epoch = std::cmp::max(now.timestamp(), device.budget_reset_at + 1);
	access_device::ActiveModel {
		id: Unchanged(id.to_string()),
		budget_reset_at: Set(epoch),
		..Default::default()
	}
	.update(&txn)
	.await?;
	audit(&txn, now, actor, "device.reset_budget", id, "").await?;
	txn.commit().await?;
	Ok(true)
}

// -------------------------------------------------------------------------------------------
// a person's devices: the limit, self-service, reading-app keys
// -------------------------------------------------------------------------------------------

/// A device that has been seen (or made) since this moment counts against its person's limit
pub fn dormant_cutoff(now: DateTime<Utc>) -> DateTime<Utc> {
	now - Duration::days(DORMANT_AFTER_DAYS)
}

/// The rows that count against a limit: of this person, not revoked, seen (or, never seen, made)
/// within the last [`DORMANT_AFTER_DAYS`] days, of either kind
fn counts_for(person_id: &str, now: DateTime<Utc>) -> Condition {
	let cutoff = dormant_cutoff(now);
	Condition::all()
		.add(access_device::Column::PersonId.eq(person_id))
		.add(access_device::Column::RevokedAt.is_null())
		.add(
			Condition::any()
				.add(access_device::Column::LastSeenAt.gt(cutoff))
				.add(
					Condition::all()
						.add(access_device::Column::LastSeenAt.is_null())
						.add(access_device::Column::CreatedAt.gt(cutoff)),
				),
		)
}

/// How many devices of the person count against their limit at `now`
pub async fn count_person_devices<C>(
	conn: &C,
	now: DateTime<Utc>,
	person_id: &str,
) -> Result<u64, DbErr>
where
	C: ConnectionTrait,
{
	access_device::Entity::find()
		.filter(counts_for(person_id, now))
		.count(conn)
		.await
}

/// The person's limit: their row's, or the default for somebody with no row
pub async fn max_devices_of<C>(conn: &C, person_id: &str) -> Result<i32, DbErr>
where
	C: ConnectionTrait,
{
	Ok(access_person::Entity::find_by_id(person_id.to_string())
		.one(conn)
		.await?
		.map_or(access_policy::DEFAULT_MAX_DEVICES, |row| row.max_devices))
}

/// A device to make: what [`insert_device_within_limit`] needs
pub struct NewDevice<'a> {
	pub id: String,
	pub invite_id: Option<&'a str>,
	pub person_id: &'a str,
	pub label: &'a str,
	pub kind: &'a str,
	pub ip: &'a str,
	pub agent: &'a str,
}

/// A device id nobody has used (a revoked device's id is never reused: a revoked reading-app key
/// would come back to life under the same id)
pub async fn unused_device_id<C>(conn: &C, label: &str) -> Result<String, DbErr>
where
	C: ConnectionTrait,
{
	let mut id = new_device_id(label);
	for _ in 0..8 {
		if find_device(conn, &id).await?.is_none() {
			return Ok(id);
		}
		id = new_device_id(label);
	}
	Ok(id)
}

/// Makes a device for a person **only if they have fewer than `limit` devices that count**, as
/// one statement: ten sign-ins at once at two of three make exactly one device. `None` is "at the
/// limit"
pub async fn insert_device_within_limit<C>(
	conn: &C,
	now: DateTime<Utc>,
	device: NewDevice<'_>,
	limit: i32,
) -> Result<Option<access_device::Model>, DbErr>
where
	C: ConnectionTrait,
{
	let cutoff = dormant_cutoff(now);
	let sql = "INSERT INTO access_devices \
		(id, invite_id, person_id, label, created_at, first_seen_at, last_seen_at, last_ip, last_agent, revoked_at, revoked_by, budget_reset_at, kind) \
		SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, 0, ? \
		WHERE (SELECT COUNT(*) FROM access_devices \
			WHERE person_id = ? AND revoked_at IS NULL \
			AND COALESCE(last_seen_at, created_at) > ?) < ?";
	let values: Vec<Value> = vec![
		device.id.clone().into(),
		device.invite_id.map(str::to_string).into(),
		device.person_id.to_string().into(),
		clean_text(device.label, MAX_LABEL_CHARS).into(),
		now.into(),
		now.into(),
		now.into(),
		clean_text(device.ip, 64).into(),
		clean_text(device.agent, MAX_AGENT_CHARS).into(),
		device.kind.to_string().into(),
		device.person_id.to_string().into(),
		cutoff.into(),
		limit.into(),
	];
	let inserted = conn
		.execute_raw(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			sql,
			values,
		))
		.await?
		.rows_affected();
	if inserted == 0 {
		return Ok(None);
	}
	find_device(conn, &device.id).await
}

/// The person's devices that are not revoked, the oldest first: what they see in *My devices*
pub async fn devices_of_person<C>(
	conn: &C,
	person_id: &str,
) -> Result<Vec<access_device::Model>, DbErr>
where
	C: ConnectionTrait,
{
	access_device::Entity::find()
		.filter(access_device::Column::PersonId.eq(person_id))
		.filter(access_device::Column::RevokedAt.is_null())
		.order_by_asc(access_device::Column::CreatedAt)
		.all(conn)
		.await
}

/// Renames one of the person's own devices. `None` when it isn't theirs (or is revoked): a
/// person only ever reaches their own
pub async fn rename_device_of_person<C>(
	conn: &C,
	person_id: &str,
	id: &str,
	label: &str,
) -> Result<Option<access_device::Model>, AccessError>
where
	C: ConnectionTrait,
{
	let label = clean_text(label, MAX_LABEL_CHARS);
	if label.is_empty() {
		return Err(AccessError::Invalid("Give the device a name"));
	}
	let changed = access_device::Entity::update_many()
		.col_expr(access_device::Column::Label, Expr::value(label))
		.filter(access_device::Column::Id.eq(id))
		.filter(access_device::Column::PersonId.eq(person_id))
		.filter(access_device::Column::RevokedAt.is_null())
		.exec(conn)
		.await?
		.rows_affected;
	if changed == 0 {
		return Ok(None);
	}
	Ok(find_device(conn, id).await?)
}

/// A person removes one of their own devices: it is revoked (so the pass or key stops working at
/// once for the app and within seconds at the edge). `None` when it isn't theirs
pub async fn remove_device_of_person<C>(
	conn: &C,
	now: DateTime<Utc>,
	person_id: &str,
	username: &str,
	id: &str,
) -> Result<Option<access_device::Model>, DbErr>
where
	C: TransactionTrait + ConnectionTrait,
{
	let Some(device) = find_device(conn, id)
		.await?
		.filter(|device| device.person_id.as_deref() == Some(person_id))
	else {
		return Ok(None);
	};
	revoke_device_as(conn, now, id, username, "device.removed_by_person").await?;
	Ok(Some(device))
}

/// A person's reading-app keys that are not revoked
pub async fn reading_apps_of_person<C>(
	conn: &C,
	person_id: &str,
) -> Result<Vec<access_device::Model>, DbErr>
where
	C: ConnectionTrait,
{
	access_device::Entity::find()
		.filter(access_device::Column::PersonId.eq(person_id))
		.filter(access_device::Column::Kind.eq(device_kind::OPDS))
		.filter(access_device::Column::RevokedAt.is_null())
		.order_by_asc(access_device::Column::CreatedAt)
		.all(conn)
		.await
}

/// Makes a reading-app key for a person: a device of kind `opds`, within their limit. `None` is
/// "at the limit". With `replacing` the old key is revoked first, in the same transaction, so a
/// person at the limit can replace a key
pub async fn create_reading_app<C>(
	conn: &C,
	now: DateTime<Utc>,
	person_id: &str,
	username: &str,
	label: &str,
	replacing: Option<&str>,
) -> Result<Option<access_device::Model>, AccessError>
where
	C: TransactionTrait,
{
	let label = clean_text(label, MAX_LABEL_CHARS);
	if label.is_empty() {
		return Err(AccessError::Invalid("Give the reading app a name"));
	}
	let txn = conn.begin().await?;
	if let Some(old) = replacing {
		let is_theirs = find_device(&txn, old).await?.is_some_and(|device| {
			device.person_id.as_deref() == Some(person_id)
				&& device.kind == device_kind::OPDS
		});
		if !is_theirs {
			return Ok(None);
		}
		revoke_device_as(&txn, now, old, username, "opds.key_replaced").await?;
	}
	let limit = max_devices_of(&txn, person_id).await?;
	let id = unused_device_id(&txn, &label).await?;
	let created = insert_device_within_limit(
		&txn,
		now,
		NewDevice {
			id,
			invite_id: None,
			person_id,
			label: &label,
			kind: device_kind::OPDS,
			ip: "",
			agent: "",
		},
		limit,
	)
	.await?;
	if let Some(device) = &created {
		audit(&txn, now, username, "opds.key_created", &device.id, &device.label).await?;
		txn.commit().await?;
	}
	Ok(created)
}

// -------------------------------------------------------------------------------------------
// what the edge follows
// -------------------------------------------------------------------------------------------

/// What the edge needs to know, and nothing else: ids and numbers
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EdgeState {
	/// The ids of the devices that are revoked
	pub revoked: Vec<String>,
	/// device id → the epoch (seconds) of its last budget reset
	pub budget: BTreeMap<String, i64>,
}

pub async fn edge_state<C>(conn: &C) -> Result<EdgeState, DbErr>
where
	C: ConnectionTrait,
{
	let mut revoked: Vec<String> = access_device::Entity::find()
		.filter(access_device::Column::RevokedAt.is_not_null())
		.select_only()
		.column(access_device::Column::Id)
		.into_tuple::<String>()
		.all(conn)
		.await?;
	revoked.sort();

	let budget = access_device::Entity::find()
		.filter(access_device::Column::BudgetResetAt.gt(0))
		.select_only()
		.column(access_device::Column::Id)
		.column(access_device::Column::BudgetResetAt)
		.into_tuple::<(String, i64)>()
		.all(conn)
		.await?
		.into_iter()
		.collect();

	Ok(EdgeState { revoked, budget })
}

/// Deletes what is old enough to forget: the audit log, revoked devices (a pass lives 180 days,
/// so a revocation is kept 200), and invites that expired or were revoked a month ago (unless
/// a device that is still alive came from them)
pub async fn prune<C>(conn: &C, now: DateTime<Utc>) -> Result<u64, DbErr>
where
	C: ConnectionTrait,
{
	let mut removed = 0;

	removed += access_audit::Entity::delete_many()
		.filter(access_audit::Column::At.lt(now - Duration::days(AUDIT_KEEP_DAYS)))
		.exec(conn)
		.await?
		.rows_affected;

	// a revoked reading-app key is **never** forgotten: it has no expiry, so once its id left the
	// state file the edge would accept it again, and a new device that happened to get the same
	// id would bring it back to life in the app. (A pass is different: it expires on its own)
	removed += access_device::Entity::delete_many()
		.filter(
			access_device::Column::RevokedAt
				.lt(now - Duration::days(REVOKED_DEVICE_KEEP_DAYS)),
		)
		.filter(access_device::Column::Kind.ne(device_kind::OPDS))
		.exec(conn)
		.await?
		.rows_affected;

	// signup links that expired, were used up or were revoked a month ago (the accounts they made
	// forget them: the foreign key sets their link to nothing)
	let old_links = now - Duration::days(INVITE_KEEP_DAYS);
	removed += access_signup_link::Entity::delete_many()
		.filter(
			Condition::any()
				.add(access_signup_link::Column::ExpiresAt.lt(old_links))
				.add(access_signup_link::Column::RevokedAt.lt(old_links)),
		)
		.exec(conn)
		.await?
		.rows_affected;

	let live_devices_invites = Query::select()
		.column(access_device::Column::InviteId)
		.from(access_device::Entity)
		.and_where(access_device::Column::InviteId.is_not_null())
		.and_where(access_device::Column::RevokedAt.is_null())
		.to_owned();
	let old = now - Duration::days(INVITE_KEEP_DAYS);
	removed += access_invite::Entity::delete_many()
		.filter(
			sea_orm::Condition::any()
				.add(access_invite::Column::ExpiresAt.lt(old))
				.add(access_invite::Column::RevokedAt.lt(old)),
		)
		.filter(access_invite::Column::Id.not_in_subquery(live_devices_invites))
		.exec(conn)
		.await?
		.rows_affected;

	Ok(removed)
}

// -------------------------------------------------------------------------------------------
// the sign-in log, the lock rule and the unblock
// -------------------------------------------------------------------------------------------

/// What is recorded of a sign-in beyond `user_login_activity`'s own columns
#[derive(Debug, Clone)]
pub struct SignInMark {
	/// The pass id of a sign-in through the public edge
	pub device_id: Option<String>,
	pub reason: Option<&'static str>,
	/// Whether the lock rule counts it (when it is a failure)
	pub counts_toward_lock: bool,
}

impl SignInMark {
	/// A sign-in the lock rule counts, as it always has
	pub fn counting(device_id: Option<String>, reason: Option<&'static str>) -> Self {
		Self {
			device_id,
			reason,
			counts_toward_lock: true,
		}
	}

	/// A failure that was not a guess at the account's password (somebody else's pass, the owner
	/// through the edge): recorded, never counted
	pub fn ignored_by_lock(device_id: Option<String>, reason: &'static str) -> Self {
		Self {
			device_id,
			reason: Some(reason),
			counts_toward_lock: false,
		}
	}
}

/// Records the mark of a sign-in. An attempt with no pass, no reason and the usual counting needs
/// none: nothing is written for it
pub async fn mark_sign_in<C>(
	conn: &C,
	login_activity_id: i64,
	mark: &SignInMark,
) -> Result<(), DbErr>
where
	C: ConnectionTrait,
{
	if mark.device_id.is_none() && mark.reason.is_none() && mark.counts_toward_lock {
		return Ok(());
	}
	access_signin_mark::ActiveModel {
		login_activity_id: Set(login_activity_id),
		device_id: Set(mark.device_id.clone()),
		failure_reason: Set(mark.reason.map(str::to_string)),
		counts_toward_lock: Set(mark.counts_toward_lock),
	}
	.insert(conn)
	.await?;
	Ok(())
}

/// The failed sign-ins of a user in `[since, until]` that the lock rule counts: every failure
/// but the ones marked as not counting
pub async fn count_lock_failures<C>(
	conn: &C,
	user_id: &str,
	since: DateTimeWithTimeZone,
	until: DateTimeWithTimeZone,
) -> Result<u64, DbErr>
where
	C: ConnectionTrait,
{
	let ignored = Query::select()
		.column(access_signin_mark::Column::LoginActivityId)
		.from(access_signin_mark::Entity)
		.and_where(access_signin_mark::Column::CountsTowardLock.eq(false))
		.to_owned();

	user_login_activity::Entity::find()
		.filter(user_login_activity::Column::UserId.eq(user_id))
		.filter(user_login_activity::Column::Timestamp.gte(since))
		.filter(user_login_activity::Column::Timestamp.lte(until))
		.filter(user_login_activity::Column::AuthenticationSuccessful.eq(false))
		.filter(user_login_activity::Column::Id.not_in_subquery(ignored))
		.count(conn)
		.await
}

/// Unlocks an account **and forgives** the failures of the last 24 hours, in one transaction: the
/// lock rule counts every failure of that window, so an unlock that only cleared the flag would
/// lock the account again on its next typo. The failures stay in the sign-in log, marked as not
/// counting. Returns whether the user exists
pub async fn unblock_user<C>(
	conn: &C,
	now: DateTime<Utc>,
	user_id: &str,
	actor: &str,
) -> Result<bool, DbErr>
where
	C: TransactionTrait,
{
	let txn = conn.begin().await?;

	let found = user::Entity::update_many()
		.col_expr(user::Column::IsLocked, Expr::value(false))
		.filter(user::Column::Id.eq(user_id))
		.exec(&txn)
		.await?
		.rows_affected
		> 0;
	if !found {
		return Ok(false);
	}

	let since = DateTimeWithTimeZone::from(now - Duration::hours(LOCK_WINDOW_HOURS));
	let failed: Vec<i64> = user_login_activity::Entity::find()
		.filter(user_login_activity::Column::UserId.eq(user_id))
		.filter(user_login_activity::Column::AuthenticationSuccessful.eq(false))
		.filter(user_login_activity::Column::Timestamp.gte(since))
		.select_only()
		.column(user_login_activity::Column::Id)
		.into_tuple::<i64>()
		.all(&txn)
		.await?;

	for chunk in failed.chunks(400) {
		let chunk = chunk.to_vec();
		access_signin_mark::Entity::update_many()
			.col_expr(
				access_signin_mark::Column::CountsTowardLock,
				Expr::value(false),
			)
			.filter(access_signin_mark::Column::LoginActivityId.is_in(chunk.clone()))
			.exec(&txn)
			.await?;

		let marked: Vec<i64> = access_signin_mark::Entity::find()
			.filter(access_signin_mark::Column::LoginActivityId.is_in(chunk.clone()))
			.select_only()
			.column(access_signin_mark::Column::LoginActivityId)
			.into_tuple::<i64>()
			.all(&txn)
			.await?;
		let unmarked: Vec<access_signin_mark::ActiveModel> = chunk
			.into_iter()
			.filter(|id| !marked.contains(id))
			.map(|id| access_signin_mark::ActiveModel {
				login_activity_id: Set(id),
				device_id: Set(None),
				failure_reason: Set(None),
				counts_toward_lock: Set(false),
			})
			.collect();
		if !unmarked.is_empty() {
			access_signin_mark::Entity::insert_many(unmarked)
				.exec(&txn)
				.await?;
		}
	}

	audit(&txn, now, actor, "user.unblock", user_id, "").await?;
	txn.commit().await?;
	Ok(true)
}

pub struct SignInRow {
	pub activity: user_login_activity::Model,
	pub mark: Option<access_signin_mark::Model>,
	/// The name the owner gave the device the sign-in came with
	pub device_label: Option<String>,
}

/// A page of sign-ins, the latest first, with what is known of each, and how many there are
pub async fn list_sign_ins<C>(
	conn: &C,
	failures_only: bool,
	page: u64,
	page_size: u64,
) -> Result<(Vec<SignInRow>, u64), DbErr>
where
	C: ConnectionTrait,
{
	let mut select = user_login_activity::Entity::find();
	if failures_only {
		select = select
			.filter(user_login_activity::Column::AuthenticationSuccessful.eq(false));
	}
	let paginator = select
		.order_by_desc(user_login_activity::Column::Timestamp)
		.order_by_desc(user_login_activity::Column::Id)
		.paginate(conn, std::cmp::max(page_size, 1));
	let total = paginator.num_items().await?;
	let activities = paginator.fetch_page(page.saturating_sub(1)).await?;

	let ids: Vec<i64> = activities.iter().map(|row| row.id).collect();
	let marks: HashMap<i64, access_signin_mark::Model> = if ids.is_empty() {
		HashMap::new()
	} else {
		access_signin_mark::Entity::find()
			.filter(access_signin_mark::Column::LoginActivityId.is_in(ids))
			.all(conn)
			.await?
			.into_iter()
			.map(|mark| (mark.login_activity_id, mark))
			.collect()
	};

	let device_ids: Vec<String> = marks
		.values()
		.filter_map(|mark| mark.device_id.clone())
		.collect();
	let labels: HashMap<String, String> = if device_ids.is_empty() {
		HashMap::new()
	} else {
		access_device::Entity::find()
			.filter(access_device::Column::Id.is_in(device_ids))
			.all(conn)
			.await?
			.into_iter()
			.map(|device| (device.id, device.label))
			.collect()
	};

	let rows = activities
		.into_iter()
		.map(|activity| {
			let mark = marks.get(&activity.id).cloned();
			let device_label = mark
				.as_ref()
				.and_then(|mark| mark.device_id.as_ref())
				.and_then(|device_id| labels.get(device_id).cloned());
			SignInRow {
				activity,
				mark,
				device_label,
			}
		})
		.collect();
	Ok((rows, total))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockedBy {
	/// The lock rule: too many failed sign-ins
	Failures,
	/// The owner locked the account
	Owner,
	/// Locked before this was recorded
	Unknown,
}

pub struct BlockedUser {
	pub user: user::Model,
	pub failures_last_24h: u64,
	pub last_failure_at: Option<DateTimeWithTimeZone>,
	pub locked_by: LockedBy,
}

/// The accounts that are locked right now, with their failures of the last 24 hours and who
/// locked them
pub async fn blocked_users<C>(
	conn: &C,
	now: DateTime<Utc>,
) -> Result<Vec<BlockedUser>, DbErr>
where
	C: ConnectionTrait,
{
	let locked = user::Entity::find()
		.filter(user::Column::IsLocked.eq(true))
		.filter(user::Column::DeletedAt.is_null())
		.order_by_asc(user::Column::Username)
		.all(conn)
		.await?;

	let since = DateTimeWithTimeZone::from(now - Duration::hours(LOCK_WINDOW_HOURS));
	let mut blocked = Vec::with_capacity(locked.len());
	for user in locked {
		let failures = user_login_activity::Entity::find()
			.filter(user_login_activity::Column::UserId.eq(user.id.clone()))
			.filter(user_login_activity::Column::AuthenticationSuccessful.eq(false))
			.filter(user_login_activity::Column::Timestamp.gte(since))
			.order_by_desc(user_login_activity::Column::Timestamp)
			.all(conn)
			.await?;

		let last_lock = access_audit::Entity::find()
			.filter(access_audit::Column::Target.eq(user.id.clone()))
			.filter(access_audit::Column::Action.is_in([
				"user.lock",
				"user.locked_by_failures",
				"user.unblock",
			]))
			.order_by_desc(access_audit::Column::Id)
			.one(conn)
			.await?;
		let locked_by = match last_lock.as_ref().map(|row| row.action.as_str()) {
			Some("user.locked_by_failures") => LockedBy::Failures,
			Some("user.lock") => LockedBy::Owner,
			_ => LockedBy::Unknown,
		};

		blocked.push(BlockedUser {
			failures_last_24h: failures.len() as u64,
			last_failure_at: failures.first().map(|row| row.timestamp),
			user,
			locked_by,
		});
	}
	Ok(blocked)
}

// -------------------------------------------------------------------------------------------
// people, and the password the owner set
// -------------------------------------------------------------------------------------------

pub struct Person {
	pub user: user::Model,
	pub last_sign_in_at: Option<DateTimeWithTimeZone>,
	pub must_change_password: bool,
}

/// The accounts the owner can manage access for: everyone who is not removed, the owner included
/// (their row says so), with their last successful sign-in
pub async fn people<C>(conn: &C) -> Result<Vec<Person>, DbErr>
where
	C: ConnectionTrait,
{
	let users = user::Entity::find()
		.filter(user::Column::DeletedAt.is_null())
		.order_by_asc(user::Column::Username)
		.all(conn)
		.await?;
	let must_change: Vec<String> = access_password_reset::Entity::find()
		.select_only()
		.column(access_password_reset::Column::UserId)
		.into_tuple::<String>()
		.all(conn)
		.await?;

	let mut rows = Vec::with_capacity(users.len());
	for user in users {
		let last_sign_in_at = user_login_activity::Entity::find()
			.filter(user_login_activity::Column::UserId.eq(user.id.clone()))
			.filter(user_login_activity::Column::AuthenticationSuccessful.eq(true))
			.order_by_desc(user_login_activity::Column::Timestamp)
			.one(conn)
			.await?
			.map(|row| row.timestamp);
		rows.push(Person {
			must_change_password: must_change.contains(&user.id),
			last_sign_in_at,
			user,
		});
	}
	Ok(rows)
}

/// The user must choose their own password before anything else: the owner set it. Replaces an
/// earlier requirement
pub async fn require_password_change<C>(
	conn: &C,
	now: DateTime<Utc>,
	user_id: &str,
	set_by: &str,
) -> Result<(), DbErr>
where
	C: ConnectionTrait,
{
	access_password_reset::Entity::delete_by_id(user_id.to_string())
		.exec(conn)
		.await?;
	access_password_reset::ActiveModel {
		user_id: Set(user_id.to_string()),
		set_at: Set(now),
		set_by: Set(clean_text(set_by, 64)),
	}
	.insert(conn)
	.await?;
	Ok(())
}

/// The user chose their own password: the requirement is over. Returns whether there was one
pub async fn clear_password_change<C>(conn: &C, user_id: &str) -> Result<bool, DbErr>
where
	C: ConnectionTrait,
{
	Ok(
		access_password_reset::Entity::delete_by_id(user_id.to_string())
			.exec(conn)
			.await?
			.rows_affected
			> 0,
	)
}

/// Whether the user has to choose their own password before anything else
pub async fn must_change_password<C>(conn: &C, user_id: &str) -> Result<bool, DbErr>
where
	C: ConnectionTrait,
{
	Ok(
		access_password_reset::Entity::find_by_id(user_id.to_string())
			.one(conn)
			.await?
			.is_some(),
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_device_id_is_a_valid_pass_name() {
		for label in [
			"Ana's phone",
			"  ",
			"ÑÑÑ",
			"A very long name of a device that goes on and on",
			"phone",
			"-",
			"x",
		] {
			let id = new_device_id(label);
			assert!(is_valid_device_id(&id), "{label:?} -> {id}");
			assert!(id.len() <= 23, "{id}");
		}
		assert_eq!(device_slug("Ana's phone"), "ana-s-phone");
		assert_eq!(device_slug("  "), "device");
		assert_eq!(
			device_slug("A very long name of a device"),
			"a-very-long-name"
		);
	}

	#[test]
	fn two_device_ids_of_one_label_differ() {
		assert_ne!(new_device_id("phone"), new_device_id("phone"));
	}

	#[test]
	fn pass_names_are_checked() {
		assert!(is_valid_device_id("ana-phone-k3f9x"));
		assert!(is_valid_device_id("a"));
		assert!(!is_valid_device_id(""));
		assert!(!is_valid_device_id("Ana"));
		assert!(!is_valid_device_id("a.b"));
		assert!(!is_valid_device_id("a_b"));
		assert!(!is_valid_device_id(&"a".repeat(25)));
	}

	#[test]
	fn text_from_outside_is_made_safe() {
		assert_eq!(clean_text("  Ana\u{0}'s\nphone \u{7f}", 40), "Ana'sphone");
		assert_eq!(clean_text("abcdef", 3), "abc");
		assert_eq!(clean_text("ab  cd", 3), "ab");
		// a name can't carry a character that turns the text around, or hides in it
		assert_eq!(clean_text("\u{202e}x\u{200b}y\u{feff}", 10), "xy");
	}

	fn invite(
		uses: i32,
		max_uses: i32,
		expires_in_hours: i64,
		revoked: bool,
	) -> access_invite::Model {
		let now = Utc::now();
		access_invite::Model {
			id: "i".into(),
			token_hash: "h".into(),
			person_id: "p".into(),
			label: "l".into(),
			created_at: now,
			created_by: "o".into(),
			expires_at: now + Duration::hours(expires_in_hours),
			max_uses,
			uses,
			revoked_at: revoked.then_some(now),
		}
	}

	#[test]
	fn an_invite_has_one_status() {
		let now = Utc::now();
		assert_eq!(
			invite_status(&invite(0, 3, 1, false), now),
			InviteStatus::Unused
		);
		assert_eq!(
			invite_status(&invite(1, 3, 1, false), now),
			InviteStatus::Used
		);
		assert_eq!(
			invite_status(&invite(3, 3, 1, false), now),
			InviteStatus::UsedUp
		);
		assert_eq!(
			invite_status(&invite(0, 3, -1, false), now),
			InviteStatus::Expired
		);
		// revoked wins over everything
		assert_eq!(
			invite_status(&invite(3, 3, -1, true), now),
			InviteStatus::Revoked
		);
	}
}
