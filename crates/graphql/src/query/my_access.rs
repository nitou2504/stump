use async_graphql::{Context, Object, Result};
use chrono::Utc;
use models::services::access as service;
use stump_core::access::pass;

use crate::{
	data::{AuthContext, CoreContext, EdgeRequest, ServiceContext},
	object::my_access::{MyDevice, MyDevices, MyReadingApp},
};

/// The address a reading app is given for the key of device `device_id`: the public address when
/// the request came through the edge, the request's own address otherwise. Refused when the
/// server has no key to make one with
pub(crate) fn reading_app_url(ctx: &Context<'_>, device_id: &str) -> Result<String> {
	let core = ctx.data::<CoreContext>()?;
	let edge = core.config.edge.as_ref();
	let key = edge
		.and_then(|edge| edge.usable_key())
		.ok_or("Reading apps are not set up on this server")?;
	let via_public = ctx
		.data_opt::<EdgeRequest>()
		.is_some_and(|edge| edge.via_public);
	let base = if via_public {
		edge.and_then(|edge| edge.public_base_url())
			.ok_or("The public address is not set")?
	} else {
		ctx.data::<ServiceContext>()?.url()
	};
	Ok(format!(
		"{base}/opds/{}/v1.2/catalog",
		pass::mint_reading_key(key, device_id)
	))
}

/// A person's own devices and reading apps: for the person only. Every query is scoped by the
/// signed-in user's id; there is no argument that names somebody else. Not owner-only and not
/// refused through the public edge, because people use these from the internet
#[derive(Default)]
pub struct MyAccessQuery;

#[Object]
impl MyAccessQuery {
	/// The signed-in person's devices (browsers that hold a pass, reading-app keys) that are not
	/// removed, and how many they may have
	async fn my_devices(&self, ctx: &Context<'_>) -> Result<MyDevices> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let current = ctx
			.data_opt::<EdgeRequest>()
			.and_then(|edge| edge.device.as_ref())
			.map(|device| device.id.clone());

		let devices = service::devices_of_person(conn, &user.id).await?;
		Ok(MyDevices {
			devices: devices
				.into_iter()
				.map(|device| MyDevice::new(device, current.as_deref()))
				.collect(),
			active: service::count_person_devices(conn, Utc::now(), &user.id).await?,
			limit: service::max_devices_of(conn, &user.id).await?,
		})
	}

	/// The signed-in person's reading apps, each with the address to give the app
	async fn my_reading_apps(&self, ctx: &Context<'_>) -> Result<Vec<MyReadingApp>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let apps = service::reading_apps_of_person(conn, &user.id).await?;
		apps.into_iter()
			.map(|app| {
				Ok(MyReadingApp {
					url: reading_app_url(ctx, &app.id)?,
					id: app.id,
					label: app.label,
					created_at: app.created_at,
					last_seen_at: app.last_seen_at,
				})
			})
			.collect()
	}
}
