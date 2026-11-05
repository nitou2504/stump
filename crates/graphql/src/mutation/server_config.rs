use crate::{
	data::CoreContext,
	guard::PermissionGuard,
	query::server_config::{MetadataFetchSettings, OnlineLookupSettings},
};
use async_graphql::{Context, Object, Result};
use models::{entity::server_config, shared::enums::UserPermission};
use sea_orm::{prelude::*, ActiveValue::Set, IntoActiveModel};
use stump_core::utils::encryption::encrypt_string;

#[derive(Default)]
pub struct ServerConfigMutation;

// TODO: add hook into refreshing JWT secrets, so that if somehow they were compromised we have a friendly way for
// folks to rotate em. it's stored in db so if compromised i think you have bigger problems, but still

#[Object]
impl ServerConfigMutation {
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageServer)")]
	async fn update_public_url(
		&self,
		ctx: &Context<'_>,
		public_url: String,
	) -> Result<server_config::Model> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let config = server_config::Entity::find()
			.one(conn)
			.await?
			// Note: This would be a pretty critical bug
			.ok_or("Server configuration not found")?;

		let mut config = config.into_active_model();
		let trimmed = public_url.trim();
		config.public_url = Set(if trimmed.is_empty() {
			None
		} else {
			Some(trimmed.to_string())
		});
		// ^ empty urls would be invalid, but see also:
		// https://discord.com/channels/972593831172272148/1565087706252447865/1565087706252447865

		let updated_config = config.update(conn).await?;
		Ok(updated_config)
	}

	/// Sets (or, with null or an empty string, removes) the Google Books API key the online
	/// lookups of *Add books* and *Suggest genres* use. It is stored encrypted and never sent
	/// back (`onlineLookupSettings` only says whether one is set)
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageServer)")]
	async fn update_online_lookup_settings(
		&self,
		ctx: &Context<'_>,
		google_books_api_key: Option<String>,
	) -> Result<OnlineLookupSettings> {
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();
		let config = server_config::Entity::find()
			.one(conn)
			.await?
			.ok_or("Server configuration not found")?;
		let key = google_books_api_key
			.map(|k| k.trim().to_string())
			.filter(|k| !k.is_empty());
		let stored = match key {
			Some(key) => {
				if key.contains(char::is_whitespace) || key.len() > 200 {
					return Err("That doesn't look like a Google API key".into());
				}
				let encryption_key = core.get_encryption_key().await?;
				Some(encrypt_string(&key, &encryption_key)?)
			},
			None => None,
		};
		let mut active = config.into_active_model();
		active.google_books_api_key = Set(stored);
		let updated = active.update(conn).await?;
		Ok(OnlineLookupSettings::of(&updated))
	}

	/// Turns the automatic metadata fetches on or off: what a scan asks the metadata
	/// providers about the books it added, and the scheduled retries
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageServer)")]
	async fn update_metadata_fetch_settings(
		&self,
		ctx: &Context<'_>,
		fetch_after_scan: bool,
	) -> Result<MetadataFetchSettings> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let config = server_config::Entity::find()
			.one(conn)
			.await?
			.ok_or("Server configuration not found")?;
		let mut active = config.into_active_model();
		active.fetch_metadata_after_scan = Set(fetch_after_scan);
		let updated = active.update(conn).await?;
		Ok(MetadataFetchSettings::of(&updated))
	}
}
