use async_graphql::{ComplexObject, SimpleObject};
use metadata_integrations::MetadataField;
use models::entity::media_metadata;
use stump_core::utils::serde::comma_separated_list_to_vec;

#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct MediaMetadata {
	#[graphql(flatten)]
	pub model: media_metadata::Model,
}

impl From<media_metadata::Model> for MediaMetadata {
	fn from(model: media_metadata::Model) -> Self {
		Self { model }
	}
}

#[ComplexObject]
impl MediaMetadata {
	async fn writers(&self) -> Vec<String> {
		self.model
			.writers
			.clone()
			.map(comma_separated_list_to_vec)
			.unwrap_or_default()
	}

	async fn genres(&self) -> Vec<String> {
		self.model
			.genres
			.clone()
			.map(comma_separated_list_to_vec)
			.unwrap_or_default()
	}

	async fn characters(&self) -> Vec<String> {
		self.model
			.characters
			.clone()
			.map(comma_separated_list_to_vec)
			.unwrap_or_default()
	}

	async fn colorists(&self) -> Vec<String> {
		self.model
			.colorists
			.clone()
			.map(comma_separated_list_to_vec)
			.unwrap_or_default()
	}

	async fn cover_artists(&self) -> Vec<String> {
		self.model
			.cover_artists
			.clone()
			.map(comma_separated_list_to_vec)
			.unwrap_or_default()
	}

	async fn editors(&self) -> Vec<String> {
		self.model
			.editors
			.clone()
			.map(comma_separated_list_to_vec)
			.unwrap_or_default()
	}

	async fn inkers(&self) -> Vec<String> {
		self.model
			.inkers
			.clone()
			.map(comma_separated_list_to_vec)
			.unwrap_or_default()
	}

	async fn letterers(&self) -> Vec<String> {
		self.model
			.letterers
			.clone()
			.map(comma_separated_list_to_vec)
			.unwrap_or_default()
	}

	async fn links(&self) -> Vec<String> {
		self.model
			.links
			.clone()
			.map(comma_separated_list_to_vec)
			.unwrap_or_default()
	}

	async fn pencillers(&self) -> Vec<String> {
		self.model
			.pencillers
			.clone()
			.map(comma_separated_list_to_vec)
			.unwrap_or_default()
	}

	async fn teams(&self) -> Vec<String> {
		self.model
			.teams
			.clone()
			.map(comma_separated_list_to_vec)
			.unwrap_or_default()
	}

	async fn locked_fields(&self) -> Vec<MetadataField> {
		self.model
			.locked_fields
			.as_ref()
			.and_then(|v| serde_json::from_value(v.clone()).ok())
			.unwrap_or_default()
	}
}

#[cfg(test)]
mod tests {
	use async_graphql::{EmptyMutation, EmptySubscription, Object, Schema};
	use serde_json::json;

	use super::*;

	struct Root(MediaMetadata);

	#[Object]
	impl Root {
		async fn metadata(&self) -> MediaMetadata {
			self.0.clone()
		}
	}

	/// What a client gets for `genres` and `writers` of a book stored with these two texts
	async fn lists_of(genres: Option<&str>, writers: Option<&str>) -> serde_json::Value {
		let book = MediaMetadata::from(media_metadata::Model {
			genres: genres.map(str::to_string),
			writers: writers.map(str::to_string),
			..Default::default()
		});
		let schema = Schema::build(Root(book), EmptyMutation, EmptySubscription).finish();
		let response = schema.execute("{ metadata { genres writers } }").await;
		assert!(response.errors.is_empty(), "{:?}", response.errors);
		response.data.into_json().unwrap()["metadata"].clone()
	}

	#[tokio::test]
	async fn a_stored_leading_comma_is_not_an_empty_genre_or_writer() {
		// what an EPUB with an empty first `<dc:subject/>` was stored as
		let lists = lists_of(
			Some(", Biology & Nature, Form: Reportage, Level: General Reader"),
			Some(", Charlotte Brontë"),
		)
		.await;
		assert_eq!(
			lists,
			json!({
				"genres": ["Biology & Nature", "Form: Reportage", "Level: General Reader"],
				"writers": ["Charlotte Brontë"],
			})
		);
	}

	#[tokio::test]
	async fn doubled_trailing_and_blank_lists_have_no_empty_items() {
		assert_eq!(
			lists_of(Some("A,, B, "), Some(" , ")).await,
			json!({ "genres": ["A", "B"], "writers": [] })
		);
		// no value at all is an empty list, as before
		assert_eq!(
			lists_of(None, None).await,
			json!({ "genres": [], "writers": [] })
		);
	}
}
