use async_graphql::{Context, Object, Result, SimpleObject};
use models::{entity::server_config, shared::enums::UserPermission};
use sea_orm::EntityTrait;

use crate::{data::CoreContext, guard::PermissionGuard};

#[derive(Default)]
pub struct ServerConfigQuery;

/// The online lookups of *Add books* and *Suggest genres* (Open Library and Google Books)
#[derive(Debug, Clone, SimpleObject)]
pub struct OnlineLookupSettings {
	/// Whether a Google Books API key is set (the key itself is never sent back). Without one,
	/// Google Books lookups share Google's anonymous daily quota, which is often used up
	pub google_books_api_key_set: bool,
}

impl OnlineLookupSettings {
	pub fn of(config: &server_config::Model) -> Self {
		Self {
			google_books_api_key_set: config
				.google_books_api_key
				.as_deref()
				.is_some_and(|k| !k.is_empty()),
		}
	}
}

#[Object]
impl ServerConfigQuery {
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageServer)")]
	async fn server_config(&self, ctx: &Context<'_>) -> Result<server_config::Model> {
		let core = ctx.data::<CoreContext>()?;

		let config = server_config::Entity::find()
			.one(core.conn.as_ref())
			.await?
			.ok_or("Server configuration not found")?;

		Ok(config)
	}

	/// The online lookups' settings (whether a Google Books API key is set)
	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageServer)")]
	async fn online_lookup_settings(
		&self,
		ctx: &Context<'_>,
	) -> Result<OnlineLookupSettings> {
		let core = ctx.data::<CoreContext>()?;
		let config = server_config::Entity::find()
			.one(core.conn.as_ref())
			.await?
			.ok_or("Server configuration not found")?;
		Ok(OnlineLookupSettings::of(&config))
	}
}
