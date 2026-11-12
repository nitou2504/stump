//! Signup links: the owner makes a link, somebody opens it and **makes their own account**.
//!
//! - A link is for one person (`single`, one use) or for several (`open`, a budget of accounts),
//!   valid until it expires, and carries a *preset*: the permissions, the libraries, the age
//!   limit and the device limit the account gets. The preset is checked against the friend set
//!   twice: when the link is made, and again when it is used, so a tampered row (or a client that
//!   skips the owner's page) can never hand out more than a friend may have.
//! - Only the SHA-256 of the token is stored (the link is shown once).
//! - Using a link is **one transaction** that starts with the spend: one conditional `UPDATE`, so
//!   ten parallel uses of a three-use link make exactly three accounts. A name that is taken, or
//!   a preset whose libraries are all gone, rolls everything back: nothing is spent.
//! - The account is made with its preset's permissions, only the preset's libraries (every other
//!   library hidden, and every library decided: see `access_people`), a row of its own in
//!   `access_people` with *Public sign-in* on, and its first device.

use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};
use sea_orm::{
	prelude::*,
	sea_query::Expr,
	ActiveValue::Set,
	ColumnTrait, ConnectionTrait, DbErr, EntityTrait, ExprTrait, PaginatorTrait,
	QueryFilter, QueryOrder, QuerySelect, TransactionSession, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
	entity::{
		access_device, access_invite, access_person, access_signup_link, library, user,
	},
	services::{
		access::{
			audit, clean_text, device_kind, insert_device_within_limit,
			revoke_devices_of_person, unused_device_id, AccessError, NewDevice,
			MAX_AGENT_CHARS,
		},
		access_people::{ensure_exclusion, mark_libraries_decided},
		access_policy::{
			self, device_label, MAX_MAX_DEVICES, MIN_MAX_DEVICES,
		},
		user::{create_user_with_preferences, NewAgeRestriction},
	},
	shared::{enums::UserPermission, permission_set::PermissionSet},
};

/// The most characters of a link's note
pub const MAX_LABEL_CHARS: usize = 60;
/// A link is valid for 1 hour to 30 days
pub const MAX_VALID_HOURS: i64 = 720;
/// The budget of an open link
pub const MIN_OPEN_USES: i32 = 2;
pub const MAX_OPEN_USES: i32 = 25;
/// The most libraries a preset can name
const MAX_PRESET_LIBRARIES: usize = 200;
/// The highest age limit a preset can set
const MAX_AGE: i32 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignupKind {
	Single,
	Open,
}

impl SignupKind {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Single => "single",
			Self::Open => "open",
		}
	}

	pub fn parse(text: &str) -> Option<Self> {
		match text {
			"single" => Some(Self::Single),
			"open" => Some(Self::Open),
			_ => None,
		}
	}
}

/// What an account made through a link gets. Stored in the link as JSON
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
	pub permissions: Vec<UserPermission>,
	/// The libraries the account sees: every other library is hidden from it
	pub library_ids: Vec<String>,
	pub age_restriction: Option<PresetAge>,
	/// Devices the person may have
	pub max_devices: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetAge {
	pub age: i32,
	pub restrict_on_unset: bool,
}

/// Checks a preset: only friend-set permissions (so never *Upload files* or *Scan library*),
/// at least one library, a sane age limit and device limit. It does not look at the database
pub fn validate_preset(preset: &Preset) -> Result<(), AccessError> {
	let beyond = access_policy::outside_friend_set(&preset.permissions);
	if !beyond.is_empty() {
		return Err(AccessError::Message(format!(
			"A link can't give these permissions: {}",
			beyond
				.iter()
				.map(ToString::to_string)
				.collect::<Vec<_>>()
				.join(", ")
		)));
	}
	if preset.library_ids.is_empty() {
		return Err(AccessError::Invalid(
			"Choose at least one library: otherwise they would see nothing",
		));
	}
	if preset.library_ids.len() > MAX_PRESET_LIBRARIES {
		return Err(AccessError::Invalid("Too many libraries"));
	}
	if !(MIN_MAX_DEVICES..=MAX_MAX_DEVICES).contains(&preset.max_devices) {
		return Err(AccessError::Invalid("That number of devices is not allowed"));
	}
	if let Some(age) = preset.age_restriction {
		if !(1..=MAX_AGE).contains(&age.age) {
			return Err(AccessError::Invalid("That age limit is not allowed"));
		}
	}
	Ok(())
}

/// The preset of a stored link, when it parses and is still valid
pub fn preset_of(link: &access_signup_link::Model) -> Option<Preset> {
	let preset: Preset = serde_json::from_str(&link.preset).ok()?;
	validate_preset(&preset).ok()?;
	Some(preset)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignupStatus {
	Active,
	UsedUp,
	Expired,
	Revoked,
}

pub fn signup_status(link: &access_signup_link::Model, now: DateTime<Utc>) -> SignupStatus {
	if link.revoked_at.is_some() {
		SignupStatus::Revoked
	} else if link.expires_at <= now {
		SignupStatus::Expired
	} else if link.uses >= link.max_uses {
		SignupStatus::UsedUp
	} else {
		SignupStatus::Active
	}
}

pub struct NewSignupLink {
	pub label: String,
	pub kind: SignupKind,
	/// The owner's username
	pub created_by: String,
	/// The SHA-256 of the token (hex); the token itself is never given to this module
	pub token_hash: String,
	pub valid_hours: i64,
	/// The budget: ignored for a `single` link, which is always 1
	pub max_uses: i32,
	pub preset: Preset,
}

/// Makes a signup link. The preset is checked (friend set, libraries that exist, limits); the
/// audit row is written with it
pub async fn create_signup_link<C>(
	conn: &C,
	now: DateTime<Utc>,
	new: NewSignupLink,
) -> Result<access_signup_link::Model, AccessError>
where
	C: TransactionTrait,
{
	let label = clean_text(&new.label, MAX_LABEL_CHARS);
	if label.is_empty() {
		return Err(AccessError::Invalid(
			"Write a note so you remember who the link is for",
		));
	}
	if !(1..=MAX_VALID_HOURS).contains(&new.valid_hours) {
		return Err(AccessError::Invalid("That validity is not allowed"));
	}
	let max_uses = match new.kind {
		SignupKind::Single => 1,
		SignupKind::Open => {
			if !(MIN_OPEN_USES..=MAX_OPEN_USES).contains(&new.max_uses) {
				return Err(AccessError::Invalid(
					"An open link can make 2 to 25 accounts",
				));
			}
			new.max_uses
		},
	};
	validate_preset(&new.preset)?;

	let txn = conn.begin().await?;

	let wanted: HashSet<&String> = new.preset.library_ids.iter().collect();
	let existing = library::Entity::find()
		.filter(library::Column::Id.is_in(new.preset.library_ids.clone()))
		.count(&txn)
		.await?;
	if existing as usize != wanted.len() {
		return Err(AccessError::Invalid("One of those libraries does not exist"));
	}
	if access_invite::Entity::find()
		.filter(access_invite::Column::TokenHash.eq(new.token_hash.clone()))
		.count(&txn)
		.await?
		> 0
	{
		return Err(AccessError::Invalid("Try again"));
	}

	let preset_text = serde_json::to_string(&new.preset)
		.map_err(|_| AccessError::Invalid("That preset can't be saved"))?;
	let link = access_signup_link::ActiveModel {
		id: Set(Uuid::new_v4().to_string()),
		token_hash: Set(new.token_hash),
		label: Set(label.clone()),
		kind: Set(new.kind.as_str().to_string()),
		created_at: Set(now),
		created_by: Set(clean_text(&new.created_by, 64)),
		expires_at: Set(now + Duration::hours(new.valid_hours)),
		max_uses: Set(max_uses),
		uses: Set(0),
		revoked_at: Set(None),
		preset: Set(preset_text),
	}
	.insert(&txn)
	.await?;

	audit(
		&txn,
		now,
		&link.created_by,
		"signup.link_created",
		&link.id,
		&format!(
			"{label}: {} link, {max_uses} accounts, {} hours",
			link.kind, new.valid_hours
		),
	)
	.await?;
	txn.commit().await?;
	Ok(link)
}

/// A page of signup links, the latest first, and how many there are
pub async fn list_signup_links<C>(
	conn: &C,
	page: u64,
	page_size: u64,
) -> Result<(Vec<access_signup_link::Model>, u64), DbErr>
where
	C: ConnectionTrait,
{
	let paginator = access_signup_link::Entity::find()
		.order_by_desc(access_signup_link::Column::CreatedAt)
		.paginate(conn, std::cmp::max(page_size, 1));
	let total = paginator.num_items().await?;
	let rows = paginator.fetch_page(page.saturating_sub(1)).await?;
	Ok((rows, total))
}

/// Links that can still be used at `now`
pub async fn active_signup_links<C>(
	conn: &C,
	now: DateTime<Utc>,
) -> Result<Vec<access_signup_link::Model>, DbErr>
where
	C: ConnectionTrait,
{
	Ok(access_signup_link::Entity::find()
		.filter(access_signup_link::Column::RevokedAt.is_null())
		.filter(access_signup_link::Column::ExpiresAt.gt(now))
		.all(conn)
		.await?
		.into_iter()
		.filter(|link| link.uses < link.max_uses)
		.collect())
}

/// The link a token opens, **only if it can be used now**: this is what the page does on a `GET`.
/// It changes nothing
pub async fn peek_signup_link<C>(
	conn: &C,
	now: DateTime<Utc>,
	token_hash: &str,
) -> Result<Option<access_signup_link::Model>, DbErr>
where
	C: ConnectionTrait,
{
	Ok(access_signup_link::Entity::find()
		.filter(access_signup_link::Column::TokenHash.eq(token_hash))
		.one(conn)
		.await?
		.filter(|link| signup_status(link, now) == SignupStatus::Active)
		.filter(|link| preset_of(link).is_some()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revoked {
	/// The link was not revoked before
	pub changed: bool,
	/// Accounts turned off: *Public sign-in* off, devices revoked, sessions ended
	pub accounts_disabled: u64,
}

/// Revokes a signup link: nobody can make an account with it any more. With `disable_accounts`
/// the accounts it already made are turned off too (their *Public sign-in* goes off, their
/// devices are revoked and their sessions end; the accounts stay for the owner to keep or
/// remove). Done even when the link was already revoked, so "turn them off" can be said later
pub async fn revoke_signup_link<C>(
	conn: &C,
	now: DateTime<Utc>,
	id: &str,
	actor: &str,
	disable_accounts: bool,
) -> Result<Revoked, DbErr>
where
	C: TransactionTrait,
{
	let txn = conn.begin().await?;
	let changed = access_signup_link::Entity::update_many()
		.col_expr(access_signup_link::Column::RevokedAt, Expr::value(now))
		.filter(access_signup_link::Column::Id.eq(id))
		.filter(access_signup_link::Column::RevokedAt.is_null())
		.exec(&txn)
		.await?
		.rows_affected
		> 0;

	let mut accounts_disabled = 0;
	if disable_accounts {
		let people = access_person::Entity::find()
			.filter(access_person::Column::SignupLinkId.eq(id))
			.all(&txn)
			.await?;
		for person in people {
			access_person::Entity::update_many()
				.col_expr(access_person::Column::PublicSignin, Expr::value(false))
				.filter(access_person::Column::UserId.eq(person.user_id.clone()))
				.exec(&txn)
				.await?;
			revoke_devices_of_person(&txn, now, &person.user_id, actor).await?;
			accounts_disabled += 1;
		}
	}
	if changed || accounts_disabled > 0 {
		audit(
			&txn,
			now,
			actor,
			"signup.link_revoked",
			id,
			&format!("{accounts_disabled} accounts turned off"),
		)
		.await?;
	}
	txn.commit().await?;
	Ok(Revoked {
		changed,
		accounts_disabled,
	})
}

/// What a visitor typed at a signup link. The username has been checked (and lowercased) and
/// the password hashed, outside this transaction, so the writer lock is never held for a hash
pub struct Signup<'a> {
	pub token_hash: &'a str,
	pub username: &'a str,
	pub hashed_password: String,
	pub ip: &'a str,
	pub agent: &'a str,
	/// Make the account's first device. `false` when the browser already holds a pass: a pass that
	/// is there is never replaced
	pub make_device: bool,
}

/// The account a signup made
pub struct Created {
	pub user: user::Model,
	/// The first device, when one was made
	pub device: Option<access_device::Model>,
	pub link_id: String,
}

#[derive(Debug)]
pub enum SignupError {
	/// Unknown, expired, used up, revoked, or its libraries are all gone: one answer for all,
	/// so nothing can be learnt from it
	Invalid,
	/// Somebody has that name (case-insensitively, removed accounts included). Nothing was spent
	UsernameTaken,
	Db(DbErr),
}

impl From<DbErr> for SignupError {
	fn from(error: DbErr) -> Self {
		Self::Db(error)
	}
}

fn is_name_taken_error(error: &DbErr) -> bool {
	error
		.to_string()
		.contains("UNIQUE constraint failed: users.username")
}

/// Uses a signup link: spends one use, makes the account, its row, its libraries and its first
/// device, in one transaction (see the module docs)
pub async fn redeem_signup_link<C>(
	conn: &C,
	now: DateTime<Utc>,
	signup: Signup<'_>,
) -> Result<Created, SignupError>
where
	C: TransactionTrait,
{
	let txn = conn.begin().await?;

	// the spend comes first: it is a write, so the transaction holds the writer lock from its first
	// statement, and the budget can't be passed by parallel uses
	let spent = access_signup_link::Entity::update_many()
		.col_expr(
			access_signup_link::Column::Uses,
			Expr::col(access_signup_link::Column::Uses).add(1),
		)
		.filter(access_signup_link::Column::TokenHash.eq(signup.token_hash))
		.filter(access_signup_link::Column::RevokedAt.is_null())
		.filter(access_signup_link::Column::ExpiresAt.gt(now))
		.filter(
			Expr::col(access_signup_link::Column::Uses)
				.lt(Expr::col(access_signup_link::Column::MaxUses)),
		)
		.exec(&txn)
		.await?
		.rows_affected;
	if spent != 1 {
		return Err(SignupError::Invalid);
	}

	let link = access_signup_link::Entity::find()
		.filter(access_signup_link::Column::TokenHash.eq(signup.token_hash))
		.one(&txn)
		.await?
		.ok_or(SignupError::Invalid)?;
	// checked again here: the row may have been changed since the owner made it
	let preset = preset_of(&link).ok_or(SignupError::Invalid)?;

	// the preset's libraries that still exist; none left means the link can't make a useful account
	let allowed: Vec<String> = library::Entity::find()
		.filter(library::Column::Id.is_in(preset.library_ids.clone()))
		.select_only()
		.column(library::Column::Id)
		.into_tuple::<String>()
		.all(&txn)
		.await?;
	if allowed.is_empty() {
		return Err(SignupError::Invalid);
	}

	// names are unique whatever their case, removed accounts included (the database's own check is
	// case-sensitive)
	let taken = user::Entity::find()
		.filter(Expr::cust_with_values(
			"LOWER(username) = ?",
			[signup.username.to_lowercase()],
		))
		.count(&txn)
		.await?
		> 0;
	if taken {
		return Err(SignupError::UsernameTaken);
	}

	let age_restriction = preset.age_restriction.map(|age| NewAgeRestriction {
		age: age.age,
		restrict_on_unset: age.restrict_on_unset,
	});
	let account = create_user_with_preferences(
		&txn,
		user::ActiveModel {
			is_server_owner: Set(false),
			username: Set(signup.username.to_string()),
			hashed_password: Set(signup.hashed_password),
			permissions: Set(PermissionSet::new(preset.permissions.clone())
				.resolve_into_string()),
			// a session for every device, and some room for a browser that was closed without
			// signing out
			max_sessions_allowed: Set(Some(preset.max_devices * 2)),
			..Default::default()
		},
		age_restriction,
	)
	.await
	.map_err(|error| {
		if is_name_taken_error(&error) {
			SignupError::UsernameTaken
		} else {
			SignupError::Db(error)
		}
	})?;

	access_person::ActiveModel {
		user_id: Set(account.id.clone()),
		public_signin: Set(true),
		max_devices: Set(preset.max_devices),
		new_libraries_hidden: Set(true),
		signup_link_id: Set(Some(link.id.clone())),
		created_at: Set(now),
	}
	.insert(&txn)
	.await?;

	// the preset's libraries are the only ones they see; every library is decided (so a later
	// reconcile never touches what the owner chooses afterwards)
	let all: Vec<String> = library::Entity::find()
		.select_only()
		.column(library::Column::Id)
		.into_tuple::<String>()
		.all(&txn)
		.await?;
	for library_id in all.iter().filter(|id| !allowed.contains(id)) {
		ensure_exclusion(&txn, &account.id, library_id).await?;
	}
	mark_libraries_decided(&txn, &account.id).await?;

	let device = if signup.make_device {
		let label = device_label(signup.agent);
		let id = unused_device_id(&txn, &label).await?;
		Some(
			insert_device_within_limit(
				&txn,
				now,
				NewDevice {
					id,
					invite_id: None,
					person_id: &account.id,
					label: &label,
					kind: device_kind::BROWSER,
					ip: signup.ip,
					agent: &clean_text(signup.agent, MAX_AGENT_CHARS),
				},
				preset.max_devices,
			)
			.await?
			.ok_or_else(|| {
				SignupError::Db(DbErr::Custom(
					"a new account has no room for a device".into(),
				))
			})?,
		)
	} else {
		None
	};

	audit(
		&txn,
		now,
		"public",
		"signup.created",
		&account.id,
		&format!("link {} ({})", link.id, link.label),
	)
	.await?;

	txn.commit().await?;
	Ok(Created {
		user: account,
		device,
		link_id: link.id,
	})
}
