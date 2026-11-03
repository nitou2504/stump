//! Editing an existing book's metadata from the web.
//!
//! A rescan rebuilds a book's metadata from its file whenever the file changes (book-curator
//! rewrites files), and "Rebuild metadata" always does, so an edit kept only in Stump's
//! database does not last. Here the changed values are written into the book file the way
//! the intake and book-curator write them (EPUB OPF, PDF Info dictionary and Keywords, CBZ
//! ComicInfo.xml). Only the fields that changed are touched, everything else in the file is
//! kept as it is. The new file is prepared next to the book under a hidden name, read back
//! with Stump's own readers, and only then replaces the book; the original goes to the
//! quarantine folder first when one is set. Changes a file cannot carry (other fields, CBR
//! files, a failed write) stay in the database and are locked so rescans keep them.
//!
//! Every change is also appended to `stump_edits.csv`, next to the intake CSV, so the
//! curator can pick the owner's corrections up.

use std::{
	collections::{BTreeSet, HashMap},
	fs::File,
	io::{Read, Write},
	path::{Path, PathBuf},
};

use metadata_integrations::MetadataField;
use models::{
	entity::{library, library_config, media, media_metadata},
	shared::{
		enums::OrganizationMode,
		intake::{
			IntakeGenreSuggestion, IntakeMetadata, IntakeSeriesKind, IntakeSimilarBook,
			IntakeSource,
		},
	},
};
use regex::Regex;
use sea_orm::{prelude::Decimal, EntityTrait};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive};

use super::{
	csv_path_for,
	library_index::{is_facet, library_index, norm, LibraryIndex},
	lookup::{self, strip_html},
	naming::display_name,
	settings_for,
	suggest::{merge_online, rank_suggestions, score_tags},
	write::{
		append_pdf_info, check_xml, comic_info, esc, file_as, num,
		opf_path, order_tags, pdf_string, rewrite_zip, FileMetadata, KeptPdfInfo,
		WriteError, WriteResult,
	},
};
use crate::{
	media::processor::{
		pdf::{read_info_values, PdfInfoValues},
		process_metadata,
	},
	postproc::{refresh_media_after_swap, swap},
	CoreError, CoreResult, Ctx,
};

/// The edits log, next to the intake CSV
pub const EDITS_CSV_NAME: &str = "stump_edits.csv";

const EDITS_CSV_HEADER: &[&str] = &[
	"path", "field", "old", "new", "written_to", "edited_by", "edited_at",
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
				"title", "writers", "series", "number", "summary", "genres", "ageRating",
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

/// The fields that differ between the stored metadata and the edited one
pub fn changed_fields(
	old: &media_metadata::Model,
	new: &media_metadata::Model,
) -> Vec<ChangedField> {
	use MetadataField as F;
	let mut out = vec![];
	let mut push = |field: &'static str, lock: Option<F>, old: String, new: String| {
		if old != new {
			out.push(ChangedField {
				field,
				lock,
				old,
				new,
			});
		}
	};
	push("title", Some(F::Title), text(&old.title), text(&new.title));
	push("titleSort", Some(F::TitleSort), text(&old.title_sort), text(&new.title_sort));
	push("writers", Some(F::Writers), text(&old.writers), text(&new.writers));
	push("series", Some(F::Series), text(&old.series), text(&new.series));
	push("number", Some(F::Number), decimal_text(&old.number), decimal_text(&new.number));
	push("seriesGroup", Some(F::SeriesGroup), text(&old.series_group), text(&new.series_group));
	push("storyArc", Some(F::StoryArc), text(&old.story_arc), text(&new.story_arc));
	push(
		"storyArcNumber",
		Some(F::StoryArcNumber),
		decimal_text(&old.story_arc_number),
		decimal_text(&new.story_arc_number),
	);
	push("volume", None, int_text(&old.volume), int_text(&new.volume));
	push("summary", Some(F::Summary), text(&old.summary), text(&new.summary));
	push("notes", Some(F::Notes), text(&old.notes), text(&new.notes));
	push("genres", Some(F::Genres), text(&old.genres), text(&new.genres));
	push("format", Some(F::Format), text(&old.format), text(&new.format));
	push("year", Some(F::Year), int_text(&old.year), int_text(&new.year));
	push("month", Some(F::Year), int_text(&old.month), int_text(&new.month));
	push("day", Some(F::Year), int_text(&old.day), int_text(&new.day));
	push("pencillers", Some(F::Pencillers), text(&old.pencillers), text(&new.pencillers));
	push("inkers", Some(F::Inkers), text(&old.inkers), text(&new.inkers));
	push("colorists", Some(F::Colorists), text(&old.colorists), text(&new.colorists));
	push("letterers", Some(F::Letterers), text(&old.letterers), text(&new.letterers));
	push(
		"coverArtists",
		Some(F::CoverArtists),
		text(&old.cover_artists),
		text(&new.cover_artists),
	);
	push("editors", Some(F::Editors), text(&old.editors), text(&new.editors));
	push("publisher", Some(F::Publisher), text(&old.publisher), text(&new.publisher));
	push("links", Some(F::Links), text(&old.links), text(&new.links));
	push("characters", Some(F::Characters), text(&old.characters), text(&new.characters));
	push("teams", Some(F::Teams), text(&old.teams), text(&new.teams));
	push("pageCount", Some(F::PageCount), int_text(&old.page_count), int_text(&new.page_count));
	push("ageRating", Some(F::AgeRating), int_text(&old.age_rating), int_text(&new.age_rating));
	push(
		"identifierAmazon",
		Some(F::IdentifierAmazon),
		text(&old.identifier_amazon),
		text(&new.identifier_amazon),
	);
	push(
		"identifierCalibre",
		Some(F::IdentifierCalibre),
		text(&old.identifier_calibre),
		text(&new.identifier_calibre),
	);
	push(
		"identifierGoogle",
		Some(F::IdentifierGoogle),
		text(&old.identifier_google),
		text(&new.identifier_google),
	);
	push("identifierIsbn", Some(F::Isbn), text(&old.identifier_isbn), text(&new.identifier_isbn));
	push(
		"identifierMobiAsin",
		Some(F::IdentifierMobiAsin),
		text(&old.identifier_mobi_asin),
		text(&new.identifier_mobi_asin),
	);
	push(
		"identifierUuid",
		Some(F::IdentifierUuid),
		text(&old.identifier_uuid),
		text(&new.identifier_uuid),
	);
	push("language", Some(F::Language), text(&old.language), text(&new.language));
	out
}

/// What a format's file gets from the changed fields (the new values)
pub fn file_changes(
	format: EditableFormat,
	changed: &[ChangedField],
	new: &media_metadata::Model,
) -> FileChanges {
	let has = |field: &str| changed.iter().any(|c| c.field == field) && format.carries(field);
	let mut out = FileChanges::default();
	if has("title") {
		// a book keeps a title in its file: removing it stays a database change
		if let Some(title) = new.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
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
pub fn fields_written(changed: &[ChangedField], changes: &FileChanges) -> Vec<&'static str> {
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
		.map(|v| v.replace(',', " ").split_whitespace().collect::<Vec<_>>().join(" "))
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

	let block = Regex::new(r"(?s)(<(?:\w+:)?metadata\b[^>]*>)(.*?)(</(?:\w+:)?metadata>)")
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
	let epub3 = Regex::new(r#"version=["']3"#).expect("valid regex").is_match(&package);
	if !head.contains("xmlns:opf") && !package.contains("xmlns:opf") {
		head = format!("{} xmlns:opf=\"http://www.idpf.org/2007/opf\">", &head[..head.len() - 1]);
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
			let re = Regex::new(&format!(r#"\s*<meta\b[^>]*refines=["']#{id}["'][^>]*/>"#))
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
		let re = Regex::new(r"(?s)(<dc:title\b[^>]*>)(.*?)(</dc:title>)").expect("valid regex");
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
		remove(&mut body, r#"\s*<meta\b[^>]*name=["']bcurator:(?:estante|shelf)["'][^>]*/>"#);
		for tag in genres {
			add.push(format!("<dc:subject>{}</dc:subject>", esc(tag)));
		}
		let shelf = genres.iter().find(|t| !t.contains(':') && !is_facet(t));
		if let Some(shelf) = shelf {
			add.push(format!("<meta name=\"bcurator:shelf\" content=\"{}\"/>", esc(shelf)));
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
		remove(&mut body, r#"\s*<meta[^>]*name=["']calibre:series(?:_index)?["'][^>]*/>"#);
		if let Some(series) = series {
			add.push(format!("<meta name=\"calibre:series\" content=\"{}\"/>", esc(series)));
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
		remove(&mut body, r"(?s)\s*<dc:description\b[^>]*>.*?</dc:description>");
		remove(&mut body, r"\s*<dc:description\b[^>]*/>");
		if let Some(summary) = summary {
			add.push(format!("<dc:description>{}</dc:description>", esc(summary)));
		}
	}

	if let Some(date) = &changes.date {
		// the publication date: modification dates (opf:event) stay
		let re = Regex::new(r"(?s)\s*<dc:date\b([^>]*)>([^<]*)</dc:date>").expect("valid regex");
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
		remove(&mut body, r#"\s*<meta\b[^>]*name=["']typicalAgeRange["'][^>]*/>"#);
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
	let result = rewrite_zip(
		path,
		&tmp,
		&|name| name == opf_name,
		&mut |writer| {
			writer.start_file(
				opf_name.as_str(),
				SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
			)?;
			writer.write_all(new_opf.as_bytes())?;
			Ok(())
		},
	)
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
	let full = Regex::new(&format!(r"(?s)\s*<{tag}(?:\s[^>]*)?>.*?</{tag}>")).expect("valid regex");
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
		set_comic_element(&mut xml, "Writer", Some(&joined).filter(|w| !w.is_empty()).map(|w| w.as_str()));
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
			Some((y, m, d)) => (Some(y.to_string()), m.map(|m| m.to_string()), d.map(|d| d.to_string())),
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
		set_comic_element(&mut xml, "Genre", Some(joined.as_str()).filter(|g| !g.is_empty()));
	}
	if let Some(age) = &changes.age_rating {
		let value = age.map(|a| if a >= 18 { "Adults Only 18+" } else if a <= 0 { "Everyone" } else { "Teen" });
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
				SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
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
	value.replace(", ", " · ").replace(',', " ·").replace(';', " ·")
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
			Some((key, value)) => curator.push((key.trim().to_string(), value.trim().to_string())),
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
		set_curator(
			"series_index",
			series.as_ref().and(number.map(num)),
		);
	}
	if let Some(age) = &changes.age_rating {
		set_curator(
			"age",
			age.map(|a| if a >= 18 { "18+".to_string() } else { "All ages".to_string() }),
		);
	}
	let mut seen = BTreeSet::new();
	tags.into_iter()
		.chain(curator.into_iter().map(|(k, v)| format!("bcurator:{k}={v}")))
		.filter(|k| seen.insert(k.clone()))
		.collect::<Vec<_>>()
		.join(", ")
}

/// The PDF Info entries after the changes, keeping what did not change
pub fn pdf_info_after(existing: &PdfInfoValues, changes: &FileChanges) -> Vec<(String, String)> {
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
	/// The changed fields kept in Stump only (locked so rescans keep them)
	pub db_only_fields: Vec<&'static str>,
	pub locks: Vec<MetadataField>,
	/// Why the file was not written, or a note about it
	pub message: Option<String>,
	pub quarantine_path: Option<String>,
	pub csv_path: Option<String>,
}

/// What the file says after the write, for the fields that were written
fn verify_read_back(
	written: &crate::metadata::ProcessedMediaMetadata,
	changes: &FileChanges,
	format: EditableFormat,
) -> Result<(), String> {
	let mut wrong = vec![];
	let set = |v: &[String]| v.iter().map(|s| s.trim().to_lowercase()).collect::<BTreeSet<_>>();
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
		if written.publisher.as_deref().map(str::trim) != publisher.as_deref().map(str::trim) {
			wrong.push("publisher");
		}
	}
	if let Some(Some(language)) = &changes.language {
		if written.language.as_deref().map(str::trim) != Some(language.trim()) {
			wrong.push("language");
		}
	}
	if let Some(age) = &changes.age_rating {
		let expected = age.map(|a| if a >= 18 { 18 } else if a <= 0 { 0 } else { a });
		if format != EditableFormat::Cbz || age.is_none_or(|a| a >= 18 || a <= 0) {
			if written.age_rating.map(|a| a.max(0)) != expected.map(|a| a.max(0)) {
				wrong.push("age rating");
			}
		}
	}
	if wrong.is_empty() {
		Ok(())
	} else {
		Err(format!("the file read back without the new {}", wrong.join(", ")))
	}
}

/// The hidden copy of a book next to it that receives the edit (scans skip hidden files)
fn work_path(path: &Path) -> PathBuf {
	let name = path
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_else(|| "book".into());
	path.with_file_name(format!(".{name}.stump-edit"))
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
		if let Ok(meta) = std::fs::metadata(&src) {
			let _ = std::fs::set_permissions(&dst, meta.permissions());
		}
		match format {
			EditableFormat::Epub => write_epub_changes(&dst, &changes_owned),
			EditableFormat::Cbz => write_cbz_changes(&dst, &changes_owned),
			EditableFormat::Pdf => append_pdf_info(
				&dst,
				&pdf_info_after(&existing_pdf.unwrap_or_default(), &changes_owned),
			),
		}
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

/// Puts the edited copy in the book's place: the original goes to the quarantine folder
/// first when one is set (never deleted); else the copy just replaces it
fn replace_book(
	path: &Path,
	work: &Path,
	quarantine: Option<(PathBuf, PathBuf)>,
) -> std::io::Result<Option<PathBuf>> {
	match quarantine {
		Some((batch, rel)) => {
			let out = swap::quarantine_then_rename(path, work, &batch, &rel)?;
			Ok(Some(out.quarantine_path))
		},
		None => {
			std::fs::rename(work, path)?;
			Ok(None)
		},
	}
}

/// Writes an edit into the book file and logs it: the stored metadata was `old`, the user
/// saved `new`. Returns which changed fields the file now carries and which stay in Stump
/// only (to lock).
pub async fn write_edit(
	ctx: &Ctx,
	book: &media::Model,
	library: &library::Model,
	library_config: &library_config::Model,
	old: &media_metadata::Model,
	new: &media_metadata::Model,
	username: &str,
) -> FileEditOutcome {
	let changed = changed_fields(old, new);
	let path = PathBuf::from(&book.path);
	let format = EditableFormat::of(&path);

	let mut outcome = FileEditOutcome {
		status: FileWriteStatus::NotNeeded,
		file_fields: vec![],
		db_only_fields: vec![],
		locks: vec![],
		message: None,
		quarantine_path: None,
		csv_path: None,
	};
	let changes = format
		.map(|f| file_changes(f, &changed, new))
		.unwrap_or_default();
	let mut written: Vec<&'static str> = vec![];
	if let (Some(format), false) = (format, changes.is_empty()) {
		match prepare_edited_copy(ctx, &path, format, &changes).await {
			Ok(work) => {
				let quarantine = ctx.config.quarantine_directory().map(|root| {
					(
						swap::batch_dir(&root, "edit", chrono::Utc::now()),
						swap::quarantine_rel(Path::new(&library.path), &path),
					)
				});
				let (p, w) = (path.clone(), work.clone());
				let replaced =
					tokio::task::spawn_blocking(move || replace_book(&p, &w, quarantine)).await;
				match replaced {
					Ok(Ok(quarantined)) => {
						outcome.status = FileWriteStatus::Written;
						outcome.quarantine_path =
							quarantined.map(|q| q.to_string_lossy().into_owned());
						written = fields_written(&changed, &changes);
						if let Err(error) =
							refresh_media_after_swap(ctx.conn.as_ref(), &ctx.config, library_config, book)
								.await
						{
							tracing::warn!(?error, path = %book.path, "Could not refresh an edited book");
						}
					},
					Ok(Err(error)) => {
						let _ = tokio::fs::remove_file(&work).await;
						outcome.status = FileWriteStatus::Failed;
						outcome.message = Some(format!("The book file could not be replaced: {error}"));
					},
					Err(error) => {
						let _ = tokio::fs::remove_file(&work).await;
						outcome.status = FileWriteStatus::Failed;
						outcome.message = Some(error.to_string());
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
			"This kind of file can't carry metadata: the changes are kept in Stump".to_string(),
		);
	}

	for change in &changed {
		if written.contains(&change.field) {
			outcome.file_fields.push(change.field);
		} else {
			outcome.db_only_fields.push(change.field);
			if let Some(lock) = change.lock {
				if !outcome.locks.contains(&lock) {
					outcome.locks.push(lock);
				}
			}
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
			let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
			let rows = changed
				.iter()
				.map(|c| {
					vec![
						rel.clone(),
						c.field.to_string(),
						c.old.clone(),
						c.new.clone(),
						if written.contains(&c.field) { "file" } else { "stump" }.to_string(),
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
		language: meta.language.as_deref().and_then(super::naming::language_code),
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
		.ok_or_else(|| CoreError::NotFound(format!("Library {} not found", book.library_id)))?;
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
		assert!(file_changes(EditableFormat::Epub, &changed, &new).language.is_some());
		assert!(file_changes(EditableFormat::Pdf, &changed, &new).language.is_none());
		assert!(file_changes(EditableFormat::Cbz, &changed, &new).language.is_none());
		assert_eq!(EditableFormat::of(Path::new("/a/b.kepub.epub")), Some(EditableFormat::Epub));
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
			clean_writers(&["Pantano, Robert".into(), "Robert Pantano".into(), " ".into()]),
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
		assert!(new.contains(r#"<meta name="bcurator:shelf" content="Literary Fiction"/>"#));
		// untouched: title and its refines, creator, dates, language, identifier
		assert!(new.contains(r##"<meta refines="#t1" property="title-type">main</meta>"##));
		assert!(new.contains(r##"<meta refines="#c1" property="file-as">Pantano, Robert</meta>"##));
		assert!(new.contains("<dc:date>2020-07-01</dc:date>"));
		assert!(new.contains("<dc:identifier id=\"id\">urn:isbn:9798667150664</dc:identifier>"));

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
		assert!(new.contains(r##"<meta refines="#bcurator-series" property="group-position">2</meta>"##));
		assert!(new.contains("<dc:date>2021-03</dc:date>"));
		assert!(new.contains(r#"<dc:date opf:event="modification">2024-01-01</dc:date>"#));
		assert!(!new.contains("2020-07-01"));
		assert!(new.contains("<dc:description>A &lt;b&gt;bold&lt;/b&gt; novel</dc:description>"));
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
			genres: Some(vec!["Manga & Comics".into(), "Manga & Comics: Slice of Life".into()]),
			summary: Some(Some("Everyday adventures".into())),
			..Default::default()
		};
		let xml = apply_comic_info_changes(Some(existing), &changes);
		check_xml(&xml).unwrap();
		assert!(xml.contains("<Genre>Manga &amp; Comics, Manga &amp; Comics: Slice of Life</Genre>"));
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
		assert_eq!(keys, vec!["Title", "Author", "Keywords", "Creator", "ModDate"]);
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
		zip.start_file("META-INF/container.xml", SimpleFileOptions::default()).unwrap();
		zip.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
		zip.start_file("OEBPS/content.opf", SimpleFileOptions::default()).unwrap();
		zip.write_all(OPF3.as_bytes()).unwrap();
		zip.start_file("OEBPS/chapter.xhtml", SimpleFileOptions::default()).unwrap();
		zip.write_all(b"<html><body><p>Hello</p></body></html>").unwrap();
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
		archive.by_name("OEBPS/content.opf").unwrap().read_to_string(&mut opf).unwrap();
		assert!(opf.contains("<dc:subject>Philosophy</dc:subject>"));
		assert!(!opf.contains("Physics"));
		let mut chapter = String::new();
		archive.by_name("OEBPS/chapter.xhtml").unwrap().read_to_string(&mut chapter).unwrap();
		assert_eq!(chapter, "<html><body><p>Hello</p></body></html>");
		// the intake's writer and this one agree on what a curated file looks like
		let again = dir.path().join("again.epub");
		std::fs::copy(&epub, &again).unwrap();
		write_epub(&again, &FileMetadata { tags: vec!["Philosophy".into()], ..Default::default() }).unwrap();

		let cbz = dir.path().join("v01.cbz");
		let mut zip = zip::ZipWriter::new(File::create(&cbz).unwrap());
		zip.start_file("001.png", SimpleFileOptions::default()).unwrap();
		zip.write_all(b"png").unwrap();
		zip.finish().unwrap();
		write_cbz_changes(&cbz, &changes).unwrap();
		let mut archive = ZipArchive::new(File::open(&cbz).unwrap()).unwrap();
		assert_eq!(archive.len(), 2);
		let mut info = String::new();
		archive.by_name("ComicInfo.xml").unwrap().read_to_string(&mut info).unwrap();
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
		archive.by_name("ComicInfo.xml").unwrap().read_to_string(&mut info).unwrap();
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
}
