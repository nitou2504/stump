//! What an item looks like to the person looking at it.
//!
//! The analysis of an item sees the whole library: the author folders on disk, the tags of every
//! book, similar books and duplicates by title or by file. That is what an approver needs, and
//! what somebody who needs approval must not get a word of when it comes from books they can't
//! see (a library hidden from them, an age restriction, a book removed from the library): a title
//! in "similar books" is a title. So nothing is taken out of the stored item; each person gets
//! the item **through** their own limits, at the moment they ask.
//!
//! - Everything the analysis took from the whole library (the kind of series, the suggested
//!   tags and how sure they are, the similar books, the warnings, the confidence) is replaced by
//!   what the analysis found over the books the uploader can read alone, its **uploader view**
//!   (`IntakeProposal::uploader_view`). An item without one is shown without any of it: the
//!   confidence of a folder that matches, or of a tag that books vote for, says what the library
//!   holds, hidden books included.
//! - `similarBooks` and `duplicates` keep only the books the viewer can read (and a duplicate
//!   carries no path). The warning that a book may already be in the library comes back only
//!   while a duplicate they can read is left to explain it.
//! - Folder candidates, the destination, the folder and the file names are for approvers: they
//!   are the library's own layout, and a name that exists or doesn't is an answer about the
//!   library.
//! - Suggested tags are kept when a book the viewer can read carries them (or the viewer's own
//!   file did), with that count and no reasons: "3 other books by this author have it" is about
//!   books.
//! - What stops an item from being filed is shown without what only the library knows (that a
//!   file with the name already exists).

use std::{
	collections::{HashMap, HashSet},
	sync::{Arc, LazyLock, Mutex},
	time::{Duration, Instant},
};

use models::{
	entity::{media, media_metadata, user::AuthUser},
	shared::intake::{IntakeDecision, IntakeDestination, IntakeProposal},
};
use sea_orm::{prelude::*, QuerySelect};

use super::library_index::{is_facet, norm};
use crate::CoreResult;

/// The warning an analysis adds when the library may already hold the book
pub const DUPLICATE_WARNING: &str = "This book may already be in the library";

/// What only the library can say about a filing: not for somebody who may not file
pub const EXISTS_PROBLEM: &str = "A file with that name is already in the library";

/// Why a book that was approved wasn't filed: a copy of one of its files was added to the library
/// after the book was looked at. Only an approver is told (to a friend it would say that the
/// library holds the file, books they can't read included)
pub const ADDED_SINCE: &str =
	"A copy of this book was added to the library after it was looked at: approve it again to file it as well";

/// How long the tags a viewer can read are remembered
const VOCABULARY_TTL: Duration = Duration::from_secs(60);

type Vocabulary = Arc<HashMap<String, i32>>;

static VOCABULARY: LazyLock<Mutex<HashMap<(String, String), (Instant, Vocabulary)>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

/// Which of these books the viewer can read: their library is not hidden from them, the book is
/// within their age restriction, and it wasn't removed from the library
pub async fn visible_media_ids(
	conn: &DatabaseConnection,
	viewer: &AuthUser,
	ids: &[String],
) -> CoreResult<HashSet<String>> {
	if ids.is_empty() {
		return Ok(HashSet::new());
	}
	Ok(media::Entity::find_for_user(viewer)
		.select_only()
		.column(media::Column::Id)
		.filter(media::Column::Id.is_in(ids.to_vec()))
		.into_tuple::<String>()
		.all(conn)
		.await?
		.into_iter()
		.collect())
}

fn split_tags(genres: &str) -> impl Iterator<Item = String> + '_ {
	genres
		.split(',')
		.map(str::trim)
		.filter(|tag| !tag.is_empty())
		.map(str::to_string)
}

/// The tags of the books of a library the viewer can read, with how many carry each
pub async fn viewer_vocabulary(
	conn: &DatabaseConnection,
	viewer: &AuthUser,
	library_id: &str,
) -> CoreResult<Vocabulary> {
	let key = (viewer.id.clone(), library_id.to_string());
	if let Some((at, vocabulary)) =
		VOCABULARY.lock().ok().and_then(|c| c.get(&key).cloned())
	{
		if at.elapsed() < VOCABULARY_TTL {
			return Ok(vocabulary);
		}
	}
	let rows = media::Entity::find_for_user(viewer)
		.filter(media::Column::LibraryId.eq(library_id))
		.select_only()
		.column(media_metadata::Column::Genres)
		.into_tuple::<Option<String>>()
		.all(conn)
		.await?;
	let mut counts: HashMap<String, i32> = HashMap::new();
	for genres in rows.into_iter().flatten() {
		for tag in split_tags(&genres) {
			*counts.entry(tag).or_default() += 1;
		}
	}
	let vocabulary = Arc::new(counts);
	if let Ok(mut cache) = VOCABULARY.lock() {
		cache.retain(|_, (at, _)| at.elapsed() < VOCABULARY_TTL);
		cache.insert(key, (Instant::now(), vocabulary.clone()));
	}
	Ok(vocabulary)
}

/// Forgets what was remembered about the tags a viewer can read (tests; and a filing, which adds books)
pub fn forget_vocabularies() {
	if let Ok(mut cache) = VOCABULARY.lock() {
		cache.clear();
	}
}

/// A tag-picker entry for somebody who may not see the whole library: the tags of the books they
/// can read, most used first, with their counts
pub async fn viewer_tags(
	conn: &DatabaseConnection,
	viewer: &AuthUser,
	library_id: &str,
) -> CoreResult<Vec<(String, i32, bool)>> {
	let vocabulary = viewer_vocabulary(conn, viewer, library_id).await?;
	let mut tags: Vec<(String, i32, bool)> = vocabulary
		.iter()
		.map(|(tag, count)| (tag.clone(), *count, is_facet(tag)))
		.collect();
	tags.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
	Ok(tags)
}

/// The proposal as somebody who may not file books themselves gets it
pub async fn proposal_for_uploader(
	conn: &DatabaseConnection,
	viewer: &AuthUser,
	library_id: &str,
	mut proposal: IntakeProposal,
) -> CoreResult<IntakeProposal> {
	match proposal.uploader_view.take() {
		Some(view) => {
			proposal.series_kind = view.series_kind;
			proposal.genre_suggestions = view.genre_suggestions;
			proposal.similar_books = view.similar_books;
			proposal.warnings = view.warnings;
			proposal.confidence = view.confidence;
		},
		None => {
			// no view of their own (it was analysed for somebody who files books themselves):
			// nothing that came from the library
			proposal.series_kind = None;
			proposal.genre_suggestions.clear();
			proposal.similar_books.clear();
			proposal.warnings.clear();
			proposal.confidence = 0.0;
		},
	}
	let mut ids: Vec<String> = proposal
		.similar_books
		.iter()
		.map(|b| b.media_id.clone())
		.chain(proposal.duplicates.iter().map(|d| d.media_id.clone()))
		.collect();
	ids.sort();
	ids.dedup();
	let visible = visible_media_ids(conn, viewer, &ids).await?;

	proposal
		.similar_books
		.retain(|b| visible.contains(&b.media_id));
	proposal
		.duplicates
		.retain(|d| visible.contains(&d.media_id));
	for duplicate in &mut proposal.duplicates {
		// a path is the library's layout
		duplicate.path = String::new();
	}
	proposal.warnings.retain(|w| w != DUPLICATE_WARNING);
	if !proposal.duplicates.is_empty() {
		proposal.warnings.push(DUPLICATE_WARNING.to_string());
	}

	// the folders are the library's own layout
	proposal.author_folders.clear();
	proposal.series_folders.clear();

	if !proposal.genre_suggestions.is_empty() {
		let vocabulary = viewer_vocabulary(conn, viewer, library_id).await?;
		let by_norm: HashMap<String, i32> = vocabulary
			.iter()
			.map(|(tag, count)| (norm(tag), *count))
			.collect();
		let own: HashSet<String> =
			proposal.metadata.subjects.iter().map(|s| norm(s)).collect();
		proposal.genre_suggestions.retain_mut(|suggestion| {
			let key = norm(&suggestion.value);
			let count = by_norm.get(&key).copied();
			suggestion.library_count = count.unwrap_or(0);
			suggestion.reasons.clear();
			count.is_some() || own.contains(&key)
		});
	}
	Ok(proposal)
}

/// The decision as somebody who may not file books themselves gets it: where it goes is not
/// theirs, and tags only the library's hidden books carry are not shown
pub async fn decision_for_uploader(
	conn: &DatabaseConnection,
	viewer: &AuthUser,
	library_id: &str,
	mut decision: IntakeDecision,
	subjects: &[String],
) -> CoreResult<IntakeDecision> {
	decision.folder = String::new();
	decision.file_names = vec![];
	if !decision.metadata.genres.is_empty() {
		let vocabulary = viewer_vocabulary(conn, viewer, library_id).await?;
		let known: HashSet<String> = vocabulary.keys().map(|tag| norm(tag)).collect();
		let own: HashSet<String> = subjects.iter().map(|s| norm(s)).collect();
		decision.metadata.genres.retain(|tag| {
			let key = norm(tag);
			known.contains(&key) || own.contains(&key)
		});
	}
	Ok(decision)
}

/// The destination of an item is for approvers
pub fn destination_for_uploader(
	_destination: IntakeDestination,
) -> Option<IntakeDestination> {
	None
}

/// What keeps an item from being filed, as somebody who may not file books themselves reads it:
/// what they can fix. Whether a file with that name is already in the library is not for them
/// (and neither is a folder they can't choose).
pub fn problems_for_uploader(problems: Vec<String>) -> Vec<String> {
	problems
		.into_iter()
		.filter(|problem| !only_the_library_knows(problem))
		.collect()
}

/// Whether a reason is an answer about the library (its folders, its names, its books) and not
/// about what an uploader sent
fn only_the_library_knows(problem: &str) -> bool {
	problem == EXISTS_PROBLEM
		|| problem == ADDED_SINCE
		|| problem == "Choose a folder"
		|| problem == "The folder is outside the library"
		|| problem.starts_with("Invalid file name")
		|| problem.starts_with("The file name '")
		|| problem == "Two files would get the same name"
}

/// Why the last try with an item failed, as somebody who may not file books themselves reads it:
/// what is left when the answers about the library are taken out (`None` when none is)
pub fn error_for_uploader(error: &str) -> Option<String> {
	let kept: Vec<&str> = error
		.split("; ")
		.filter(|reason| !only_the_library_knows(reason))
		.collect();
	(!kept.is_empty()).then(|| kept.join("; "))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn an_uploader_is_told_what_they_can_fix() {
		let problems = vec![
			"Choose a folder".to_string(),
			"Add a title".to_string(),
			"Add an author".to_string(),
			"Invalid file name '../x'".to_string(),
			"Two files would get the same name".to_string(),
			EXISTS_PROBLEM.to_string(),
		];
		assert_eq!(
			problems_for_uploader(problems),
			["Add a title", "Add an author"]
		);
	}

	#[test]
	fn an_uploader_is_not_told_what_only_the_library_knows_about_a_failure() {
		// the reasons are joined with "; ": the ones about the library go, the others stay
		assert_eq!(
			error_for_uploader(&format!("Add a title; {EXISTS_PROBLEM}")),
			Some("Add a title".to_string())
		);
		assert_eq!(error_for_uploader(EXISTS_PROBLEM), None);
		assert_eq!(error_for_uploader(ADDED_SINCE), None);
		assert_eq!(
			error_for_uploader("Analysis failed: the file is damaged"),
			Some("Analysis failed: the file is damaged".to_string())
		);
	}

	#[test]
	fn tags_are_split_the_way_the_library_writes_them() {
		let tags: Vec<String> =
			split_tags("Fantasy, Origin: English ,, Horror").collect();
		assert_eq!(tags, ["Fantasy", "Origin: English", "Horror"]);
	}
}
