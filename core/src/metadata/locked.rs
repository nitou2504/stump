//! Locked metadata fields: values someone set by hand that no automatic process replaces.
//! Metadata fetches already respect them; rescans that rebuild a book's metadata from its
//! file keep them too (along with the lock list itself and the provider it came from).

use metadata_integrations::MetadataField;
use models::entity::media_metadata;
use sea_orm::ActiveValue::Set;

/// The locked fields stored on a metadata row
pub fn locked_fields_of(json: &Option<serde_json::Value>) -> Vec<MetadataField> {
	json.as_ref()
		.and_then(|v| serde_json::from_value(v.clone()).ok())
		.unwrap_or_default()
}

/// Keeps, in metadata rebuilt from a file, the values of the existing row's locked fields,
/// its lock list and its provider bookkeeping
pub fn keep_locked_fields(
	active: &mut media_metadata::ActiveModel,
	existing: &media_metadata::Model,
) {
	use MetadataField as F;
	active.locked_fields = Set(existing.locked_fields.clone());
	active.metadata_source = Set(existing.metadata_source.clone());
	active.metadata_external_id = Set(existing.metadata_external_id.clone());
	let e = existing;
	for field in locked_fields_of(&existing.locked_fields) {
		match field {
			F::Title => active.title = Set(e.title.clone()),
			F::TitleSort => active.title_sort = Set(e.title_sort.clone()),
			F::Summary | F::DescriptionFormatted => {
				active.summary = Set(e.summary.clone())
			},
			F::Genres => active.genres = Set(e.genres.clone()),
			F::Publisher => active.publisher = Set(e.publisher.clone()),
			F::Year | F::ReleaseDate => {
				active.year = Set(e.year);
				active.month = Set(e.month);
				active.day = Set(e.day);
			},
			F::AgeRating => active.age_rating = Set(e.age_rating),
			F::PageCount => active.page_count = Set(e.page_count),
			F::Isbn => active.identifier_isbn = Set(e.identifier_isbn.clone()),
			F::Colorists => active.colorists = Set(e.colorists.clone()),
			F::Letterers => active.letterers = Set(e.letterers.clone()),
			F::CoverArtists => active.cover_artists = Set(e.cover_artists.clone()),
			F::Writers => active.writers = Set(e.writers.clone()),
			F::Format => active.format = Set(e.format.clone()),
			F::Number => active.number = Set(e.number),
			F::Series => active.series = Set(e.series.clone()),
			F::SeriesGroup => active.series_group = Set(e.series_group.clone()),
			F::Notes => active.notes = Set(e.notes.clone()),
			F::Language => active.language = Set(e.language.clone()),
			F::Editors => active.editors = Set(e.editors.clone()),
			F::Inkers => active.inkers = Set(e.inkers.clone()),
			F::Teams => active.teams = Set(e.teams.clone()),
			F::Links => active.links = Set(e.links.clone()),
			F::Characters => active.characters = Set(e.characters.clone()),
			F::StoryArc => active.story_arc = Set(e.story_arc.clone()),
			F::StoryArcNumber => active.story_arc_number = Set(e.story_arc_number),
			F::Pencillers => active.pencillers = Set(e.pencillers.clone()),
			F::IdentifierAmazon => {
				active.identifier_amazon = Set(e.identifier_amazon.clone())
			},
			F::IdentifierCalibre => {
				active.identifier_calibre = Set(e.identifier_calibre.clone())
			},
			F::IdentifierGoogle => {
				active.identifier_google = Set(e.identifier_google.clone())
			},
			F::IdentifierMobiAsin => {
				active.identifier_mobi_asin = Set(e.identifier_mobi_asin.clone())
			},
			F::IdentifierUuid => active.identifier_uuid = Set(e.identifier_uuid.clone()),
			F::Rating => active.rating = Set(e.rating),
			// not stored on a book's metadata row
			F::Tags
			| F::Artists
			| F::Cover
			| F::Status
			| F::VolumeCount
			| F::BookType
			| F::Imprint
			| F::PublicationRun
			| F::ComicId
			| F::MetaType
			| F::ComicImage => {},
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::metadata::ProcessedMediaMetadata;

	#[test]
	fn rebuilt_metadata_keeps_locked_values_and_the_lock_list() {
		let existing = media_metadata::Model {
			genres: Some("Philosophy".into()),
			notes: Some("mine".into()),
			title: Some("Old title".into()),
			locked_fields: Some(serde_json::json!(["GENRES", "NOTES"])),
			metadata_source: Some("HARDCOVER".into()),
			..Default::default()
		};
		let from_file = ProcessedMediaMetadata {
			title: Some("New title".into()),
			genres: Some(vec!["Physics & Astronomy".into()]),
			..Default::default()
		};
		let mut active = from_file.into_active_model();
		keep_locked_fields(&mut active, &existing);
		assert_eq!(active.genres, Set(Some("Philosophy".to_string())));
		assert_eq!(active.notes, Set(Some("mine".to_string())));
		// not locked: the file wins
		assert_eq!(active.title, Set(Some("New title".to_string())));
		assert_eq!(active.locked_fields, Set(existing.locked_fields.clone()));
		assert_eq!(active.metadata_source, Set(Some("HARDCOVER".to_string())));
		assert_eq!(
			locked_fields_of(&existing.locked_fields),
			vec![MetadataField::Genres, MetadataField::Notes]
		);
	}
}
