//! Shared types of the book intake (the per-user "Add books" inbox). They are stored as JSON
//! on `intake_items` rows and exposed as-is through GraphQL.

use std::collections::HashMap;

use async_graphql::{Enum, InputObject, SimpleObject};
use sea_orm::{prelude::*, DeriveActiveEnum, EnumIter, FromJsonQueryResult};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Where an inbox item is in its life
#[derive(
	Eq,
	Copy,
	Hash,
	Debug,
	Clone,
	Default,
	EnumIter,
	PartialEq,
	Serialize,
	Deserialize,
	DeriveActiveEnum,
	Enum,
	EnumString,
	Display,
)]
#[sea_orm(
	rs_type = "String",
	rename_all = "SCREAMING_SNAKE_CASE",
	db_type = "String(StringLen::None)"
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum IntakeState {
	/// Uploaded; its metadata is being read, looked up and matched against the library
	#[default]
	Analysing,
	/// Something important is missing or uncertain (author folder, genres, title...)
	NeedsReview,
	/// Everything needed to file it is known and confident; it can be accepted as is
	Ready,
	/// Written into the library
	Filed,
	/// Analysis or filing failed, see `error`
	Failed,
	/// Thrown away by the uploader; its staged files are deleted
	Discarded,
}

/// A single book, or a series (a folder or zip of volumes: manga, light novels)
#[derive(
	Eq,
	Copy,
	Hash,
	Debug,
	Clone,
	Default,
	EnumIter,
	PartialEq,
	Serialize,
	Deserialize,
	DeriveActiveEnum,
	Enum,
	EnumString,
	Display,
)]
#[sea_orm(
	rs_type = "String",
	rename_all = "SCREAMING_SNAKE_CASE",
	db_type = "String(StringLen::None)"
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum IntakeKind {
	#[default]
	Book,
	Series,
}

/// Which series root a series item (or a single volume) goes to
#[derive(
	Eq, Copy, Hash, Debug, Clone, PartialEq, Serialize, Deserialize, Enum, EnumString, Display,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum IntakeSeriesKind {
	Manga,
	LightNovel,
}

/// How an online source answered a lookup
#[derive(
	Eq, Copy, Hash, Debug, Clone, PartialEq, Serialize, Deserialize, Enum, EnumString, Display,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum OnlineLookupStatus {
	/// The book was found, with subjects
	Ok,
	/// The book was found, but the source has no subjects for it
	NoSubjects,
	/// The source answered, but has no matching book
	NoMatch,
	/// The source refused for now: its request quota (Google Books' shared daily quota
	/// without an API key) is used up
	RateLimited,
	/// The source failed (network, timeout, an unexpected answer, a refused API key)
	Error,
	/// The source isn't set up: Hardcover's metadata provider is off or has no API token
	NotConfigured,
}

/// One online source's answer to a lookup, for the review screens
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "OnlineLookup")]
pub struct OnlineLookup {
	/// `OPEN_LIBRARY`, `GOOGLE_BOOKS` or `HARDCOVER`
	pub source: IntakeSource,
	pub status: OnlineLookupStatus,
	/// How many subjects it gave
	pub subjects: i32,
	/// What happened, in a few words (e.g. "daily limit reached: add an API key in
	/// Settings")
	pub message: String,
}

/// Where a metadata value came from, so the review screen can say so
#[derive(
	Eq, Copy, Hash, Debug, Clone, PartialEq, Serialize, Deserialize, Enum, EnumString, Display,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum IntakeSource {
	/// The book file's own metadata (OPF, PDF Info, ComicInfo)
	File,
	/// Parsed from the file or folder name
	FileName,
	OpenLibrary,
	GoogleBooks,
	Hardcover,
	/// Inferred from books already in the library (same author, series, similar text)
	Library,
	/// Set by the uploader
	User,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "IntakeFieldSource")]
pub struct IntakeFieldSource {
	/// The camelCase name of the field (`title`, `authors`, `series`, `seriesIndex`,
	/// `language`, `year`, `publisher`, `description`, `isbn`, `subjects`)
	pub field: String,
	pub source: IntakeSource,
}

/// The descriptive metadata of an item, with where each value came from
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "IntakeMetadata")]
#[serde(default)]
pub struct IntakeMetadata {
	pub title: Option<String>,
	pub authors: Vec<String>,
	pub series: Option<String>,
	pub series_index: Option<f64>,
	/// Two-letter code (`en`, `es`, ...)
	pub language: Option<String>,
	/// First publication year when known (Open Library), else the edition year
	pub year: Option<i32>,
	pub publisher: Option<String>,
	pub description: Option<String>,
	pub isbn: Option<String>,
	/// Raw subjects found in the file and online, before matching to the library's tags
	pub subjects: Vec<String>,
	/// How much a subject counts (0-1) when its only source weighs it (Hardcover: by how many
	/// of its readers gave it); the others count fully
	#[graphql(skip)]
	#[serde(skip_serializing_if = "HashMap::is_empty")]
	pub subject_weights: HashMap<String, f64>,
	/// The tags that will be written: library genres (`Big`, `Big: Small`) and facets
	/// (`Origin: X`, `Era: X`, ...)
	pub genres: Vec<String>,
	/// 0 (all ages) or 18 (adults only), when known
	pub age_rating: Option<i32>,
	/// 0-5
	pub rating: Option<f64>,
	pub sources: Vec<IntakeFieldSource>,
}

impl IntakeMetadata {
	pub fn source_of(&self, field: &str) -> Option<IntakeSource> {
		self.sources
			.iter()
			.find(|s| s.field == field)
			.map(|s| s.source)
	}

	pub fn set_source(&mut self, field: &str, source: IntakeSource) {
		self.sources.retain(|s| s.field != field);
		self.sources.push(IntakeFieldSource {
			field: field.to_string(),
			source,
		});
	}
}

/// One uploaded file of an item (a book, or a volume of a series)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "IntakeFile")]
#[serde(default)]
pub struct IntakeFile {
	/// The name it was uploaded with (for a folder upload, its path inside the folder)
	pub original_name: String,
	/// The name of the staged copy inside the item's staging directory
	#[graphql(skip)]
	pub staged_name: String,
	pub size: i64,
	/// `epub`, `pdf`, `cbz`, `cbr`
	pub format: String,
	/// Stump's own file hash, used to find duplicates
	pub hash: Option<String>,
	/// The volume number, for a series
	pub volume: Option<f64>,
	/// The title read from this file (volumes may carry their own)
	pub title: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct IntakeFiles(pub Vec<IntakeFile>);

/// An existing (or proposed new) folder the item can go to
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "IntakeFolderCandidate")]
#[serde(default)]
pub struct IntakeFolderCandidate {
	/// The folder name, relative to the library root (author folders) or to the series
	/// root (series folders)
	pub folder: String,
	/// How many books the library already has in it
	pub books: i32,
	/// 0-1
	pub confidence: f64,
	/// Why it is suggested, e.g. "same author written 'Eco, Umberto'"
	pub reason: String,
	/// The folder does not exist yet and would be created
	pub is_new: bool,
}

/// A tag proposed for the item, taken from the tags the library already uses
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "IntakeGenreSuggestion")]
#[serde(default)]
pub struct IntakeGenreSuggestion {
	pub value: String,
	/// 0-1
	pub confidence: f64,
	/// Short explanations, e.g. "3 other books by this author have it"
	pub reasons: Vec<String>,
	/// How many books of the library carry it
	pub library_count: i32,
	/// Whether it is part of the default choice
	pub selected: bool,
	/// `GENRE` (a `Big` or `Big: Small` tag) or `FACET` (`Origin:`, `Era:`, `Level:`...)
	pub kind: String,
}

/// A book already in the library, shown to compare against
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "IntakeSimilarBook")]
#[serde(default)]
pub struct IntakeSimilarBook {
	pub media_id: String,
	pub title: String,
	pub authors: Vec<String>,
	pub genres: Vec<String>,
	/// Why it is similar (`same author`, `same series`, `similar description`)
	pub reason: String,
}

/// A book in the library that looks like the same book
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "IntakeDuplicate")]
#[serde(default)]
pub struct IntakeDuplicate {
	pub media_id: String,
	pub path: String,
	pub title: String,
	/// `SAME_FILE` (identical hash) or `SAME_TITLE` (same author and title)
	pub reason: String,
	/// The uploaded file it matches
	pub file_name: String,
}

/// What the server proposes after analysing an item
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, SimpleObject, FromJsonQueryResult)]
#[graphql(name = "IntakeProposal")]
#[serde(default)]
pub struct IntakeProposal {
	/// The merged metadata (file, file name, online lookups), with sources
	pub metadata: IntakeMetadata,
	/// Set when the item looks like manga or a light novel
	pub series_kind: Option<IntakeSeriesKind>,
	/// Author folders, best first (books)
	pub author_folders: Vec<IntakeFolderCandidate>,
	/// Series folders, best first (series and series volumes)
	pub series_folders: Vec<IntakeFolderCandidate>,
	/// Tags from the library's vocabulary, best first
	pub genre_suggestions: Vec<IntakeGenreSuggestion>,
	pub similar_books: Vec<IntakeSimilarBook>,
	pub duplicates: Vec<IntakeDuplicate>,
	/// Plain-language notes for the uploader (e.g. "No author found")
	pub warnings: Vec<String>,
	/// Overall confidence, 0-1; under the threshold the item needs review
	pub confidence: f64,
	/// Whether an online source found the book (a failed or empty lookup isn't one)
	pub looked_up_online: bool,
	/// How each online source answered, when they were asked
	pub online_lookups: Vec<OnlineLookup>,
}

/// The values that will be filed: the proposal's defaults, then the uploader's edits
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, SimpleObject, FromJsonQueryResult)]
#[graphql(name = "IntakeDecision")]
#[serde(default)]
pub struct IntakeDecision {
	pub metadata: IntakeMetadata,
	/// `None` files a book under its author folder; `Some` files it (or every volume)
	/// under the manga or light novel root
	pub series_kind: Option<IntakeSeriesKind>,
	/// The author folder (books) or the series folder (series), a single path component
	pub folder: String,
	/// The file name per uploaded file (same order as the item's files), without folder
	pub file_names: Vec<String>,
	/// Per-file volume numbers for series (same order as the item's files)
	pub volumes: Vec<Option<f64>>,
}

/// Where each file of an item will go (or went)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "IntakeDestinationFile")]
#[serde(default)]
pub struct IntakeDestinationFile {
	pub original_name: String,
	/// Relative to the library root, e.g. `Umberto_Eco/El_nombre_de_la_rosa.epub`
	pub relative_path: String,
	/// Something already exists at that path (filing will fail for this file)
	pub exists: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, SimpleObject, FromJsonQueryResult)]
#[graphql(name = "IntakeDestination")]
#[serde(default)]
pub struct IntakeDestination {
	/// The folder, relative to the library root
	pub relative_dir: String,
	/// Whether that folder already exists
	pub dir_exists: bool,
	pub files: Vec<IntakeDestinationFile>,
}

/// What filing did
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, SimpleObject, FromJsonQueryResult)]
#[graphql(name = "IntakeResult")]
#[serde(default)]
pub struct IntakeResult {
	/// Absolute paths of the filed files
	pub paths: Vec<String>,
	/// Notes, e.g. a metadata field that could not be written into a PDF
	pub notes: Vec<String>,
}

/// The uploader's review of an item. Omitted fields keep their current value.
#[derive(Clone, Debug, Default, InputObject)]
pub struct IntakeReviewInput {
	pub title: Option<String>,
	pub authors: Option<Vec<String>>,
	pub series: Option<String>,
	pub series_index: Option<f64>,
	pub language: Option<String>,
	pub year: Option<i32>,
	pub publisher: Option<String>,
	pub description: Option<String>,
	pub genres: Option<Vec<String>>,
	pub age_rating: Option<i32>,
	/// File the item as a series under this root. Use `clearSeriesKind` to file a book
	/// under its author folder again
	pub series_kind: Option<IntakeSeriesKind>,
	#[graphql(default)]
	pub clear_series_kind: bool,
	/// The author or series folder (a single path component; may be new)
	pub folder: Option<String>,
	/// One per file, same order as the item's files
	pub file_names: Option<Vec<String>>,
	/// One per file, same order as the item's files
	pub volumes: Option<Vec<Option<f64>>>,
}
