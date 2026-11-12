//! A person's own devices and reading apps, for the person only (*Settings → My devices* and
//! *Reading apps*). Every resolver that makes these is scoped by the signed-in user's id, never by
//! an id the caller passes alone: a person never reaches somebody else's.

use async_graphql::SimpleObject;
use chrono::{DateTime, Utc};
use models::entity::access_device;

use super::access::AccessDeviceKind;

/// One of the person's devices: a browser that holds a pass, or a reading-app key
#[derive(Debug, Clone, SimpleObject)]
pub struct MyDevice {
	pub id: String,
	/// The name shown in the list ("Chrome on Android"); the person can rename it
	pub label: String,
	pub kind: AccessDeviceKind,
	/// This is the device the request came from
	pub current: bool,
	pub created_at: DateTime<Utc>,
	pub last_seen_at: Option<DateTime<Utc>>,
	/// The address of the last request that came with it
	pub last_ip: String,
}

impl MyDevice {
	pub fn new(model: access_device::Model, current_device: Option<&str>) -> Self {
		Self {
			current: current_device == Some(model.id.as_str()),
			kind: AccessDeviceKind::parse(&model.kind),
			id: model.id,
			label: model.label,
			created_at: model.created_at,
			last_seen_at: model.last_seen_at,
			last_ip: model.last_ip,
		}
	}
}

#[derive(Debug, Clone, SimpleObject)]
pub struct MyDevices {
	pub devices: Vec<MyDevice>,
	/// Devices that count against the limit now (one unseen for 60 days no longer counts)
	pub active: u64,
	/// How many devices the person may have
	pub limit: i32,
}

/// A reading-app address: the key is part of the address, so keep it to yourself
#[derive(Debug, Clone, SimpleObject)]
pub struct MyReadingApp {
	pub id: String,
	pub label: String,
	/// The address to give the reading app (OPDS catalogue). Leave its username and password empty
	pub url: String,
	pub created_at: DateTime<Utc>,
	pub last_seen_at: Option<DateTime<Utc>>,
}
