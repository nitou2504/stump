//! Turns what was read and looked up into a proposal: merged metadata with sources, author
//! or series folders, tags from the library's vocabulary with reasons, similar books and
//! duplicates, and an overall confidence.

use std::collections::HashMap;

use models::shared::intake::{
	IntakeFolderCandidate, IntakeGenreSuggestion, IntakeKind, IntakeMetadata,
	IntakeProposal, IntakeSeriesKind, IntakeSimilarBook, IntakeSource,
};

use super::{
	library_index::{norm, LibraryIndex},
	lookup::OnlineRecord,
	naming::{fold_name, series_folder_name},
};

/// Under this overall confidence an item needs the uploader's review
pub const READY_CONFIDENCE: f64 = 0.7;

/// Library tags for subjects found in files and online catalogues (English and Spanish)
const SYNONYMS: &[(&str, &str)] = &[
	("science fiction", "Science Fiction"),
	("sci fi", "Science Fiction"),
	("scifi", "Science Fiction"),
	("ciencia ficcion", "Science Fiction"),
	("fantasy", "Fantasy"),
	("fantasia", "Fantasy"),
	("fantasy fiction", "Fantasy"),
	("horror", "Horror"),
	("terror", "Horror"),
	("horror tales", "Horror"),
	("mystery", "Mystery & Thriller"),
	("misterio", "Mystery & Thriller"),
	("thriller", "Mystery & Thriller"),
	("suspense", "Mystery & Thriller"),
	("crime", "Mystery & Thriller"),
	("detective and mystery stories", "Mystery & Thriller"),
	("policiaca", "Mystery & Thriller"),
	("policiaco", "Mystery & Thriller"),
	("novela negra", "Mystery & Thriller"),
	("intriga", "Mystery & Thriller"),
	("historical fiction", "Historical Fiction"),
	("novela historica", "Historical Fiction"),
	("historica", "Historical Fiction"),
	("romance", "Romance"),
	("romantica", "Romance"),
	("love stories", "Romance"),
	("adventure", "Adventure"),
	("aventuras", "Adventure"),
	("adventure stories", "Adventure"),
	("humor", "Humor & Satire"),
	("humour", "Humor & Satire"),
	("satire", "Humor & Satire"),
	("poetry", "Poetry"),
	("poesia", "Poetry"),
	("drama", "Plays"),
	("teatro", "Plays"),
	("plays", "Plays"),
	("literary fiction", "Literary Fiction"),
	("fiction", "Literary Fiction"),
	("novela", "Literary Fiction"),
	("ficcion", "Literary Fiction"),
	("contemporanea", "Literary Fiction"),
	("history", "History"),
	("historia", "History"),
	("biography", "Biography & Memoir"),
	("biografia", "Biography & Memoir"),
	("autobiography", "Biography & Memoir"),
	("memorias", "Biography & Memoir"),
	("memoir", "Biography & Memoir"),
	("philosophy", "Philosophy"),
	("filosofia", "Philosophy"),
	("psychology", "Psychology & Mind"),
	("psicologia", "Psychology & Mind"),
	("religion", "Religion & Mythology"),
	("mythology", "Religion & Mythology"),
	("mitologia", "Religion & Mythology"),
	("politics", "Politics & Current Affairs"),
	("politica", "Politics & Current Affairs"),
	("political science", "Politics & Current Affairs"),
	("sociology", "Society & Culture"),
	("sociologia", "Society & Culture"),
	("social science", "Society & Culture"),
	("economics", "Economics & Business"),
	("economia", "Economics & Business"),
	("business", "Economics & Business"),
	("business economics", "Economics & Business"),
	("mathematics", "Mathematics"),
	("matematicas", "Mathematics"),
	("physics", "Physics & Astronomy"),
	("fisica", "Physics & Astronomy"),
	("astronomy", "Physics & Astronomy"),
	("astronomia", "Physics & Astronomy"),
	("cosmology", "Physics & Astronomy"),
	("chemistry", "Chemistry"),
	("quimica", "Chemistry"),
	("biology", "Biology & Nature"),
	("biologia", "Biology & Nature"),
	("nature", "Biology & Nature"),
	("naturaleza", "Biology & Nature"),
	("science", "Science (General)"),
	("ciencia", "Science (General)"),
	("divulgacion", "Science (General)"),
	("popular science", "Science (General)"),
	("medicine", "Medicine & Health"),
	("medicina", "Medicine & Health"),
	("health", "Medicine & Health"),
	("salud", "Medicine & Health"),
	("computers", "Computing"),
	("computing", "Computing"),
	("informatica", "Computing"),
	("programming", "Computing"),
	("technology", "Technology & Engineering"),
	("tecnologia", "Technology & Engineering"),
	("engineering", "Technology & Engineering"),
	("art", "Arts"),
	("arte", "Arts"),
	("music", "Arts"),
	("musica", "Arts"),
	("language", "Language & Writing"),
	("linguistics", "Language & Writing"),
	("linguistica", "Language & Writing"),
	("literary criticism", "Literary Criticism"),
	("critica literaria", "Literary Criticism"),
	("ensayo", "Literary Criticism"),
	("self help", "Self-Help"),
	("autoayuda", "Self-Help"),
	("travel", "Travel & Exploration"),
	("viajes", "Travel & Exploration"),
	("cooking", "Food & Home"),
	("cocina", "Food & Home"),
	("sports", "Sports & Games"),
	("deporte", "Sports & Games"),
	("games", "Sports & Games"),
	("chess", "Sports & Games"),
	("reference", "Reference"),
	("comics", "Manga & Comics"),
	("comic", "Manga & Comics"),
	("manga", "Manga & Comics"),
	("graphic novels", "Manga & Comics"),
	("light novel", "Light Novels"),
	("light novels", "Light Novels"),
	("novela ligera", "Light Novels"),
	("juvenile fiction", "Audience: Young Adult"),
	("young adult", "Audience: Young Adult"),
	("juvenil", "Audience: Young Adult"),
	("children", "Audience: Children"),
	("infantil", "Audience: Children"),
	("short stories", "Form: Short Stories"),
	("relatos", "Form: Short Stories"),
	("cuentos", "Form: Short Stories"),
	("essays", "Form: Essays"),
];

/// Facets only ever assigned by hand (awards, canons, courses, collections, sources)
const HAND_PICKED_FACETS: &[&str] = &["Award", "Laureate", "Canon", "Course", "Collection", "Source"];

/// Publishers whose EPUBs are light novels
const LIGHT_NOVEL_PUBLISHERS: &[&str] = &[
	"yen press", "yen on", "j-novel club", "seven seas", "one peace books", "vertical",
	"kadokawa", "dengeki", "fujimi", "ize press",
];

/// Everything analysis found, ready to be scored
pub struct Inputs<'a> {
	pub kind: IntakeKind,
	pub index: &'a LibraryIndex,
	/// Merged metadata of the file(s), with sources
	pub file_meta: IntakeMetadata,
	pub online: Vec<OnlineRecord>,
	/// File extensions of the item's files (`cbz`, `epub`, ...)
	pub formats: Vec<String>,
	/// Whether any file carries ComicInfo `Manga` or the item came as a folder of volumes
	pub series_hint: Option<IntakeSeriesKind>,
	/// The uploaded folder or zip name, for series items
	pub group_name: Option<String>,
	/// `(media_id, cosine)` of books with similar descriptions
	pub similar: Vec<(u32, f32)>,
}

/// Merges online records into the file's metadata: the file wins, then Open Library, then
/// Google Books, then Hardcover (its description and series fill what is missing). Subjects
/// are pooled; one only a weighing source gives (Hardcover) keeps its weight.
pub fn merge_online(meta: &mut IntakeMetadata, online: &[OnlineRecord]) {
	let file_named = |m: &IntakeMetadata, field: &str| {
		matches!(m.source_of(field), None | Some(IntakeSource::FileName))
	};
	for record in online {
		let source = record.source.unwrap_or(IntakeSource::OpenLibrary);
		if file_named(meta, "authors") && !record.authors.is_empty() && meta.authors.is_empty()
		{
			meta.authors = record.authors.clone();
			meta.set_source("authors", source);
		}
		if meta.language.is_none() {
			if let Some(language) = &record.language {
				meta.language = Some(language.clone());
				meta.set_source("language", source);
			}
		}
		// Open Library's first publication year beats an edition year from the file
		if let Some(year) = record.first_publish_year {
			let better = source == IntakeSource::OpenLibrary
				&& meta.year.is_none_or(|y| year < y);
			if meta.year.is_none() || better {
				meta.year = Some(year);
				meta.set_source("year", source);
			}
		}
		if meta.publisher.is_none() {
			if let Some(publisher) = &record.publisher {
				meta.publisher = Some(publisher.clone());
				meta.set_source("publisher", source);
			}
		}
		let short = meta
			.description
			.as_deref()
			.is_none_or(|d| d.split_whitespace().count() < 15);
		if short {
			if let Some(description) = &record.description {
				meta.description = Some(description.clone());
				meta.set_source("description", source);
			}
		}
		if meta.isbn.is_none() {
			if let Some(isbn) = &record.isbn {
				meta.isbn = Some(isbn.clone());
				meta.set_source("isbn", source);
			}
		}
		if meta.series.is_none() {
			if let Some(series) = &record.series {
				meta.series = Some(series.clone());
				meta.set_source("series", source);
				if let Some(index) =
					record.series_index.filter(|_| meta.series_index.is_none())
				{
					meta.series_index = Some(index);
					meta.set_source("seriesIndex", source);
				}
			}
		}
		for subject in &record.subjects {
			let weight = record.subject_weights.get(subject).copied();
			if !meta.subjects.contains(subject) {
				meta.subjects.push(subject.clone());
				if let Some(weight) = weight {
					meta.subject_weights.insert(subject.clone(), weight);
				}
			} else if let Some(current) = meta.subject_weights.get(subject).copied() {
				// given again: by a source that weighs it (the stronger weight), or fully
				match weight {
					Some(weight) => meta
						.subject_weights
						.insert(subject.clone(), current.max(weight)),
					None => meta.subject_weights.remove(subject),
				};
			}
		}
	}
}

/// The evidence for one tag: its summed weight and why
#[derive(Default, Debug, Clone)]
pub struct Score {
	pub value: f64,
	pub reasons: Vec<String>,
}

fn add(scores: &mut HashMap<u32, Score>, tag: u32, value: f64, reason: String) {
	let entry = scores.entry(tag).or_default();
	entry.value += value;
	if !entry.reasons.contains(&reason) && entry.reasons.len() < 4 {
		entry.reasons.push(reason);
	}
}

/// Splits a catalogue subject ("Fiction, science fiction, general", "Fiction / Historical")
/// into the pieces worth matching
fn subject_pieces(subject: &str) -> Vec<String> {
	let mut pieces: Vec<String> = subject
		.split([',', ';', '/', '|'])
		.flat_map(|p| p.split(" -- "))
		.map(norm)
		.filter(|p| !p.is_empty() && p != "general" && p != "fiction general")
		.collect();
	let whole = norm(subject);
	if !pieces.contains(&whole) {
		pieces.insert(0, whole);
	}
	pieces
}

fn tag_votes(
	scores: &mut HashMap<u32, Score>,
	index: &LibraryIndex,
	books: &[u32],
	weight: f64,
	what: &str,
	exclude: Option<u32>,
) {
	let books: Vec<u32> = books.iter().copied().filter(|b| Some(*b) != exclude).collect();
	if books.is_empty() {
		return;
	}
	let mut counts: HashMap<u32, u32> = HashMap::new();
	for book in &books {
		for tag in &index.books[*book as usize].genres {
			*counts.entry(*tag).or_default() += 1;
		}
	}
	let n = books.len() as f64;
	for (tag, count) in counts {
		let share = count as f64 / n;
		add(
			scores,
			tag,
			weight * share,
			format!("{count} of {} {what} have it", books.len()),
		);
	}
}

fn era_for(year: i32) -> &'static str {
	if year < 1900 {
		"Era: Pre-1900"
	} else if year < 1990 {
		"Era: 20th Century"
	} else {
		"Era: Contemporary"
	}
}

/// Ranked tag suggestions with the default choice marked
pub fn suggest_genres(
	index: &LibraryIndex,
	meta: &IntakeMetadata,
	author_folder: Option<&str>,
	similar: &[(u32, f32)],
	series_kind: Option<IntakeSeriesKind>,
) -> Vec<IntakeGenreSuggestion> {
	let scores = score_tags(index, meta, author_folder, similar, series_kind, None);
	rank_suggestions(index, scores, series_kind)
}

/// The evidence for each of the library's tags: the subjects (file and online), the same
/// author's and series' books, books with a similar description, the era and the shelf of
/// manga / light novels. `exclude` leaves one library book (the book being edited) out of
/// the votes.
pub fn score_tags(
	index: &LibraryIndex,
	meta: &IntakeMetadata,
	author_folder: Option<&str>,
	similar: &[(u32, f32)],
	series_kind: Option<IntakeSeriesKind>,
	exclude: Option<u32>,
) -> HashMap<u32, Score> {
	let mut scores: HashMap<u32, Score> = HashMap::new();
	let file_subjects = meta.source_of("subjects") == Some(IntakeSource::File);

	// 1. subjects, from the file and online catalogues (a weighed one by its weight)
	for subject in &meta.subjects {
		let weight = meta
			.subject_weights
			.get(subject)
			.copied()
			.unwrap_or(1.0)
			.clamp(0.0, 1.0);
		let whole = norm(subject);
		if let Some(tag) = index.vocab_index(&whole) {
			let (value, why) = if file_subjects {
				(1.0, "in the file".to_string())
			} else {
				(0.7, format!("subject '{subject}'"))
			};
			add(&mut scores, tag, value * weight, why);
			continue;
		}
		for piece in subject_pieces(subject) {
			if let Some(tag) = index.vocab_index(&piece) {
				add(&mut scores, tag, 0.6 * weight, format!("subject '{subject}'"));
			} else if let Some((_, canonical)) = SYNONYMS.iter().find(|(s, _)| *s == piece) {
				if let Some(tag) = index.vocab_index(&norm(canonical)) {
					add(&mut scores, tag, 0.5 * weight, format!("subject '{subject}'"));
				}
			} else {
				for tag in index.small_tags(&piece) {
					add(&mut scores, *tag, 0.5 * weight, format!("subject '{subject}'"));
				}
			}
		}
	}

	// 2. the same author's books, the same series
	let mut author_books: Vec<u32> = vec![];
	if let Some(folder) = author_folder {
		author_books.extend_from_slice(index.books_in_folder(folder));
	}
	for author in &meta.authors {
		for book in index.books_by_author(author) {
			if !author_books.contains(book) {
				author_books.push(*book);
			}
		}
	}
	tag_votes(&mut scores, index, &author_books, 0.8, "books by this author", exclude);
	if let Some(series) = &meta.series {
		let series_books = index.books_in_series(series);
		tag_votes(&mut scores, index, series_books, 0.9, "books of this series", exclude);
	}

	// 3. books with similar descriptions
	let similar: Vec<(u32, f32)> = similar
		.iter()
		.copied()
		.filter(|(b, _)| Some(*b) != exclude)
		.collect();
	let total: f32 = similar.iter().map(|(_, s)| s).sum();
	if total > 0.0 {
		let mut weights: HashMap<u32, (f32, u32)> = HashMap::new();
		for (book, sim) in &similar {
			for tag in &index.books[*book as usize].genres {
				let e = weights.entry(*tag).or_default();
				e.0 += sim;
				e.1 += 1;
			}
		}
		for (tag, (weight, count)) in weights {
			if count >= 2 {
				add(
					&mut scores,
					tag,
					0.6 * (weight / total) as f64,
					format!("{count} books with a similar description have it"),
				);
			}
		}
	}

	// 4. the era from the first publication year
	if let Some(year) = meta.year {
		if meta.source_of("year") == Some(IntakeSource::OpenLibrary) {
			if let Some(tag) = index.vocab_index(&norm(era_for(year))) {
				add(&mut scores, tag, 0.6, format!("first published in {year}"));
			}
		}
	}

	// 5. manga and light novels carry their shelf tag
	let shelf = match series_kind {
		Some(IntakeSeriesKind::Manga) => Some("Manga & Comics"),
		Some(IntakeSeriesKind::LightNovel) => Some("Light Novels"),
		None => None,
	};
	if let Some(tag) = shelf.and_then(|s| index.vocab_index(&norm(s))) {
		add(&mut scores, tag, 1.0, "manga / light novel".to_string());
	}

	scores
}

/// The suggestions from scored tags, best first, with the default choice marked: at most
/// three big and three small tags, one facet per prefix, never a hand-picked facet
pub fn rank_suggestions(
	index: &LibraryIndex,
	scores: HashMap<u32, Score>,
	series_kind: Option<IntakeSeriesKind>,
) -> Vec<IntakeGenreSuggestion> {
	let mut ranked: Vec<(u32, Score)> = scores.into_iter().collect();
	ranked.sort_by(|a, b| {
		b.1.value
			.total_cmp(&a.1.value)
			.then(index.vocabulary[b.0 as usize].count.cmp(&index.vocabulary[a.0 as usize].count))
	});

	let mut out: Vec<IntakeGenreSuggestion> = vec![];
	let (mut big, mut small) = (0, 0);
	let mut taken_facets: Vec<String> = vec![];
	let other_shelf = match series_kind {
		Some(IntakeSeriesKind::Manga) => Some("Light Novels"),
		Some(IntakeSeriesKind::LightNovel) => Some("Manga & Comics"),
		None => None,
	};
	for (tag, score) in ranked.into_iter().take(16) {
		let entry = &index.vocabulary[tag as usize];
		let confidence = score.value.min(1.0);
		let mut selected = confidence >= 0.5;
		if selected {
			if entry.facet {
				let prefix = entry.value.split(':').next().unwrap_or("").trim().to_string();
				if HAND_PICKED_FACETS.contains(&prefix.as_str()) {
					// awards, canons, courses, collections: picked by hand, never a default
					selected = false;
				} else {
					// one origin, one era, one level, ... at most
					selected = !taken_facets.contains(&prefix);
					taken_facets.push(prefix);
				}
			} else if Some(entry.value.as_str()) == other_shelf {
				// a light novel is not a manga (and vice versa), whatever a catalogue says
				selected = false;
			} else if entry.value.contains(':') {
				small += 1;
				selected = small <= 3;
			} else {
				big += 1;
				selected = big <= 3;
			}
		}
		out.push(IntakeGenreSuggestion {
			value: entry.value.clone(),
			confidence: (confidence * 100.0).round() / 100.0,
			reasons: score.reasons,
			library_count: entry.count,
			selected,
			kind: if entry.facet { "FACET" } else { "GENRE" }.to_string(),
		});
	}

	// a selected `Big: Small` brings its `Big`
	let parents: Vec<String> = out
		.iter()
		.filter(|s| s.selected && s.kind == "GENRE")
		.filter_map(|s| s.value.split_once(':').map(|(b, _)| b.trim().to_string()))
		.collect();
	for parent in parents {
		if let Some(existing) = out.iter_mut().find(|s| s.value == parent) {
			existing.selected = true;
		} else if let Some(tag) = index.vocab_index(&norm(&parent)) {
			let entry = &index.vocabulary[tag as usize];
			out.push(IntakeGenreSuggestion {
				value: entry.value.clone(),
				confidence: 0.6,
				reasons: vec!["the parent of a suggested tag".to_string()],
				library_count: entry.count,
				selected: true,
				kind: "GENRE".to_string(),
			});
		}
	}
	out
}

/// Author folder candidates: existing folders first, then the folder to create
pub fn author_folder_candidates(
	index: &LibraryIndex,
	authors: &[String],
) -> Vec<IntakeFolderCandidate> {
	let Some(first) = authors.first() else {
		return vec![];
	};
	let mut out: Vec<IntakeFolderCandidate> = index
		.match_author_folder(first)
		.into_iter()
		.map(|m| {
			let folder = &index.author_folders[m.folder];
			IntakeFolderCandidate {
				folder: folder.name.clone(),
				books: folder.books,
				confidence: m.confidence,
				reason: m.reason,
				is_new: false,
			}
		})
		.collect();
	let new_folder = fold_name(first);
	if !new_folder.is_empty() && !out.iter().any(|c| c.folder == new_folder) {
		let exists = index.author_folders.iter().any(|f| f.name == new_folder);
		out.push(IntakeFolderCandidate {
			folder: new_folder,
			books: 0,
			confidence: if out.is_empty() { 0.75 } else { 0.3 },
			reason: if out.is_empty() {
				"new author: a new folder".to_string()
			} else {
				"a new folder".to_string()
			},
			is_new: !exists,
		});
	}
	out
}

/// Series folder candidates under the manga or light novel root
pub fn series_folder_candidates(
	index: &LibraryIndex,
	series: &str,
	kind: IntakeSeriesKind,
) -> Vec<IntakeFolderCandidate> {
	let light_novel = kind == IntakeSeriesKind::LightNovel;
	let folders = if light_novel {
		&index.light_novel_folders
	} else {
		&index.manga_folders
	};
	let mut out: Vec<IntakeFolderCandidate> = index
		.match_series_folder(series, light_novel)
		.into_iter()
		.map(|(i, confidence)| IntakeFolderCandidate {
			folder: folders[i].name.clone(),
			books: folders[i].books,
			confidence,
			reason: if confidence >= 1.0 {
				"this series is already in the library".to_string()
			} else {
				"a series with a similar name".to_string()
			},
			is_new: false,
		})
		.collect();
	let new_folder = series_folder_name(series);
	if !new_folder.is_empty() && !out.iter().any(|c| c.folder == new_folder) {
		out.push(IntakeFolderCandidate {
			folder: new_folder,
			books: 0,
			confidence: if out.is_empty() { 0.75 } else { 0.3 },
			reason: "a new series folder".to_string(),
			is_new: true,
		});
	}
	out
}

/// Manga, light novel or neither, from formats, publisher, subjects and the upload shape
pub fn detect_series_kind(
	kind: IntakeKind,
	formats: &[String],
	meta: &IntakeMetadata,
	hint: Option<IntakeSeriesKind>,
	index: &LibraryIndex,
) -> Option<IntakeSeriesKind> {
	if hint.is_some() {
		return hint;
	}
	if formats.iter().any(|f| f == "cbz" || f == "cbr") {
		return Some(IntakeSeriesKind::Manga);
	}
	let publisher = meta.publisher.as_deref().map(str::to_lowercase).unwrap_or_default();
	let ln_subject = meta.subjects.iter().any(|s| {
		let n = norm(s);
		n.contains("light novel") || n.contains("novela ligera")
	});
	let ln_publisher = LIGHT_NOVEL_PUBLISHERS.iter().any(|p| publisher.contains(p));
	let known_ln_series = meta
		.series
		.as_deref()
		.is_some_and(|s| !index.match_series_folder(s, true).is_empty());
	if ln_subject || known_ln_series || (ln_publisher && meta.series.is_some()) {
		return Some(IntakeSeriesKind::LightNovel);
	}
	if kind == IntakeKind::Series && formats.iter().all(|f| f == "epub") && meta.series.is_some() {
		// a folder of EPUB volumes from a manga/LN publisher reads as light novels
		if ln_publisher {
			return Some(IntakeSeriesKind::LightNovel);
		}
	}
	None
}

/// Builds the whole proposal
pub fn build_proposal(inputs: Inputs<'_>) -> IntakeProposal {
	let Inputs {
		kind,
		index,
		file_meta,
		online,
		formats,
		series_hint,
		group_name,
		similar,
	} = inputs;
	let mut meta = file_meta;
	merge_online(&mut meta, &online);
	if kind == IntakeKind::Series && meta.series.is_none() {
		if let Some(name) = &group_name {
			meta.series = Some(name.clone());
			meta.set_source("series", IntakeSource::FileName);
		}
	}

	let series_kind = detect_series_kind(kind, &formats, &meta, series_hint, index);
	let mut warnings = vec![];

	let author_folders = if series_kind.is_none() {
		author_folder_candidates(index, &meta.authors)
	} else {
		vec![]
	};
	let series_folders = match (series_kind, meta.series.as_deref()) {
		(Some(sk), Some(series)) => series_folder_candidates(index, series, sk),
		_ => vec![],
	};

	let similar_books: Vec<IntakeSimilarBook> = {
		let mut out = vec![];
		let mut seen = std::collections::HashSet::new();
		let author_folder = author_folders.first().filter(|c| !c.is_new).map(|c| c.folder.clone());
		let mut by_author: Vec<u32> = author_folder
			.as_deref()
			.map(|f| index.books_in_folder(f).to_vec())
			.unwrap_or_default();
		if let Some(author) = meta.authors.first() {
			by_author.extend_from_slice(index.books_by_author(author));
		}
		for book in by_author.into_iter().take(12) {
			if seen.insert(book) && out.len() < 3 {
				out.push((book, "same author"));
			}
		}
		for (book, _) in similar.iter().take(6) {
			if seen.insert(*book) && out.len() < 6 {
				out.push((*book, "similar description"));
			}
		}
		out.into_iter()
			.map(|(book, reason)| {
				let b = &index.books[book as usize];
				IntakeSimilarBook {
					media_id: b.media_id.clone(),
					title: b.title.clone(),
					authors: b.writers.clone(),
					genres: b
						.genres
						.iter()
						.map(|g| index.vocabulary[*g as usize].value.clone())
						.collect(),
					reason: reason.to_string(),
				}
			})
			.collect()
	};

	let chosen_author_folder = author_folders
		.first()
		.filter(|c| !c.is_new)
		.map(|c| c.folder.clone());
	let genre_suggestions = suggest_genres(
		index,
		&meta,
		chosen_author_folder.as_deref(),
		&similar,
		series_kind,
	);

	// confidence: the weakest of what filing needs
	let title_confidence: f64 = match meta.source_of("title") {
		Some(IntakeSource::File) | Some(IntakeSource::User) => 1.0,
		Some(IntakeSource::OpenLibrary)
		| Some(IntakeSource::GoogleBooks)
		| Some(IntakeSource::Hardcover) => 0.9,
		Some(_) => 0.7,
		None => 0.0,
	};
	if meta.title.is_none() && series_kind.is_none() {
		warnings.push("No title found".to_string());
	}
	let place_confidence = if series_kind.is_some() {
		series_folders.first().map(|c| c.confidence).unwrap_or(0.0)
	} else {
		author_folders.first().map(|c| c.confidence).unwrap_or(0.0)
	};
	if series_kind.is_none() && meta.authors.is_empty() {
		warnings.push("No author found: pick or type the author".to_string());
	}
	if series_kind.is_some() && meta.series.is_none() {
		warnings.push("No series name found".to_string());
	}
	let genre_confidence = if series_kind.is_some() {
		1.0
	} else {
		genre_suggestions
			.iter()
			.filter(|s| s.selected && s.kind == "GENRE")
			.map(|s| s.confidence)
			.fold(0.0, f64::max)
	};
	if series_kind.is_none() && genre_confidence < 0.5 {
		warnings.push("No confident genre: please choose the tags".to_string());
	}
	let title_part = if series_kind.is_some() { 1.0 } else { title_confidence };
	let confidence = title_part.min(place_confidence).min(genre_confidence.max(0.0));

	IntakeProposal {
		metadata: meta,
		series_kind,
		author_folders,
		series_folders,
		genre_suggestions,
		similar_books,
		duplicates: vec![],
		warnings,
		confidence: (confidence * 100.0).round() / 100.0,
		looked_up_online: !online.is_empty(),
		online_lookups: vec![],
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn splits_catalogue_subjects() {
		let pieces = subject_pieces("Fiction, science fiction, general");
		assert!(pieces.contains(&"science fiction".to_string()));
		assert!(!pieces.contains(&"general".to_string()));
		let pieces = subject_pieces("Fiction / Historical");
		assert!(pieces.contains(&"historical".to_string()));
	}

	#[test]
	fn merges_online_records_without_overriding_the_file() {
		let mut meta = IntakeMetadata {
			title: Some("El nombre de la rosa".into()),
			authors: vec!["Umberto Eco".into()],
			year: Some(2011),
			..Default::default()
		};
		meta.set_source("title", IntakeSource::File);
		meta.set_source("authors", IntakeSource::File);
		meta.set_source("year", IntakeSource::File);
		let online = vec![OnlineRecord {
			source: Some(IntakeSource::OpenLibrary),
			authors: vec!["Someone Else".into()],
			first_publish_year: Some(1980),
			subjects: vec!["Fiction".into()],
			language: Some("it".into()),
			..Default::default()
		}];
		merge_online(&mut meta, &online);
		assert_eq!(meta.authors, vec!["Umberto Eco"]);
		assert_eq!(meta.year, Some(1980));
		assert_eq!(meta.source_of("year"), Some(IntakeSource::OpenLibrary));
		assert_eq!(meta.language.as_deref(), Some("it"));
		assert_eq!(meta.subjects, vec!["Fiction"]);
	}

	#[test]
	fn keeps_hardcover_weights_and_fills_a_missing_series() {
		let mut meta = IntakeMetadata {
			subjects: vec!["History".into()],
			..Default::default()
		};
		meta.set_source("subjects", IntakeSource::File);
		let online = vec![
			OnlineRecord {
				source: Some(IntakeSource::OpenLibrary),
				subjects: vec!["Islam".into()],
				..Default::default()
			},
			OnlineRecord {
				source: Some(IntakeSource::Hardcover),
				subjects: vec![
					"Religion".into(),
					"History".into(),
					"Islam".into(),
					"Theology".into(),
				],
				subject_weights: HashMap::from([
					("History".to_string(), 0.75),
					("Islam".to_string(), 0.8),
					("Theology".to_string(), 0.6),
				]),
				description: Some(
					"The sacred book of Islam, its history and how it is read.".into(),
				),
				series: Some("Very Short Introductions".into()),
				series_index: Some(143.0),
				..Default::default()
			},
		];
		merge_online(&mut meta, &online);
		assert_eq!(
			meta.subjects,
			vec!["History", "Islam", "Religion", "Theology"]
		);
		// the file's and Open Library's subjects count fully: only Hardcover's own keep a weight
		assert_eq!(
			meta.subject_weights,
			HashMap::from([("Theology".to_string(), 0.6)])
		);
		assert_eq!(meta.series.as_deref(), Some("Very Short Introductions"));
		assert_eq!(meta.series_index, Some(143.0));
		assert_eq!(meta.source_of("series"), Some(IntakeSource::Hardcover));
		assert_eq!(meta.source_of("description"), Some(IntakeSource::Hardcover));

		// a book's own series stays
		let mut meta = IntakeMetadata {
			series: Some("Cambridge Concise Histories".into()),
			..Default::default()
		};
		merge_online(&mut meta, &online);
		assert_eq!(meta.series.as_deref(), Some("Cambridge Concise Histories"));
		assert_eq!(meta.series_index, None);
	}

	/// A subject few of Hardcover's readers gave counts by its weight: suggested, not ticked
	#[tokio::test]
	async fn scores_a_weighed_subject_by_its_weight() {
		use ::tests::fake_data;
		use models::entity::media_metadata;
		use sea_orm::{ActiveModelTrait, ActiveValue::Set};

		let conn = ::tests::db::test_database().await;
		let dir = tempfile::tempdir().unwrap();
		let root = dir.path().join("biblioteca");
		std::fs::create_dir_all(root.join("Otros")).unwrap();
		let library = fake_data::Library {
			path: Some(root.to_string_lossy().into_owned()),
			..Default::default()
		}
		.insert(&conn)
		.await;
		let series = fake_data::Series {
			library_id: Some(library.id.clone()),
			..Default::default()
		}
		.insert(&conn)
		.await;
		for (rel, genres) in [
			("Otros/a.epub", "History, Fantasy"),
			("Otros/b.epub", "Philosophy"),
		] {
			let book = fake_data::Media {
				series_id: series.id.clone(),
				path: Some(root.join(rel).to_string_lossy().into_owned()),
				..Default::default()
			}
			.insert(&conn)
			.await;
			media_metadata::ActiveModel {
				media_id: Set(Some(book.id)),
				title: Set(Some(rel.to_string())),
				genres: Set(Some(genres.to_string())),
				..Default::default()
			}
			.insert(&conn)
			.await
			.unwrap();
		}
		let index = super::super::library_index::library_index(
			&conn,
			&library.id,
			&root,
			"_Manga",
			"_Novela ligera",
		)
		.await
		.unwrap();
		let mut meta = IntakeMetadata {
			subjects: vec!["History".into(), "Fantasy".into()],
			..Default::default()
		};
		meta.subject_weights.insert("Fantasy".into(), 0.55);
		let scores = score_tags(&index, &meta, None, &[], None, None);
		let score = |tag: &str| {
			index
				.vocab_index(&norm(tag))
				.and_then(|t| scores.get(&t))
				.map(|s| s.value)
				.unwrap_or(0.0)
		};
		assert!((score("History") - 0.7).abs() < 1e-9, "{scores:?}");
		assert!((score("Fantasy") - 0.385).abs() < 1e-9, "{scores:?}");
		let suggestions = rank_suggestions(&index, scores, None);
		let selected = |value: &str| {
			suggestions
				.iter()
				.find(|s| s.value == value)
				.map(|s| s.selected)
		};
		assert_eq!(selected("History"), Some(true));
		assert_eq!(selected("Fantasy"), Some(false));
	}
}
