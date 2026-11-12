//! What the owner should look at, computed on request from small tables (devices, sessions, the
//! sign-in marks, a few audit rows and the signup links). Nothing here is stored: the sidebar's
//! badge always agrees with the page.
//!
//! *State* warnings last while they are true (a person at their device limit). *Event* warnings
//! fade after [`EVENT_DAYS`] days (a person was refused at the limit; a name was slowed down).
//! Every warning is a **warning**: the owner asked to be told about each of these, so each counts
//! in the badge.
//!
//! The owner can mark warnings as seen (`access_warning_reviews`). A dismissed warning is hidden
//! while its count and its newest event are no higher than when it was dismissed: a new event, or
//! a worse state, shows it again, and an event fading out of the window does not. The badge counts
//! the visible warnings only, so it always agrees with the list.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use sea_orm::{
	prelude::*, sea_query::OnConflict, ColumnTrait, ConnectionTrait, DbErr, EntityTrait,
	QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::{
	entity::{
		access_audit, access_device, access_person, access_signin_mark,
		access_warning_review, intake_item, library, session, user, user_login_activity,
	},
	services::{
		access::{self, reason, LockedBy},
		access_policy::{self, DEFAULT_MAX_DEVICES},
		signup,
	},
	shared::intake::IntakeState,
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
	/// An account that is locked after too many wrong passwords
	AccountLocked,
	/// Devices that still work but are leftovers: made in the terminal, of a person who is gone,
	/// or not used for [`access_policy::DORMANT_AFTER_DAYS`] days
	LeftoverDevices,
	/// Books that wait for the owner's approval
	BooksWaiting,
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
	/// The time of the newest fact behind the warning: the newest event, or the newest change of a
	/// state. What dismissing remembers
	pub last_at: DateTime<Utc>,
	pub detail: Option<String>,
	/// When the owner marked it as seen, while it is still hidden. A warning that got worse since
	/// is not dismissed any more
	pub dismissed_at: Option<DateTime<Utc>>,
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
		last_at: since,
		detail: None,
		dismissed_at: None,
	}
}

/// The leading number of an audit detail like `12 failures`
fn leading_number(text: &str) -> i64 {
	text.split_whitespace()
		.next()
		.and_then(|word| word.parse().ok())
		.unwrap_or(1)
}

/// What the audit log says about names that do not exist: the failures summed from the aggregated
/// rows, and whether any such name was slowed down
#[derive(Default)]
struct UnknownNames {
	/// The failures counted by the `signin.unknown_names` rows
	failures: i64,
	/// The slow-downs of names that do not exist
	slow_downs: i64,
	first: Option<DateTime<Utc>>,
	last: Option<DateTime<Utc>>,
}

impl UnknownNames {
	fn note(&mut self, at: DateTime<Utc>, count: i64, slowed: bool) {
		if slowed {
			self.slow_downs += count;
		} else {
			self.failures += count;
		}
		self.first = Some(self.first.map_or(at, |first| first.min(at)));
		self.last = Some(self.last.map_or(at, |last| last.max(at)));
	}

	/// One warning, if there was anything: the failures, or else the slow-downs, as the count
	fn into_warning(self) -> Option<Warning> {
		let first = self.first?;
		let count = if self.failures > 0 {
			self.failures
		} else {
			self.slow_downs.max(1)
		};
		Some(Warning {
			count,
			last_at: self.last.unwrap_or(first),
			detail: Some(
				if self.slow_downs > 0 {
					"slowed"
				} else {
					"names that do not exist"
				}
				.to_string(),
			),
			..warn(WarningKind::SigninFailures, first)
		})
	}
}

/// Every warning at `now`, the alerts first, each with when it was dismissed if it is hidden
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
	// failures on names that do not exist, and the slow-downs of such names, are one warning
	let mut unknown = UnknownNames::default();
	for event in events {
		let (kind, person) = match event.action.as_str() {
			// an event of a person who is gone says nothing the owner can act on
			"signin.device_limit" => {
				if !accounts.contains_key(&event.target) {
					continue;
				}
				(WarningKind::DeviceLimitHit, Some(event.target.clone()))
			},
			"signin.backoff" => {
				if !accounts.contains_key(&event.target) {
					unknown.note(event.at, 1, true);
					continue;
				}
				(WarningKind::NameBackoff, Some(event.target.clone()))
			},
			"signin.global_budget" => (WarningKind::GlobalBudget, None),
			_ => {
				unknown.note(event.at, leading_number(&event.detail), false);
				continue;
			},
		};
		let key = (kind, person.clone().unwrap_or_default());
		grouped
			.entry(key)
			.and_modify(|w| {
				w.count += 1;
				w.last_at = w.last_at.max(event.at);
			})
			.or_insert_with(|| Warning {
				person_id: person,
				..warn(kind, event.at)
			});
	}
	found.extend(grouped.into_values());
	found.extend(unknown.into_warning());

	// bursts of failed sign-ins for one person in the last hour
	let hour = now - Duration::hours(1);
	let failures: Vec<(String, DateTimeWithTimeZone)> =
		user_login_activity::Entity::find()
			.filter(user_login_activity::Column::AuthenticationSuccessful.eq(false))
			.filter(user_login_activity::Column::Timestamp.gt(hour))
			.select_only()
			.column(user_login_activity::Column::UserId)
			.column(user_login_activity::Column::Timestamp)
			.into_tuple::<(String, DateTimeWithTimeZone)>()
			.all(conn)
			.await?;
	let mut per_person: HashMap<String, (usize, DateTime<Utc>)> = HashMap::new();
	for (user_id, at) in failures {
		let at = at.with_timezone(&Utc);
		let entry = per_person.entry(user_id).or_insert((0, at));
		entry.0 += 1;
		entry.1 = entry.1.max(at);
	}
	for (person_id, (count, last)) in per_person {
		if count >= FAILURES_PER_HOUR && accounts.contains_key(&person_id) {
			found.push(Warning {
				person_id: Some(person_id.clone()),
				count: count as i64,
				last_at: last,
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
		let mut by_person: HashMap<String, (i64, DateTime<Utc>, DateTime<Utc>)> =
			HashMap::new();
		for chunk in tried.chunks(400) {
			for activity in user_login_activity::Entity::find()
				.filter(user_login_activity::Column::Id.is_in(chunk.to_vec()))
				.filter(user_login_activity::Column::Timestamp.gt(fade))
				.all(conn)
				.await?
			{
				let at = activity.timestamp.with_timezone(&Utc);
				let entry = by_person.entry(activity.user_id).or_insert((0, at, at));
				entry.0 += 1;
				entry.1 = entry.1.min(at);
				entry.2 = entry.2.max(at);
			}
		}
		for (person_id, (count, since, last)) in by_person {
			if accounts.contains_key(&person_id) {
				found.push(Warning {
					person_id: Some(person_id.clone()),
					count,
					last_at: last,
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
		// the newest use of a link is the newest account it made
		let newest_use = people
			.values()
			.filter(|row| row.signup_link_id.as_deref() == Some(link.id.as_str()))
			.map(|row| row.created_at)
			.max()
			.unwrap_or(link.created_at)
			.max(link.created_at);
		if link.max_uses >= signup::MIN_OPEN_USES && remaining <= 1 {
			found.push(Warning {
				link_id: Some(link.id.clone()),
				count: remaining,
				last_at: newest_use,
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
				last_at: newest_use,
				detail: Some(link.label.clone()),
				..warn(WarningKind::LinkNoLibraries, link.created_at)
			});
		}
	}

	// accounts locked after too many wrong passwords (one the owner locked on purpose is not a
	// warning: they know)
	for blocked in access::blocked_users(conn, now).await? {
		if blocked.user.is_server_owner || blocked.locked_by == LockedBy::Owner {
			continue;
		}
		let last = blocked
			.last_failure_at
			.map_or(blocked.user.created_at.with_timezone(&Utc), |at| {
				at.with_timezone(&Utc)
			});
		found.push(Warning {
			person_id: Some(blocked.user.id.clone()),
			count: blocked.failures_last_24h as i64,
			last_at: last,
			detail: Some(blocked.user.username.clone()),
			..warn(WarningKind::AccountLocked, last)
		});
	}

	// devices that still work but are leftovers
	let live: HashSet<String> = user::Entity::find()
		.filter(user::Column::DeletedAt.is_null())
		.select_only()
		.column(user::Column::Id)
		.into_tuple::<String>()
		.all(conn)
		.await?
		.into_iter()
		.collect();
	let mut leftovers = 0_i64;
	let mut newest: Option<DateTime<Utc>> = None;
	for device in access_device::Entity::find()
		.filter(access_device::Column::RevokedAt.is_null())
		.all(conn)
		.await?
	{
		let seen = device.last_seen_at.unwrap_or(device.created_at);
		let terminal = device.person_id.is_none();
		let gone = device.person_id.as_ref().is_some_and(|id| !live.contains(id));
		if terminal || gone || seen <= cutoff {
			leftovers += 1;
			let latest = seen.max(device.created_at);
			newest = Some(newest.map_or(latest, |at| at.max(latest)));
		}
	}
	if let (true, Some(at)) = (leftovers > 0, newest) {
		found.push(Warning {
			count: leftovers,
			..warn(WarningKind::LeftoverDevices, at)
		});
	}

	// books that wait for the owner's approval
	let waiting: Vec<Option<DateTimeWithTimeZone>> = intake_item::Entity::find()
		.filter(intake_item::Column::State.eq(IntakeState::PendingApproval))
		.select_only()
		.column(intake_item::Column::SubmittedAt)
		.into_tuple::<Option<DateTimeWithTimeZone>>()
		.all(conn)
		.await?;
	if !waiting.is_empty() {
		let newest = waiting
			.iter()
			.flatten()
			.map(|at| at.with_timezone(&Utc))
			.max()
			.unwrap_or(now);
		found.push(Warning {
			count: waiting.len() as i64,
			..warn(WarningKind::BooksWaiting, newest)
		});
	}

	found.sort_by(|a, b| {
		b.severity
			.cmp(&a.severity)
			.then(a.kind.cmp(&b.kind))
			.then(a.id().cmp(&b.id()))
	});

	// what the owner has marked as seen stays hidden until something new or worse happens
	let reviews: HashMap<String, access_warning_review::Model> =
		access_warning_review::Entity::find()
			.all(conn)
			.await?
			.into_iter()
			.map(|row| (row.warning_id.clone(), row))
			.collect();
	for warning in &mut found {
		if let Some(review) = reviews.get(&warning.id()) {
			if warning.count <= review.count_at_dismiss
				&& warning.last_at <= review.last_at_dismiss
			{
				warning.dismissed_at = Some(review.dismissed_at);
			}
		}
	}
	Ok(found)
}

/// The warnings the owner has not marked as seen
pub async fn visible_warnings<C>(
	conn: &C,
	now: DateTime<Utc>,
) -> Result<Vec<Warning>, DbErr>
where
	C: ConnectionTrait,
{
	Ok(warnings(conn, now)
		.await?
		.into_iter()
		.filter(|warning| warning.dismissed_at.is_none())
		.collect())
}

/// How many warnings are visible: what the sidebar's badge shows, always the list's length
pub async fn warning_count<C>(conn: &C, now: DateTime<Utc>) -> Result<usize, DbErr>
where
	C: ConnectionTrait,
{
	Ok(visible_warnings(conn, now).await?.len())
}

/// Marks warnings as seen: the ones among the visible warnings with these ids (an unknown id is
/// ignored), or every visible warning when `ids` is `None`. Returns how many it dismissed. Rows of
/// warnings that went away more than 30 days ago are dropped
pub async fn dismiss<C>(
	conn: &C,
	now: DateTime<Utc>,
	by: &str,
	ids: Option<&[String]>,
) -> Result<usize, DbErr>
where
	C: ConnectionTrait,
{
	let visible = visible_warnings(conn, now).await?;
	let chosen: Vec<&Warning> = visible
		.iter()
		.filter(|warning| ids.is_none_or(|ids| ids.contains(&warning.id())))
		.collect();
	for warning in &chosen {
		let row = access_warning_review::ActiveModel {
			warning_id: Set(warning.id()),
			dismissed_at: Set(now),
			dismissed_by: Set(Some(by.to_string())),
			count_at_dismiss: Set(warning.count),
			last_at_dismiss: Set(warning.last_at),
		};
		access_warning_review::Entity::insert(row)
			.on_conflict(
				OnConflict::column(access_warning_review::Column::WarningId)
					.update_columns([
						access_warning_review::Column::DismissedAt,
						access_warning_review::Column::DismissedBy,
						access_warning_review::Column::CountAtDismiss,
						access_warning_review::Column::LastAtDismiss,
					])
					.to_owned(),
			)
			.exec(conn)
			.await?;
	}

	// forget the rows of warnings that are gone for good
	let current: HashSet<String> = warnings(conn, now)
		.await?
		.iter()
		.map(Warning::id)
		.collect();
	let stale = access_warning_review::Entity::find()
		.filter(access_warning_review::Column::DismissedAt.lt(now - Duration::days(30)))
		.all(conn)
		.await?;
	for row in stale {
		if !current.contains(&row.warning_id) {
			access_warning_review::Entity::delete_by_id(row.warning_id)
				.exec(conn)
				.await?;
		}
	}
	Ok(chosen.len())
}

/// Shows dismissed warnings again (Undo): deletes the rows of these ids. Returns how many rows
/// there were
pub async fn restore<C>(conn: &C, ids: &[String]) -> Result<usize, DbErr>
where
	C: ConnectionTrait,
{
	let result = access_warning_review::Entity::delete_many()
		.filter(access_warning_review::Column::WarningId.is_in(ids.to_vec()))
		.exec(conn)
		.await?;
	Ok(result.rows_affected as usize)
}
