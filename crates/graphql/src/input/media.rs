use async_graphql::{InputObject, MaybeUndefined, OneofObject, ID};
use models::{
	entity::{bookmark, media_annotation, media_metadata, user::AuthUser},
	shared::{
		enums::{AnnotationStyle, MetadataProvider},
		readium::ReadiumLocator,
	},
};
use sea_orm::{prelude::*, ActiveValue::Set, IntoActiveModel};

#[derive(Debug, Clone, InputObject)]
pub struct EpubProgressInput {
	pub locator: ReadiumLocator,
	pub percentage: Option<Decimal>,
	pub is_complete: Option<bool>,
	pub elapsed_seconds_delta: Option<i64>,
	pub device_id: Option<String>,
	pub reset_elapsed_seconds: Option<bool>,
}

#[derive(Default, Debug, Clone, InputObject)]
pub struct PagedProgressInput {
	pub page: i32,
	pub elapsed_seconds_delta: Option<i64>,
	pub device_id: Option<String>,
	pub reset_elapsed_seconds: Option<bool>,
}

#[derive(Debug, Clone, OneofObject)]
pub enum MediaProgressInput {
	Epub(Box<EpubProgressInput>),
	Paged(PagedProgressInput),
}

impl MediaProgressInput {
	/// whether applying the input should first reset the elapsed seconds for the media progress
	pub fn reset_elapsed_seconds(&self) -> bool {
		match self {
			MediaProgressInput::Epub(epub_progress) => {
				epub_progress.reset_elapsed_seconds
			},
			MediaProgressInput::Paged(paged_progress) => {
				paged_progress.reset_elapsed_seconds
			},
		}
		.unwrap_or(false)
	}
}

/// The locator `type` of a PDF page (annotations and bookmarks of the PDF reader)
pub const PDF_LOCATOR_TYPE: &str = "application/pdf";

#[derive(InputObject)]
pub struct BookmarkInput {
	pub media_id: String,
	pub locator: ReadiumLocator,
	pub preview_content: Option<String>,
}

impl BookmarkInput {
	pub fn into_active_model(&self, user: &AuthUser) -> bookmark::ActiveModel {
		bookmark::ActiveModel {
			id: Set(Uuid::new_v4().to_string()),
			locator: Set(Some(self.locator.clone())),
			preview_content: Set(self.preview_content.clone()),
			media_id: Set(self.media_id.clone()),
			user_id: Set(user.id.clone()),
			// PDF bookmarks record their page (1-based) so paged clients can use it; EPUB
			// bookmarks keep -1 (a Readium position is not a page)
			page: Set(Some(if self.locator.r#type == PDF_LOCATOR_TYPE {
				self.locator.page().unwrap_or(-1)
			} else {
				-1
			})),
			..Default::default()
		}
	}
}

#[derive(Debug, Clone, InputObject)]
pub struct MediaMetadataInput {
	pub title: MaybeUndefined<String>,
	pub title_sort: MaybeUndefined<String>,
	pub series: MaybeUndefined<String>,
	pub series_group: MaybeUndefined<String>,
	pub story_arc: MaybeUndefined<String>,
	pub story_arc_number: MaybeUndefined<Decimal>,
	pub number: MaybeUndefined<Decimal>,
	pub volume: MaybeUndefined<i32>,
	pub summary: MaybeUndefined<String>,
	pub notes: MaybeUndefined<String>,
	pub genres: MaybeUndefined<Vec<String>>,
	pub format: MaybeUndefined<String>,
	pub year: MaybeUndefined<i32>,
	pub month: MaybeUndefined<i32>,
	pub day: MaybeUndefined<i32>,
	pub writers: MaybeUndefined<Vec<String>>,
	pub pencillers: MaybeUndefined<Vec<String>>,
	pub inkers: MaybeUndefined<Vec<String>>,
	pub colorists: MaybeUndefined<Vec<String>>,
	pub letterers: MaybeUndefined<Vec<String>>,
	pub cover_artists: MaybeUndefined<Vec<String>>,
	pub editors: MaybeUndefined<Vec<String>>,
	pub publisher: MaybeUndefined<String>,
	pub links: MaybeUndefined<Vec<String>>,
	pub characters: MaybeUndefined<Vec<String>>,
	pub teams: MaybeUndefined<Vec<String>>,
	pub page_count: MaybeUndefined<i32>,
	pub age_rating: MaybeUndefined<i32>,
	/// The book's rating, 0 to 5 (decimals allowed). EPUBs get calibre's half stars and a
	/// precise `bcurator:rating` meta, PDFs a `bcurator:rating=` keyword field, CBZs ComicInfo's
	/// `CommunityRating`
	pub rating: MaybeUndefined<Decimal>,
	pub identifier_amazon: MaybeUndefined<String>,
	pub identifier_calibre: MaybeUndefined<String>,
	pub identifier_google: MaybeUndefined<String>,
	pub identifier_isbn: MaybeUndefined<String>,
	pub identifier_mobi_asin: MaybeUndefined<String>,
	pub identifier_uuid: MaybeUndefined<String>,
	pub language: MaybeUndefined<String>,
}

impl MediaMetadataInput {
	/// Why the input can't be saved, if it can't
	pub fn problems(&self) -> Vec<String> {
		let mut problems = vec![];
		if let MaybeUndefined::Value(rating) = &self.rating {
			if *rating < Decimal::ZERO || *rating > Decimal::from(5) {
				problems.push(format!("rating must be between 0 and 5 (got {rating})"));
			}
		}
		if let MaybeUndefined::Value(age) = &self.age_rating {
			if *age < 0 {
				problems.push(format!("ageRating can't be negative (got {age})"));
			}
		}
		problems
	}
}

impl IntoActiveModel<media_metadata::ActiveModel> for MediaMetadataInput {
	fn into_active_model(self) -> media_metadata::ActiveModel {
		media_metadata::ActiveModel {
			title: metadata_value(self.title),
			title_sort: metadata_value(self.title_sort),
			series: metadata_value(self.series),
			series_group: metadata_value(self.series_group),
			story_arc: metadata_value(self.story_arc),
			story_arc_number: metadata_value(self.story_arc_number),
			number: metadata_value(self.number),
			volume: metadata_value(self.volume),
			summary: metadata_value(self.summary),
			notes: metadata_value(self.notes),
			genres: metadata_value(into_array_string(self.genres)),
			format: metadata_value(self.format),
			year: metadata_value(self.year),
			month: metadata_value(self.month),
			day: metadata_value(self.day),
			writers: metadata_value(into_array_string(self.writers)),
			pencillers: metadata_value(into_array_string(self.pencillers)),
			inkers: metadata_value(into_array_string(self.inkers)),
			colorists: metadata_value(into_array_string(self.colorists)),
			letterers: metadata_value(into_array_string(self.letterers)),
			cover_artists: metadata_value(into_array_string(self.cover_artists)),
			editors: metadata_value(into_array_string(self.editors)),
			publisher: metadata_value(self.publisher),
			links: metadata_value(into_array_string(self.links)),
			characters: metadata_value(into_array_string(self.characters)),
			teams: metadata_value(into_array_string(self.teams)),
			page_count: metadata_value(self.page_count),
			age_rating: metadata_value(self.age_rating),
			rating: metadata_value(self.rating),
			identifier_amazon: metadata_value(self.identifier_amazon),
			identifier_calibre: metadata_value(self.identifier_calibre),
			identifier_google: metadata_value(self.identifier_google),
			identifier_isbn: metadata_value(self.identifier_isbn),
			identifier_mobi_asin: metadata_value(self.identifier_mobi_asin),
			identifier_uuid: metadata_value(self.identifier_uuid),
			language: metadata_value(self.language),
			..Default::default()
		}
	}
}

fn metadata_value<T>(value: MaybeUndefined<T>) -> sea_orm::ActiveValue<Option<T>>
where
	Option<T>: Into<sea_orm::Value>,
{
	Option::<Option<T>>::from(value)
		.map(Set)
		.unwrap_or_default()
}

fn into_array_string(value: MaybeUndefined<Vec<String>>) -> MaybeUndefined<String> {
	value.map(|values| values.filter(|v| !v.is_empty()).map(|v| v.join(", ")))
}

/// One book of `editMediaMetadataBatch`: named by `id` or by `path` (as Stump stores it; both
/// may be given, then they must agree), with the values to set
#[derive(Debug, Clone, InputObject)]
pub struct MediaMetadataBatchInput {
	pub id: Option<ID>,
	pub path: Option<String>,
	pub input: MediaMetadataInput,
}

/// How `editMediaMetadataBatch` saves
#[derive(Debug, Clone, InputObject)]
pub struct MediaMetadataBatchOptions {
	/// Only report what would change: nothing is saved or written
	#[graphql(default = false)]
	pub dry_run: bool,
	/// Write the changes into the book files (else they stay in Stump only, locked)
	#[graphql(default = true)]
	pub write_to_file: bool,
}

impl Default for MediaMetadataBatchOptions {
	fn default() -> Self {
		Self {
			dry_run: false,
			write_to_file: true,
		}
	}
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateAnnotationInput {
	pub media_id: String,
	pub locator: ReadiumLocator,
	pub annotation_text: Option<String>,
	#[graphql(default)]
	pub style: AnnotationStyle,
}

impl CreateAnnotationInput {
	pub fn into_active_model(self, user: &AuthUser) -> media_annotation::ActiveModel {
		media_annotation::ActiveModel {
			locator: Set(self.locator),
			annotation_text: Set(self.annotation_text),
			style: Set(self.style),
			media_id: Set(self.media_id),
			user_id: Set(user.id.clone()),
			..Default::default()
		}
	}
}

/// Changes an annotation's note and/or style. An omitted field is left unchanged, and a
/// null `annotationText` removes the note
#[derive(Debug, Clone, InputObject)]
pub struct UpdateAnnotationInput {
	pub id: String,
	pub annotation_text: MaybeUndefined<String>,
	pub style: Option<AnnotationStyle>,
}

impl UpdateAnnotationInput {
	pub fn apply(self, active_model: &mut media_annotation::ActiveModel) {
		match self.annotation_text {
			MaybeUndefined::Undefined => {},
			MaybeUndefined::Null => active_model.annotation_text = Set(None),
			MaybeUndefined::Value(text) => {
				active_model.annotation_text = Set(Some(text).filter(|t| !t.is_empty()))
			},
		}
		if let Some(style) = self.style {
			active_model.style = Set(style);
		}
	}
}

/// A manual override for searching metadata providers for a single media item. When
/// provided, the caller's fields take precedence over whatever is already stored on the
/// media, and the search is restricted to a single provider if one is given.
#[derive(Debug, Clone, Default, InputObject)]
pub struct MediaMetadataSearchInput {
	pub title: Option<String>,
	pub author: Option<String>,
	pub isbn: Option<String>,
	pub year: Option<i32>,
	/// The issue number (for comics/manga)
	pub number: Option<f64>,
	/// The volume ID to search within, which will swap to a more precise lookup if provided alongside
	/// `number`
	pub comic_vine_volume_id: Option<String>,
	/// Restrict the search to this provider only. If omitted, all enabled providers
	/// configured for the media's library type are searched.
	pub provider: Option<MetadataProvider>,
	pub limit: Option<i32>,
}
