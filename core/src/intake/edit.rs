//! Editing an existing book's metadata from the web.
//!
//! A rescan rebuilds a book's metadata from its file whenever the file changes (book-curator
//! rewrites files), and "Rebuild metadata" always does, so an edit kept only in Stump's
//! database does not last. Here the changed values are written into the book file the way
//! the intake and book-curator write them (EPUB OPF, PDF Info dictionary and Keywords, CBZ
//! ComicInfo.xml). Only the fields that changed are touched, everything else in the file is
//! kept as it is. The new file is prepared next to the book under a hidden name, read back
//! with Stump's own readers, and only then renamed over the book (an atomic swap: the
//! original stays untouched on any failure). Changes a file cannot carry (other fields, CBR
//! files, a failed write) stay in the database. Every edited field is locked, so rescans
//! keep it even if another tool rewrites the file.
//!
//! Every save is recorded in `media_metadata_edits` (one row per changed field, old and new
//! value), which is what "Revert" works from, and appended to `stump_edits.csv`, next to the
//! intake CSV, so the curator can pick the owner's corrections up.

use std::{
	collections::{BTreeSet, HashMap},
	fs::File,
	io::{Read, Write},
	path::{Path, PathBuf},
};

use metadata_integrations::MetadataField;
use models::{
	entity::{library, library_config, media, media_metadata, media_metadata_edit, user},
	shared::{
		enums::OrganizationMode,
		intake::{
			IntakeGenreSuggestion, IntakeMetadata, IntakeSeriesKind, IntakeSimilarBook,
			IntakeSource,
		},
	},
};
use regex::Regex;
use sea_orm::{
	prelude::Decimal, ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait,
	IntoActiveModel, QueryFilter, QueryOrder, TransactionTrait,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive};

use super::{
	csv_path_for,
	library_index::{is_facet, library_index, norm, LibraryIndex},
	lookup::{self, strip_html},
	naming::display_name,
	settings_for,
	suggest::{merge_online, rank_suggestions, score_tags},
	write::{
		append_pdf_info, check_xml, comic_info, esc, file_as, num, opf_path, order_tags,
		pdf_string, rewrite_zip, FileMetadata, KeptPdfInfo, WriteError, WriteResult,
	},
};
use crate::{
	media::processor::{
		pdf::{read_info_values, PdfInfoValues},
		process_metadata,
	},
	metadata::{
		locked::locked_fields_of,
		membership::{remove_series_thumbnails, sync_media_membership},
	},
	postproc::refresh_media_after_swap,
	CoreError, CoreResult, Ctx,
};

/// The edits log, next to the intake CSV
pub const EDITS_CSV_NAME: &str = "stump_edits.csv";

const EDITS_CSV_HEADER: &[&str] = &[
	"path",
	"field",
	"old",
	"new",
	"written_to",
	"edited_by",
	"edited_at",
];

/// The book formats whose files carry the edited metadata
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditableFormat {
	Epub,
	Pdf,
	Cbz,
}

impl EditableFormat {
	pub fn of(path: &Path) -> Option<Self> {
		let name = path.file_name()?.to_string_lossy().to_lowercase();
		if name.ends_with(".epub") {
			Some(Self::Epub)
		} else if name.ends_with(".pdf") {
			Some(Self::Pdf)
		} else if name.ends_with(".cbz") {
			Some(Self::Cbz)
		} else {
			None
		}
	}

	/// Whether this format's file carries a field (written here and read back by Stump)
	pub fn carries(&self, field: &str) -> bool {
		let fields: &[&str] = match self {
			Self::Epub => &[
				"title",
				"writers",
				"series",
				"number",
				"language",
				"summary",
				"year",
				"month",
				"day",
				"publisher",
				"genres",
				"ageRating",
			],
			// PDFs have no language, publisher or date of their own (CreationDate is when the
			// file was made); series, number and age live in the curator's Keywords entries
			Self::Pdf => &[
				"title",
				"writers",
				"series",
				"number",
				"summary",
				"genres",
				"ageRating",
			],
			// Stump reads no ComicInfo language (LanguageISO)
			Self::Cbz => &[
				"title",
				"writers",
				"series",
				"number",
				"summary",
				"year",
				"month",
				"day",
				"publisher",
				"genres",
				"ageRating",
			],
		};
		fields.contains(&field)
	}
}

/// One changed field: its camelCase name, the lock that keeps it in the database, and
/// the old and new values as text (for the edits log)
#[derive(Debug, Clone, PartialEq)]
pub struct ChangedField {
	pub field: &'static str,
	pub lock: Option<MetadataField>,
	pub old: String,
	pub new: String,
}

/// The new values of the fields a file can carry, for the ones that changed
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileChanges {
	pub title: Option<Option<String>>,
	pub writers: Option<Vec<String>>,
	/// Series and number change together: both are rewritten
	pub series: Option<(Option<String>, Option<f64>)>,
	pub language: Option<Option<String>>,
	pub summary: Option<Option<String>>,
	/// Year, month and day of publication
	pub date: Option<Option<(i32, Option<i32>, Option<i32>)>>,
	pub publisher: Option<Option<String>>,
	pub genres: Option<Vec<String>>,
	pub age_rating: Option<Option<i32>>,
}

impl FileChanges {
	pub fn is_empty(&self) -> bool {
		*self == Self::default()
	}
}

/// A comma-separated column as its values
pub fn split_list(value: &Option<String>) -> Vec<String> {
	value
		.as_deref()
		.unwrap_or("")
		.split(',')
		.map(str::trim)
		.filter(|s| !s.is_empty())
		.map(str::to_string)
		.collect()
}

fn text(value: &Option<String>) -> String {
	value.clone().unwrap_or_default()
}

fn decimal_text(value: &Option<Decimal>) -> String {
	value.map(|d| d.normalize().to_string()).unwrap_or_default()
}

fn int_text(value: &Option<i32>) -> String {
	value.map(|v| v.to_string()).unwrap_or_default()
}

fn decimal_f64(value: &Option<Decimal>) -> Option<f64> {
	value.and_then(|d| d.to_string().parse::<f64>().ok())
}

/// The fields an edit can change, in the order they are reported: the camelCase name and
/// the lock that keeps the field through rescans (a date's parts share the year's lock)
pub const EDIT_FIELDS: &[(&str, Option<MetadataField>)] = &[
	("title", Some(MetadataField::Title)),
	("titleSort", Some(MetadataField::TitleSort)),
	("writers", Some(MetadataField::Writers)),
	("series", Some(MetadataField::Series)),
	("number", Some(MetadataField::Number)),
	("seriesGroup", Some(MetadataField::SeriesGroup)),
	("storyArc", Some(MetadataField::StoryArc)),
	("storyArcNumber", Some(MetadataField::StoryArcNumber)),
	("volume", None),
	("summary", Some(MetadataField::Summary)),
	("notes", Some(MetadataField::Notes)),
	("genres", Some(MetadataField::Genres)),
	("format", Some(MetadataField::Format)),
	("year", Some(MetadataField::Year)),
	("month", Some(MetadataField::Year)),
	("day", Some(MetadataField::Year)),
	("pencillers", Some(MetadataField::Pencillers)),
	("inkers", Some(MetadataField::Inkers)),
	("colorists", Some(MetadataField::Colorists)),
	("letterers", Some(MetadataField::Letterers)),
	("coverArtists", Some(MetadataField::CoverArtists)),
	("editors", Some(MetadataField::Editors)),
	("publisher", Some(MetadataField::Publisher)),
	("links", Some(MetadataField::Links)),
	("characters", Some(MetadataField::Characters)),
	("teams", Some(MetadataField::Teams)),
	("pageCount", Some(MetadataField::PageCount)),
	("ageRating", Some(MetadataField::AgeRating)),
	("identifierAmazon", Some(MetadataField::IdentifierAmazon)),
	("identifierCalibre", Some(MetadataField::IdentifierCalibre)),
	("identifierGoogle", Some(MetadataField::IdentifierGoogle)),
	("identifierIsbn", Some(MetadataField::Isbn)),
	(
		"identifierMobiAsin",
		Some(MetadataField::IdentifierMobiAsin),
	),
	("identifierUuid", Some(MetadataField::IdentifierUuid)),
	("language", Some(MetadataField::Language)),
];

/// A field's value as text, the way the metadata row stores it (lists comma-separated);
/// empty when it has none
pub fn field_text(meta: &media_metadata::Model, field: &str) -> String {
	match field {
		"title" => text(&meta.title),
		"titleSort" => text(&meta.title_sort),
		"writers" => text(&meta.writers),
		"series" => text(&meta.series),
		"number" => decimal_text(&meta.number),
		"seriesGroup" => text(&meta.series_group),
		"storyArc" => text(&meta.story_arc),
		"storyArcNumber" => decimal_text(&meta.story_arc_number),
		"volume" => int_text(&meta.volume),
		"summary" => text(&meta.summary),
		"notes" => text(&meta.notes),
		"genres" => text(&meta.genres),
		"format" => text(&meta.format),
		"year" => int_text(&meta.year),
		"month" => int_text(&meta.month),
		"day" => int_text(&meta.day),
		"pencillers" => text(&meta.pencillers),
		"inkers" => text(&meta.inkers),
		"colorists" => text(&meta.colorists),
		"letterers" => text(&meta.letterers),
		"coverArtists" => text(&meta.cover_artists),
		"editors" => text(&meta.editors),
		"publisher" => text(&meta.publisher),
		"links" => text(&meta.links),
		"characters" => text(&meta.characters),
		"teams" => text(&meta.teams),
		"pageCount" => int_text(&meta.page_count),
		"ageRating" => int_text(&meta.age_rating),
		"identifierAmazon" => text(&meta.identifier_amazon),
		"identifierCalibre" => text(&meta.identifier_calibre),
		"identifierGoogle" => text(&meta.identifier_google),
		"identifierIsbn" => text(&meta.identifier_isbn),
		"identifierMobiAsin" => text(&meta.identifier_mobi_asin),
		"identifierUuid" => text(&meta.identifier_uuid),
		"language" => text(&meta.language),
		_ => String::new(),
	}
}

/// Sets one field of a metadata row from its text form ([`field_text`]; empty: no value).
/// `false` for an unknown field or a number that doesn't parse.
pub fn set_field_from_text(
	active: &mut media_metadata::ActiveModel,
	field: &str,
	value: &str,
) -> bool {
	let trimmed = value.trim();
	let string = || (!trimmed.is_empty()).then(|| value.to_string());
	let int = || -> Option<Option<i32>> {
		if trimmed.is_empty() {
			Some(None)
		} else {
			trimmed.parse::<i32>().ok().map(Some)
		}
	};
	let decimal = || -> Option<Option<Decimal>> {
		if trimmed.is_empty() {
			Some(None)
		} else {
			trimmed.parse::<Decimal>().ok().map(Some)
		}
	};
	match field {
		"title" => active.title = Set(string()),
		"titleSort" => active.title_sort = Set(string()),
		"writers" => active.writers = Set(string()),
		"series" => active.series = Set(string()),
		"seriesGroup" => active.series_group = Set(string()),
		"storyArc" => active.story_arc = Set(string()),
		"summary" => active.summary = Set(string()),
		"notes" => active.notes = Set(string()),
		"genres" => active.genres = Set(string()),
		"format" => active.format = Set(string()),
		"pencillers" => active.pencillers = Set(string()),
		"inkers" => active.inkers = Set(string()),
		"colorists" => active.colorists = Set(string()),
		"letterers" => active.letterers = Set(string()),
		"coverArtists" => active.cover_artists = Set(string()),
		"editors" => active.editors = Set(string()),
		"publisher" => active.publisher = Set(string()),
		"links" => active.links = Set(string()),
		"characters" => active.characters = Set(string()),
		"teams" => active.teams = Set(string()),
		"identifierAmazon" => active.identifier_amazon = Set(string()),
		"identifierCalibre" => active.identifier_calibre = Set(string()),
		"identifierGoogle" => active.identifier_google = Set(string()),
		"identifierIsbn" => active.identifier_isbn = Set(string()),
		"identifierMobiAsin" => active.identifier_mobi_asin = Set(string()),
		"identifierUuid" => active.identifier_uuid = Set(string()),
		"language" => active.language = Set(string()),
		"number" | "storyArcNumber" => {
			let Some(number) = decimal() else {
				return false;
			};
			if field == "number" {
				active.number = Set(number);
			} else {
				active.story_arc_number = Set(number);
			}
		},
		"volume" | "year" | "month" | "day" | "pageCount" | "ageRating" => {
			let Some(number) = int() else {
				return false;
			};
			match field {
				"volume" => active.volume = Set(number),
				"year" => active.year = Set(number),
				"month" => active.month = Set(number),
				"day" => active.day = Set(number),
				"pageCount" => active.page_count = Set(number),
				_ => active.age_rating = Set(number),
			}
		},
		_ => return false,
	}
	true
}

/// The fields that differ between the stored metadata and the edited one
pub fn changed_fields(
	old: &media_metadata::Model,
	new: &media_metadata::Model,
) -> Vec<ChangedField> {
	EDIT_FIELDS
		.iter()
		.filter_map(|&(field, lock)| {
			let (old, new) = (field_text(old, field), field_text(new, field));
			(old != new).then_some(ChangedField {
				field,
				lock,
				old,
				new,
			})
		})
		.collect()
}

/// What a format's file gets from the changed fields (the new values)
pub fn file_changes(
	format: EditableFormat,
	changed: &[ChangedField],
	new: &media_metadata::Model,
) -> FileChanges {
	let has =
		|field: &str| changed.iter().any(|c| c.field == field) && format.carries(field);
	let mut out = FileChanges::default();
	if has("title") {
		// a book keeps a title in its file: removing it stays a database change
		if let Some(title) = new
			.title
			.as_deref()
			.map(str::trim)
			.filter(|t| !t.is_empty())
		{
			out.title = Some(Some(title.to_string()));
		}
	}
	if has("writers") {
		out.writers = Some(split_list(&new.writers));
	}
	if has("series") || has("number") {
		out.series = Some((
			new.series
				.as_deref()
				.map(str::trim)
				.filter(|s| !s.is_empty())
				.map(str::to_string),
			decimal_f64(&new.number),
		));
	}
	if has("language") {
		out.language = Some(new.language.clone().filter(|l| !l.trim().is_empty()));
	}
	if has("summary") {
		out.summary = Some(new.summary.clone().filter(|s| !s.trim().is_empty()));
	}
	if has("year") || has("month") || has("day") {
		out.date = Some(new.year.map(|y| (y, new.month, new.day)));
	}
	if has("publisher") {
		out.publisher = Some(new.publisher.clone().filter(|p| !p.trim().is_empty()));
	}
	if has("genres") {
		out.genres = Some(split_list(&new.genres));
	}
	if has("ageRating") {
		out.age_rating = Some(new.age_rating);
	}
	out
}

/// The changed fields a write of `changes` covers
pub fn fields_written(
	changed: &[ChangedField],
	changes: &FileChanges,
) -> Vec<&'static str> {
	changed
		.iter()
		.filter(|c| match c.field {
			"title" => changes.title.is_some(),
			"writers" => changes.writers.is_some(),
			"series" | "number" => changes.series.is_some(),
			"language" => changes.language.is_some(),
			"summary" => changes.summary.is_some(),
			"year" | "month" | "day" => changes.date.is_some(),
			"publisher" => changes.publisher.is_some(),
			"genres" => changes.genres.is_some(),
			"ageRating" => changes.age_rating.is_some(),
			_ => false,
		})
		.map(|c| c.field)
		.collect()
}

/// Cleans genres typed or picked in the editor: one tag per comma or semicolon, trimmed,
/// de-duplicated, in the curator's order (big tags, `Big: Small` tags, facets)
pub fn clean_genres(values: &[String]) -> Vec<String> {
	let split: Vec<String> = values
		.iter()
		.flat_map(|v| v.split([',', ';']))
		.map(|v| v.split_whitespace().collect::<Vec<_>>().join(" "))
		.filter(|v| !v.is_empty())
		.collect();
	order_tags(&split)
}

/// Cleans authors typed in the editor: `Eco, Umberto` becomes `Umberto Eco`, since Stump
/// splits writers on commas
pub fn clean_writers(values: &[String]) -> Vec<String> {
	let mut seen = BTreeSet::new();
	values
		.iter()
		.map(|v| display_name(v))
		.map(|v| {
			v.replace(',', " ")
				.split_whitespace()
				.collect::<Vec<_>>()
				.join(" ")
		})
		.filter(|v| !v.is_empty() && seen.insert(v.to_lowercase()))
		.collect()
}

fn age_value(age: i32) -> String {
	match age {
		a if a <= 0 => "All ages".to_string(),
		a => format!("{a}+"),
	}
}

fn date_value((year, month, day): (i32, Option<i32>, Option<i32>)) -> String {
	match (month, day) {
		(Some(m), Some(d)) => format!("{year:04}-{m:02}-{d:02}"),
		(Some(m), None) => format!("{year:04}-{m:02}"),
		_ => format!("{year:04}"),
	}
}

// ------------------------------------------------------------------ EPUB

/// Applies the changes to an OPF's text: each changed field replaces what the file had
/// (and a removed value is removed); everything else is left as it is
pub fn apply_opf_changes(opf: &str, changes: &FileChanges) -> WriteResult<String> {
	let decl = Regex::new(r#"^(<\?xml[^>]*encoding=["'])[^"']+"#).expect("valid regex");
	let opf = decl.replace(opf, "${1}UTF-8").into_owned();

	let block =
		Regex::new(r"(?s)(<(?:\w+:)?metadata\b[^>]*>)(.*?)(</(?:\w+:)?metadata>)")
			.expect("valid regex");
	let Some(caps) = block.captures(&opf) else {
		return Err(WriteError::Invalid("OPF has no metadata block".into()));
	};
	let whole = caps.get(0).expect("match");
	let mut head = caps[1].to_string();
	let mut body = caps[2].to_string();
	let tail = caps[3].to_string();
	let package = Regex::new(r"<package\b[^>]*>")
		.expect("valid regex")
		.find(&opf)
		.map(|m| m.as_str().to_string())
		.unwrap_or_default();
	let epub3 = Regex::new(r#"version=["']3"#)
		.expect("valid regex")
		.is_match(&package);
	if !head.contains("xmlns:opf") && !package.contains("xmlns:opf") {
		head = format!(
			"{} xmlns:opf=\"http://www.idpf.org/2007/opf\">",
			&head[..head.len() - 1]
		);
	}
	if !head.contains("xmlns:dc") && !package.contains("xmlns:dc") {
		head = format!(
			"{} xmlns:dc=\"http://purl.org/dc/elements/1.1/\">",
			&head[..head.len() - 1]
		);
	}

	let mut add: Vec<String> = vec![];
	let remove = |body: &mut String, pattern: &str| {
		let re = Regex::new(pattern).expect("valid regex");
		*body = re.replace_all(body, "").into_owned();
	};
	let remove_refines = |body: &mut String, ids: Vec<String>| {
		for id in ids {
			let id = regex::escape(&id);
			let re = Regex::new(&format!(
				r#"(?s)\s*<meta\b[^>]*refines=["']#{id}["'][^>]*>.*?</meta>"#
			))
			.expect("valid regex");
			*body = re.replace_all(body, "").into_owned();
			let re =
				Regex::new(&format!(r#"\s*<meta\b[^>]*refines=["']#{id}["'][^>]*/>"#))
					.expect("valid regex");
			*body = re.replace_all(body, "").into_owned();
		}
	};
	let ids_of = |body: &str, pattern: &str| -> Vec<String> {
		Regex::new(pattern)
			.expect("valid regex")
			.captures_iter(body)
			.map(|c| c[1].to_string())
			.collect()
	};

	if let Some(Some(title)) = &changes.title {
		let re = Regex::new(r"(?s)(<dc:title\b[^>]*>)(.*?)(</dc:title>)")
			.expect("valid regex");
		if let Some(c) = re.captures(&body) {
			let replaced = format!("{}{}{}", &c[1], esc(title), &c[3]);
			body = body.replacen(c.get(0).expect("match").as_str(), &replaced, 1);
		} else {
			remove(&mut body, r"\s*<dc:title\b[^>]*/>");
			add.insert(0, format!("<dc:title>{}</dc:title>", esc(title)));
		}
	}

	if let Some(writers) = &changes.writers {
		let ids = ids_of(&body, r#"<dc:creator\b[^>]*\bid=["']([^"']+)["']"#);
		remove_refines(&mut body, ids);
		remove(&mut body, r"(?s)\s*<dc:creator\b[^>]*>.*?</dc:creator>");
		remove(&mut body, r"\s*<dc:creator\b[^>]*/>");
		for (i, author) in writers.iter().enumerate() {
			if epub3 {
				let id = format!("stump-creator-{}", i + 1);
				add.push(format!(
					"<dc:creator id=\"{id}\">{}</dc:creator><meta refines=\"#{id}\" property=\"file-as\">{}</meta><meta refines=\"#{id}\" property=\"role\" scheme=\"marc:relators\">aut</meta>",
					esc(author),
					esc(&file_as(author))
				));
			} else {
				add.push(format!(
					"<dc:creator opf:file-as=\"{}\" opf:role=\"aut\">{}</dc:creator>",
					esc(&file_as(author)),
					esc(author)
				));
			}
		}
	}

	if let Some(genres) = &changes.genres {
		remove(&mut body, r"(?s)\s*<dc:subject\b[^>]*>.*?</dc:subject>");
		remove(&mut body, r"\s*<dc:subject\b[^>]*/>");
		remove(
			&mut body,
			r#"\s*<meta\b[^>]*name=["']bcurator:(?:estante|shelf)["'][^>]*/>"#,
		);
		for tag in genres {
			add.push(format!("<dc:subject>{}</dc:subject>", esc(tag)));
		}
		let shelf = genres.iter().find(|t| !t.contains(':') && !is_facet(t));
		if let Some(shelf) = shelf {
			add.push(format!(
				"<meta name=\"bcurator:shelf\" content=\"{}\"/>",
				esc(shelf)
			));
		}
	}

	if let Some((series, number)) = &changes.series {
		let ids = ids_of(
			&body,
			r#"<meta[^>]*property=["']belongs-to-collection["'][^>]*id=["']([^"']+)["']"#,
		);
		remove_refines(&mut body, ids);
		remove(
			&mut body,
			r#"(?s)\s*<meta[^>]*property=["']belongs-to-collection["'][^>]*>.*?</meta>"#,
		);
		remove(
			&mut body,
			r#"\s*<meta[^>]*name=["']calibre:series(?:_index)?["'][^>]*/>"#,
		);
		if let Some(series) = series {
			add.push(format!(
				"<meta name=\"calibre:series\" content=\"{}\"/>",
				esc(series)
			));
			if let Some(index) = number {
				add.push(format!(
					"<meta name=\"calibre:series_index\" content=\"{}\"/>",
					num(*index)
				));
			}
			if epub3 {
				let mut collection = format!(
					"<meta property=\"belongs-to-collection\" id=\"bcurator-series\">{}</meta><meta refines=\"#bcurator-series\" property=\"collection-type\">series</meta>",
					esc(series)
				);
				if let Some(index) = number {
					collection.push_str(&format!(
						"<meta refines=\"#bcurator-series\" property=\"group-position\">{}</meta>",
						num(*index)
					));
				}
				add.push(collection);
			}
		}
	}

	if let Some(language) = &changes.language {
		remove(&mut body, r"\s*<dc:language\b[^>]*>[^<]*</dc:language>");
		remove(&mut body, r"\s*<dc:language\b[^>]*/>");
		if let Some(language) = language {
			add.push(format!("<dc:language>{}</dc:language>", esc(language)));
		}
	}

	if let Some(summary) = &changes.summary {
		remove(
			&mut body,
			r"(?s)\s*<dc:description\b[^>]*>.*?</dc:description>",
		);
		remove(&mut body, r"\s*<dc:description\b[^>]*/>");
		if let Some(summary) = summary {
			add.push(format!("<dc:description>{}</dc:description>", esc(summary)));
		}
	}

	if let Some(date) = &changes.date {
		// the publication date: modification dates (opf:event) stay
		let re = Regex::new(r"(?s)\s*<dc:date\b([^>]*)>([^<]*)</dc:date>")
			.expect("valid regex");
		body = re
			.replace_all(&body, |c: &regex::Captures| {
				if c[1].to_lowercase().contains("modification") {
					c[0].to_string()
				} else {
					String::new()
				}
			})
			.into_owned();
		if let Some(date) = date {
			add.push(format!("<dc:date>{}</dc:date>", date_value(*date)));
		}
	}

	if let Some(publisher) = &changes.publisher {
		remove(&mut body, r"(?s)\s*<dc:publisher\b[^>]*>.*?</dc:publisher>");
		remove(&mut body, r"\s*<dc:publisher\b[^>]*/>");
		if let Some(publisher) = publisher {
			add.push(format!("<dc:publisher>{}</dc:publisher>", esc(publisher)));
		}
	}

	if let Some(age) = &changes.age_rating {
		remove(
			&mut body,
			r#"\s*<meta\b[^>]*name=["']typicalAgeRange["'][^>]*/>"#,
		);
		if let Some(age) = age {
			add.push(format!(
				"<meta name=\"typicalAgeRange\" content=\"{}\"/>",
				age_value(*age)
			));
		}
	}

	let body = if add.is_empty() {
		body
	} else {
		format!("{}\n    {}\n  ", body.trim_end(), add.join("\n    "))
	};
	Ok(format!(
		"{}{}{}{}{}",
		&opf[..whole.start()],
		head,
		body,
		tail,
		&opf[whole.end()..]
	))
}

/// Writes the changes into an EPUB file (in place, through a temporary file)
fn write_epub_changes(path: &Path, changes: &FileChanges) -> WriteResult<()> {
	let (opf_name, opf) = {
		let mut archive = ZipArchive::new(File::open(path)?)?;
		let name = opf_path(&mut archive)?;
		let mut bytes = vec![];
		archive.by_name(&name)?.read_to_end(&mut bytes)?;
		let text = String::from_utf8_lossy(&bytes)
			.trim_end_matches('\0')
			.to_string();
		(name, text)
	};
	let new_opf = apply_opf_changes(&opf, changes)?;
	if new_opf == opf {
		return Ok(());
	}
	check_xml(&new_opf)?;
	let tmp = super::write::temp_path(path);
	let result = rewrite_zip(path, &tmp, &|name| name == opf_name, &mut |writer| {
		writer.start_file(
			opf_name.as_str(),
			SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
		)?;
		writer.write_all(new_opf.as_bytes())?;
		Ok(())
	})
	.and_then(|_| {
		let mut check = ZipArchive::new(File::open(&tmp)?)?;
		let mut text = String::new();
		check.by_name(&opf_name)?.read_to_string(&mut text)?;
		if text != new_opf {
			return Err(WriteError::Invalid("rewritten EPUB check failed".into()));
		}
		std::fs::rename(&tmp, path)?;
		Ok(())
	});
	if result.is_err() {
		let _ = std::fs::remove_file(&tmp);
	}
	result
}

// ------------------------------------------------------------------ CBZ

/// Sets (or adds, or removes) one element of a ComicInfo document
fn set_comic_element(xml: &mut String, tag: &str, value: Option<&str>) {
	let full = Regex::new(&format!(r"(?s)\s*<{tag}(?:\s[^>]*)?>.*?</{tag}>"))
		.expect("valid regex");
	let empty = Regex::new(&format!(r"\s*<{tag}(?:\s[^>]*)?/>")).expect("valid regex");
	*xml = full.replace_all(xml, "").into_owned();
	*xml = empty.replace_all(xml, "").into_owned();
	if let Some(value) = value {
		if let Some(end) = xml.rfind("</ComicInfo>") {
			xml.insert_str(end, &format!("  <{tag}>{}</{tag}>\n", esc(value)));
		}
	}
}

/// The ComicInfo.xml after the changes: an existing one keeps everything else (pages,
/// credits, ...), a missing one is created like the intake does
pub fn apply_comic_info_changes(existing: Option<&str>, changes: &FileChanges) -> String {
	let Some(existing) = existing.filter(|x| x.contains("</ComicInfo>")) else {
		let mut meta = FileMetadata::default();
		if let Some(Some(title)) = &changes.title {
			meta.title = Some(title.clone());
		}
		if let Some(writers) = &changes.writers {
			meta.authors = writers.clone();
		}
		if let Some((series, number)) = &changes.series {
			meta.series = series.clone();
			meta.series_index = *number;
		}
		if let Some(Some(summary)) = &changes.summary {
			meta.description = Some(summary.clone());
		}
		if let Some(Some((year, _, _))) = &changes.date {
			meta.year = Some(*year);
		}
		if let Some(genres) = &changes.genres {
			meta.tags = genres.clone();
		}
		if let Some(age) = &changes.age_rating {
			meta.age_rating = *age;
		}
		let mut xml = comic_info(&meta);
		// comic_info() writes a language; with no ComicInfo before there was none
		xml = Regex::new(r"<LanguageISO>[^<]*</LanguageISO>")
			.expect("valid regex")
			.replace(&xml, "")
			.into_owned();
		if let Some(Some(publisher)) = &changes.publisher {
			set_comic_element(&mut xml, "Publisher", Some(publisher));
		}
		return xml;
	};
	let mut xml = existing.to_string();
	if let Some(Some(title)) = &changes.title {
		set_comic_element(&mut xml, "Title", Some(title));
	}
	if let Some(writers) = &changes.writers {
		let joined = writers.join(", ");
		set_comic_element(
			&mut xml,
			"Writer",
			Some(&joined).filter(|w| !w.is_empty()).map(|w| w.as_str()),
		);
	}
	if let Some((series, number)) = &changes.series {
		set_comic_element(&mut xml, "Series", series.as_deref());
		let number = number.map(num);
		set_comic_element(&mut xml, "Number", number.as_deref());
	}
	if let Some(summary) = &changes.summary {
		set_comic_element(&mut xml, "Summary", summary.as_deref());
	}
	if let Some(date) = &changes.date {
		let (year, month, day) = match date {
			Some((y, m, d)) => (
				Some(y.to_string()),
				m.map(|m| m.to_string()),
				d.map(|d| d.to_string()),
			),
			None => (None, None, None),
		};
		set_comic_element(&mut xml, "Year", year.as_deref());
		set_comic_element(&mut xml, "Month", month.as_deref());
		set_comic_element(&mut xml, "Day", day.as_deref());
	}
	if let Some(publisher) = &changes.publisher {
		set_comic_element(&mut xml, "Publisher", publisher.as_deref());
	}
	if let Some(genres) = &changes.genres {
		let joined = genres.join(", ");
		set_comic_element(
			&mut xml,
			"Genre",
			Some(joined.as_str()).filter(|g| !g.is_empty()),
		);
	}
	if let Some(age) = &changes.age_rating {
		let value = age.map(|a| {
			if a >= 18 {
				"Adults Only 18+"
			} else if a <= 0 {
				"Everyone"
			} else {
				"Teen"
			}
		});
		set_comic_element(&mut xml, "AgeRating", value);
	}
	xml
}

/// Writes the changes into a CBZ file's ComicInfo.xml (in place, through a temporary file)
fn write_cbz_changes(path: &Path, changes: &FileChanges) -> WriteResult<()> {
	let (existing, info_name) = {
		let mut archive = ZipArchive::new(File::open(path)?)?;
		let name = archive
			.file_names()
			.find(|n| n.eq_ignore_ascii_case("comicinfo.xml"))
			.map(str::to_string);
		match name {
			Some(name) => {
				let mut text = String::new();
				archive.by_name(&name)?.read_to_string(&mut text)?;
				(Some(text), name)
			},
			None => (None, "ComicInfo.xml".to_string()),
		}
	};
	let info = apply_comic_info_changes(existing.as_deref(), changes);
	if existing.as_deref() == Some(info.as_str()) {
		return Ok(());
	}
	check_xml(&info)?;
	let tmp = super::write::temp_path(path);
	let original_count = ZipArchive::new(File::open(path)?)?.len();
	let result = rewrite_zip(
		path,
		&tmp,
		&|name| name.eq_ignore_ascii_case("comicinfo.xml"),
		&mut |writer| {
			writer.start_file(
				info_name.as_str(),
				SimpleFileOptions::default()
					.compression_method(CompressionMethod::Deflated),
			)?;
			writer.write_all(info.as_bytes())?;
			Ok(())
		},
	)
	.and_then(|_| {
		let mut check = ZipArchive::new(File::open(&tmp)?)?;
		let mut text = String::new();
		check.by_name(&info_name)?.read_to_string(&mut text)?;
		let expected = original_count + usize::from(existing.is_none());
		if check.len() != expected || text != info {
			return Err(WriteError::Invalid("rewritten CBZ check failed".into()));
		}
		std::fs::rename(&tmp, path)?;
		Ok(())
	});
	if result.is_err() {
		let _ = std::fs::remove_file(&tmp);
	}
	result
}

// ------------------------------------------------------------------ PDF

/// A PDF Keywords value without commas inside its items (Stump splits on them)
fn keyword_item(value: &str) -> String {
	value
		.replace(", ", " · ")
		.replace(',', " ·")
		.replace(';', " ·")
}

/// The Keywords after the changes: the tags replaced when genres changed, the curator's
/// `bcurator:` entries for series, number and age replaced when those changed, and every
/// other entry (e.g. `bcurator:rating=`) kept
pub fn pdf_keywords_after(existing: Option<&str>, changes: &FileChanges) -> String {
	let mut tags: Vec<String> = vec![];
	let mut curator: Vec<(String, String)> = vec![];
	for item in existing
		.unwrap_or("")
		.split([',', ';'])
		.map(str::trim)
		.filter(|s| !s.is_empty())
	{
		match item
			.strip_prefix("bcurator:")
			.and_then(|rest| rest.split_once('='))
		{
			Some((key, value)) => {
				curator.push((key.trim().to_string(), value.trim().to_string()))
			},
			None => tags.push(item.to_string()),
		}
	}
	if let Some(genres) = &changes.genres {
		tags = genres.iter().map(|g| keyword_item(g)).collect();
	}
	let mut set_curator = |key: &str, value: Option<String>| {
		let at = curator.iter().position(|(k, _)| k == key);
		match (at, value) {
			(Some(i), Some(v)) => curator[i].1 = v,
			(Some(i), None) => {
				curator.remove(i);
			},
			(None, Some(v)) => curator.push((key.to_string(), v)),
			(None, None) => {},
		}
	};
	if let Some((series, number)) = &changes.series {
		set_curator("series", series.as_deref().map(keyword_item));
		set_curator("series_index", series.as_ref().and(number.map(num)));
	}
	if let Some(age) = &changes.age_rating {
		set_curator(
			"age",
			age.map(|a| {
				if a >= 18 {
					"18+".to_string()
				} else {
					"All ages".to_string()
				}
			}),
		);
	}
	let mut seen = BTreeSet::new();
	tags.into_iter()
		.chain(
			curator
				.into_iter()
				.map(|(k, v)| format!("bcurator:{k}={v}")),
		)
		.filter(|k| seen.insert(k.clone()))
		.collect::<Vec<_>>()
		.join(", ")
}

/// The PDF Info entries after the changes, keeping what did not change
pub fn pdf_info_after(
	existing: &PdfInfoValues,
	changes: &FileChanges,
) -> Vec<(String, String)> {
	let mut info: Vec<(String, String)> = vec![];
	let title = match &changes.title {
		Some(Some(title)) => Some(title.clone()),
		_ => existing.title.clone(),
	};
	if let Some(title) = title {
		info.push(("Title".into(), pdf_string(&title)));
	}
	let author = match &changes.writers {
		Some(writers) => Some(writers.join(", ")).filter(|a| !a.is_empty()),
		None => existing.author.clone(),
	};
	if let Some(author) = author {
		info.push(("Author".into(), pdf_string(&author)));
	}
	let subject = match &changes.summary {
		Some(summary) => summary.clone(),
		None => existing.subject.clone(),
	};
	if let Some(subject) = subject {
		info.push(("Subject".into(), pdf_string(&subject)));
	}
	let keywords = pdf_keywords_after(existing.keywords.as_deref(), changes);
	if !keywords.is_empty() {
		info.push(("Keywords".into(), pdf_string(&keywords)));
	}
	KeptPdfInfo {
		creator: existing.creator.clone(),
		producer: existing.producer.clone(),
		creation_date: existing.creation_date.clone(),
	}
	.push_entries(&mut info);
	info
}

// ------------------------------------------------------------------ the whole edit

/// How a metadata edit reached the book file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileWriteStatus {
	/// The file now carries the changed values
	Written,
	/// None of the changes is something the file carries
	NotNeeded,
	/// The file could not take the changes: they are kept in Stump only (and locked)
	Failed,
}

/// The outcome of writing an edit into its book file
#[derive(Debug, Clone)]
pub struct FileEditOutcome {
	pub status: FileWriteStatus,
	/// The changed fields now in the file
	pub file_fields: Vec<&'static str>,
	/// The changed fields kept in Stump only
	pub db_only_fields: Vec<&'static str>,
	/// The locks that keep the edited fields through rescans (set when saving)
	pub locks: Vec<MetadataField>,
	/// Why the file was not written, or a note about it
	pub message: Option<String>,
	pub csv_path: Option<String>,
}

impl FileEditOutcome {
	fn nothing() -> Self {
		Self {
			status: FileWriteStatus::NotNeeded,
			file_fields: vec![],
			db_only_fields: vec![],
			locks: vec![],
			message: None,
			csv_path: None,
		}
	}
}

/// What the file says after the write, for the fields that were written
fn verify_read_back(
	written: &crate::metadata::ProcessedMediaMetadata,
	changes: &FileChanges,
	format: EditableFormat,
) -> Result<(), String> {
	let mut wrong = vec![];
	let set = |v: &[String]| {
		v.iter()
			.map(|s| s.trim().to_lowercase())
			.collect::<BTreeSet<_>>()
	};
	if let Some(Some(title)) = &changes.title {
		if written.title.as_deref().map(str::trim) != Some(title.trim()) {
			wrong.push("title");
		}
	}
	if let Some(writers) = &changes.writers {
		if set(written.writers.as_deref().unwrap_or_default()) != set(writers) {
			wrong.push("authors");
		}
	}
	if let Some(genres) = &changes.genres {
		// a PDF's Keywords only lose the commas inside an item
		let expected: Vec<String> = match format {
			EditableFormat::Pdf => genres.iter().map(|g| keyword_item(g)).collect(),
			_ => genres.clone(),
		};
		let found: Vec<String> = written
			.genres
			.as_deref()
			.unwrap_or_default()
			.iter()
			.flat_map(|g| g.split(','))
			.map(|g| g.trim().to_string())
			.filter(|g| !g.is_empty())
			.collect();
		if set(&found) != set(&expected) {
			wrong.push("genres");
		}
	}
	if let Some((series, number)) = &changes.series {
		if written.series.as_deref().map(str::trim) != series.as_deref().map(str::trim) {
			wrong.push("series");
		}
		if series.is_some() {
			let same = match (written.number, number) {
				(Some(a), Some(b)) => (a - b).abs() < 1e-6,
				(None, None) => true,
				_ => false,
			};
			if !same {
				wrong.push("number");
			}
		}
	}
	if let Some(Some(year)) = changes.date.map(|d| d.map(|(y, _, _)| y)) {
		if written.year != Some(year) {
			wrong.push("year");
		}
	}
	if let Some(summary) = &changes.summary {
		if written.summary.is_some() != summary.is_some() {
			wrong.push("summary");
		}
	}
	if let Some(publisher) = &changes.publisher {
		if written.publisher.as_deref().map(str::trim)
			!= publisher.as_deref().map(str::trim)
		{
			wrong.push("publisher");
		}
	}
	if let Some(Some(language)) = &changes.language {
		if written.language.as_deref().map(str::trim) != Some(language.trim()) {
			wrong.push("language");
		}
	}
	if let Some(age) = &changes.age_rating {
		let expected = age.map(|a| {
			if a >= 18 {
				18
			} else if a <= 0 {
				0
			} else {
				a
			}
		});
		if format != EditableFormat::Cbz || age.is_none_or(|a| a >= 18 || a <= 0) {
			if written.age_rating.map(|a| a.max(0)) != expected.map(|a| a.max(0)) {
				wrong.push("age rating");
			}
		}
	}
	if wrong.is_empty() {
		Ok(())
	} else {
		Err(format!(
			"the file read back without the new {}",
			wrong.join(", ")
		))
	}
}

/// The hidden copy of a book next to it that receives the edit (scans skip hidden files;
/// the name keeps the book's extension, which the readers look at)
fn work_path(path: &Path) -> PathBuf {
	let name = path
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_else(|| "book".into());
	path.with_file_name(format!(".stump-edit.{name}"))
}

/// A file's size and modification time, to notice it changing under an edit
fn file_stamp(path: &Path) -> std::io::Result<(u64, Option<std::time::SystemTime>)> {
	let meta = std::fs::metadata(path)?;
	Ok((meta.len(), meta.modified().ok()))
}

/// Writes `changes` into a copy of the book next to it, reads the copy back with Stump's
/// readers and returns its path, ready to replace the book
async fn prepare_edited_copy(
	ctx: &Ctx,
	path: &Path,
	format: EditableFormat,
	changes: &FileChanges,
) -> Result<PathBuf, String> {
	let work = work_path(path);
	let existing_pdf = if format == EditableFormat::Pdf {
		let (path, pdfium) = (path.to_path_buf(), ctx.config.pdfium_path.clone());
		Some(
			tokio::task::spawn_blocking(move || read_info_values(&path, &pdfium))
				.await
				.map_err(|e| e.to_string())?
				.map_err(|e| format!("could not read the PDF's metadata: {e}"))?,
		)
	} else {
		None
	};
	let (src, dst, changes_owned) = (path.to_path_buf(), work.clone(), changes.clone());
	let written = tokio::task::spawn_blocking(move || -> WriteResult<()> {
		std::fs::copy(&src, &dst)?;
		match format {
			EditableFormat::Epub => write_epub_changes(&dst, &changes_owned),
			EditableFormat::Cbz => write_cbz_changes(&dst, &changes_owned),
			EditableFormat::Pdf => append_pdf_info(
				&dst,
				&pdf_info_after(&existing_pdf.unwrap_or_default(), &changes_owned),
			),
		}?;
		// the archive writers make a new file: give it the book's permissions back
		std::fs::set_permissions(&dst, std::fs::metadata(&src)?.permissions())?;
		Ok(())
	})
	.await
	.map_err(|e| e.to_string())?;
	if let Err(error) = written {
		let _ = tokio::fs::remove_file(&work).await;
		return Err(error.to_string());
	}
	let read = process_metadata(&work, OrganizationMode::Metadata, &ctx.config).await;
	let checked = match read {
		Ok(Some(meta)) => verify_read_back(&meta, changes, format),
		Ok(None) => Err("the file read back without metadata".to_string()),
		Err(error) => Err(format!("the file could not be read back: {error}")),
	};
	if let Err(error) = checked {
		let _ = tokio::fs::remove_file(&work).await;
		return Err(error);
	}
	Ok(work)
}

/// Puts the edited copy in the book's place: the copy is flushed to disk and renamed over
/// the book (same folder, so the swap is atomic). Refused when the book changed since the
/// copy was taken (another tool rewrote it); the book is then left as it is.
fn replace_book(
	path: &Path,
	work: &Path,
	stamp: (u64, Option<std::time::SystemTime>),
) -> std::io::Result<()> {
	if file_stamp(path)? != stamp {
		return Err(std::io::Error::other(
			"the book file changed while it was being edited",
		));
	}
	File::open(work)?.sync_all()?;
	std::fs::rename(work, path)?;
	if let Some(dir) = path.parent() {
		// the rename itself, on disk (best effort: not every filesystem allows it)
		let _ = File::open(dir).and_then(|d| d.sync_all());
	}
	Ok(())
}

/// Writes the changed fields of an edit into the book file and logs them in the edits CSV:
/// the book's metadata now is `new`. Returns which changed fields the file now carries and
/// which stay in Stump only.
pub async fn write_edit(
	ctx: &Ctx,
	book: &media::Model,
	library: &library::Model,
	library_config: &library_config::Model,
	changed: &[ChangedField],
	new: &media_metadata::Model,
	username: &str,
) -> FileEditOutcome {
	let path = PathBuf::from(&book.path);
	let format = EditableFormat::of(&path);

	let mut outcome = FileEditOutcome::nothing();
	let changes = format
		.map(|f| file_changes(f, changed, new))
		.unwrap_or_default();
	let mut written: Vec<&'static str> = vec![];
	if let (Some(format), false) = (format, changes.is_empty()) {
		let stamp = {
			let p = path.clone();
			tokio::task::spawn_blocking(move || file_stamp(&p)).await
		};
		let prepared = match stamp {
			// boxed, like the other steps of a save: their futures are big, and nested
			// they overflowed a worker thread's stack (debug builds)
			Ok(Ok(stamp)) => Box::pin(prepare_edited_copy(ctx, &path, format, &changes))
				.await
				.map(|work| (work, stamp)),
			Ok(Err(error)) => Err(format!("the book file could not be read: {error}")),
			Err(error) => Err(error.to_string()),
		};
		match prepared {
			Ok((work, stamp)) => {
				let (p, w) = (path.clone(), work.clone());
				let replaced =
					tokio::task::spawn_blocking(move || replace_book(&p, &w, stamp))
						.await;
				match replaced {
					Ok(Ok(())) => {
						outcome.status = FileWriteStatus::Written;
						written = fields_written(changed, &changes);
						// the new size and hashes, and a modification time after the file's,
						// so the next scan doesn't rebuild the book
						if let Err(error) = Box::pin(refresh_media_after_swap(
							ctx.conn.as_ref(),
							&ctx.config,
							library_config,
							book,
						))
						.await
						{
							tracing::warn!(?error, path = %book.path, "Could not refresh an edited book");
						}
					},
					Ok(Err(error)) => {
						let _ = tokio::fs::remove_file(&work).await;
						outcome.status = FileWriteStatus::Failed;
						outcome.message =
							Some(format!("The book file was not changed: {error}"));
					},
					Err(error) => {
						let _ = tokio::fs::remove_file(&work).await;
						outcome.status = FileWriteStatus::Failed;
						outcome.message =
							Some(format!("The book file was not changed: {error}"));
					},
				}
			},
			Err(error) => {
				outcome.status = FileWriteStatus::Failed;
				outcome.message = Some(format!("The book file was not changed: {error}"));
			},
		}
	} else if format.is_none() && !changed.is_empty() {
		outcome.message = Some(
			"This kind of file can't carry metadata: the changes are kept in Stump"
				.to_string(),
		);
	}

	for change in changed {
		if written.contains(&change.field) {
			outcome.file_fields.push(change.field);
		} else {
			outcome.db_only_fields.push(change.field);
		}
	}
	if outcome.status == FileWriteStatus::Failed {
		tracing::warn!(path = %book.path, message = ?outcome.message, "A metadata edit stayed in Stump only");
	}

	// the edits log, for the curator
	if !changed.is_empty() {
		if let Ok(settings) = settings_for(ctx.conn.as_ref(), &library.id).await {
			let root = PathBuf::from(&library.path);
			let csv = csv_path_for(&root, &settings).with_file_name(EDITS_CSV_NAME);
			let csv_dir = csv.parent().map(Path::to_path_buf).unwrap_or(root);
			let rel = path
				.strip_prefix(&csv_dir)
				.unwrap_or(&path)
				.to_string_lossy()
				.into_owned();
			let now =
				chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
			let rows = changed
				.iter()
				.map(|c| {
					vec![
						rel.clone(),
						c.field.to_string(),
						c.old.clone(),
						c.new.clone(),
						if written.contains(&c.field) {
							"file"
						} else {
							"stump"
						}
						.to_string(),
						username.to_string(),
						now.clone(),
					]
				})
				.collect();
			match super::append_csv_with_header(&csv, EDITS_CSV_HEADER, rows).await {
				Ok(()) => outcome.csv_path = Some(csv.to_string_lossy().into_owned()),
				Err(error) => tracing::warn!(?error, "Could not write the edits CSV"),
			}
		}
	}
	super::library_index::invalidate_index(&library.id);
	outcome
}

// ------------------------------------------------------------------ saving and history

/// The locks that keep changed fields through rescans: every field edited by hand is
/// locked, so a rescan after another tool rewrote the file (book-curator replaces
/// subjects) keeps what was set
pub fn locks_for(changed: &[ChangedField]) -> Vec<MetadataField> {
	let mut locks = vec![];
	for lock in changed.iter().filter_map(|c| c.lock) {
		if !locks.contains(&lock) {
			locks.push(lock);
		}
	}
	locks
}

/// Who saves an edit
#[derive(Debug, Clone, Copy)]
pub struct Editor<'a> {
	pub id: &'a str,
	pub username: &'a str,
}

/// A saved metadata edit
#[derive(Debug, Clone)]
pub struct SavedEdit {
	/// The save's id in the edit history (`None` when nothing changed)
	pub save_id: Option<String>,
	pub outcome: FileEditOutcome,
	/// The book after the save
	pub book: media::Model,
	/// For a revert: the fields left as they are because they changed again since
	pub skipped: Vec<String>,
}

fn non_empty(value: &str) -> Option<String> {
	(!value.is_empty()).then(|| value.to_string())
}

/// Saves an edit of a book's metadata: the database row first (with the book's series
/// membership), then the book file ([`write_edit`]), the locks for the edited fields and
/// a row per changed field in the edit history. `active` holds the edited values (fields
/// left unset stay as they are); `reverts` names the save a revert undoes.
pub async fn save_edit(
	ctx: &Ctx,
	book: media::Model,
	existing: Option<media_metadata::Model>,
	mut active: media_metadata::ActiveModel,
	editor: Editor<'_>,
	reverts: Option<&str>,
) -> CoreResult<SavedEdit> {
	let conn = ctx.conn.as_ref();
	let (library, library_config) = library::Entity::find_by_id(book.library_id.clone())
		.find_also_related(library_config::Entity)
		.one(conn)
		.await?
		.ok_or_else(|| {
			CoreError::NotFound(format!("Library {} not found", book.library_id))
		})?;
	let library_config = library_config
		.ok_or_else(|| CoreError::NotFound("Library configuration".to_string()))?;
	let old = existing.clone().unwrap_or_default();

	let txn = conn.begin().await?;
	active.media_id = Set(Some(book.id.clone()));
	let mut updated = if let Some(existing) = &existing {
		active.id = Set(existing.id);
		active.update(&txn).await?
	} else {
		active.insert(&txn).await?
	};
	let changed = changed_fields(&old, &updated);
	// the edited fields are locked along with their values, before the file changes: a
	// scan that the file change sets off (the library watcher) then keeps them, even the
	// ones the file can't carry
	let locks = locks_for(&changed);
	let mut locked = locked_fields_of(&updated.locked_fields);
	let before = locked.len();
	for lock in &locks {
		if !locked.contains(lock) {
			locked.push(*lock);
		}
	}
	if locked.len() != before {
		let mut lock = updated.clone().into_active_model();
		lock.locked_fields = Set(Some(serde_json::to_value(&locked)?));
		updated = lock.update(&txn).await?;
	}
	let (book, thumbnails) =
		sync_media_membership(&txn, book, updated.series.clone()).await?;
	txn.commit().await?;
	remove_series_thumbnails(thumbnails).await;

	let mut outcome = Box::pin(write_edit(
		ctx,
		&book,
		&library,
		&library_config,
		&changed,
		&updated,
		editor.username,
	))
	.await;
	outcome.locks = locks;

	let save_id = if changed.is_empty() {
		None
	} else {
		let save_id = uuid::Uuid::new_v4().to_string();
		let now = chrono::Utc::now();
		let rows = changed.iter().map(|c| media_metadata_edit::ActiveModel {
			media_id: Set(book.id.clone()),
			save_id: Set(save_id.clone()),
			field: Set(c.field.to_string()),
			old_value: Set(non_empty(&c.old)),
			new_value: Set(non_empty(&c.new)),
			edited_by: Set(Some(editor.id.to_string())),
			edited_at: Set(now),
			written_to_file: Set(outcome.file_fields.contains(&c.field)),
			reverts_save_id: Set(reverts.map(str::to_string)),
			..Default::default()
		});
		media_metadata_edit::Entity::insert_many(rows)
			.exec(conn)
			.await?;
		Some(save_id)
	};

	let book = media::Entity::find_by_id(book.id.clone())
		.one(conn)
		.await?
		.unwrap_or(book);
	Ok(SavedEdit {
		save_id,
		outcome,
		book,
		skipped: vec![],
	})
}

/// The edit that puts back the values a save changed, for the fields that still have the
/// value it gave them (the others changed again since: they are left, and listed)
pub fn revert_changes(
	current: &media_metadata::Model,
	rows: &[media_metadata_edit::Model],
) -> (media_metadata::ActiveModel, Vec<String>, usize) {
	let mut active = current.clone().into_active_model();
	let mut skipped = vec![];
	let mut reverted = 0;
	for row in rows {
		let now = field_text(current, &row.field);
		if now != row.new_value.clone().unwrap_or_default() {
			skipped.push(row.field.clone());
			continue;
		}
		if set_field_from_text(
			&mut active,
			&row.field,
			row.old_value.as_deref().unwrap_or(""),
		) {
			reverted += 1;
		} else {
			skipped.push(row.field.clone());
		}
	}
	(active, skipped, reverted)
}

/// Reverts a save: its fields get their old values back through the same path as an edit
/// (book file, database, locks), recorded as a new save that names the reverted one
pub async fn revert_save(
	ctx: &Ctx,
	book: media::Model,
	existing: Option<media_metadata::Model>,
	save_id: &str,
	editor: Editor<'_>,
) -> CoreResult<SavedEdit> {
	let rows = media_metadata_edit::Entity::find()
		.filter(media_metadata_edit::Column::MediaId.eq(book.id.clone()))
		.filter(media_metadata_edit::Column::SaveId.eq(save_id))
		.order_by_asc(media_metadata_edit::Column::Id)
		.all(ctx.conn.as_ref())
		.await?;
	if rows.is_empty() {
		return Err(CoreError::NotFound(
			"This edit is not in the book's history".to_string(),
		));
	}
	let current = existing
		.clone()
		.ok_or_else(|| CoreError::NotFound("The book has no metadata".to_string()))?;
	let (active, skipped, reverted) = revert_changes(&current, &rows);
	if reverted == 0 {
		return Err(CoreError::BadRequest(
			"Nothing to revert: every field of this edit has changed again since"
				.to_string(),
		));
	}
	let mut saved = Box::pin(save_edit(
		ctx,
		book,
		existing,
		active,
		editor,
		Some(save_id),
	))
	.await?;
	saved.skipped = skipped;
	Ok(saved)
}

/// One save of a book's edit history
#[derive(Debug, Clone)]
pub struct EditHistorySave {
	pub save_id: String,
	pub edited_at: chrono::DateTime<chrono::Utc>,
	/// The username of who saved it
	pub edited_by: Option<String>,
	pub reverts_save_id: Option<String>,
	/// The save that reverted this one
	pub reverted_by: Option<String>,
	pub changes: Vec<media_metadata_edit::Model>,
}

/// Groups a book's edit rows (newest first) into saves, at most `limit` of them
pub fn group_saves(
	rows: Vec<media_metadata_edit::Model>,
	usernames: &HashMap<String, String>,
	limit: usize,
) -> Vec<EditHistorySave> {
	let reverted_by: HashMap<String, String> = rows
		.iter()
		.filter_map(|r| r.reverts_save_id.clone().map(|s| (s, r.save_id.clone())))
		.collect();
	let mut saves: Vec<EditHistorySave> = vec![];
	for row in rows {
		match saves.iter_mut().find(|s| s.save_id == row.save_id) {
			Some(save) => save.changes.push(row),
			None => {
				if saves.len() == limit {
					continue;
				}
				saves.push(EditHistorySave {
					save_id: row.save_id.clone(),
					edited_at: row.edited_at,
					edited_by: row
						.edited_by
						.as_ref()
						.and_then(|id| usernames.get(id).cloned()),
					reverts_save_id: row.reverts_save_id.clone(),
					reverted_by: reverted_by.get(&row.save_id).cloned(),
					changes: vec![row],
				});
			},
		}
	}
	for save in &mut saves {
		// in the order the editor shows the fields
		save.changes.sort_by_key(|c| {
			EDIT_FIELDS
				.iter()
				.position(|(f, _)| *f == c.field)
				.unwrap_or(usize::MAX)
		});
	}
	saves
}

/// A book's latest saves, newest first
pub async fn edit_history(
	ctx: &Ctx,
	media_id: &str,
	limit: usize,
) -> CoreResult<Vec<EditHistorySave>> {
	let conn = ctx.conn.as_ref();
	let rows = media_metadata_edit::Entity::find()
		.filter(media_metadata_edit::Column::MediaId.eq(media_id))
		.order_by_desc(media_metadata_edit::Column::EditedAt)
		.order_by_desc(media_metadata_edit::Column::Id)
		.all(conn)
		.await?;
	let ids: BTreeSet<String> = rows.iter().filter_map(|r| r.edited_by.clone()).collect();
	let usernames: HashMap<String, String> = if ids.is_empty() {
		HashMap::new()
	} else {
		user::Entity::find()
			.filter(user::Column::Id.is_in(ids))
			.all(conn)
			.await?
			.into_iter()
			.map(|u| (u.id, u.username))
			.collect()
	};
	Ok(group_saves(rows, &usernames, limit))
}

// ------------------------------------------------------------------ suggestions

/// How well one of the book's current tags fits, judged from everything but the book
/// itself
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentTagFit {
	pub value: String,
	pub kind: &'static str,
	/// The evidence for it, 0-1
	pub confidence: f64,
	/// Whether anything besides the book itself supports it (facets always fit)
	pub fits: bool,
	pub reasons: Vec<String>,
	pub library_count: i32,
}

/// Genre suggestions for a book already in the library
#[derive(Debug, Clone, Default)]
pub struct ExistingBookSuggestions {
	pub suggestions: Vec<IntakeGenreSuggestion>,
	pub current: Vec<CurrentTagFit>,
	pub similar_books: Vec<IntakeSimilarBook>,
	/// Subjects found online
	pub subjects: Vec<String>,
	pub looked_up_online: bool,
}

/// Under this evidence a current genre does not fit the book
pub const FIT_THRESHOLD: f64 = 0.25;

/// The metadata the suggestions work from: what Stump knows about the book, without its
/// current tags (they are what is being judged)
pub fn intake_metadata_of(meta: &media_metadata::Model) -> IntakeMetadata {
	let mut out = IntakeMetadata {
		title: meta.title.clone(),
		authors: split_list(&meta.writers),
		series: meta.series.clone().filter(|s| !s.trim().is_empty()),
		series_index: decimal_f64(&meta.number),
		language: meta
			.language
			.as_deref()
			.and_then(super::naming::language_code),
		year: meta.year,
		publisher: meta.publisher.clone(),
		description: meta
			.summary
			.as_deref()
			.map(strip_html)
			.filter(|d| !d.is_empty()),
		isbn: meta.identifier_isbn.clone(),
		..Default::default()
	};
	for field in ["title", "authors", "series", "year", "description", "isbn"] {
		out.set_source(field, IntakeSource::File);
	}
	out
}

fn judge_current(
	index: &LibraryIndex,
	current: &[String],
	scores: &HashMap<u32, super::suggest::Score>,
) -> Vec<CurrentTagFit> {
	current
		.iter()
		.map(|value| {
			let facet = is_facet(value);
			let tag = index.vocab_index(&norm(value));
			let score = tag.and_then(|t| scores.get(&t));
			let confidence = score.map(|s| s.value.min(1.0)).unwrap_or(0.0);
			let library_count = tag
				.map(|t| index.vocabulary[t as usize].count)
				.unwrap_or(0);
			CurrentTagFit {
				value: value.clone(),
				kind: if facet { "FACET" } else { "GENRE" },
				confidence: (confidence * 100.0).round() / 100.0,
				fits: facet || confidence >= FIT_THRESHOLD,
				reasons: score.map(|s| s.reasons.clone()).unwrap_or_else(|| {
					vec!["nothing else points to it: not the author's other books, similar books or catalogue subjects".to_string()]
				}),
				library_count,
			}
		})
		.collect()
}

/// Suggests tags for a book already in the library from its author's and series' other
/// books, books with similar descriptions and (when `online`) Open Library / Google Books
/// subjects, and judges its current tags against the same evidence
pub async fn suggest_for_book(
	ctx: &Ctx,
	book: &media::Model,
	meta: &media_metadata::Model,
	online: Option<bool>,
) -> CoreResult<ExistingBookSuggestions> {
	let conn = ctx.conn.as_ref();
	let library = library::Entity::find_by_id(book.library_id.clone())
		.one(conn)
		.await?
		.ok_or_else(|| {
			CoreError::NotFound(format!("Library {} not found", book.library_id))
		})?;
	let settings = settings_for(conn, &library.id).await?;
	let index = library_index(
		conn,
		&library.id,
		Path::new(&library.path),
		&settings.manga_dir,
		&settings.light_novel_dir,
	)
	.await?;
	let me = index.book_position(&book.id);

	let mut intake_meta = intake_metadata_of(meta);
	let wants_online = online.unwrap_or(settings.online_lookup);
	let format = EditableFormat::of(Path::new(&book.path));
	let is_comic = matches!(format, Some(EditableFormat::Cbz))
		|| book.extension.eq_ignore_ascii_case("cbr");
	let records = if wants_online && !is_comic {
		lookup::lookup(
			intake_meta.isbn.as_deref(),
			intake_meta.title.as_deref(),
			&intake_meta.authors,
		)
		.await
	} else {
		vec![]
	};
	merge_online(&mut intake_meta, &records);

	let description = intake_meta.description.clone().unwrap_or_default();
	let similar: Vec<(u32, f32)> = index
		.similar_by_description(conn, &description, 12)
		.await
		.into_iter()
		.filter(|(b, _)| Some(*b) != me)
		.collect();
	let folder = me
		.and_then(|i| index.books[i as usize].folder.clone())
		.filter(|f| !f.starts_with('_'));
	let series_kind = me
		.and_then(|i| index.books[i as usize].folder.as_deref())
		.and_then(|f| {
			if f == settings.manga_dir {
				Some(IntakeSeriesKind::Manga)
			} else if f == settings.light_novel_dir {
				Some(IntakeSeriesKind::LightNovel)
			} else {
				None
			}
		});
	let scores = score_tags(
		&index,
		&intake_meta,
		folder.as_deref(),
		&similar,
		series_kind,
		me,
	);
	let current = split_list(&meta.genres);
	let fits = judge_current(&index, &current, &scores);
	let suggestions = rank_suggestions(&index, scores, series_kind);

	let similar_books = {
		let mut out = vec![];
		let mut seen = BTreeSet::new();
		let mut by_author: Vec<u32> = folder
			.as_deref()
			.map(|f| index.books_in_folder(f).to_vec())
			.unwrap_or_default();
		for author in &intake_meta.authors {
			by_author.extend_from_slice(index.books_by_author(author));
		}
		for b in by_author.into_iter().filter(|b| Some(*b) != me) {
			if seen.insert(b) && out.len() < 3 {
				out.push((b, "same author"));
			}
		}
		for (b, _) in similar.iter().take(6) {
			if seen.insert(*b) && out.len() < 6 {
				out.push((*b, "similar description"));
			}
		}
		out.into_iter()
			.map(|(b, reason)| {
				let book = &index.books[b as usize];
				IntakeSimilarBook {
					media_id: book.media_id.clone(),
					title: book.title.clone(),
					authors: book.writers.clone(),
					genres: book
						.genres
						.iter()
						.map(|g| index.vocabulary[*g as usize].value.clone())
						.collect(),
					reason: reason.to_string(),
				}
			})
			.collect()
	};

	Ok(ExistingBookSuggestions {
		suggestions,
		current: fits,
		similar_books,
		subjects: intake_meta.subjects.clone(),
		looked_up_online: !records.is_empty(),
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::intake::write::{pdf_keywords, write_cbz, write_epub};

	fn model() -> media_metadata::Model {
		media_metadata::Model {
			title: Some("Notes from the End of Everything".into()),
			writers: Some("Robert Pantano".into()),
			genres: Some(
				"Physics & Astronomy, Physics & Astronomy: Astronomy, Level: General Reader, Collection: Owned"
					.into(),
			),
			year: Some(2020),
			language: Some("en".into()),
			..Default::default()
		}
	}

	#[test]
	fn finds_changed_fields_and_what_each_format_carries() {
		let old = model();
		let mut new = model();
		new.genres = Some("Literary Fiction, Literary Fiction: Philosophical, Level: General Reader, Collection: Owned".into());
		new.notes = Some("read twice".into());
		let changed = changed_fields(&old, &new);
		assert_eq!(
			changed.iter().map(|c| c.field).collect::<Vec<_>>(),
			vec!["notes", "genres"]
		);
		let epub = file_changes(EditableFormat::Epub, &changed, &new);
		assert_eq!(
			epub.genres.as_deref(),
			Some(
				&[
					"Literary Fiction".to_string(),
					"Literary Fiction: Philosophical".into(),
					"Level: General Reader".into(),
					"Collection: Owned".into()
				][..]
			)
		);
		assert!(epub.title.is_none() && epub.writers.is_none());
		assert_eq!(fields_written(&changed, &epub), vec!["genres"]);

		// a language change: in EPUBs, not in PDFs or CBZs
		new.language = Some("es".into());
		let changed = changed_fields(&old, &new);
		assert!(file_changes(EditableFormat::Epub, &changed, &new)
			.language
			.is_some());
		assert!(file_changes(EditableFormat::Pdf, &changed, &new)
			.language
			.is_none());
		assert!(file_changes(EditableFormat::Cbz, &changed, &new)
			.language
			.is_none());
		assert_eq!(
			EditableFormat::of(Path::new("/a/b.kepub.epub")),
			Some(EditableFormat::Epub)
		);
		assert_eq!(EditableFormat::of(Path::new("/a/b.cbr")), None);
	}

	#[test]
	fn cleans_typed_genres_and_writers() {
		assert_eq!(
			clean_genres(&[
				"Origin: American".into(),
				"Philosophy, Biography & Memoir".into(),
				"  Philosophy ".into(),
				"Philosophy: Existentialism & Phenomenology".into(),
			]),
			vec![
				"Philosophy",
				"Biography & Memoir",
				"Philosophy: Existentialism & Phenomenology",
				"Origin: American"
			]
		);
		assert_eq!(
			clean_writers(&[
				"Pantano, Robert".into(),
				"Robert Pantano".into(),
				" ".into()
			]),
			vec!["Robert Pantano"]
		);
	}

	const OPF3: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title id="t1">Notes from the End of Everything</dc:title>
    <meta refines="#t1" property="title-type">main</meta>
    <dc:creator id="c1">Robert Pantano</dc:creator>
    <meta refines="#c1" property="file-as">Pantano, Robert</meta>
    <dc:subject>Physics &amp; Astronomy</dc:subject>
    <dc:subject>Physics &amp; Astronomy: Astronomy</dc:subject>
    <dc:subject>Level: General Reader</dc:subject>
    <meta name="bcurator:shelf" content="Physics &amp; Astronomy"/>
    <dc:date>2020-07-01</dc:date>
    <dc:date opf:event="modification">2024-01-01</dc:date>
    <dc:language>en</dc:language>
    <dc:identifier id="id">urn:isbn:9798667150664</dc:identifier>
  </metadata>
</package>"##;

	#[test]
	fn rewrites_only_the_changed_fields_of_an_opf() {
		let changes = FileChanges {
			genres: Some(vec![
				"Literary Fiction".into(),
				"Literary Fiction: Philosophical".into(),
				"Level: General Reader".into(),
			]),
			..Default::default()
		};
		let new = apply_opf_changes(OPF3, &changes).unwrap();
		check_xml(&new).unwrap();
		assert!(!new.contains("Physics"));
		assert!(new.contains("<dc:subject>Literary Fiction: Philosophical</dc:subject>"));
		assert!(
			new.contains(r#"<meta name="bcurator:shelf" content="Literary Fiction"/>"#)
		);
		// untouched: title and its refines, creator, dates, language, identifier
		assert!(
			new.contains(r##"<meta refines="#t1" property="title-type">main</meta>"##)
		);
		assert!(new.contains(
			r##"<meta refines="#c1" property="file-as">Pantano, Robert</meta>"##
		));
		assert!(new.contains("<dc:date>2020-07-01</dc:date>"));
		assert!(new
			.contains("<dc:identifier id=\"id\">urn:isbn:9798667150664</dc:identifier>"));

		let changes = FileChanges {
			title: Some(Some("Notes & More".into())),
			writers: Some(vec!["Robert Pantano".into(), "Ana López".into()]),
			series: Some((Some("Pursuit".into()), Some(2.0))),
			date: Some(Some((2021, Some(3), None))),
			summary: Some(Some("A <b>bold</b> novel".into())),
			publisher: Some(Some("Pursuit of Wonder".into())),
			age_rating: Some(Some(18)),
			language: Some(Some("es".into())),
			genres: Some(vec![]),
		};
		let new = apply_opf_changes(OPF3, &changes).unwrap();
		check_xml(&new).unwrap();
		assert!(new.contains(r#"<dc:title id="t1">Notes &amp; More</dc:title>"#));
		assert!(!new.contains("refines=\"#c1\""));
		assert!(new.contains(r##"<dc:creator id="stump-creator-2">Ana López</dc:creator><meta refines="#stump-creator-2" property="file-as">López, Ana</meta>"##));
		assert!(!new.contains("<dc:subject>"));
		assert!(!new.contains("bcurator:shelf"));
		assert!(new.contains(r#"<meta name="calibre:series" content="Pursuit"/>"#));
		assert!(new.contains(
			r##"<meta refines="#bcurator-series" property="group-position">2</meta>"##
		));
		assert!(new.contains("<dc:date>2021-03</dc:date>"));
		assert!(new.contains(r#"<dc:date opf:event="modification">2024-01-01</dc:date>"#));
		assert!(!new.contains("2020-07-01"));
		assert!(new.contains(
			"<dc:description>A &lt;b&gt;bold&lt;/b&gt; novel</dc:description>"
		));
		assert!(new.contains("<dc:publisher>Pursuit of Wonder</dc:publisher>"));
		assert!(new.contains(r#"<meta name="typicalAgeRange" content="18+"/>"#));
		assert!(new.contains("<dc:language>es</dc:language>"));
		assert!(!new.contains("<dc:language>en</dc:language>"));

		// removing the series removes every trace of it
		let removed = apply_opf_changes(
			&new,
			&FileChanges {
				series: Some((None, None)),
				summary: Some(None),
				..Default::default()
			},
		)
		.unwrap();
		assert!(!removed.contains("calibre:series"));
		assert!(!removed.contains("belongs-to-collection"));
		assert!(!removed.contains("bcurator-series"));
		assert!(!removed.contains("dc:description"));
	}

	#[test]
	fn edits_comic_info_in_place() {
		let existing = r#"<?xml version="1.0"?>
<ComicInfo>
  <Series>Yotsuba&amp;!</Series>
  <Number>3</Number>
  <Writer>Kiyohiko Azuma</Writer>
  <Genre>Manga &amp; Comics</Genre>
  <PageCount>200</PageCount>
  <Pages>
    <Page Image="0" Type="FrontCover" />
  </Pages>
</ComicInfo>"#;
		let changes = FileChanges {
			genres: Some(vec![
				"Manga & Comics".into(),
				"Manga & Comics: Slice of Life".into(),
			]),
			summary: Some(Some("Everyday adventures".into())),
			..Default::default()
		};
		let xml = apply_comic_info_changes(Some(existing), &changes);
		check_xml(&xml).unwrap();
		assert!(xml.contains(
			"<Genre>Manga &amp; Comics, Manga &amp; Comics: Slice of Life</Genre>"
		));
		assert!(xml.contains("<Summary>Everyday adventures</Summary>"));
		assert!(xml.contains(r#"<Page Image="0" Type="FrontCover" />"#));
		assert!(xml.contains("<Writer>Kiyohiko Azuma</Writer>"));
		assert_eq!(xml.matches("<Genre>").count(), 1);

		let created = apply_comic_info_changes(None, &changes);
		check_xml(&created).unwrap();
		assert!(created.contains("<Genre>"));
		assert!(!created.contains("LanguageISO"));
	}

	#[test]
	fn rewrites_pdf_keywords_keeping_curator_entries() {
		let existing = "Physics & Astronomy, Level: General Reader, bcurator:series=Saga · One, bcurator:series_index=2, bcurator:rating=4.12";
		let changes = FileChanges {
			genres: Some(vec!["Philosophy".into(), "Memoir, Essays".into()]),
			..Default::default()
		};
		assert_eq!(
			pdf_keywords_after(Some(existing), &changes),
			"Philosophy, Memoir · Essays, bcurator:series=Saga · One, bcurator:series_index=2, bcurator:rating=4.12"
		);
		let changes = FileChanges {
			series: Some((None, None)),
			age_rating: Some(Some(18)),
			..Default::default()
		};
		assert_eq!(
			pdf_keywords_after(Some(existing), &changes),
			"Physics & Astronomy, Level: General Reader, bcurator:rating=4.12, bcurator:age=18+"
		);
		// the intake's keywords read back the same way
		let meta = FileMetadata {
			tags: vec!["History".into()],
			series: Some("S".into()),
			series_index: Some(1.0),
			..Default::default()
		};
		assert_eq!(
			pdf_keywords_after(Some(&pdf_keywords(&meta)), &FileChanges::default()),
			pdf_keywords(&meta)
		);
		let info = pdf_info_after(
			&PdfInfoValues {
				title: Some("T".into()),
				author: Some("Eco, Umberto".into()),
				creator: Some("Word".into()),
				keywords: Some(existing.into()),
				..Default::default()
			},
			&FileChanges {
				writers: Some(vec!["Umberto Eco".into()]),
				..Default::default()
			},
		);
		let keys: Vec<&str> = info.iter().map(|(k, _)| k.as_str()).collect();
		assert_eq!(
			keys,
			vec!["Title", "Author", "Keywords", "Creator", "ModDate"]
		);
		assert_eq!(info[1].1, "(Umberto Eco)");
	}

	fn epub_file(dir: &Path) -> PathBuf {
		let path = dir.join("book.epub");
		let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
		zip.start_file(
			"mimetype",
			SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
		)
		.unwrap();
		zip.write_all(b"application/epub+zip").unwrap();
		zip.start_file("META-INF/container.xml", SimpleFileOptions::default())
			.unwrap();
		zip.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
		zip.start_file("OEBPS/content.opf", SimpleFileOptions::default())
			.unwrap();
		zip.write_all(OPF3.as_bytes()).unwrap();
		zip.start_file("OEBPS/chapter.xhtml", SimpleFileOptions::default())
			.unwrap();
		zip.write_all(b"<html><body><p>Hello</p></body></html>")
			.unwrap();
		zip.finish().unwrap();
		path
	}

	#[test]
	fn writes_epub_and_cbz_files_keeping_every_other_entry() {
		let dir = tempfile::tempdir().unwrap();
		let epub = epub_file(dir.path());
		let changes = FileChanges {
			genres: Some(vec!["Philosophy".into()]),
			..Default::default()
		};
		write_epub_changes(&epub, &changes).unwrap();
		let mut archive = ZipArchive::new(File::open(&epub).unwrap()).unwrap();
		assert_eq!(archive.by_index(0).unwrap().name(), "mimetype");
		let mut opf = String::new();
		archive
			.by_name("OEBPS/content.opf")
			.unwrap()
			.read_to_string(&mut opf)
			.unwrap();
		assert!(opf.contains("<dc:subject>Philosophy</dc:subject>"));
		assert!(!opf.contains("Physics"));
		let mut chapter = String::new();
		archive
			.by_name("OEBPS/chapter.xhtml")
			.unwrap()
			.read_to_string(&mut chapter)
			.unwrap();
		assert_eq!(chapter, "<html><body><p>Hello</p></body></html>");
		// the intake's writer and this one agree on what a curated file looks like
		let again = dir.path().join("again.epub");
		std::fs::copy(&epub, &again).unwrap();
		write_epub(
			&again,
			&FileMetadata {
				tags: vec!["Philosophy".into()],
				..Default::default()
			},
		)
		.unwrap();

		let cbz = dir.path().join("v01.cbz");
		let mut zip = zip::ZipWriter::new(File::create(&cbz).unwrap());
		zip.start_file("001.png", SimpleFileOptions::default())
			.unwrap();
		zip.write_all(b"png").unwrap();
		zip.finish().unwrap();
		write_cbz_changes(&cbz, &changes).unwrap();
		let mut archive = ZipArchive::new(File::open(&cbz).unwrap()).unwrap();
		assert_eq!(archive.len(), 2);
		let mut info = String::new();
		archive
			.by_name("ComicInfo.xml")
			.unwrap()
			.read_to_string(&mut info)
			.unwrap();
		assert!(info.contains("<Genre>Philosophy</Genre>"));
		// a second edit replaces the element instead of adding one
		write_cbz_changes(
			&cbz,
			&FileChanges {
				genres: Some(vec!["History".into()]),
				..Default::default()
			},
		)
		.unwrap();
		let mut archive = ZipArchive::new(File::open(&cbz).unwrap()).unwrap();
		assert_eq!(archive.len(), 2);
		let mut info = String::new();
		archive
			.by_name("ComicInfo.xml")
			.unwrap()
			.read_to_string(&mut info)
			.unwrap();
		assert!(info.contains("<Genre>History</Genre>"));
		assert!(!info.contains("Philosophy"));
		let _ = write_cbz;
	}

	#[test]
	fn read_back_check_catches_missing_values() {
		let changes = FileChanges {
			genres: Some(vec!["Philosophy".into(), "Biography & Memoir".into()]),
			title: Some(Some("T".into())),
			..Default::default()
		};
		let good = crate::metadata::ProcessedMediaMetadata {
			title: Some("T".into()),
			genres: Some(vec!["Biography & Memoir".into(), "Philosophy".into()]),
			..Default::default()
		};
		assert!(verify_read_back(&good, &changes, EditableFormat::Epub).is_ok());
		let bad = crate::metadata::ProcessedMediaMetadata {
			title: Some("T".into()),
			genres: Some(vec!["Philosophy".into()]),
			..Default::default()
		};
		assert!(verify_read_back(&bad, &changes, EditableFormat::Epub)
			.unwrap_err()
			.contains("genres"));
		// PDFs keep genres in one Keywords string, split on commas
		let pdf = crate::metadata::ProcessedMediaMetadata {
			title: Some("T".into()),
			genres: Some(vec!["Philosophy, Biography & Memoir".into()]),
			..Default::default()
		};
		assert!(verify_read_back(&pdf, &changes, EditableFormat::Pdf).is_ok());
	}
	#[test]
	fn every_edit_field_round_trips_through_its_text() {
		let meta = media_metadata::Model {
			title: Some("T".into()),
			title_sort: Some("T, The".into()),
			writers: Some("A, B".into()),
			series: Some("S".into()),
			number: Some(Decimal::new(25, 1)),
			series_group: Some("G".into()),
			story_arc: Some("Arc".into()),
			story_arc_number: Some(Decimal::new(3, 0)),
			volume: Some(2),
			summary: Some(" spaced summary ".into()),
			notes: Some("n".into()),
			genres: Some("Philosophy, Level: General Reader".into()),
			format: Some("f".into()),
			year: Some(2020),
			month: Some(7),
			day: Some(16),
			pencillers: Some("p".into()),
			inkers: Some("i".into()),
			colorists: Some("c".into()),
			letterers: Some("l".into()),
			cover_artists: Some("ca".into()),
			editors: Some("e".into()),
			publisher: Some("Pub".into()),
			links: Some("https://x".into()),
			characters: Some("ch".into()),
			teams: Some("te".into()),
			page_count: Some(300),
			age_rating: Some(18),
			identifier_amazon: Some("am".into()),
			identifier_calibre: Some("cal".into()),
			identifier_google: Some("go".into()),
			identifier_isbn: Some("978".into()),
			identifier_mobi_asin: Some("asin".into()),
			identifier_uuid: Some("uuid".into()),
			language: Some("en".into()),
			..Default::default()
		};
		let empty = media_metadata::Model::default();
		let changed = changed_fields(&empty, &meta);
		assert_eq!(
			changed.len(),
			EDIT_FIELDS.len(),
			"every field differs from empty"
		);
		for (field, _) in EDIT_FIELDS {
			// setting a field from its text gives the same value back
			let mut active = empty.clone().into_active_model();
			assert!(set_field_from_text(
				&mut active,
				field,
				&field_text(&meta, field)
			));
			let mut back = empty.clone();
			apply_set_values(&mut back, active);
			assert_eq!(
				field_text(&back, field),
				field_text(&meta, field),
				"{field}"
			);
			// and an empty text clears it
			let mut active = meta.clone().into_active_model();
			assert!(set_field_from_text(&mut active, field, ""));
			let mut back = meta.clone();
			apply_set_values(&mut back, active);
			assert_eq!(field_text(&back, field), "", "{field}");
		}
		let mut active = empty.clone().into_active_model();
		assert!(!set_field_from_text(&mut active, "year", "MMXX"));
		assert!(!set_field_from_text(&mut active, "nope", "x"));
	}

	/// The values an active model sets, applied to a model (what an update would store)
	fn apply_set_values(
		model: &mut media_metadata::Model,
		active: media_metadata::ActiveModel,
	) {
		use sea_orm::{ActiveValue, Iterable, ModelTrait};
		for column in media_metadata::Column::iter() {
			if let ActiveValue::Set(value) = active.get(column) {
				model.set(column, value);
			}
		}
	}

	fn edit_row(
		save_id: &str,
		field: &str,
		old: Option<&str>,
		new: Option<&str>,
	) -> media_metadata_edit::Model {
		media_metadata_edit::Model {
			id: 0,
			media_id: "book".into(),
			save_id: save_id.into(),
			field: field.into(),
			old_value: old.map(str::to_string),
			new_value: new.map(str::to_string),
			edited_by: Some("u1".into()),
			edited_at: chrono::Utc::now(),
			written_to_file: true,
			reverts_save_id: None,
		}
	}

	#[test]
	fn a_revert_puts_back_only_the_fields_that_did_not_change_since() {
		let current = media_metadata::Model {
			id: 7,
			genres: Some("Philosophy, Biography & Memoir".into()),
			title: Some("Changed again".into()),
			year: None,
			..Default::default()
		};
		let rows = vec![
			edit_row(
				"s1",
				"genres",
				Some("Physics & Astronomy"),
				Some("Philosophy, Biography & Memoir"),
			),
			edit_row("s1", "title", Some("Old"), Some("New")),
			edit_row("s1", "year", Some("2020"), None),
		];
		let (active, skipped, reverted) = revert_changes(&current, &rows);
		assert_eq!(reverted, 2);
		assert_eq!(skipped, vec!["title".to_string()]);
		let mut back = current.clone();
		apply_set_values(&mut back, active);
		assert_eq!(back.genres.as_deref(), Some("Physics & Astronomy"));
		assert_eq!(back.year, Some(2020));
		assert_eq!(back.title.as_deref(), Some("Changed again"));
	}

	#[test]
	fn history_groups_rows_into_saves_newest_first() {
		let mut revert = edit_row("s2", "genres", Some("B"), Some("A"));
		revert.reverts_save_id = Some("s1".into());
		let rows = vec![
			revert,
			edit_row("s1", "title", Some("x"), Some("y")),
			edit_row("s1", "genres", Some("A"), Some("B")),
			edit_row("s0", "notes", None, Some("n")),
		];
		let users = HashMap::from([("u1".to_string(), "g3".to_string())]);
		let saves = group_saves(rows.clone(), &users, 2);
		assert_eq!(saves.len(), 2);
		assert_eq!(saves[0].save_id, "s2");
		assert_eq!(saves[0].reverts_save_id.as_deref(), Some("s1"));
		assert_eq!(saves[1].reverted_by.as_deref(), Some("s2"));
		assert_eq!(saves[1].edited_by.as_deref(), Some("g3"));
		// in the editor's order: title before genres
		let fields: Vec<&str> =
			saves[1].changes.iter().map(|c| c.field.as_str()).collect();
		assert_eq!(fields, vec!["title", "genres"]);
		assert_eq!(group_saves(rows, &users, 20).len(), 3);
	}

	const BOOK_OPF: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Notes from the End of Everything</dc:title>
    <dc:creator>Robert Pantano</dc:creator>
    <dc:subject>Physics &amp; Astronomy</dc:subject>
    <dc:subject>Level: General Reader</dc:subject>
    <dc:language>en</dc:language>
    <dc:identifier id="id">urn:uuid:1</dc:identifier>
  </metadata>
  <manifest>
    <item id="c1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="c1"/>
  </spine>
</package>"##;

	fn readable_epub(path: &Path) {
		let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
		zip.start_file(
			"mimetype",
			SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
		)
		.unwrap();
		zip.write_all(b"application/epub+zip").unwrap();
		zip.start_file("META-INF/container.xml", SimpleFileOptions::default())
			.unwrap();
		zip.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
		zip.start_file("OEBPS/content.opf", SimpleFileOptions::default())
			.unwrap();
		zip.write_all(BOOK_OPF.as_bytes()).unwrap();
		zip.start_file("OEBPS/chapter.xhtml", SimpleFileOptions::default())
			.unwrap();
		zip.write_all(b"<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>c</title></head><body><p>Hello</p></body></html>").unwrap();
		zip.finish().unwrap();
	}

	fn opf_of(path: &Path) -> String {
		let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
		let mut opf = String::new();
		archive
			.by_name("OEBPS/content.opf")
			.unwrap()
			.read_to_string(&mut opf)
			.unwrap();
		opf
	}

	#[tokio::test]
	async fn saving_and_reverting_an_edit_rewrites_the_file_and_keeps_a_history() {
		use ::tests::fake_data;

		// the real migrations, so the history table is checked against the entity
		let db = crate::database::connect_at("sqlite::memory:")
			.await
			.unwrap();
		let dir = tempfile::tempdir().unwrap();
		let root = dir.path().join("biblioteca");
		std::fs::create_dir_all(root.join("Robert_Pantano")).unwrap();
		let book_path = root.join("Robert_Pantano/Notes.epub");
		readable_epub(&book_path);
		{
			use std::os::unix::fs::PermissionsExt;
			std::fs::set_permissions(&book_path, std::fs::Permissions::from_mode(0o640))
				.unwrap();
		}

		let library = fake_data::Library {
			path: Some(root.to_string_lossy().into_owned()),
			..Default::default()
		}
		.insert(&db)
		.await;
		let series = fake_data::Series {
			library_id: Some(library.id.clone()),
			..Default::default()
		}
		.insert(&db)
		.await;
		let book = fake_data::Media {
			series_id: series.id.clone(),
			path: Some(book_path.to_string_lossy().into_owned()),
			..Default::default()
		}
		.insert(&db)
		.await;
		let editor = fake_data::User::new("g3").insert(&db).await;
		let meta = media_metadata::ActiveModel {
			media_id: Set(Some(book.id.clone())),
			title: Set(Some("Notes from the End of Everything".into())),
			writers: Set(Some("Robert Pantano".into())),
			genres: Set(Some("Physics & Astronomy, Level: General Reader".into())),
			notes: Set(Some("mine".into())),
			..Default::default()
		}
		.insert(&db)
		.await
		.unwrap();
		let ctx = Ctx::for_testing(db);
		let editor = Editor {
			id: &editor.id,
			username: &editor.username,
		};

		// an edit: genres (in the file) and notes (in Stump only)
		let edit = media_metadata::ActiveModel {
			genres: Set(Some(
				"Philosophy, Biography & Memoir, Level: General Reader".into(),
			)),
			notes: Set(None),
			..Default::default()
		};
		let saved = save_edit(&ctx, book.clone(), Some(meta.clone()), edit, editor, None)
			.await
			.unwrap();
		assert_eq!(
			saved.outcome.status,
			FileWriteStatus::Written,
			"{:?}",
			saved.outcome.message
		);
		assert_eq!(saved.outcome.file_fields, vec!["genres"]);
		assert_eq!(saved.outcome.db_only_fields, vec!["notes"]);
		let opf = opf_of(&book_path);
		assert!(opf.contains("<dc:subject>Biography &amp; Memoir</dc:subject>"));
		assert!(!opf.contains("Physics"));
		assert!(opf.contains("<dc:title>Notes from the End of Everything</dc:title>"));
		// the book was swapped in place: no copy of the original, no leftovers
		let mut names: Vec<String> = std::fs::read_dir(root.join("Robert_Pantano"))
			.unwrap()
			.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
			.collect();
		names.sort();
		assert_eq!(names, vec!["Notes.epub"]);
		{
			use std::os::unix::fs::PermissionsExt;
			let mode = std::fs::metadata(&book_path).unwrap().permissions().mode();
			assert_eq!(mode & 0o777, 0o640, "the book keeps its permissions");
		}
		assert!(dir.path().join("biblioteca/stump_edits.csv").exists());

		let conn = ctx.conn.as_ref();
		let stored = media_metadata::Entity::find()
			.filter(media_metadata::Column::MediaId.eq(book.id.clone()))
			.one(conn)
			.await
			.unwrap()
			.unwrap();
		assert_eq!(
			stored.genres.as_deref(),
			Some("Philosophy, Biography & Memoir, Level: General Reader")
		);
		let locked = locked_fields_of(&stored.locked_fields);
		assert!(
			locked.contains(&MetadataField::Genres)
				&& locked.contains(&MetadataField::Notes)
		);
		let rows = media_metadata_edit::Entity::find().all(conn).await.unwrap();
		assert_eq!(rows.len(), 2);
		let genres_row = rows.iter().find(|r| r.field == "genres").unwrap();
		assert!(genres_row.written_to_file);
		assert_eq!(
			genres_row.old_value.as_deref(),
			Some("Physics & Astronomy, Level: General Reader")
		);
		assert_eq!(genres_row.edited_by.as_deref(), Some(editor.id));
		let notes_row = rows.iter().find(|r| r.field == "notes").unwrap();
		assert!(!notes_row.written_to_file);
		assert_eq!(notes_row.new_value, None);

		// reverting the save puts the old values back in the file and in Stump
		let save_id = saved.save_id.clone().unwrap();
		let reverted =
			revert_save(&ctx, book.clone(), Some(stored.clone()), &save_id, editor)
				.await
				.unwrap();
		assert_eq!(reverted.outcome.status, FileWriteStatus::Written);
		assert!(reverted.skipped.is_empty());
		let opf = opf_of(&book_path);
		assert!(opf.contains("<dc:subject>Physics &amp; Astronomy</dc:subject>"));
		assert!(!opf.contains("Biography"));
		let stored = media_metadata::Entity::find()
			.filter(media_metadata::Column::MediaId.eq(book.id.clone()))
			.one(conn)
			.await
			.unwrap()
			.unwrap();
		assert_eq!(
			stored.genres.as_deref(),
			Some("Physics & Astronomy, Level: General Reader")
		);
		assert_eq!(stored.notes.as_deref(), Some("mine"));

		let history = edit_history(&ctx, &book.id, 20).await.unwrap();
		assert_eq!(history.len(), 2);
		assert_eq!(
			history[0].reverts_save_id.as_deref(),
			Some(save_id.as_str())
		);
		assert_eq!(
			history[1].reverted_by,
			history.first().map(|s| s.save_id.clone())
		);
		assert_eq!(history[1].edited_by.as_deref(), Some("g3"));

		// a second revert of the same save has nothing left to put back
		assert!(
			revert_save(&ctx, book.clone(), Some(stored), &save_id, editor)
				.await
				.is_err()
		);

		// the history goes with its book
		media::Entity::delete_by_id(book.id.clone())
			.exec(conn)
			.await
			.unwrap();
		assert!(media_metadata_edit::Entity::find()
			.all(conn)
			.await
			.unwrap()
			.is_empty());
	}
}
