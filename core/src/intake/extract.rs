//! Reads what an uploaded file says about itself: its embedded metadata (through Stump's
//! own readers, so filing and scanning agree), its hash and its file name.

use std::path::Path;

use models::shared::{
	enums::OrganizationMode,
	intake::{IntakeMetadata, IntakeSource},
};

use super::naming::{language_code, parse_file_name, split_authors, ParsedFileName};
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

pub async fn extract_file(path: &Path, config: &StumpConfig) -> ExtractedFile {
	let stem = path
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

	let parsed = &file.parsed_name;
	if meta.title.is_none() {
		if let Some(title) = &parsed.title {
			meta.title = Some(title.clone());
			meta.set_source("title", IntakeSource::FileName);
		}
	}
	if meta.authors.is_empty() && !parsed.authors.is_empty() {
		meta.authors = parsed.authors.clone();
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

#[cfg(test)]
mod tests {
	use super::*;

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
	}
}
