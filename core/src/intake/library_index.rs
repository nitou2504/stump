//! A read-only picture of one library used to make intake suggestions: its author and
//! series folders, the tags it uses, which tags each author's and series' books carry, and
//! (built only when needed) a small TF-IDF index over the book descriptions.
//!
//! It is built on demand and dropped [`INDEX_TTL`] after it was built, so an idle server
//! keeps none of it in memory.

use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	sync::{Arc, LazyLock, Mutex, OnceLock},
	time::{Duration, Instant},
};

use sea_orm::{DatabaseConnection, FromQueryResult, Statement};

use super::naming::{folder_keys, name_keys, name_tokens, NameKeys};
use crate::CoreResult;

/// How long a built index is kept for the next analyses
pub const INDEX_TTL: Duration = Duration::from_secs(10 * 60);

/// Tag prefixes the library uses for facets (not genres)
pub const FACET_PREFIXES: &[&str] = &[
	"Origin",
	"Era",
	"Level",
	"Audience",
	"Form",
	"Award",
	"Laureate",
	"Canon",
	"Course",
	"Collection",
	"Source",
];

/// Whether a tag is a facet (`Origin: X`, `Illustrated`, ...) rather than a genre
pub fn is_facet(tag: &str) -> bool {
	tag == "Illustrated"
		|| tag
			.split_once(':')
			.is_some_and(|(prefix, _)| FACET_PREFIXES.contains(&prefix.trim()))
}

/// Normalises a tag or subject for comparisons: folded, lowercase, single spaces
pub fn norm(text: &str) -> String {
	name_tokens(text).join(" ")
}

#[derive(Debug, Clone)]
pub struct FolderInfo {
	pub name: String,
	pub keys: NameKeys,
	pub books: i32,
}

#[derive(Debug, Clone)]
pub struct VocabEntry {
	pub value: String,
	pub norm: String,
	pub count: i32,
	pub facet: bool,
}

#[derive(Debug, Clone)]
pub struct IndexedBook {
	pub media_id: String,
	pub title: String,
	pub title_norm: String,
	pub writers: Vec<String>,
	/// Indices into [`LibraryIndex::vocabulary`]
	pub genres: Vec<u32>,
	pub series_norm: Option<String>,
	/// The first folder under the library root
	pub folder: Option<String>,
}

/// TF-IDF vectors of the library's book descriptions, as postings per term
#[derive(Debug, Default)]
pub struct DescriptionIndex {
	terms: HashMap<String, u32>,
	idf: Vec<f32>,
	postings: Vec<Vec<(u32, f32)>>,
}

#[derive(Debug)]
pub struct LibraryIndex {
	pub library_id: String,
	pub root: PathBuf,
	pub built_at: Instant,
	pub author_folders: Vec<FolderInfo>,
	/// ordered/sorted name keys -> author folder index, from the folder names
	folder_by_key: HashMap<String, usize>,
	/// ordered name keys of authors as the library's books write them -> the folder holding
	/// most of their books (index) and the name as written
	alias_by_key: HashMap<String, (usize, String)>,
	pub manga_folders: Vec<FolderInfo>,
	pub light_novel_folders: Vec<FolderInfo>,
	pub vocabulary: Vec<VocabEntry>,
	vocab_by_norm: HashMap<String, u32>,
	/// The `Small` part of `Big: Small` tags -> those tags
	small_by_norm: HashMap<String, Vec<u32>>,
	pub books: Vec<IndexedBook>,
	books_by_writer: HashMap<String, Vec<u32>>,
	books_by_folder: HashMap<String, Vec<u32>>,
	books_by_series: HashMap<String, Vec<u32>>,
	descriptions: OnceLock<DescriptionIndex>,
}

#[derive(Debug, FromQueryResult)]
struct BookRow {
	id: String,
	path: String,
	title: Option<String>,
	writers: Option<String>,
	genres: Option<String>,
	series: Option<String>,
}

#[derive(Debug, FromQueryResult)]
struct SummaryRow {
	id: String,
	summary: Option<String>,
}

static CACHE: LazyLock<Mutex<HashMap<String, Arc<LibraryIndex>>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

/// Drops every index older than [`INDEX_TTL`]
pub fn drop_stale_indexes() {
	if let Ok(mut cache) = CACHE.lock() {
		cache.retain(|_, index| index.built_at.elapsed() < INDEX_TTL);
	}
}

/// Forgets the index of a library (after books were filed into it)
pub fn invalidate_index(library_id: &str) {
	if let Ok(mut cache) = CACHE.lock() {
		cache.remove(library_id);
	}
}

/// The cached index of a library, built when missing or stale
pub async fn library_index(
	conn: &DatabaseConnection,
	library_id: &str,
	root: &Path,
	manga_dir: &str,
	light_novel_dir: &str,
) -> CoreResult<Arc<LibraryIndex>> {
	drop_stale_indexes();
	if let Some(index) = CACHE.lock().ok().and_then(|c| c.get(library_id).cloned()) {
		return Ok(index);
	}
	let index =
		Arc::new(build_index(conn, library_id, root, manga_dir, light_novel_dir).await?);
	if let Ok(mut cache) = CACHE.lock() {
		cache.insert(library_id.to_string(), index.clone());
	}
	// the index is only worth keeping while uploads are being reviewed
	tokio::spawn(async {
		tokio::time::sleep(INDEX_TTL + Duration::from_secs(5)).await;
		drop_stale_indexes();
	});
	Ok(index)
}

fn list_folders(dir: &Path) -> Vec<String> {
	let Ok(entries) = std::fs::read_dir(dir) else {
		return vec![];
	};
	let mut names: Vec<String> = entries
		.filter_map(Result::ok)
		.filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
		.filter_map(|e| e.file_name().into_string().ok())
		.filter(|n| !n.starts_with('.'))
		.collect();
	names.sort();
	names
}

fn split_list(value: &Option<String>) -> Vec<String> {
	value
		.as_deref()
		.unwrap_or("")
		.split(',')
		.map(str::trim)
		.filter(|s| !s.is_empty())
		.map(str::to_string)
		.collect()
}

async fn build_index(
	conn: &DatabaseConnection,
	library_id: &str,
	root: &Path,
	manga_dir: &str,
	light_novel_dir: &str,
) -> CoreResult<LibraryIndex> {
	let started = Instant::now();
	let backend = conn.get_database_backend();
	let rows = BookRow::find_by_statement(Statement::from_sql_and_values(
		backend,
		r"SELECT m.id AS id, m.path AS path, mm.title AS title, mm.writers AS writers,
			mm.genres AS genres, mm.series AS series
		FROM media m LEFT JOIN media_metadata mm ON mm.media_id = m.id
		WHERE m.library_id = ? AND m.deleted_at IS NULL",
		[library_id.into()],
	))
	.all(conn)
	.await?;

	let root_owned = root.to_path_buf();
	let manga_root = root.join(manga_dir);
	let light_novel_root = root.join(light_novel_dir);
	let (folder_names, manga_names, light_novel_names) =
		tokio::task::spawn_blocking(move || {
			let folders: Vec<String> = list_folders(&root_owned)
				.into_iter()
				.filter(|n| !n.starts_with('_'))
				.collect();
			(
				folders,
				list_folders(&manga_root),
				list_folders(&light_novel_root),
			)
		})
		.await
		.map_err(|e| crate::CoreError::InternalError(e.to_string()))?;

	let mut vocabulary: Vec<VocabEntry> = vec![];
	let mut vocab_by_value: HashMap<String, u32> = HashMap::new();
	let mut books = Vec::with_capacity(rows.len());
	let mut folder_book_counts: HashMap<String, i32> = HashMap::new();
	let mut series_book_counts: HashMap<String, i32> = HashMap::new();

	for row in rows {
		let rel = Path::new(&row.path)
			.strip_prefix(root)
			.ok()
			.map(Path::to_path_buf);
		let mut components = rel
			.as_ref()
			.map(|p| {
				p.components()
					.map(|c| c.as_os_str().to_string_lossy().into_owned())
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		let folder = (components.len() >= 2).then(|| components.remove(0));
		if let Some(folder) = &folder {
			*folder_book_counts.entry(folder.clone()).or_default() += 1;
			// series folders: <manga_dir>/<Series>/<file>
			if (folder == manga_dir || folder == light_novel_dir) && components.len() >= 2
			{
				*series_book_counts
					.entry(format!("{folder}/{}", components[0]))
					.or_default() += 1;
			}
		}

		let genres = split_list(&row.genres)
			.into_iter()
			.map(|g| {
				*vocab_by_value.entry(g.clone()).or_insert_with(|| {
					vocabulary.push(VocabEntry {
						norm: norm(&g),
						facet: is_facet(&g),
						value: g,
						count: 0,
					});
					(vocabulary.len() - 1) as u32
				})
			})
			.collect::<Vec<u32>>();
		for g in &genres {
			vocabulary[*g as usize].count += 1;
		}

		let title = row.title.clone().unwrap_or_default();
		books.push(IndexedBook {
			media_id: row.id,
			title_norm: norm(&title),
			title,
			writers: split_list(&row.writers),
			genres,
			series_norm: row.series.as_deref().map(norm).filter(|s| !s.is_empty()),
			folder,
		});
	}

	let author_folders: Vec<FolderInfo> = folder_names
		.into_iter()
		.map(|name| FolderInfo {
			keys: folder_keys(&name),
			books: folder_book_counts.get(&name).copied().unwrap_or(0),
			name,
		})
		.collect();
	let mut folder_by_key = HashMap::new();
	for (i, folder) in author_folders.iter().enumerate() {
		folder_by_key
			.entry(folder.keys.ordered.clone())
			.or_insert(i);
		folder_by_key.entry(folder.keys.sorted.clone()).or_insert(i);
	}
	let folder_index_by_name: HashMap<&str, usize> = author_folders
		.iter()
		.enumerate()
		.map(|(i, f)| (f.name.as_str(), i))
		.collect();

	// how the books write their first author -> the author folders holding those books
	let mut alias_votes: HashMap<String, HashMap<usize, (u32, String)>> = HashMap::new();
	let mut books_by_writer: HashMap<String, Vec<u32>> = HashMap::new();
	let mut books_by_folder: HashMap<String, Vec<u32>> = HashMap::new();
	let mut books_by_series: HashMap<String, Vec<u32>> = HashMap::new();
	for (i, book) in books.iter().enumerate() {
		for writer in &book.writers {
			books_by_writer
				.entry(name_keys(writer).ordered)
				.or_default()
				.push(i as u32);
		}
		if let Some(folder) = &book.folder {
			books_by_folder
				.entry(folder.clone())
				.or_default()
				.push(i as u32);
			if let (Some(&fi), Some(first)) = (
				folder_index_by_name.get(folder.as_str()),
				book.writers.first(),
			) {
				let vote = alias_votes
					.entry(name_keys(first).ordered)
					.or_default()
					.entry(fi)
					.or_insert((0, first.clone()));
				vote.0 += 1;
			}
		}
		if let Some(series) = &book.series_norm {
			books_by_series
				.entry(series.clone())
				.or_default()
				.push(i as u32);
		}
	}

	// an author whose books sit in several folders goes with the one holding most of them
	let alias_by_key: HashMap<String, (usize, String)> = alias_votes
		.into_iter()
		.filter_map(|(key, folders)| {
			folders
				.into_iter()
				.max_by(|(fa, (a, _)), (fb, (b, _))| a.cmp(b).then(fb.cmp(fa)))
				.map(|(folder, (_, written))| (key, (folder, written)))
		})
		.collect();

	let series_folders = |names: Vec<String>, dir: &str| -> Vec<FolderInfo> {
		names
			.into_iter()
			.map(|name| FolderInfo {
				keys: folder_keys(&name),
				books: series_book_counts
					.get(&format!("{dir}/{name}"))
					.copied()
					.unwrap_or(0),
				name,
			})
			.collect()
	};

	let mut vocab_by_norm = HashMap::new();
	let mut small_by_norm: HashMap<String, Vec<u32>> = HashMap::new();
	for (i, entry) in vocabulary.iter().enumerate() {
		vocab_by_norm.entry(entry.norm.clone()).or_insert(i as u32);
		if !entry.facet {
			if let Some((_, small)) = entry.value.split_once(':') {
				small_by_norm.entry(norm(small)).or_default().push(i as u32);
			}
		}
	}

	tracing::debug!(
		library_id,
		books = books.len(),
		author_folders = author_folders.len(),
		tags = vocabulary.len(),
		elapsed_ms = started.elapsed().as_millis() as u64,
		"Built the intake library index"
	);

	Ok(LibraryIndex {
		library_id: library_id.to_string(),
		root: root.to_path_buf(),
		built_at: Instant::now(),
		author_folders,
		folder_by_key,
		alias_by_key,
		manga_folders: series_folders(manga_names, manga_dir),
		light_novel_folders: series_folders(light_novel_names, light_novel_dir),
		vocabulary,
		vocab_by_norm,
		small_by_norm,
		books,
		books_by_writer,
		books_by_folder,
		books_by_series,
		descriptions: OnceLock::new(),
	})
}

/// How an author name matched an existing folder
#[derive(Debug, Clone, PartialEq)]
pub struct FolderMatch {
	pub folder: usize,
	pub confidence: f64,
	pub reason: String,
}

impl LibraryIndex {
	/// Existing author folders for an author name, best first
	pub fn match_author_folder(&self, author: &str) -> Vec<FolderMatch> {
		let keys = name_keys(author);
		let mut out: Vec<FolderMatch> = vec![];
		let mut seen = HashSet::new();
		let mut push = |m: FolderMatch, out: &mut Vec<FolderMatch>| {
			if seen.insert(m.folder) {
				out.push(m);
			}
		};
		if keys.ordered.is_empty() {
			return out;
		}
		if let Some(&i) = self.folder_by_key.get(&keys.ordered) {
			push(
				FolderMatch {
					folder: i,
					confidence: 1.0,
					reason: "same author".to_string(),
				},
				&mut out,
			);
		}
		if let Some((i, written)) = self.alias_by_key.get(&keys.ordered) {
			push(
				FolderMatch {
					folder: *i,
					confidence: 0.95,
					reason: format!("the library writes this author '{written}'"),
				},
				&mut out,
			);
		}
		if let Some(&i) = self.folder_by_key.get(&keys.sorted) {
			push(
				FolderMatch {
					folder: i,
					confidence: 0.9,
					reason: "same author, names in another order".to_string(),
				},
				&mut out,
			);
		}
		if out.is_empty() && keys.ordered.len() >= 6 {
			// a close spelling (Dostoievski / Dostoyevski), never more than a suggestion
			let mut close: Vec<(f64, usize)> = self
				.author_folders
				.iter()
				.enumerate()
				.map(|(i, f)| (strsim::jaro_winkler(&keys.ordered, &f.keys.ordered), i))
				.filter(|(score, _)| *score >= 0.93)
				.collect();
			close.sort_by(|a, b| b.0.total_cmp(&a.0));
			for (score, i) in close.into_iter().take(3) {
				push(
					FolderMatch {
						folder: i,
						confidence: (score - 0.25).clamp(0.4, 0.7),
						reason: "similar spelling".to_string(),
					},
					&mut out,
				);
			}
		}
		out
	}

	/// Existing series folders (under the manga or light novel root) for a series name
	pub fn match_series_folder(
		&self,
		series: &str,
		light_novel: bool,
	) -> Vec<(usize, f64)> {
		let folders = if light_novel {
			&self.light_novel_folders
		} else {
			&self.manga_folders
		};
		let keys = folder_keys(series);
		if keys.ordered.is_empty() {
			return vec![];
		}
		let mut out: Vec<(usize, f64)> = folders
			.iter()
			.enumerate()
			.filter_map(|(i, f)| {
				if f.keys.ordered == keys.ordered {
					Some((i, 1.0))
				} else {
					let score = strsim::jaro_winkler(&keys.ordered, &f.keys.ordered);
					(score >= 0.9).then_some((i, (score - 0.2).clamp(0.4, 0.75)))
				}
			})
			.collect();
		out.sort_by(|a, b| b.1.total_cmp(&a.1));
		out.truncate(3);
		out
	}

	/// The position of a library book in [`LibraryIndex::books`]
	pub fn book_position(&self, media_id: &str) -> Option<u32> {
		self.books
			.iter()
			.position(|b| b.media_id == media_id)
			.map(|i| i as u32)
	}

	pub fn vocab_index(&self, norm_value: &str) -> Option<u32> {
		self.vocab_by_norm.get(norm_value).copied()
	}

	pub fn small_tags(&self, norm_value: &str) -> &[u32] {
		self.small_by_norm
			.get(norm_value)
			.map(Vec::as_slice)
			.unwrap_or(&[])
	}

	/// Books whose writers include this author
	pub fn books_by_author(&self, author: &str) -> &[u32] {
		self.books_by_writer
			.get(&name_keys(author).ordered)
			.map(Vec::as_slice)
			.unwrap_or(&[])
	}

	pub fn books_in_folder(&self, folder: &str) -> &[u32] {
		self.books_by_folder
			.get(folder)
			.map(Vec::as_slice)
			.unwrap_or(&[])
	}

	pub fn books_in_series(&self, series: &str) -> &[u32] {
		self.books_by_series
			.get(&norm(series))
			.map(Vec::as_slice)
			.unwrap_or(&[])
	}

	/// Library books with the same title by the same author (normalised)
	pub fn same_title(&self, title: &str, authors: &[String]) -> Vec<u32> {
		let title_norm = norm(title);
		if title_norm.is_empty() {
			return vec![];
		}
		let author_keys: HashSet<String> =
			authors.iter().map(|a| name_keys(a).ordered).collect();
		self.books
			.iter()
			.enumerate()
			.filter(|(_, b)| b.title_norm == title_norm)
			.filter(|(_, b)| {
				author_keys.is_empty()
					|| b.writers
						.iter()
						.any(|w| author_keys.contains(&name_keys(w).ordered))
			})
			.map(|(i, _)| i as u32)
			.collect()
	}

	/// The books with the most similar descriptions, with their cosine similarity
	pub async fn similar_by_description(
		&self,
		conn: &DatabaseConnection,
		text: &str,
		limit: usize,
	) -> Vec<(u32, f32)> {
		if text.split_whitespace().count() < 8 {
			return vec![];
		}
		if self.descriptions.get().is_none() {
			match build_description_index(conn, &self.library_id, &self.books).await {
				Ok(index) => {
					let _ = self.descriptions.set(index);
				},
				Err(error) => {
					tracing::warn!(?error, "Failed to build the description index");
					return vec![];
				},
			}
		}
		let Some(index) = self.descriptions.get() else {
			return vec![];
		};
		index.query(text, limit)
	}
}

const STOPWORDS: &[&str] = &[
	"the", "and", "for", "that", "with", "this", "from", "his", "her", "their", "they",
	"was", "are", "but", "not", "have", "has", "had", "who", "which", "will", "into",
	"its", "one", "all", "more", "about", "when", "what", "she", "him", "them", "there",
	"been", "were", "than", "can", "also", "book", "books", "new", "los", "las", "del",
	"una", "uno", "con", "por", "para", "que", "como", "sus", "mas", "pero", "sobre",
	"entre", "este", "esta", "libro", "novela", "obra", "autor", "edition", "edicion",
	"your", "you", "our", "how", "after", "before", "over", "out", "only", "most",
	"other", "such", "these", "those",
];

fn tokenize(text: &str) -> Vec<String> {
	name_tokens(text)
		.into_iter()
		.filter(|t| t.len() >= 3 && !t.chars().all(|c| c.is_ascii_digit()))
		.filter(|t| !STOPWORDS.contains(&t.as_str()))
		.collect()
}

async fn build_description_index(
	conn: &DatabaseConnection,
	library_id: &str,
	books: &[IndexedBook],
) -> CoreResult<DescriptionIndex> {
	let started = Instant::now();
	let rows = SummaryRow::find_by_statement(Statement::from_sql_and_values(
		conn.get_database_backend(),
		r"SELECT m.id AS id, mm.summary AS summary
		FROM media m JOIN media_metadata mm ON mm.media_id = m.id
		WHERE m.library_id = ? AND m.deleted_at IS NULL AND mm.summary IS NOT NULL",
		[library_id.into()],
	))
	.all(conn)
	.await?;
	let position: HashMap<&str, u32> = books
		.iter()
		.enumerate()
		.map(|(i, b)| (b.media_id.as_str(), i as u32))
		.collect();

	let docs: Vec<(u32, HashMap<String, f32>)> = rows
		.into_iter()
		.filter_map(|row| {
			let book = *position.get(row.id.as_str())?;
			let mut tf: HashMap<String, f32> = HashMap::new();
			for token in tokenize(row.summary.as_deref().unwrap_or("")) {
				*tf.entry(token).or_default() += 1.0;
			}
			(!tf.is_empty()).then_some((book, tf))
		})
		.collect();

	let n = docs.len().max(1) as f32;
	let mut df: HashMap<&str, u32> = HashMap::new();
	for (_, tf) in &docs {
		for term in tf.keys() {
			*df.entry(term.as_str()).or_default() += 1;
		}
	}
	let mut index = DescriptionIndex::default();
	for (term, count) in &df {
		// a term in a single description or in a third of them says nothing
		if *count >= 2 && (*count as f32) <= n * 0.3 {
			let id = index.idf.len() as u32;
			index.terms.insert((*term).to_string(), id);
			index.idf.push((n / *count as f32).ln());
			index.postings.push(vec![]);
		}
	}
	for (book, tf) in &docs {
		let weights: Vec<(u32, f32)> = tf
			.iter()
			.filter_map(|(term, f)| {
				let id = *index.terms.get(term)?;
				Some((id, (1.0 + f.ln()) * index.idf[id as usize]))
			})
			.collect();
		let length = weights.iter().map(|(_, w)| w * w).sum::<f32>().sqrt();
		if length == 0.0 {
			continue;
		}
		for (id, w) in weights {
			index.postings[id as usize].push((*book, w / length));
		}
	}
	tracing::debug!(
		docs = docs.len(),
		terms = index.terms.len(),
		elapsed_ms = started.elapsed().as_millis() as u64,
		"Built the intake description index"
	);
	Ok(index)
}

impl DescriptionIndex {
	pub fn query(&self, text: &str, limit: usize) -> Vec<(u32, f32)> {
		let mut tf: HashMap<u32, f32> = HashMap::new();
		for token in tokenize(text) {
			if let Some(&id) = self.terms.get(&token) {
				*tf.entry(id).or_default() += 1.0;
			}
		}
		let mut weights: Vec<(u32, f32)> = tf
			.into_iter()
			.map(|(id, f)| (id, (1.0 + f.ln()) * self.idf[id as usize]))
			.collect();
		let length = weights.iter().map(|(_, w)| w * w).sum::<f32>().sqrt();
		if length == 0.0 {
			return vec![];
		}
		weights.sort_by(|a, b| b.1.total_cmp(&a.1));
		weights.truncate(40);
		let mut scores: HashMap<u32, f32> = HashMap::new();
		for (id, w) in weights {
			for (book, bw) in &self.postings[id as usize] {
				*scores.entry(*book).or_default() += (w / length) * bw;
			}
		}
		let mut ranked: Vec<(u32, f32)> = scores.into_iter().collect();
		ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
		ranked.truncate(limit);
		ranked
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn tells_facets_from_genres() {
		assert!(is_facet("Origin: French"));
		assert!(is_facet("Era: 20th Century"));
		assert!(is_facet("Illustrated"));
		assert!(!is_facet("Science Fiction"));
		assert!(!is_facet("Science Fiction: Space Opera"));
		assert!(!is_facet("Estante: Ciencia"));
	}

	#[test]
	fn ranks_descriptions_by_shared_rare_terms() {
		let mut index = DescriptionIndex::default();
		for (i, term) in ["dragon", "wizard", "galaxy", "starship"]
			.iter()
			.enumerate()
		{
			index.terms.insert(term.to_string(), i as u32);
			index.idf.push(1.0);
			index.postings.push(vec![]);
		}
		// book 0: fantasy, book 1: space opera
		index.postings[0].push((0, 0.7));
		index.postings[1].push((0, 0.7));
		index.postings[2].push((1, 0.7));
		index.postings[3].push((1, 0.7));
		let ranked = index.query("A young wizard must slay the dragon of the north", 5);
		assert_eq!(ranked.first().map(|r| r.0), Some(0));
		assert!(index
			.query("nothing in common at all here really", 5)
			.is_empty());
	}

	/// How the library's own books write an author leads to the folder holding most of them:
	/// no catalogue file is read
	#[tokio::test]
	async fn learns_how_the_library_writes_authors_from_its_books() {
		use ::tests::fake_data;
		use models::entity::media_metadata;
		use sea_orm::{ActiveModelTrait, ActiveValue::Set};

		let conn = ::tests::db::test_database().await;
		let dir = tempfile::tempdir().unwrap();
		let root = dir.path().join("biblioteca");
		for folder in ["Fiodor_Dostoyevski", "Otros"] {
			std::fs::create_dir_all(root.join(folder)).unwrap();
		}
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
		for (rel, writer) in [
			(
				"Fiodor_Dostoyevski/Crimen_y_castigo.epub",
				"Fiódor M. Dostoievski",
			),
			(
				"Fiodor_Dostoyevski/El_jugador.epub",
				"Fiódor M. Dostoievski",
			),
			("Otros/Antologia.epub", "Fiódor M. Dostoievski"),
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
				writers: Set(Some(writer.to_string())),
				..Default::default()
			}
			.insert(&conn)
			.await
			.unwrap();
		}
		let index = build_index(&conn, &library.id, &root, "_Manga", "_Novela ligera")
			.await
			.unwrap();
		assert!(!root.parent().unwrap().join("catalogo.csv").exists());
		let matches = index.match_author_folder("Fiódor M. Dostoievski");
		let best = &matches[0];
		assert_eq!(index.author_folders[best.folder].name, "Fiodor_Dostoyevski");
		assert!(
			best.reason.contains("the library writes this author"),
			"{}",
			best.reason
		);
		assert_eq!(index.author_folders[best.folder].books, 2);
	}
}
