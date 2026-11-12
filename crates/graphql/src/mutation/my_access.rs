use async_graphql::{Context, Object, Result, ID};
use chrono::Utc;
use models::{services::access as service, shared::enums::UserPermission};
use tower_sessions::Session;

use super::access::{access_error, tell_the_edge};
use crate::{
	data::{AuthContext, CoreContext, EdgeRequest},
	guard::SameOriginGuard,
	object::my_access::{MyDevice, MyReadingApp},
	query::my_access::reading_app_url,
};

/// What a person does with their own devices and reading apps. Not owner-only and **not** refused
/// through the public edge, because people do it from the internet; every one is for the
/// signed-in person's own rows (the id they pass must be theirs, or nothing happens), and a
/// browser another site steered here is refused
#[derive(Default)]
pub struct MyAccessMutation;

fn refuse_the_owner(user: &models::entity::user::AuthUser) -> Result<()> {
	if user.is_server_owner {
		return Err(
			"The server owner reads through the web app, or with an API key".into()
		);
	}
	Ok(())
}

#[Object]
impl MyAccessMutation {
	/// Renames one of your devices
	#[graphql(guard = "SameOriginGuard")]
	async fn rename_my_device(
		&self,
		ctx: &Context<'_>,
		id: ID,
		label: String,
	) -> Result<MyDevice> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let current = ctx
			.data_opt::<EdgeRequest>()
			.and_then(|edge| edge.device.as_ref())
			.map(|device| device.id.clone());

		let renamed = service::rename_device_of_person(conn, &user.id, id.as_str(), &label)
			.await
			.map_err(access_error)?
			.ok_or("That device was not found")?;
		Ok(MyDevice::new(renamed, current.as_deref()))
	}

	/// Removes one of your devices: its pass (or reading-app key) stops working, at once for the app
	/// and within seconds at the edge. Removing the device you are using also signs you out. Returns
	/// whether something was removed
	#[graphql(guard = "SameOriginGuard")]
	async fn remove_my_device(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let current = ctx
			.data_opt::<EdgeRequest>()
			.and_then(|edge| edge.device.as_ref())
			.map(|device| device.id.clone());

		let removed = service::remove_device_of_person(
			conn,
			Utc::now(),
			&user.id,
			&user.username,
			id.as_str(),
		)
		.await?;
		if removed.is_none() {
			return Ok(false);
		}
		tell_the_edge(ctx).await?;
		if current.as_deref() == Some(id.as_str()) {
			// the device you are on is gone: so is the session it holds
			if let Some(session) = ctx.data_opt::<Session>() {
				session.delete().await?;
			}
		}
		Ok(true)
	}

	/// Makes a reading-app address (for KOReader and other OPDS readers) and counts it as one of
	/// your devices. Needs *Download files*. Refused when you already use all your devices
	#[graphql(guard = "SameOriginGuard")]
	async fn create_reading_app(
		&self,
		ctx: &Context<'_>,
		label: String,
	) -> Result<MyReadingApp> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		refuse_the_owner(user)?;
		ensure_can_read_with_an_app(user)?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let created = service::create_reading_app(
			conn,
			Utc::now(),
			&user.id,
			&user.username,
			&label,
			None,
		)
		.await
		.map_err(access_error)?
		.ok_or_else(|| device_limit_message())?;
		my_reading_app(ctx, created)
	}

	/// Replaces a reading-app address with a new one: the old one stops working at once
	#[graphql(guard = "SameOriginGuard")]
	async fn replace_reading_app(&self, ctx: &Context<'_>, id: ID) -> Result<MyReadingApp> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		refuse_the_owner(user)?;
		ensure_can_read_with_an_app(user)?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let old = service::reading_apps_of_person(conn, &user.id)
			.await?
			.into_iter()
			.find(|app| app.id == id.as_str())
			.ok_or("That reading app was not found")?;
		let created = service::create_reading_app(
			conn,
			Utc::now(),
			&user.id,
			&user.username,
			&old.label,
			Some(&old.id),
		)
		.await
		.map_err(access_error)?
		.ok_or_else(|| device_limit_message())?;
		tell_the_edge(ctx).await?;
		my_reading_app(ctx, created)
	}

	/// Removes a reading-app address: it stops working at once. Returns whether something was
	/// removed
	#[graphql(guard = "SameOriginGuard")]
	async fn remove_reading_app(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let is_a_reading_app = service::reading_apps_of_person(conn, &user.id)
			.await?
			.iter()
			.any(|app| app.id == id.as_str());
		if !is_a_reading_app {
			return Ok(false);
		}
		let removed = service::remove_device_of_person(
			conn,
			Utc::now(),
			&user.id,
			&user.username,
			id.as_str(),
		)
		.await?
		.is_some();
		if removed {
			tell_the_edge(ctx).await?;
		}
		Ok(removed)
	}
}

fn ensure_can_read_with_an_app(user: &models::entity::user::AuthUser) -> Result<()> {
	if !user.permissions.contains(&UserPermission::DownloadFile) {
		return Err(
			"You can't download books, so a reading app would have nothing to read".into(),
		);
	}
	Ok(())
}

fn device_limit_message() -> async_graphql::Error {
	"You already use all your devices. Remove one in My devices first, or ask the person who runs this server"
		.into()
}

fn my_reading_app(
	ctx: &Context<'_>,
	device: models::entity::access_device::Model,
) -> Result<MyReadingApp> {
	Ok(MyReadingApp {
		url: reading_app_url(ctx, &device.id)?,
		id: device.id,
		label: device.label,
		created_at: device.created_at,
		last_seen_at: device.last_seen_at,
	})
}
