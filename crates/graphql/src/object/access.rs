//! The *Public access* page: what the owner sees of who can reach Stump from the internet.
//!
//! Everything here is behind the owner-only guards of `query::access` and `mutation::access`
//! (and refused on requests that came through the public edge): the objects carry device
//! addresses, browsers and who was locked out, none of which another user may see. A person is an
//! [`AccessPerson`] here, not a `ReaderProfile`: this page has to show the people who are locked
//! or removed too.
//!
//! No object has a field for a token, a key, a link (but the one made by `createAccessInvite`,
//! once) or a password.

use async_graphql::{
	dataloader::DataLoader, ComplexObject, Context, CustomValidator, Enum, InputObject,
	InputValueError, Result, SimpleObject,
};
use chrono::{DateTime, Utc};
use models::{
	entity::{access_audit, access_device, access_invite, user},
	services::{
		access::{
			self as service, BlockedUser, InviteStatus, LockedBy, Person, SignInRow,
		},
		access_people::PersonAccess,
	},
	shared::enums::UserPermission,
};
use sea_orm::prelude::DateTimeWithTimeZone;

use super::access_signup::AccessSigninStats;
use crate::{
	loader::access_person::AccessPersonLoader,
	pagination::{OffsetPagination, OffsetPaginationInfo},
};

/// The validity an invite can be given, in hours: an hour, a day, three days, a week, 30 days
pub const INVITE_HOURS: [i32; 5] = [1, 24, 72, 168, 720];

fn utc(at: DateTimeWithTimeZone) -> DateTime<Utc> {
	at.with_timezone(&Utc)
}

/// A page is at least the first and holds at least one row
#[derive(Default)]
pub struct AccessPaginationValidator;

impl CustomValidator<OffsetPagination> for AccessPaginationValidator {
	fn check(
		&self,
		input: &OffsetPagination,
	) -> Result<(), InputValueError<OffsetPagination>> {
		if !input.zero_based.unwrap_or(false) && input.page == 0 {
			Err(InputValueError::custom("Page must be greater than 0"))
		} else if input.page_size == Some(0) {
			Err(InputValueError::custom("Page size must be greater than 0"))
		} else {
			Ok(())
		}
	}
}

/// The 1-based page and the size to ask the service for
pub fn page_of(pagination: &OffsetPagination) -> (u64, u64) {
	let size = pagination.limit().max(1);
	(pagination.offset() / size + 1, size)
}

/// A person, as the owner sees them on this page
#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct AccessPerson {
	pub id: String,
	pub username: String,
	/// Locked: by too many failed sign-ins, or by the owner
	pub locked: bool,
	/// Removed: the account is gone, what it had on this page is what remains
	pub removed: bool,
	pub is_server_owner: bool,
}

impl From<user::Model> for AccessPerson {
	fn from(model: user::Model) -> Self {
		Self {
			locked: model.is_locked,
			removed: model.deleted_at.is_some(),
			is_server_owner: model.is_server_owner,
			id: model.id,
			username: model.username,
		}
	}
}

/// What is set up, and what is missing, for people to reach Stump from the internet
#[derive(Debug, Clone, SimpleObject)]
pub struct AccessOverview {
	/// There is a key (at least 64 characters) to sign passes with
	pub has_key: bool,
	/// The public address, which invite links are made from, is set
	pub has_public_url: bool,
	/// The folder the edge's state file goes in is set
	pub has_state_dir: bool,
	/// Everything needed to make invite links and have revocations followed is set
	pub configured: bool,
	/// A request through the public edge must carry a valid pass
	pub require_pass: bool,
	/// The address people reach Stump at from the internet
	pub public_url: Option<String>,
	/// When the app last wrote the file the edge follows
	pub state_written_at: Option<DateTime<Utc>>,
	/// Devices that hold a pass and were not revoked
	pub active_devices: u64,
	/// Invites that can still be redeemed
	pub open_invites: u64,
	/// Public sign-in is switched on (`STUMP_EDGE_PUBLIC_SIGNIN`): signup links and the sign-in
	/// page work from the internet, and so do reading-app addresses
	pub public_signin: bool,
	/// What the guard of the public sign-in page has seen lately
	pub signin_stats: AccessSigninStats,
	/// Signup links that can still make accounts
	pub active_signup_links: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AccessInviteStatus {
	/// Not used yet, still valid
	Unused,
	/// Used at least once, uses left
	Used,
	/// Every use is spent
	UsedUp,
	Expired,
	Revoked,
}

impl From<InviteStatus> for AccessInviteStatus {
	fn from(status: InviteStatus) -> Self {
		match status {
			InviteStatus::Unused => Self::Unused,
			InviteStatus::Used => Self::Used,
			InviteStatus::UsedUp => Self::UsedUp,
			InviteStatus::Expired => Self::Expired,
			InviteStatus::Revoked => Self::Revoked,
		}
	}
}

/// An invite link the owner made. The link itself is never kept: it was shown once
#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct AccessInvite {
	pub id: String,
	/// The device name the owner typed
	pub label: String,
	pub created_at: DateTime<Utc>,
	/// The admin who made it
	pub created_by: String,
	pub expires_at: DateTime<Utc>,
	pub max_uses: i32,
	pub uses: i32,
	pub status: AccessInviteStatus,
	#[graphql(skip)]
	person_id: String,
}

impl AccessInvite {
	pub fn new(model: access_invite::Model, now: DateTime<Utc>) -> Self {
		Self {
			status: service::invite_status(&model, now).into(),
			id: model.id,
			label: model.label,
			created_at: model.created_at,
			created_by: model.created_by,
			expires_at: model.expires_at,
			max_uses: model.max_uses,
			uses: model.uses,
			person_id: model.person_id,
		}
	}
}

#[ComplexObject]
impl AccessInvite {
	/// The person the invite is for
	async fn person(&self, ctx: &Context<'_>) -> Result<Option<AccessPerson>> {
		let loader = ctx.data::<DataLoader<AccessPersonLoader>>()?;
		Ok(loader.load_one(self.person_id.clone()).await?)
	}
}

/// The invite that was just made, with the only copy of its link
#[derive(Debug, Clone, SimpleObject)]
pub struct CreatedAccessInvite {
	pub invite: AccessInvite,
	/// The link to send. It exists only in this response: Stump keeps a hash of the token, so it
	/// can't show the link again
	pub link: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AccessDeviceStatus {
	Active,
	Revoked,
}

/// What a device is: a browser's pass, or a reading-app key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AccessDeviceKind {
	Browser,
	Opds,
}

impl AccessDeviceKind {
	pub fn parse(text: &str) -> Self {
		if text == service::device_kind::OPDS {
			Self::Opds
		} else {
			Self::Browser
		}
	}
}

/// A browser that holds a pass
#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct AccessDevice {
	/// The pass id
	pub id: String,
	/// The name the owner gave it ("Ana's phone"); a legacy pass is named after its id
	pub label: String,
	/// Made in the terminal (`invite.sh`): the pass is not tied to a person
	pub legacy: bool,
	pub created_at: DateTime<Utc>,
	pub first_seen_at: Option<DateTime<Utc>>,
	pub last_seen_at: Option<DateTime<Utc>>,
	/// The address of the last request that came with the pass
	pub last_ip: String,
	/// The browser of the last request that came with the pass
	pub last_agent: String,
	pub status: AccessDeviceStatus,
	pub revoked_at: Option<DateTime<Utc>>,
	pub revoked_by: Option<String>,
	/// A browser's pass, or a reading-app key
	pub kind: AccessDeviceKind,
	#[graphql(skip)]
	person_id: Option<String>,
}

impl From<access_device::Model> for AccessDevice {
	fn from(model: access_device::Model) -> Self {
		Self {
			legacy: model.person_id.is_none(),
			status: if model.revoked_at.is_some() {
				AccessDeviceStatus::Revoked
			} else {
				AccessDeviceStatus::Active
			},
			id: model.id,
			label: model.label,
			created_at: model.created_at,
			first_seen_at: model.first_seen_at,
			last_seen_at: model.last_seen_at,
			last_ip: model.last_ip,
			last_agent: model.last_agent,
			revoked_at: model.revoked_at,
			revoked_by: model.revoked_by,
			kind: AccessDeviceKind::parse(&model.kind),
			person_id: model.person_id,
		}
	}
}

#[ComplexObject]
impl AccessDevice {
	/// The person the device was made for. None for a legacy pass. A person who is removed is
	/// still returned, with `removed` set
	async fn person(&self, ctx: &Context<'_>) -> Result<Option<AccessPerson>> {
		let Some(person_id) = self.person_id.clone() else {
			return Ok(None);
		};
		let loader = ctx.data::<DataLoader<AccessPersonLoader>>()?;
		Ok(loader.load_one(person_id).await?)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AccessSignInReason {
	WrongPassword,
	/// The account was locked: the answer is the same whatever the password
	Locked,
	/// The server owner's password, through the public edge: refused whatever it was
	OwnerViaEdge,
	/// A pass made for somebody else
	WrongPerson,
	/// The right password of an account that is not allowed in from the internet
	NotAllowedPublicly,
	/// The right password, but the person already uses all their devices
	DeviceLimit,
}

fn sign_in_reason(text: &str) -> Option<AccessSignInReason> {
	match text {
		service::reason::WRONG_PASSWORD => Some(AccessSignInReason::WrongPassword),
		service::reason::LOCKED => Some(AccessSignInReason::Locked),
		service::reason::OWNER_VIA_EDGE => Some(AccessSignInReason::OwnerViaEdge),
		service::reason::WRONG_PERSON => Some(AccessSignInReason::WrongPerson),
		service::reason::NOT_ALLOWED_PUBLICLY => Some(AccessSignInReason::NotAllowedPublicly),
		service::reason::DEVICE_LIMIT => Some(AccessSignInReason::DeviceLimit),
		_ => None,
	}
}

/// One sign-in attempt of the account's login activity, with what is known of how it came
#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct AccessSignIn {
	pub id: i64,
	pub at: DateTime<Utc>,
	pub ip: String,
	pub user_agent: String,
	pub successful: bool,
	/// The pass the sign-in came with, when it came through the public edge
	pub device_id: Option<String>,
	/// The name the owner gave that device
	pub device_label: Option<String>,
	/// Why a failure failed, when it is known
	pub reason: Option<AccessSignInReason>,
	/// A failure the owner forgave when they unblocked the account: it no longer counts towards
	/// a lock. The attempts that never counted (the owner's password through the edge, somebody
	/// else's pass) are not "cleared"
	pub cleared: bool,
	#[graphql(skip)]
	person_id: String,
}

impl From<SignInRow> for AccessSignIn {
	fn from(row: SignInRow) -> Self {
		let reason = row
			.mark
			.as_ref()
			.and_then(|mark| mark.failure_reason.as_deref())
			.and_then(sign_in_reason);
		let never_counted = matches!(
			reason,
			Some(AccessSignInReason::OwnerViaEdge | AccessSignInReason::WrongPerson)
		);
		let cleared = !row.activity.authentication_successful
			&& !never_counted
			&& row
				.mark
				.as_ref()
				.is_some_and(|mark| !mark.counts_toward_lock);
		Self {
			id: row.activity.id,
			at: utc(row.activity.timestamp),
			ip: row.activity.ip_address,
			user_agent: row.activity.user_agent,
			successful: row.activity.authentication_successful,
			device_id: row.mark.as_ref().and_then(|mark| mark.device_id.clone()),
			device_label: row.device_label,
			reason,
			cleared,
			person_id: row.activity.user_id,
		}
	}
}

#[ComplexObject]
impl AccessSignIn {
	/// Whose account the attempt was for
	async fn person(&self, ctx: &Context<'_>) -> Result<Option<AccessPerson>> {
		let loader = ctx.data::<DataLoader<AccessPersonLoader>>()?;
		Ok(loader.load_one(self.person_id.clone()).await?)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AccessLockedBy {
	/// Too many failed sign-ins
	Failures,
	/// The owner locked the account
	Owner,
	/// Locked before this was recorded
	Unknown,
}

impl From<LockedBy> for AccessLockedBy {
	fn from(locked_by: LockedBy) -> Self {
		match locked_by {
			LockedBy::Failures => Self::Failures,
			LockedBy::Owner => Self::Owner,
			LockedBy::Unknown => Self::Unknown,
		}
	}
}

/// An account that is locked right now
#[derive(Debug, Clone, SimpleObject)]
pub struct AccessBlockedUser {
	pub person: AccessPerson,
	/// Failed sign-ins in the last 24 hours (every one, whether it counts or not)
	pub failures_last_24h: u64,
	pub last_failure_at: Option<DateTime<Utc>>,
	pub locked_by: AccessLockedBy,
}

impl From<BlockedUser> for AccessBlockedUser {
	fn from(blocked: BlockedUser) -> Self {
		Self {
			failures_last_24h: blocked.failures_last_24h,
			last_failure_at: blocked.last_failure_at.map(utc),
			locked_by: blocked.locked_by.into(),
			person: blocked.user.into(),
		}
	}
}

/// An account and how it stands: for the owner to set a password or unblock it, and to decide
/// whether it may come in from the internet
#[derive(Debug, Clone, SimpleObject)]
pub struct AccessPersonRow {
	pub person: AccessPerson,
	pub last_sign_in_at: Option<DateTime<Utc>>,
	/// The owner set their password and they haven't chosen their own yet
	pub must_change_password: bool,
	/// May sign in from the internet (and use it afterwards)
	pub public_signin: bool,
	/// Devices they may have
	pub max_devices: i32,
	/// Devices that count against the limit now
	pub active_devices: u64,
	/// Sign-in sessions that have not expired
	pub sessions: u64,
	/// Libraries made from now on are hidden from them
	pub new_libraries_hidden: bool,
	/// The signup link that made the account, while it is still kept
	pub signup_link_id: Option<String>,
	/// The permissions that keep the account out of public sign-in: until they are removed it
	/// can't be turned on
	pub beyond_friend_set: Vec<UserPermission>,
	/// How many libraries are hidden from them
	pub hidden_libraries: u64,
}

impl AccessPersonRow {
	pub fn new(person: Person, summary: Option<PersonAccess>) -> Self {
		let summary = summary.unwrap_or(PersonAccess {
			public_signin: false,
			max_devices: 3,
			new_libraries_hidden: true,
			signup_link_id: None,
			active_devices: 0,
			sessions: 0,
			beyond_friend_set: vec![],
			hidden_libraries: 0,
		});
		Self {
			last_sign_in_at: person.last_sign_in_at.map(utc),
			must_change_password: person.must_change_password,
			person: person.user.into(),
			public_signin: summary.public_signin,
			max_devices: summary.max_devices,
			active_devices: summary.active_devices,
			sessions: summary.sessions,
			new_libraries_hidden: summary.new_libraries_hidden,
			signup_link_id: summary.signup_link_id,
			beyond_friend_set: summary.beyond_friend_set,
			hidden_libraries: summary.hidden_libraries,
		}
	}
}

/// A row of the audit log: what the owner did, or a redemption of an invite. Never a token, a
/// link or a password
#[derive(Debug, Clone, SimpleObject)]
pub struct AccessAuditEntry {
	pub id: i32,
	pub at: DateTime<Utc>,
	/// The admin's username, `system`, or `public` for a redemption
	pub actor: String,
	/// `invite.create`, `device.revoke`, ...
	pub action: String,
	/// An invite id, a device id or a user id
	pub target: String,
	pub detail: String,
}

impl From<access_audit::Model> for AccessAuditEntry {
	fn from(model: access_audit::Model) -> Self {
		Self {
			id: model.id,
			at: model.at,
			actor: model.actor,
			action: model.action,
			target: model.target,
			detail: model.detail,
		}
	}
}

#[derive(Debug, SimpleObject)]
pub struct AccessInvitePage {
	pub nodes: Vec<AccessInvite>,
	pub page_info: OffsetPaginationInfo,
}

#[derive(Debug, SimpleObject)]
pub struct AccessDevicePage {
	pub nodes: Vec<AccessDevice>,
	pub page_info: OffsetPaginationInfo,
}

#[derive(Debug, SimpleObject)]
pub struct AccessSignInPage {
	pub nodes: Vec<AccessSignIn>,
	pub page_info: OffsetPaginationInfo,
}

#[derive(Debug, SimpleObject)]
pub struct AccessAuditPage {
	pub nodes: Vec<AccessAuditEntry>,
	pub page_info: OffsetPaginationInfo,
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateAccessInviteInput {
	/// The user the link is for: anyone but the server owner
	pub person_id: String,
	/// What the device is called ("Ana's phone"), up to 40 characters
	pub label: String,
	/// How long the link works: 1, 24, 72, 168 or 720 hours
	pub valid_for_hours: i32,
	/// How many times it can be opened (each one makes a device), 1 to 10
	pub max_uses: i32,
}

#[cfg(test)]
mod tests {
	use super::*;
	use models::entity::{access_signin_mark, user_login_activity};

	fn row(successful: bool, mark: Option<(Option<&str>, bool)>) -> SignInRow {
		let now = DateTimeWithTimeZone::from(Utc::now());
		SignInRow {
			activity: user_login_activity::Model {
				id: 7,
				ip_address: "203.0.113.9".to_string(),
				user_agent: "agent".to_string(),
				authentication_successful: successful,
				timestamp: now,
				user_id: "u1".to_string(),
			},
			mark: mark.map(|(reason, counts)| access_signin_mark::Model {
				login_activity_id: 7,
				device_id: Some("ana-phone-k3f9x".to_string()),
				failure_reason: reason.map(str::to_string),
				counts_toward_lock: counts,
			}),
			device_label: Some("Ana's phone".to_string()),
		}
	}

	#[test]
	fn a_forgiven_failure_is_cleared_and_an_ignored_one_is_not() {
		let wrong = AccessSignIn::from(row(false, Some((Some("wrong_password"), true))));
		assert!(!wrong.cleared);
		assert_eq!(wrong.reason, Some(AccessSignInReason::WrongPassword));

		let forgiven =
			AccessSignIn::from(row(false, Some((Some("wrong_password"), false))));
		assert!(forgiven.cleared);

		let forgiven_without_reason = AccessSignIn::from(row(false, Some((None, false))));
		assert!(forgiven_without_reason.cleared);
		assert_eq!(forgiven_without_reason.reason, None);

		// attempts that never counted were not forgiven: they were never held against anyone
		for reason in ["owner_via_edge", "wrong_person"] {
			let ignored = AccessSignIn::from(row(false, Some((Some(reason), false))));
			assert!(!ignored.cleared, "{reason}");
			assert!(ignored.reason.is_some());
		}

		// a success is never "cleared"
		assert!(!AccessSignIn::from(row(true, Some((None, false)))).cleared);
		// an unmarked failure counts, as it always has
		assert!(!AccessSignIn::from(row(false, None)).cleared);
	}

	#[test]
	fn the_page_asked_for_is_the_page_of_the_service() {
		let pagination = OffsetPagination {
			page: 3,
			page_size: Some(50),
			zero_based: Some(false),
		};
		assert_eq!(page_of(&pagination), (3, 50));
		let zero = OffsetPagination {
			page: 0,
			page_size: Some(50),
			zero_based: Some(true),
		};
		assert_eq!(page_of(&zero), (1, 50));
	}
}
