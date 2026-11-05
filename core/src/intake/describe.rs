//! Descriptions for books already in the library, from the online lookups (Hardcover, Open
//! Library, Google Books): *Find description* in *Edit details*, and `findMediaDescriptions`
//! for scripts. Nothing is saved here: a found text goes into the editor's summary (or a
//! script's `editMediaMetadataBatch`), whose save writes the file, the edit history and the
//! lock.

use std::collections::HashSet;

use metadata_integrations::{MetadataField, RequestPriority};
use models::{
	entity::{media, media_metadata},
	shared::intake::{IntakeSource, OnlineLookup, OnlineLookupStatus},
};
use sea_orm::DatabaseConnection;

use super::{
	edit::book_lookup_query,
	lookup::{self, strip_html, LookupOutcome},
};
use crate::metadata::locked::locked_fields_of;

/// Under this many words a description is thin (`Provides information on building Web
/// applications.`): offered after fuller ones
pub const THIN_DESCRIPTION_WORDS: usize = 20;

/// A summary with fewer words than this is missing, as *Add books* treats a file's
pub const MISSING_SUMMARY_WORDS: usize = 15;

/// A description an online source has for a book
#[derive(Debug, Clone, PartialEq)]
pub struct DescriptionCandidate {
	pub source: IntakeSource,
	pub text: String,
	pub words: usize,
}

/// The descriptions found for a book and how each source answered
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FoundDescriptions {
	/// Best first: Hardcover's, Open Library's, then Google Books' (thin ones last)
	pub candidates: Vec<DescriptionCandidate>,
	/// How each source answered, said for descriptions (`found a description`, `found, but
	/// it has no description for this book`, `no matching book`, a rate limit...)
	pub lookups: Vec<OnlineLookup>,
	/// What was searched, when it came from the book's title page because its title or
	/// authors looked like junk
	pub searched_title: Option<String>,
	pub searched_authors: Vec<String>,
}

impl FoundDescriptions {
	/// Whether a source couldn't answer (a rate limit, an error): asking again later may
	/// find more
	pub fn incomplete(&self) -> bool {
		self.lookups.iter().any(|l| {
			matches!(
				l.status,
				OnlineLookupStatus::RateLimited | OnlineLookupStatus::Error
			)
		})
	}
}

/// Hardcover's descriptions first (its readers' and publishers' blurbs matched the publisher's
/// best in tests), then Open Library's, then Google Books'
fn source_rank(source: IntakeSource) -> u8 {
	match source {
		IntakeSource::Hardcover => 0,
		IntakeSource::OpenLibrary => 1,
		IntakeSource::GoogleBooks => 2,
		_ => 3,
	}
}

/// The descriptions a lookup found, best first (one per text), and its sources' answers said
/// for descriptions
pub fn description_candidates(outcome: &LookupOutcome) -> FoundDescriptions {
	let mut candidates: Vec<DescriptionCandidate> = outcome
		.records
		.iter()
		.filter_map(|record| {
			let text = strip_html(record.description.as_deref()?);
			let words = text.split_whitespace().count();
			(words > 0).then(|| DescriptionCandidate {
				source: record.source.unwrap_or(IntakeSource::OpenLibrary),
				text,
				words,
			})
		})
		.collect();
	// what each source had, before the texts another source also had are left out
	let words_by_source: Vec<(IntakeSource, usize)> =
		candidates.iter().map(|c| (c.source, c.words)).collect();
	candidates.sort_by_key(|c| (c.words < THIN_DESCRIPTION_WORDS, source_rank(c.source)));
	// a source that mirrors another's text adds nothing
	let mut seen = HashSet::new();
	candidates.retain(|c| seen.insert(c.text.to_lowercase()));

	let lookups = outcome
		.sources
		.iter()
		.map(|lookup| {
			let found = matches!(
				lookup.status,
				OnlineLookupStatus::Ok | OnlineLookupStatus::NoSubjects
			);
			if !found {
				return lookup.clone();
			}
			let message = match words_by_source.iter().find(|(s, _)| *s == lookup.source)
			{
				Some((_, words)) => format!(
					"found a description ({words} {})",
					if *words == 1 { "word" } else { "words" }
				),
				None => "found, but it has no description for this book".to_string(),
			};
			OnlineLookup {
				status: OnlineLookupStatus::Ok,
				message,
				..lookup.clone()
			}
		})
		.collect();
	FoundDescriptions {
		candidates,
		lookups,
		..Default::default()
	}
}

/// Looks a book up online for descriptions: for someone waiting (`Interactive`, *Edit
/// details*) or for a script going through many books (`Background`: Hardcover keeps it to its
/// daily budget, behind people's lookups)
pub async fn find_descriptions(
	conn: &DatabaseConnection,
	book: &media::Model,
	meta: &media_metadata::Model,
	priority: RequestPriority,
) -> FoundDescriptions {
	let (query, from_title_page) = book_lookup_query(book, meta).await;
	let outcome = lookup::lookup_for(conn, &query, priority).await;
	let mut found = description_candidates(&outcome);
	if from_title_page {
		found.searched_title = query.title.clone();
		found.searched_authors = query.authors.clone();
	}
	found
}

/// Whether a book's summary is locked: someone set (or cleared) it by hand
pub fn summary_locked(meta: &media_metadata::Model) -> bool {
	locked_fields_of(&meta.locked_fields).iter().any(|f| {
		matches!(
			f,
			MetadataField::Summary | MetadataField::DescriptionFormatted
		)
	})
}

/// Whether a book has no summary to speak of (none, or under [`MISSING_SUMMARY_WORDS`])
pub fn summary_missing(meta: &media_metadata::Model) -> bool {
	meta.summary
		.as_deref()
		.map(|s| strip_html(s).split_whitespace().count())
		.unwrap_or(0)
		< MISSING_SUMMARY_WORDS
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::intake::lookup::OnlineRecord;

	fn record(source: IntakeSource, description: Option<&str>) -> OnlineRecord {
		OnlineRecord {
			source: Some(source),
			description: description.map(str::to_string),
			..Default::default()
		}
	}

	fn lookup(
		source: IntakeSource,
		status: OnlineLookupStatus,
		message: &str,
	) -> OnlineLookup {
		OnlineLookup {
			source,
			status,
			subjects: 0,
			message: message.to_string(),
		}
	}

	const LONG: &str = "In this book the author tells how software is made, why most of it fails, and what the people who write it can learn from architecture, poetry and the history of a small company that tried to sell Lisp.";

	#[test]
	fn hardcover_first_thin_ones_last_one_per_text() {
		let outcome = LookupOutcome {
			records: vec![
				record(IntakeSource::OpenLibrary, Some(LONG)),
				record(
					IntakeSource::GoogleBooks,
					Some("Provides information on software."),
				),
				record(IntakeSource::Hardcover, Some(&format!("<p>{LONG}</p>"))),
			],
			sources: vec![
				lookup(
					IntakeSource::OpenLibrary,
					OnlineLookupStatus::NoSubjects,
					"found, but it has no subjects for this book",
				),
				lookup(
					IntakeSource::GoogleBooks,
					OnlineLookupStatus::Ok,
					"found, 2 subjects",
				),
				lookup(
					IntakeSource::Hardcover,
					OnlineLookupStatus::Ok,
					"found, 8 subjects",
				),
			],
		};
		let found = description_candidates(&outcome);
		// Open Library's text is Hardcover's: kept once, as Hardcover's
		let sources: Vec<IntakeSource> =
			found.candidates.iter().map(|c| c.source).collect();
		assert_eq!(
			sources,
			vec![IntakeSource::Hardcover, IntakeSource::GoogleBooks]
		);
		assert_eq!(found.candidates[0].text, LONG);
		assert_eq!(found.candidates[1].words, 4);
		let messages: Vec<&str> =
			found.lookups.iter().map(|l| l.message.as_str()).collect();
		assert_eq!(
			messages,
			vec![
				"found a description (39 words)",
				"found a description (4 words)",
				"found a description (39 words)",
			]
		);
		assert!(found
			.lookups
			.iter()
			.all(|l| l.status == OnlineLookupStatus::Ok));
		assert!(!found.incomplete());
	}

	#[test]
	fn failed_sources_keep_their_answer_and_make_it_incomplete() {
		let outcome = LookupOutcome {
			records: vec![],
			sources: vec![
				lookup(IntakeSource::OpenLibrary, OnlineLookupStatus::NoMatch, "no matching book"),
				lookup(
					IntakeSource::GoogleBooks,
					OnlineLookupStatus::RateLimited,
					"daily limit reached: add a Google Books API key in Settings → Metadata",
				),
				lookup(IntakeSource::Hardcover, OnlineLookupStatus::NotConfigured, "not configured"),
			],
		};
		let found = description_candidates(&outcome);
		assert!(found.candidates.is_empty());
		assert_eq!(found.lookups, outcome.sources);
		assert!(found.incomplete());
	}

	#[test]
	fn tells_missing_and_locked_summaries() {
		let mut meta = media_metadata::Model::default();
		assert!(summary_missing(&meta));
		meta.summary = Some("A short note.".to_string());
		assert!(summary_missing(&meta));
		meta.summary = Some(LONG.to_string());
		assert!(!summary_missing(&meta));
		assert!(!summary_locked(&meta));
		meta.locked_fields = Some(serde_json::json!(["GENRES", "SUMMARY"]));
		assert!(summary_locked(&meta));
	}
}
