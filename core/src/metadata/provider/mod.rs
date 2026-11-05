mod apply;
mod cache;
mod fetch;
mod job;

pub use apply::{apply_media_match, apply_series_match, find_auto_apply_candidate};
pub use cache::{ProviderCacheError, ProviderClientCache};
pub use fetch::{fetch_media_metadata, fetch_series_metadata};
pub use job::{
	MetadataFetchJob, MetadataFetchJobOutput, MetadataFetchJobParams, MetadataFetchScope,
};

/// Whether scans ask the metadata providers about the books they add, and the scheduled
/// retries run: the server's setting (*Settings → Metadata*), on unless turned off. Fetches
/// someone starts, and the online lookups, don't depend on it
pub async fn fetch_after_scan_enabled<C>(conn: &C) -> bool
where
	C: sea_orm::ConnectionTrait,
{
	use sea_orm::EntityTrait;
	models::entity::server_config::Entity::find()
		.one(conn)
		.await
		.ok()
		.flatten()
		.is_none_or(|config| config.fetch_metadata_after_scan)
}
