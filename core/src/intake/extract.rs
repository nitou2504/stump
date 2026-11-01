//! Reads what an uploaded file says about itself: its embedded metadata (through Stump's
//! own readers, so filing and scanning agree), its hash and its file name.

use std::path::Path;

use models::shared::{
	enums::OrganizationMode,
	intake::{IntakeMetadata, IntakeSource},
};

use super::naming::{
	display_name, language_code, name_keys, parse_file_name, split_authors, ParsedFileName,
};
use crate::{
	config::StumpConfig,
	media::processor::{generate_hashes, process_metadata, MediaProcessorOptions},
	metadata::media::ProcessedMediaMetadata,
};

/// What one file says about itself
#[derive(Debug, Clone, Default)]
pub struct ExtractedFile {
	pub embedded: Option<ProcessedMediaMetadata>,
	pub parsed_name: ParsedFileName,
	pub hash: Option<String>,
}

/// Reads a staged file. `original_name` is the name it was uploaded with (the staged copy
/// has a safe, folded name that would lose `Yotsuba&!` or add a position prefix).
pub async fn extract_file(path: &Path, original_name: &str, config: &StumpConfig) -> ExtractedFile {
	let stem = Path::new(original_name)
		.file_stem()
		.map(|s| s.to_string_lossy().into_owned())
		.unwrap_or_default();
	let embedded = match process_metadata(path, OrganizationMode::Metadata, config).await {
		Ok(meta) => meta,
		Err(error) => {
			tracing::debug!(?error, ?path, "Could not read embedded metadata");
			None
		},
	};
	let options = MediaProcessorOptions {
		generate_file_hashes: true,
		organization_mode: OrganizationMode::Metadata,
		cache_directory: config.cache_directory(),
		..Default::default()
	};
	let hash = generate_hashes(path, options, config)
		.await
		.ok()
		.and_then(|h| h.stump);
	ExtractedFile {
		embedded,
		parsed_name: parse_file_name(&stem),
		hash,
	}
}

fn clean(value: &Option<String>) -> Option<String> {
	value
		.as_deref()
		.map(str::trim)
		.filter(|v| !v.is_empty())
		.map(str::to_string)
}

/// Titles that are really file or tool names, never a book's title
fn junk_title(title: &str) -> bool {
	let lower = title.to_lowercase();
	lower.is_empty()
		|| lower.ends_with(".pdf")
		|| lower.ends_with(".epub")
		|| lower.ends_with(".doc")
		|| lower.ends_with(".docx")
		|| lower == "untitled"
		|| lower.starts_with("microsoft word")
		|| lower.contains("(digital)")
}

/// The metadata of one file: its embedded values first, then its file name
pub fn metadata_of(file: &ExtractedFile) -> IntakeMetadata {
	let mut meta = IntakeMetadata::default();
	if let Some(e) = &file.embedded {
		if let Some(title) = clean(&e.title).filter(|t| !junk_title(t)) {
			meta.title = Some(title);
			meta.set_source("title", IntakeSource::File);
		}
		let writers: Vec<String> = e
			.writers
			.clone()
			.unwrap_or_default()
			.into_iter()
			.flat_map(|w| split_authors(&w))
			.filter(|w| !w.trim().is_empty())
			// "Eco, Umberto" -> "Umberto Eco": Stump splits writers on commas
			.map(|w| display_name(&w))
			.collect();
		if !writers.is_empty() {
			meta.authors = writers;
			meta.set_source("authors", IntakeSource::File);
		}
		if let Some(series) = clean(&e.series) {
			meta.series = Some(series);
			meta.set_source("series", IntakeSource::File);
			if let Some(number) = e.number {
				meta.series_index = Some(number);
				meta.set_source("seriesIndex", IntakeSource::File);
			}
		}
		if let Some(language) = e.language.as_deref().and_then(language_code) {
			meta.language = Some(language);
			meta.set_source("language", IntakeSource::File);
		}
		if let Some(year) = e.year.filter(|y| (1000..=2100).contains(y)) {
			meta.year = Some(year);
			meta.set_source("year", IntakeSource::File);
		}
		if let Some(publisher) = clean(&e.publisher) {
			meta.publisher = Some(publisher);
			meta.set_source("publisher", IntakeSource::File);
		}
		if let Some(summary) = clean(&e.summary) {
			meta.description = Some(summary);
			meta.set_source("description", IntakeSource::File);
		}
		if let Some(isbn) = clean(&e.identifier_isbn) {
			meta.isbn = Some(isbn);
			meta.set_source("isbn", IntakeSource::File);
		}
		let mut subjects: Vec<String> = e.genres.clone().unwrap_or_default();
		subjects.extend(e.tags.clone().unwrap_or_default());
		subjects.retain(|s| !s.trim().is_empty() && !s.starts_with("bcurator:"));
		if !subjects.is_empty() {
			meta.subjects = subjects;
			meta.set_source("subjects", IntakeSource::File);
		}
		meta.age_rating = e.age_rating;
		meta.rating = e.rating;
	}

	if let Some(title) = meta.title.clone() {
		let (cleaned, year) = clean_title(&title, &meta.authors);
		if cleaned != title {
			meta.title = Some(cleaned);
		}
		if meta.year.is_none() {
			if let Some(year) = year {
				meta.year = Some(year);
				meta.set_source("year", IntakeSource::File);
			}
		}
	}

	let parsed = &file.parsed_name;
	if meta.title.is_none() {
		if let Some(title) = &parsed.title {
			meta.title = Some(title.clone());
			meta.set_source("title", IntakeSource::FileName);
		}
	}
	if meta.authors.is_empty() && !parsed.authors.is_empty() {
		meta.authors = parsed.authors.iter().map(|a| display_name(a)).collect();
		meta.set_source("authors", IntakeSource::FileName);
	}
	if meta.series.is_none() {
		if let Some(series) = &parsed.series {
			meta.series = Some(series.clone());
			meta.set_source("series", IntakeSource::FileName);
		}
	}
	if meta.series_index.is_none() && meta.series.is_some() {
		if let Some(volume) = parsed.volume {
			meta.series_index = Some(volume);
			meta.set_source("seriesIndex", IntakeSource::FileName);
		}
	}
	meta
}

/// Titles carried over from file names: `László Polgár - Bring Up Genius! (1989)` ->
/// (`Bring Up Genius!`, 1989) when the part before the dash is one of the authors
pub fn clean_title(title: &str, authors: &[String]) -> (String, Option<i32>) {
	let mut title = title.trim().to_string();
	if let Some((left, right)) = title.split_once(" - ") {
		let left_keys = name_keys(left);
		if !right.trim().is_empty()
			&& authors.iter().any(|a| {
				let keys = name_keys(a);
				keys.ordered == left_keys.ordered || keys.sorted == left_keys.sorted
			}) {
			title = right.trim().to_string();
		}
	}
	let mut year = None;
	if title.ends_with(')') {
		if let Some(start) = title.rfind('(') {
			let inside = &title[start + 1..title.len() - 1];
			if inside.len() == 4 && inside.chars().all(|c| c.is_ascii_digit()) {
				year = inside.parse().ok().filter(|y| (1000..=2100).contains(y));
				if year.is_some() {
					title = title[..start].trim().to_string();
				}
			}
		}
	}
	(title, year)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn cleans_titles_that_carry_the_author_and_year() {
		let authors = vec!["László Polgár".to_string()];
		assert_eq!(
			clean_title("László Polgár - Bring Up Genius! (Nevelj zsenit!) (1989)", &authors),
			("Bring Up Genius! (Nevelj zsenit!)".to_string(), Some(1989))
		);
		// a dash that is part of the title stays
		assert_eq!(
			clean_title("Cosmos - ensayo de una descripción física del mundo", &authors).0,
			"Cosmos - ensayo de una descripción física del mundo"
		);
	}

	#[test]
	fn embedded_values_win_and_the_file_name_fills_gaps() {
		let file = ExtractedFile {
			embedded: Some(ProcessedMediaMetadata {
				title: Some("El nombre de la rosa".into()),
				writers: Some(vec!["Umberto Eco".into()]),
				language: Some("spa".into()),
				genres: Some(vec!["Novela".into(), "bcurator:series=X".into()]),
				year: Some(1980),
				..Default::default()
			}),
			parsed_name: parse_file_name("Eco, Umberto - Otra cosa v02"),
			hash: None,
		};
		let meta = metadata_of(&file);
		assert_eq!(meta.title.as_deref(), Some("El nombre de la rosa"));
		assert_eq!(meta.source_of("title"), Some(IntakeSource::File));
		assert_eq!(meta.authors, vec!["Umberto Eco"]);
		assert_eq!(meta.language.as_deref(), Some("es"));
		assert_eq!(meta.subjects, vec!["Novela"]);
		assert_eq!(meta.year, Some(1980));

		let file = ExtractedFile {
			embedded: Some(ProcessedMediaMetadata {
				title: Some("Microsoft Word - draft.doc".into()),
				..Default::default()
			}),
			parsed_name: parse_file_name("Umberto Eco - El pendulo de Foucault"),
			hash: None,
		};
		let meta = metadata_of(&file);
		assert_eq!(meta.title.as_deref(), Some("El pendulo de Foucault"));
		assert_eq!(meta.source_of("title"), Some(IntakeSource::FileName));
		assert_eq!(meta.authors, vec!["Umberto Eco"]);
		assert_eq!(meta.source_of("authors"), Some(IntakeSource::FileName));

		let file = ExtractedFile {
			embedded: Some(ProcessedMediaMetadata {
				writers: Some(vec!["Eco, Umberto".into()]),
				..Default::default()
			}),
			parsed_name: parse_file_name("x"),
			hash: None,
		};
		assert_eq!(metadata_of(&file).authors, vec!["Umberto Eco"]);
	}
}
