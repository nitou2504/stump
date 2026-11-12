//! What *Public access* knows about each account: whether it may come in from the internet, how
//! many devices it may have, and which libraries it sees (and which new ones are hidden from it).
//!
//! - **Eligibility** ([`eligibility`]): the one rule for every request through the public edge,
//!   whatever the credential. The account must exist, not be the server owner, not be locked,
//!   have a password, have its *Public sign-in* switch on, and hold only friend-set permissions.
//! - **Libraries**: Stump's visibility is an exclusion model (a row in `library_exclusions` hides
//!   a library from a person), so a library made later is visible to everybody by default. People
//!   with `new_libraries_hidden` get a *decision log* instead: each (person, library) pair is
//!   decided exactly once, by inserting its decision row, and only the insert that really added
//!   the row makes the exclusion. The owner's own later changes are never overwritten.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use sea_orm::{
	prelude::*, sea_query::Query, ActiveValue::Set, ColumnTrait, ConnectionTrait,
	DbBackend, DbErr, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, QuerySelect,
	Statement, TransactionSession, TransactionTrait,
};

use crate::{
	entity::{
		access_device, access_library_decision, access_person, library,
		library_exclusion, session, user,
	},
	services::{
		access::{audit, clean_text, dormant_cutoff, AccessError},
		access_policy::{self, DEFAULT_MAX_DEVICES, MAX_MAX_DEVICES, MIN_MAX_DEVICES},
	},
	shared::enums::UserPermission,
};

/// Why an account may not come in from the internet
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotAllowed {
	/// Removed, or there is no such account
	Gone,
	/// The server owner signs in from home only
	Owner,
	Locked,
	/// It has no password (an account that signs in through an identity provider)
	NoPassword,
	/// Nobody turned *Public sign-in* on for it
	NotEnabled,
	/// It holds permissions outside the friend set
	BeyondFriendSet(Vec<UserPermission>),
}

/// Whether `account` may come in from the internet: see the module docs. `person` is its
/// `access_people` row, if it has one
pub fn eligibility(
	account: &user::Model,
	person: Option<&access_person::Model>,
) -> Result<(), NotAllowed> {
	if account.deleted_at.is_some() {
		return Err(NotAllowed::Gone);
	}
	if account.is_server_owner {
		return Err(NotAllowed::Owner);
	}
	if account.is_locked {
		return Err(NotAllowed::Locked);
	}
	if account.hashed_password.is_empty() {
		return Err(NotAllowed::NoPassword);
	}
	if !person.is_some_and(|row| row.public_signin) {
		return Err(NotAllowed::NotEnabled);
	}
	let beyond = access_policy::outside_friend_set_of(account.permissions.as_deref());
	if !beyond.is_empty() {
		return Err(NotAllowed::BeyondFriendSet(beyond));
	}
	Ok(())
}

pub async fn person_row<C>(
	conn: &C,
	user_id: &str,
) -> Result<Option<access_person::Model>, DbErr>
where
	C: ConnectionTrait,
{
	access_person::Entity::find_by_id(user_id.to_string())
		.one(conn)
		.await
}

/// Reads the account and its row and applies [`eligibility`]. `Ok(Err(..))` is the answer "not
/// allowed"; the account is returned with the row when it is
pub async fn public_eligibility<C>(
	conn: &C,
	user_id: &str,
) -> Result<Result<(user::Model, access_person::Model), NotAllowed>, DbErr>
where
	C: ConnectionTrait,
{
	let Some(account) = user::Entity::find_by_id(user_id.to_string())
		.one(conn)
		.await?
	else {
		return Ok(Err(NotAllowed::Gone));
	};
	let person = person_row(conn, user_id).await?;
	Ok(eligibility(&account, person.as_ref()).map(|()| {
		(
			account,
			person.expect("an eligible account has a row, public sign-in is on"),
		)
	}))
}

// -------------------------------------------------------------------------------------------
// libraries
// -------------------------------------------------------------------------------------------

/// Marks every library that exists now as decided for the person, leaving what they see as it is.
/// Done whenever a person's row is made, and whenever the "hide new libraries" choice is turned
/// back on: otherwise the next reconcile would hide every library they can see (B5)
pub async fn mark_libraries_decided<C>(conn: &C, user_id: &str) -> Result<u64, DbErr>
where
	C: ConnectionTrait,
{
	Ok(conn
		.execute_raw(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			"INSERT OR IGNORE INTO access_library_decisions (user_id, library_id) SELECT ?, id FROM libraries",
			[user_id.to_string().into()],
		))
		.await?
		.rows_affected())
}

/// Hides a library from a person unless it is already hidden
pub(crate) async fn ensure_exclusion<C>(
	conn: &C,
	user_id: &str,
	library_id: &str,
) -> Result<(), DbErr>
where
	C: ConnectionTrait,
{
	// `library_exclusions` has no unique (user, library) constraint, so look first
	let exists = library_exclusion::Entity::find()
		.filter(library_exclusion::Column::UserId.eq(user_id))
		.filter(library_exclusion::Column::LibraryId.eq(library_id))
		.count(conn)
		.await?
		> 0;
	if !exists {
		library_exclusion::ActiveModel {
			user_id: Set(user_id.to_string()),
			library_id: Set(library_id.to_string()),
			..Default::default()
		}
		.insert(conn)
		.await?;
	}
	Ok(())
}

/// Decides every library the person has no decision for yet: the decision row is inserted, and
/// **only the insert that really added the row** makes the exclusion, so two runs at once never
/// hide a library twice. Returns how many libraries were hidden
pub async fn hide_undecided_libraries<C>(conn: &C, user_id: &str) -> Result<u64, DbErr>
where
	C: ConnectionTrait,
{
	let decided = Query::select()
		.column(access_library_decision::Column::LibraryId)
		.from(access_library_decision::Entity)
		.and_where(access_library_decision::Column::UserId.eq(user_id))
		.to_owned();
	let undecided: Vec<String> = library::Entity::find()
		.filter(library::Column::Id.not_in_subquery(decided))
		.select_only()
		.column(library::Column::Id)
		.into_tuple::<String>()
		.all(conn)
		.await?;

	let mut hidden = 0;
	for library_id in undecided {
		let added = conn
			.execute_raw(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"INSERT OR IGNORE INTO access_library_decisions (user_id, library_id) VALUES (?, ?)",
				[user_id.to_string().into(), library_id.clone().into()],
			))
			.await?
			.rows_affected()
			== 1;
		if added {
			ensure_exclusion(conn, user_id, &library_id).await?;
			hidden += 1;
		}
	}
	Ok(hidden)
}

/// Hides every library the person has no decision for, from everybody with a row whose "hide new
/// libraries" choice is on. Run when a library is made, at start-up, every ten minutes and at
/// every public sign-in, so a library made by something that doesn't know about this (a script
/// that inserts into `libraries`) is hidden before the next sign-in. Returns how many
/// (person, library) pairs were hidden
pub async fn reconcile_libraries<C>(conn: &C) -> Result<u64, DbErr>
where
	C: ConnectionTrait,
{
	let people: Vec<String> = access_person::Entity::find()
		.filter(access_person::Column::NewLibrariesHidden.eq(true))
		.select_only()
		.column(access_person::Column::UserId)
		.into_tuple::<String>()
		.all(conn)
		.await?;
	let mut hidden = 0;
	for person in people {
		hidden += hide_undecided_libraries(conn, &person).await?;
	}
	Ok(hidden)
}

/// "Only these libraries": the person sees exactly these. Every other library is hidden and the
/// listed ones are shown; every library is decided
async fn apply_only_libraries<C>(
	conn: &C,
	user_id: &str,
	allowed: &[String],
) -> Result<(), AccessError>
where
	C: ConnectionTrait,
{
	let all: Vec<String> = library::Entity::find()
		.select_only()
		.column(library::Column::Id)
		.into_tuple::<String>()
		.all(conn)
		.await?;
	let known: HashSet<&String> = all.iter().collect();
	if allowed.iter().any(|id| !known.contains(id)) {
		return Err(AccessError::Invalid(
			"One of those libraries does not exist",
		));
	}
	for library_id in &all {
		if allowed.contains(library_id) {
			library_exclusion::Entity::delete_many()
				.filter(library_exclusion::Column::UserId.eq(user_id))
				.filter(library_exclusion::Column::LibraryId.eq(library_id.clone()))
				.exec(conn)
				.await?;
		} else {
			ensure_exclusion(conn, user_id, library_id).await?;
		}
	}
	mark_libraries_decided(conn, user_id).await?;
	Ok(())
}

// -------------------------------------------------------------------------------------------
// a person's settings
// -------------------------------------------------------------------------------------------

/// Gives a friend-class account its row with *Public sign-in* on (the owner's decision when they
/// make a device link for it). A row that exists is switched on; one that doesn't is made, with
/// every library that exists now decided
pub async fn enable_public_signin<C>(
	conn: &C,
	now: DateTime<Utc>,
	user_id: &str,
) -> Result<(), DbErr>
where
	C: ConnectionTrait,
{
	match person_row(conn, user_id).await? {
		Some(row) if row.public_signin => Ok(()),
		Some(row) => {
			let mut active = row.into_active_model();
			active.public_signin = Set(true);
			active.update(conn).await?;
			Ok(())
		},
		None => {
			access_person::ActiveModel {
				user_id: Set(user_id.to_string()),
				public_signin: Set(true),
				max_devices: Set(DEFAULT_MAX_DEVICES),
				new_libraries_hidden: Set(true),
				signup_link_id: Set(None),
				created_at: Set(now),
			}
			.insert(conn)
			.await?;
			mark_libraries_decided(conn, user_id).await?;
			Ok(())
		},
	}
}

/// What the owner changes about one person in *People*; `None` leaves a setting as it is
#[derive(Debug, Default, Clone)]
pub struct PersonAccessChange {
	pub public_signin: Option<bool>,
	pub max_devices: Option<i32>,
	pub new_libraries_hidden: Option<bool>,
	/// "Only these libraries": the person sees exactly these
	pub only_library_ids: Option<Vec<String>>,
}

fn permission_names(permissions: &[UserPermission]) -> String {
	permissions
		.iter()
		.map(ToString::to_string)
		.collect::<Vec<_>>()
		.join(", ")
}

/// Changes a person's public-access settings, in one transaction. Refuses the server owner;
/// refuses to turn public sign-in on for an account that holds permissions outside the friend set
/// (it names them) or has no password
pub async fn set_person_access<C>(
	conn: &C,
	now: DateTime<Utc>,
	actor: &str,
	user_id: &str,
	change: PersonAccessChange,
) -> Result<access_person::Model, AccessError>
where
	C: TransactionTrait,
{
	if let Some(devices) = change.max_devices {
		if !(MIN_MAX_DEVICES..=MAX_MAX_DEVICES).contains(&devices) {
			return Err(AccessError::Invalid(
				"That number of devices is not allowed",
			));
		}
	}

	let txn = conn.begin().await?;
	let account = user::Entity::find_by_id(user_id.to_string())
		.filter(user::Column::DeletedAt.is_null())
		.one(&txn)
		.await?
		.ok_or(AccessError::PersonNotAllowed)?;
	if account.is_server_owner {
		return Err(AccessError::Message(
			"The server owner signs in from home only".to_string(),
		));
	}

	if change.public_signin == Some(true) {
		let beyond = access_policy::outside_friend_set_of(account.permissions.as_deref());
		if !beyond.is_empty() {
			return Err(AccessError::Message(format!(
				"{} holds permissions that are not for the internet: {}. Remove them first",
				account.username,
				permission_names(&beyond)
			)));
		}
		if account.hashed_password.is_empty() {
			return Err(AccessError::Message(format!(
				"{} has no password, so they can't sign in from the internet",
				account.username
			)));
		}
	}

	let existing = person_row(&txn, user_id).await?;
	let row = match existing {
		None => {
			let row = access_person::ActiveModel {
				user_id: Set(user_id.to_string()),
				public_signin: Set(change.public_signin.unwrap_or(false)),
				max_devices: Set(change.max_devices.unwrap_or(DEFAULT_MAX_DEVICES)),
				new_libraries_hidden: Set(change.new_libraries_hidden.unwrap_or(true)),
				signup_link_id: Set(None),
				created_at: Set(now),
			}
			.insert(&txn)
			.await?;
			// a new row: every library that exists is decided, or the next reconcile would hide
			// all of them
			mark_libraries_decided(&txn, user_id).await?;
			row
		},
		Some(row) => {
			let hiding_back_on =
				change.new_libraries_hidden == Some(true) && !row.new_libraries_hidden;
			let mut active = row.into_active_model();
			if let Some(value) = change.public_signin {
				active.public_signin = Set(value);
			}
			if let Some(value) = change.max_devices {
				active.max_devices = Set(value);
			}
			if let Some(value) = change.new_libraries_hidden {
				active.new_libraries_hidden = Set(value);
			}
			let row = active.update(&txn).await?;
			if hiding_back_on {
				mark_libraries_decided(&txn, user_id).await?;
			}
			row
		},
	};

	if let Some(allowed) = &change.only_library_ids {
		apply_only_libraries(&txn, user_id, allowed).await?;
	}

	audit(
		&txn,
		now,
		actor,
		"person.access_set",
		user_id,
		&clean_text(
			&format!(
				"{}: public sign-in {}, {} devices, new libraries {}{}",
				account.username,
				if row.public_signin { "on" } else { "off" },
				row.max_devices,
				if row.new_libraries_hidden {
					"hidden"
				} else {
					"visible"
				},
				change
					.only_library_ids
					.as_ref()
					.map(|ids| format!(", only {} libraries", ids.len()))
					.unwrap_or_default()
			),
			300,
		),
	)
	.await?;
	txn.commit().await?;
	Ok(row)
}

// -------------------------------------------------------------------------------------------
// what the People table shows
// -------------------------------------------------------------------------------------------

/// One account as the People table shows it
#[derive(Debug, Clone)]
pub struct PersonAccess {
	pub public_signin: bool,
	pub max_devices: i32,
	pub new_libraries_hidden: bool,
	pub signup_link_id: Option<String>,
	/// Devices that count against the limit
	pub active_devices: u64,
	/// Unexpired sign-in sessions
	pub sessions: u64,
	/// The permissions that keep the account out of public sign-in
	pub beyond_friend_set: Vec<UserPermission>,
	/// Libraries hidden from the account
	pub hidden_libraries: u64,
}

/// The People table's numbers for every account in `users`, in a handful of queries
pub async fn person_summaries<C>(
	conn: &C,
	now: DateTime<Utc>,
	users: &[user::Model],
) -> Result<HashMap<String, PersonAccess>, DbErr>
where
	C: ConnectionTrait,
{
	let rows: HashMap<String, access_person::Model> = access_person::Entity::find()
		.all(conn)
		.await?
		.into_iter()
		.map(|row| (row.user_id.clone(), row))
		.collect();

	let cutoff = dormant_cutoff(now);
	let devices = access_device::Entity::find()
		.filter(access_device::Column::PersonId.is_not_null())
		.filter(access_device::Column::RevokedAt.is_null())
		.all(conn)
		.await?;
	let mut active: HashMap<String, u64> = HashMap::new();
	for device in devices {
		let seen = device.last_seen_at.unwrap_or(device.created_at);
		if let (true, Some(person)) = (seen > cutoff, device.person_id) {
			*active.entry(person).or_default() += 1;
		}
	}

	let mut sessions: HashMap<String, u64> = HashMap::new();
	for (user_id, expiry) in session::Entity::find()
		.select_only()
		.column(session::Column::UserId)
		.column(session::Column::ExpiryTime)
		.into_tuple::<(String, DateTimeWithTimeZone)>()
		.all(conn)
		.await?
	{
		if expiry > now {
			*sessions.entry(user_id).or_default() += 1;
		}
	}

	let mut hidden: HashMap<String, u64> = HashMap::new();
	for user_id in library_exclusion::Entity::find()
		.select_only()
		.column(library_exclusion::Column::UserId)
		.into_tuple::<String>()
		.all(conn)
		.await?
	{
		*hidden.entry(user_id).or_default() += 1;
	}

	Ok(users
		.iter()
		.map(|account| {
			let row = rows.get(&account.id);
			(
				account.id.clone(),
				PersonAccess {
					public_signin: row.is_some_and(|row| row.public_signin),
					max_devices: row.map_or(DEFAULT_MAX_DEVICES, |row| row.max_devices),
					new_libraries_hidden: row.is_none_or(|row| row.new_libraries_hidden),
					signup_link_id: row.and_then(|row| row.signup_link_id.clone()),
					active_devices: active.get(&account.id).copied().unwrap_or(0),
					sessions: sessions.get(&account.id).copied().unwrap_or(0),
					beyond_friend_set: access_policy::outside_friend_set_of(
						account.permissions.as_deref(),
					),
					hidden_libraries: hidden.get(&account.id).copied().unwrap_or(0),
				},
			)
		})
		.collect())
}

/// Where a signup link was used: the accounts it made, by link id
pub async fn people_of_links<C>(
	conn: &C,
	link_ids: &[String],
) -> Result<HashMap<String, Vec<user::Model>>, DbErr>
where
	C: ConnectionTrait,
{
	if link_ids.is_empty() {
		return Ok(HashMap::new());
	}
	let rows = access_person::Entity::find()
		.filter(access_person::Column::SignupLinkId.is_in(link_ids.to_vec()))
		.all(conn)
		.await?;
	let by_user: HashMap<String, String> = rows
		.into_iter()
		.filter_map(|row| row.signup_link_id.map(|link| (row.user_id, link)))
		.collect();
	let accounts = user::Entity::find()
		.filter(user::Column::Id.is_in(by_user.keys().cloned().collect::<Vec<_>>()))
		.filter(user::Column::DeletedAt.is_null())
		.order_by_asc(user::Column::Username)
		.all(conn)
		.await?;
	let mut grouped: HashMap<String, Vec<user::Model>> = HashMap::new();
	for account in accounts {
		if let Some(link) = by_user.get(&account.id) {
			grouped.entry(link.clone()).or_default().push(account);
		}
	}
	Ok(grouped)
}
