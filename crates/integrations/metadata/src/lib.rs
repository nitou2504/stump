pub mod client;
pub mod error;
pub mod merge;
mod provider;
mod providers;
pub mod rate_limit;
pub mod scoring;
pub(crate) mod serde_utils;
pub mod types;

pub use client::build_client_with_retry;
pub use error::{MetadataProviderError, MetadataResult};
pub use merge::{AutoApplyConfig, FieldMerger, MergeStrategy, MetadataFieldOverride};
pub use provider::{MetadataProvider, ProviderCredentialVerification};
pub use rate_limit::RateLimiter;
pub use scoring::MatchScorer;
pub use types::{
	ConfidenceFactor, ExternalMediaMetadata, ExternalMetadata, ExternalSeriesMetadata,
	MatchCandidate, MediaType, MetadataField, PublicationStatus, SearchOutcome,
	SearchQuery,
};

use providers::ComicVineClient;
pub use providers::{
	hardcover_background_paused, GraphQLReply, HardcoverClient, RequestPriority,
	BACKGROUND_DAILY_BUDGET,
};

pub fn create_provider(
	provider_type: &str,
	api_token: String,
) -> MetadataResult<Box<dyn MetadataProvider + Send + Sync>> {
	create_provider_for(provider_type, api_token, RequestPriority::Interactive)
}

/// A provider client whose requests are for someone waiting, or for background work (which
/// Hardcover's client budgets and puts behind people's lookups)
pub fn create_provider_for(
	provider_type: &str,
	api_token: String,
	priority: RequestPriority,
) -> MetadataResult<Box<dyn MetadataProvider + Send + Sync>> {
	match provider_type {
		"COMIC_VINE" => Ok(Box::new(ComicVineClient::new(api_token, None))),
		"HARDCOVER" => Ok(Box::new(
			HardcoverClient::new(api_token, None).with_priority(priority),
		)),
		_ => Err(MetadataProviderError::UnsupportedProvider(
			provider_type.to_string(),
		)),
	}
}
