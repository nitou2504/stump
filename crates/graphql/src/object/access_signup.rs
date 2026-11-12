//! The owner's side of friend onboarding in *Public access*: signup links (what the owner makes
//! and what each one created), the preset an account gets, the warnings, and the new numbers of
//! the People table.
//!
//! Like the rest of the page these are behind the owner-only guards of `query::access` and
//! `mutation::access` and refused on requests that came through the public edge. No object has a
//! field for a token or a password; the link itself is in the response of `createAccessSignupLink`
//! once and nowhere else.

use async_graphql::{Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};
use models::{
	entity::access_signup_link,
	services::{
		access_policy,
		access_warnings::{Severity, Warning, WarningKind},
		signup::{self, Preset, PresetAge, SignupKind, SignupStatus},
	},
	shared::enums::UserPermission,
};

use super::access::AccessPerson;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum SignupLinkKind {
	/// For one person: one use
	Single,
	/// For several people: a budget of accounts
	Open,
}

impl From<SignupKind> for SignupLinkKind {
	fn from(kind: SignupKind) -> Self {
		match kind {
			SignupKind::Single => Self::Single,
			SignupKind::Open => Self::Open,
		}
	}
}

impl From<SignupLinkKind> for SignupKind {
	fn from(kind: SignupLinkKind) -> Self {
		match kind {
			SignupLinkKind::Single => Self::Single,
			SignupLinkKind::Open => Self::Open,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum SignupLinkStatus {
	/// Can make accounts
	Active,
	/// Every use is spent
	UsedUp,
	Expired,
	Revoked,
}

impl From<SignupStatus> for SignupLinkStatus {
	fn from(status: SignupStatus) -> Self {
		match status {
			SignupStatus::Active => Self::Active,
			SignupStatus::UsedUp => Self::UsedUp,
			SignupStatus::Expired => Self::Expired,
			SignupStatus::Revoked => Self::Revoked,
		}
	}
}

/// A library a preset names
#[derive(Debug, Clone, SimpleObject)]
pub struct PresetLibrary {
	pub id: String,
	pub name: String,
}

#[derive(Debug, Clone, Copy, SimpleObject)]
pub struct SignupPresetAge {
	pub age: i32,
	pub restrict_on_unset: bool,
}

/// What an account made through a link gets
#[derive(Debug, Clone, SimpleObject)]
pub struct SignupPreset {
	pub permissions: Vec<UserPermission>,
	/// The libraries the account sees (the ones that still exist): every other library is hidden
	/// from it, and so are libraries made later
	pub libraries: Vec<PresetLibrary>,
	pub age_restriction: Option<SignupPresetAge>,
	/// Devices the person may have
	pub max_devices: i32,
}

impl SignupPreset {
	pub fn new(preset: Preset, libraries: Vec<PresetLibrary>) -> Self {
		Self {
			permissions: preset.permissions,
			libraries,
			age_restriction: preset.age_restriction.map(|age| SignupPresetAge {
				age: age.age,
				restrict_on_unset: age.restrict_on_unset,
			}),
			max_devices: preset.max_devices,
		}
	}

	/// What a link offers by default (the owner may change it per link)
	pub fn defaults(libraries: Vec<PresetLibrary>) -> Self {
		Self {
			permissions: access_policy::DEFAULT_PRESET_PERMISSIONS.to_vec(),
			libraries,
			age_restriction: None,
			max_devices: access_policy::DEFAULT_MAX_DEVICES,
		}
	}
}

/// A signup link the owner made. The link itself is never kept: it was shown once
#[derive(Debug, Clone, SimpleObject)]
pub struct AccessSignupLink {
	pub id: String,
	/// The owner's note ("book club")
	pub label: String,
	pub kind: SignupLinkKind,
	pub created_at: DateTime<Utc>,
	/// The admin who made it
	pub created_by: String,
	pub expires_at: DateTime<Utc>,
	/// The budget: how many accounts it may create
	pub max_uses: i32,
	/// How many it has created
	pub uses: i32,
	pub status: SignupLinkStatus,
	pub revoked_at: Option<DateTime<Utc>>,
	pub preset: SignupPreset,
	/// The accounts it made (the ones that still exist)
	pub people: Vec<AccessPerson>,
}

impl AccessSignupLink {
	pub fn new(
		link: access_signup_link::Model,
		now: DateTime<Utc>,
		libraries: Vec<PresetLibrary>,
		people: Vec<AccessPerson>,
	) -> Self {
		let status = signup::signup_status(&link, now).into();
		let preset = signup::preset_of(&link).unwrap_or(Preset {
			permissions: vec![],
			library_ids: vec![],
			age_restriction: None,
			max_devices: access_policy::DEFAULT_MAX_DEVICES,
		});
		Self {
			status,
			kind: SignupKind::parse(&link.kind)
				.map_or(SignupLinkKind::Single, Into::into),
			preset: SignupPreset::new(preset, libraries),
			id: link.id,
			label: link.label,
			created_at: link.created_at,
			created_by: link.created_by,
			expires_at: link.expires_at,
			max_uses: link.max_uses,
			uses: link.uses,
			revoked_at: link.revoked_at,
			people,
		}
	}
}

#[derive(Debug, SimpleObject)]
pub struct AccessSignupLinkPage {
	pub nodes: Vec<AccessSignupLink>,
	pub page_info: crate::pagination::OffsetPaginationInfo,
}

/// A link that was just made: the address is in this response and nowhere else
#[derive(Debug, Clone, SimpleObject)]
pub struct CreatedSignupLink {
	pub link: AccessSignupLink,
	pub url: String,
}

#[derive(Debug, Clone, Copy, InputObject)]
pub struct SignupPresetAgeInput {
	pub age: i32,
	pub restrict_on_unset: bool,
}

#[derive(Debug, Clone, InputObject)]
pub struct SignupPresetInput {
	/// Only permissions a friend may have: anything else is refused
	pub permissions: Vec<UserPermission>,
	pub library_ids: Vec<ID>,
	pub age_restriction: Option<SignupPresetAgeInput>,
	#[graphql(default = 3)]
	pub max_devices: i32,
}

impl SignupPresetInput {
	pub fn into_preset(self) -> Preset {
		Preset {
			permissions: self.permissions,
			library_ids: self.library_ids.into_iter().map(|id| id.to_string()).collect(),
			age_restriction: self.age_restriction.map(|age| PresetAge {
				age: age.age,
				restrict_on_unset: age.restrict_on_unset,
			}),
			max_devices: self.max_devices,
		}
	}
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateSignupLinkInput {
	/// A note so the owner remembers who it is for
	pub label: String,
	pub kind: SignupLinkKind,
	/// How long it works, in hours (1 hour to 30 days)
	#[graphql(default = 72)]
	pub valid_for_hours: i32,
	/// How many accounts it may make: ignored for one person (1), 2 to 25 for an open link
	#[graphql(default = 1)]
	pub max_uses: i32,
	pub preset: SignupPresetInput,
}

#[derive(Debug, Clone, InputObject)]
pub struct PersonAccessInput {
	pub person_id: ID,
	/// May the person sign in from the internet? Refused for an account that holds permissions a
	/// friend may not have (the error names them)
	pub public_signin: Option<bool>,
	pub max_devices: Option<i32>,
	/// Hide libraries made from now on from this person
	pub new_libraries_hidden: Option<bool>,
	/// "Only these libraries": the person sees exactly these, every other library is hidden
	pub only_library_ids: Option<Vec<ID>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AccessWarningKind {
	AtDeviceLimit,
	OverDeviceLimit,
	ManyDevices,
	DeviceLimitHit,
	ManySessions,
	SigninFailures,
	NameBackoff,
	GlobalBudget,
	NotEnabledTried,
	NotFriendClass,
	LinkNearBudget,
	LinkNoLibraries,
}

impl From<WarningKind> for AccessWarningKind {
	fn from(kind: WarningKind) -> Self {
		match kind {
			WarningKind::AtDeviceLimit => Self::AtDeviceLimit,
			WarningKind::OverDeviceLimit => Self::OverDeviceLimit,
			WarningKind::ManyDevices => Self::ManyDevices,
			WarningKind::DeviceLimitHit => Self::DeviceLimitHit,
			WarningKind::ManySessions => Self::ManySessions,
			WarningKind::SigninFailures => Self::SigninFailures,
			WarningKind::NameBackoff => Self::NameBackoff,
			WarningKind::GlobalBudget => Self::GlobalBudget,
			WarningKind::NotEnabledTried => Self::NotEnabledTried,
			WarningKind::NotFriendClass => Self::NotFriendClass,
			WarningKind::LinkNearBudget => Self::LinkNearBudget,
			WarningKind::LinkNoLibraries => Self::LinkNoLibraries,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AccessWarningSeverity {
	Warn,
	Alert,
}

impl From<Severity> for AccessWarningSeverity {
	fn from(severity: Severity) -> Self {
		match severity {
			Severity::Warn => Self::Warn,
			Severity::Alert => Self::Alert,
		}
	}
}

/// Something the owner should look at
#[derive(Debug, Clone, SimpleObject)]
pub struct AccessWarning {
	/// The same warning has the same id every time
	pub id: String,
	pub kind: AccessWarningKind,
	pub severity: AccessWarningSeverity,
	pub person: Option<AccessPerson>,
	pub link_id: Option<String>,
	/// How many times, or how many devices
	pub count: i64,
	pub since: DateTime<Utc>,
	/// A short plain-text detail (a username, a note, the permissions)
	pub detail: Option<String>,
}

impl AccessWarning {
	pub fn new(warning: Warning, person: Option<AccessPerson>) -> Self {
		Self {
			id: warning.id(),
			kind: warning.kind.into(),
			severity: warning.severity.into(),
			person,
			link_id: warning.link_id,
			count: warning.count,
			since: warning.since,
			detail: warning.detail,
		}
	}
}

/// Counters of the guard of the public sign-in page: numbers only, never names
#[derive(Debug, Clone, Copy, SimpleObject)]
pub struct AccessSigninStats {
	/// Failed attempts counted in the last 10 minutes
	pub failures_last_10_min: u32,
	/// Names that are waiting out a delay
	pub names_waiting: u32,
	/// Public sign-in is refusing everybody for a few minutes
	pub global_budget_tripped: bool,
}
