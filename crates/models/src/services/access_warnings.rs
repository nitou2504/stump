//! What the owner should look at, computed on request from small tables (devices, sessions, the
//! sign-in marks, a few audit rows and the signup links). Nothing here is stored: the sidebar's
//! badge always agrees with the page.
//!
//! *State* warnings last while they are true (a person at their device limit). *Event* warnings
//! fade after [`EVENT_DAYS`] days (a person was refused at the limit; a name was slowed down).
//! Every warning is a **warning**: the owner asked to be told about each of these, so each counts
//! in the badge.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use sea_orm::{
	prelude::*, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter,
	QueryOrder, QuerySelect,
};

use crate::{
	entity::{
		access_audit, access_device, access_person, access_signin_mark, library, session,
		user, user_login_activity,
	},
	services::{
		access::reason,
		access_policy::{self, DEFAULT_MAX_DEVICES},
		signup,
	},
};

/// Event warnings fade after this many days
pub const EVENT_DAYS: i64 = 3;
/// Failed sign-ins of one person in an hour that make a warning
pub const FAILURES_PER_HOUR: usize = 10;
/// Devices (or sessions) that make a warning, whatever the limit is
pub const MANY: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WarningKind {
	/// A person uses all their devices
	AtDeviceLimit,
	/// A person has more devices than their limit (it was lowered, or a dormant device came back)
	OverDeviceLimit,
	/// A person has four or more devices
	ManyDevices,
	/// A person was refused a new device because of their limit
	DeviceLimitHit,
	/// A person has four or more sign-in sessions
	ManySessions,
	/// A burst of failed sign-ins for one person, or for names that don't exist
	SigninFailures,
	/// A name was slowed down: somebody kept guessing at it
	NameBackoff,
	/// Public sign-in refused everybody for a while: a flood of failures
	GlobalBudget,
	/// The right password of an account that is not allowed in from the internet
	NotEnabledTried,
	/// A person with Public sign-in on holds permissions that are not for the internet
	NotFriendClass,
	/// An open link with one use left
	LinkNearBudget,
	/// A link whose libraries are all gone: nobody can use it
	LinkNoLibraries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
	Warn,
	Alert,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
	pub kind: WarningKind,
	pub severity: Severity,
	pub person_id: Option<String>,
	pub link_id: Option<String>,
	/// How many times, or how many devices
	pub count: i64,
	pub since: DateTime<Utc>,
	pub detail: Option<String>,
}

impl Warning {
	/// Stable across requests: the same warning has the same id
	pub fn id(&self) -> String {
		format!(
			"{:?}:{}",
			self.kind,
			self.person_id
				.as_deref()
				.or(self.link_id.as_deref())
				.unwrap_or("-")
		)
	}
}

fn warn(kind: WarningKind, since: DateTime<Utc>) -> Warning {
	Warning {
		kind,
		severity: if kind == WarningKind::GlobalBudget {
			Severity::Alert
		} else {
			Severity::Warn
		},
		person_id: None,
		link_id: None,
		count: 1,
		since,
		detail: None,
	}
}

/// The leading number of an audit detail like `12 failures`
fn leading_number(text: &str) -> i64 {
	text.split_whitespace()
		.next()
		.and_then(|word| word.parse().ok())
		.unwrap_or(1)
}

/// Every warning at `now`, the alerts first
pub async fn warnings<C>(conn: &C, now: DateTime<Utc>) -> Result<Vec<Warning>, DbErr>
where
	C: ConnectionTrait,
{
	let mut found = Vec::new();
	let fade = now - Duration::days(EVENT_DAYS);

	let people: HashMap<String, access_person::Model> = access_person::Entity::find()
		.all(conn)
		.await?
		.into_iter()
		.map(|row| (row.user_id.clone(), row))
		.collect();
	let accounts: HashMap<String, user::Model> = user::Entity::find()
		.filter(user::Column::DeletedAt.is_null())
		.filter(user::Column::IsServerOwner.eq(false))
		.all(conn)
		.await?
		.into_iter()
		.map(|account| (account.id.clone(), account))
		.collect();

	// devices: at the limit, over it, many
	let cutoff = access_policy::DORMANT_AFTER_DAYS;
	let cutoff = now - Duration::days(cutoff);
	let mut devices: HashMap<String, (u64, DateTime<Utc>)> = HashMap::new();
	for device in access_device::Entity::find()
		.filter(access_device::Column::PersonId.is_not_null())
		.filter(access_device::Column::RevokedAt.is_null())
		.all(conn)
		.await?
	{
		let seen = device.last_seen_at.unwrap_or(device.created_at);
		if let (true, Some(person)) = (seen > cutoff, device.person_id) {
			let entry = devices.entry(person).or_insert((0, device.created_at));
			entry.0 += 1;
			entry.1 = entry.1.max(device.created_at);
		}
	}
	for (person_id, (active, since)) in &devices {
		let Some(account) = accounts.get(person_id) else {
			continue;
		};
		let limit = people
			.get(person_id)
			.map_or(DEFAULT_MAX_DEVICES, |row| row.max_devices) as u64;
		let kind = if *active > limit {
			Some(WarningKind::OverDeviceLimit)
		} else if *active == limit {
			Some(WarningKind::AtDeviceLimit)
		} else if *active >= MANY {
			Some(WarningKind::ManyDevices)
		} else {
			None
		};
		if let Some(kind) = kind {
			found.push(Warning {
				person_id: Some(person_id.clone()),
				count: *active as i64,
				detail: Some(format!("{} of {limit}", account.username)),
				..warn(kind, *since)
			});
		}
	}

	// the sessions
	let mut sessions: HashMap<String, (u64, DateTime<Utc>)> = HashMap::new();
	for (user_id, created, expiry) in session::Entity::find()
		.select_only()
		.column(session::Column::UserId)
		.column(session::Column::CreatedAt)
		.column(session::Column::ExpiryTime)
		.into_tuple::<(String, DateTimeWithTimeZone, DateTimeWithTimeZone)>()
		.all(conn)
		.await?
	{
		if expiry > now {
			let entry = sessions
				.entry(user_id)
				.or_insert((0, created.with_timezone(&Utc)));
			entry.0 += 1;
			entry.1 = entry.1.max(created.with_timezone(&Utc));
		}
	}
	for (person_id, (count, since)) in sessions {
		if count >= MANY && accounts.contains_key(&person_id) {
			found.push(Warning {
				person_id: Some(person_id.clone()),
				count: count as i64,
				detail: accounts.get(&person_id).map(|a| a.username.clone()),
				..warn(WarningKind::ManySessions, since)
			});
		}
	}

	// events from the audit log: the limit, the slow-downs, the flood
	let events = access_audit::Entity::find()
		.filter(access_audit::Column::At.gt(fade))
		.filter(access_audit::Column::Action.is_in([
			"signin.device_limit",
			"signin.backoff",
			"signin.global_budget",
			"signin.unknown_names",
		]))
		.order_by_asc(access_audit::Column::At)
		.all(conn)
		.await?;
	let mut grouped: HashMap<(WarningKind, String), Warning> = HashMap::new();
	for event in events {
		let (kind, person) = match event.action.as_str() {
			"signin.device_limit" => {
				(WarningKind::DeviceLimitHit, Some(event.target.clone()))
			},
			"signin.backoff" => (
				WarningKind::NameBackoff,
				accounts
					.contains_key(&event.target)
					.then(|| event.target.clone()),
			),
			"signin.global_budget" => (WarningKind::GlobalBudget, None),
			_ => (WarningKind::SigninFailures, None),
		};
		let key = (kind, person.clone().unwrap_or_default());
		let weight = if event.action == "signin.unknown_names" {
			leading_number(&event.detail)
		} else {
			1
		};
		grouped
			.entry(key)
			.and_modify(|w| w.count += weight)
			.or_insert_with(|| Warning {
				person_id: person,
				count: weight,
				detail: (event.action == "signin.unknown_names")
					.then(|| "names that do not exist".to_string()),
				..warn(kind, event.at)
			});
	}
	found.extend(grouped.into_values());

	// bursts of failed sign-ins for one person in the last hour
	let hour = now - Duration::hours(1);
	let failures: Vec<String> = user_login_activity::Entity::find()
		.filter(user_login_activity::Column::AuthenticationSuccessful.eq(false))
		.filter(user_login_activity::Column::Timestamp.gt(hour))
		.select_only()
		.column(user_login_activity::Column::UserId)
		.into_tuple::<String>()
		.all(conn)
		.await?;
	let mut per_person: HashMap<String, usize> = HashMap::new();
	for user_id in failures {
		*per_person.entry(user_id).or_default() += 1;
	}
	for (person_id, count) in per_person {
		if count >= FAILURES_PER_HOUR && accounts.contains_key(&person_id) {
			found.push(Warning {
				person_id: Some(person_id.clone()),
				count: count as i64,
				detail: accounts.get(&person_id).map(|a| a.username.clone()),
				..warn(WarningKind::SigninFailures, hour)
			});
		}
	}

	// the right password of an account that is not allowed in from the internet
	let tried: Vec<i64> = access_signin_mark::Entity::find()
		.filter(
			access_signin_mark::Column::FailureReason.eq(reason::NOT_ALLOWED_PUBLICLY),
		)
		.select_only()
		.column(access_signin_mark::Column::LoginActivityId)
		.into_tuple::<i64>()
		.all(conn)
		.await?;
	if !tried.is_empty() {
		let mut by_person: HashMap<String, (i64, DateTime<Utc>)> = HashMap::new();
		for chunk in tried.chunks(400) {
			for activity in user_login_activity::Entity::find()
				.filter(user_login_activity::Column::Id.is_in(chunk.to_vec()))
				.filter(user_login_activity::Column::Timestamp.gt(fade))
				.all(conn)
				.await?
			{
				let at = activity.timestamp.with_timezone(&Utc);
				let entry = by_person.entry(activity.user_id).or_insert((0, at));
				entry.0 += 1;
				entry.1 = entry.1.min(at);
			}
		}
		for (person_id, (count, since)) in by_person {
			if accounts.contains_key(&person_id) {
				found.push(Warning {
					person_id: Some(person_id.clone()),
					count,
					detail: accounts.get(&person_id).map(|a| a.username.clone()),
					..warn(WarningKind::NotEnabledTried, since)
				});
			}
		}
	}

	// people on the internet who hold more than a friend may
	for (person_id, row) in &people {
		if !row.public_signin {
			continue;
		}
		if let Some(account) = accounts.get(person_id) {
			let beyond =
				access_policy::outside_friend_set_of(account.permissions.as_deref());
			if !beyond.is_empty() {
				found.push(Warning {
					person_id: Some(person_id.clone()),
					count: beyond.len() as i64,
					detail: Some(
						beyond
							.iter()
							.map(ToString::to_string)
							.collect::<Vec<_>>()
							.join(", "),
					),
					..warn(WarningKind::NotFriendClass, row.created_at)
				});
			}
		}
	}

	// signup links
	let existing: Vec<String> = library::Entity::find()
		.select_only()
		.column(library::Column::Id)
		.into_tuple::<String>()
		.all(conn)
		.await?;
	for link in signup::active_signup_links(conn, now).await? {
		let remaining = i64::from(link.max_uses - link.uses);
		if link.max_uses >= signup::MIN_OPEN_USES && remaining <= 1 {
			found.push(Warning {
				link_id: Some(link.id.clone()),
				count: remaining,
				detail: Some(link.label.clone()),
				..warn(WarningKind::LinkNearBudget, link.created_at)
			});
		}
		let usable = signup::preset_of(&link).is_some_and(|preset| {
			preset.library_ids.iter().any(|id| existing.contains(id))
		});
		if !usable {
			found.push(Warning {
				link_id: Some(link.id.clone()),
				count: 1,
				detail: Some(link.label.clone()),
				..warn(WarningKind::LinkNoLibraries, link.created_at)
			});
		}
	}

	found.sort_by(|a, b| {
		b.severity
			.cmp(&a.severity)
			.then(a.kind.cmp(&b.kind))
			.then(a.id().cmp(&b.id()))
	});
	Ok(found)
}

/// How many warnings there are: what the sidebar's badge shows
pub async fn warning_count<C>(conn: &C, now: DateTime<Utc>) -> Result<usize, DbErr>
where
	C: ConnectionTrait,
{
	Ok(warnings(conn, now).await?.len())
}
