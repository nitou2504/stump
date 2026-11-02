//! The PDF reader's server side: page pictures sized to the screen, selectable text layers,
//! whole-book search and the document layout, all rendered by one thread that owns PDFium.
//!
//! Why one thread: PDFium is behind a global lock (`media::processor::pdf::renderer`), and the
//! old path had every request fight for it while also prerendering ±5 pages at print quality.
//! Here requests are queued by priority (a page a reader waits for > a reader's prefetch >
//! search and read-ahead), identical requests are rendered once, a prefetch is dropped when the
//! reader has jumped elsewhere, and open documents are reused while a burst of work runs.
//!
//! Routes: `apps/server/src/routers/api/v2/pdf.rs`. Contract: `/tmp/pdf-lane/CONTRACT.md`.

mod cache;
mod engine;
mod queue;
mod search;
mod text;

use std::{
	collections::VecDeque,
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
	sync::{Arc, Condvar, Mutex, OnceLock},
	time::{Duration, Instant, UNIX_EPOCH},
};

use pdfium_render::prelude::PdfDocument;
use tokio::sync::oneshot;

pub use cache::PageCache;
pub use engine::{allowed_tiers, snap_tier, OutlineEntry, PdfLayout, TIERS};
pub use queue::{ClientKey, Priority};
pub use search::{fold_query, search_page, HitRect, SearchHit};
pub use text::PageText;

use crate::{
	config::StumpConfig,
	fs_utils::ContentType,
	media::processor::{error::MediaProcessorError, pdf::renderer},
};
use queue::{Next, Scheduler};

/// How long the render thread keeps PDFium (and its open documents) before letting the
/// scanner and thumbnail jobs have the lock
const BURST: Duration = Duration::from_millis(300);
/// Documents kept open during a burst
const OPEN_DOCUMENTS: usize = 4;
/// Text layers kept in memory (a few KB each)
const TEXT_CACHE_ENTRIES: usize = 512;
/// Layouts kept in memory
const LAYOUT_CACHE_ENTRIES: usize = 32;

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum PdfReaderError {
	#[error("The page requested was not found in the media file")]
	PageNotFound,
	#[error("The request was dropped because the reader moved elsewhere")]
	Dropped,
	#[error("{0}")]
	Failed(String),
}

impl From<MediaProcessorError> for PdfReaderError {
	fn from(error: MediaProcessorError) -> Self {
		match error {
			MediaProcessorError::PageNotFound => PdfReaderError::PageNotFound,
			other => PdfReaderError::Failed(other.to_string()),
		}
	}
}

impl From<std::io::Error> for PdfReaderError {
	fn from(error: std::io::Error) -> Self {
		PdfReaderError::Failed(error.to_string())
	}
}

/// A page picture
#[derive(Debug, Clone)]
pub struct PagePicture {
	pub bytes: Arc<Vec<u8>>,
	pub tier: u32,
	pub width: u32,
	pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum JobKind {
	Layout,
	Picture { page: i32, tier: u32 },
	Text { page: i32 },
	Legacy { page: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct JobKey {
	path: PathBuf,
	version: String,
	kind: JobKind,
}

#[derive(Debug, Clone)]
enum JobOutput {
	Layout(Arc<PdfLayout>),
	Picture(PagePicture),
	Text(Arc<PageText>),
	Legacy(ContentType, Arc<Vec<u8>>),
}

type JobResult = Result<JobOutput, PdfReaderError>;
type Waiter = oneshot::Sender<JobResult>;

struct Shared {
	scheduler: Mutex<Scheduler<JobKey, Waiter>>,
	wake: Condvar,
}

/// Small LRU over a `VecDeque` (the maps here hold at most a few hundred entries)
struct Lru<K, V> {
	capacity: usize,
	entries: VecDeque<(K, V)>,
}

impl<K: PartialEq, V: Clone> Lru<K, V> {
	fn new(capacity: usize) -> Self {
		Self {
			capacity,
			entries: VecDeque::new(),
		}
	}

	fn get(&mut self, key: &K) -> Option<V> {
		let position = self.entries.iter().position(|(k, _)| k == key)?;
		let entry = self.entries.remove(position)?;
		let value = entry.1.clone();
		self.entries.push_front(entry);
		Some(value)
	}

	fn put(&mut self, key: K, value: V) {
		if let Some(position) = self.entries.iter().position(|(k, _)| *k == key) {
			self.entries.remove(position);
		}
		self.entries.push_front((key, value));
		self.entries.truncate(self.capacity);
	}
}

pub struct PdfReaderService {
	shared: Arc<Shared>,
	config: Arc<StumpConfig>,
	cache: PageCache,
	texts: Mutex<Lru<(String, i32), Arc<PageText>>>,
	layouts: Mutex<Lru<String, Arc<PdfLayout>>>,
}

static SERVICE: OnceLock<PdfReaderService> = OnceLock::new();

/// The document version: changes when the file changes (path, size, modification time)
pub fn document_version(path: &Path) -> Result<String, PdfReaderError> {
	let metadata = std::fs::metadata(path).map_err(|_| {
		PdfReaderError::Failed(format!("The file {} is missing", path.display()))
	})?;
	let modified = metadata
		.modified()
		.ok()
		.and_then(|time| time.duration_since(UNIX_EPOCH).ok())
		.map(|duration| duration.as_secs())
		.unwrap_or_default();
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	path.hash(&mut hasher);
	metadata.len().hash(&mut hasher);
	modified.hash(&mut hasher);
	Ok(format!("{:016x}", hasher.finish()))
}

impl PdfReaderService {
	/// The process-wide service, started on first use
	pub fn global(config: &StumpConfig) -> &'static PdfReaderService {
		SERVICE.get_or_init(|| PdfReaderService::start(config.clone()))
	}

	fn start(config: StumpConfig) -> Self {
		let cache = PageCache::new(
			config.pdf_cache_directory(),
			config.pdf_cache_max_mb.saturating_mul(1024 * 1024),
		);
		cache.start_background_scan();
		let shared = Arc::new(Shared {
			scheduler: Mutex::new(Scheduler::default()),
			wake: Condvar::new(),
		});
		let config = Arc::new(config);
		let worker_shared = shared.clone();
		let worker_config = config.clone();
		std::thread::Builder::new()
			.name("pdf-render".to_string())
			.spawn(move || worker_loop(worker_shared, worker_config))
			.expect("failed to start the PDF render thread");
		Self {
			shared,
			config,
			cache,
			texts: Mutex::new(Lru::new(TEXT_CACHE_ENTRIES)),
			layouts: Mutex::new(Lru::new(LAYOUT_CACHE_ENTRIES)),
		}
	}

	pub fn cache(&self) -> &PageCache {
		&self.cache
	}

	pub fn max_tier(&self) -> u32 {
		self.config.pdf_max_tier
	}

	fn submit(
		&self,
		key: JobKey,
		priority: Priority,
		client: Option<ClientKey>,
		page: i32,
	) -> oneshot::Receiver<JobResult> {
		let (sender, receiver) = oneshot::channel();
		{
			let mut scheduler = self
				.shared
				.scheduler
				.lock()
				.unwrap_or_else(|poisoned| poisoned.into_inner());
			scheduler.submit(key, priority, client, page, sender);
		}
		self.shared.wake.notify_one();
		receiver
	}

	async fn run(
		&self,
		key: JobKey,
		priority: Priority,
		client: Option<ClientKey>,
		page: i32,
	) -> JobResult {
		self.submit(key, priority, client, page)
			.await
			.unwrap_or_else(|_| {
				Err(PdfReaderError::Failed(
					"The PDF render thread stopped".into(),
				))
			})
	}

	/// The document's layout: page sizes, outline, whether it has text, the allowed tiers
	pub async fn layout(
		&'static self,
		path: &Path,
	) -> Result<Arc<PdfLayout>, PdfReaderError> {
		self.layout_then(path, None).await
	}

	/// The layout, with the picture of the page the reader will show first queued right
	/// behind it (`first_page = (page, width, reader)`): the page request that follows the
	/// layout finds the picture in flight or cached instead of starting it.
	pub async fn layout_then(
		&'static self,
		path: &Path,
		first_page: Option<(i32, u32, ClientKey)>,
	) -> Result<Arc<PdfLayout>, PdfReaderError> {
		let version = document_version(path)?;
		let cached = self.layouts.lock().ok().and_then(|mut l| l.get(&version));
		// the layout job is queued first (synchronously), then the picture behind it
		let layout_job = match cached {
			Some(_) => None,
			None => Some(self.submit(
				JobKey {
					path: path.to_path_buf(),
					version: version.clone(),
					kind: JobKind::Layout,
				},
				Priority::Foreground,
				None,
				0,
			)),
		};
		if let Some((page, width, client)) = first_page {
			self.prepare_picture(path, page, width, client);
		}
		if let Some(layout) = cached {
			return Ok(layout);
		}
		let result = layout_job
			.expect("submitted above")
			.await
			.unwrap_or_else(|_| {
				Err(PdfReaderError::Failed(
					"The PDF render thread stopped".into(),
				))
			})?;
		match result {
			JobOutput::Layout(layout) => {
				if let Ok(mut layouts) = self.layouts.lock() {
					layouts.put(version, layout.clone());
				}
				Ok(layout)
			},
			_ => Err(PdfReaderError::Failed("unexpected render result".into())),
		}
	}

	/// Starts rendering a page the reader is about to show, at prefetch priority (behind any
	/// page a reader waits for) and counted as the reader's page, then caches it
	fn prepare_picture(
		&'static self,
		path: &Path,
		page: i32,
		width: u32,
		client: ClientKey,
	) {
		if page < 1 {
			return;
		}
		let Ok(version) = document_version(path) else {
			return;
		};
		let tier = snap_tier(width, self.max_tier());
		let file_name = format!("pdfr_{version}_{page}_{tier}.webp");
		if self.cache.path_for(&file_name).exists() {
			return;
		}
		self.note_foreground(Some(client.clone()), page);
		let receiver = self.submit(
			JobKey {
				path: path.to_path_buf(),
				version,
				kind: JobKind::Picture { page, tier },
			},
			Priority::Prefetch,
			Some(client),
			page,
		);
		let cache = self.cache.clone();
		tokio::spawn(async move {
			if let Ok(Ok(JobOutput::Picture(picture))) = receiver.await {
				let _ = tokio::task::spawn_blocking(move || {
					cache.write(&file_name, &picture.bytes)
				})
				.await;
			}
		});
	}

	/// A page picture at the tier that covers `width` pixels
	pub async fn picture(
		&self,
		path: &Path,
		page: i32,
		width: u32,
		priority: Priority,
		client: Option<ClientKey>,
	) -> Result<PagePicture, PdfReaderError> {
		if page < 1 {
			return Err(PdfReaderError::PageNotFound);
		}
		let version = document_version(path)?;
		let tier = snap_tier(width, self.max_tier());
		let file_name = format!("pdfr_{version}_{page}_{tier}.webp");

		let cache = self.cache.clone();
		let cached_name = file_name.clone();
		let cached = tokio::task::spawn_blocking(move || cache.read(&cached_name))
			.await
			.ok()
			.flatten();
		if let Some(bytes) = cached {
			if priority == Priority::Foreground {
				// still tell the scheduler where the reader is, so stale prefetches drop
				self.note_foreground(client, page);
			}
			let (width, height) = imagesize::blob_size(&bytes)
				.map(|size| (size.width as u32, size.height as u32))
				.unwrap_or((tier, 0));
			return Ok(PagePicture {
				bytes: Arc::new(bytes),
				tier,
				width,
				height,
			});
		}

		let key = JobKey {
			path: path.to_path_buf(),
			version,
			kind: JobKind::Picture { page, tier },
		};
		match self.run(key, priority, client, page).await? {
			JobOutput::Picture(picture) => {
				let cache = self.cache.clone();
				let bytes = picture.bytes.clone();
				tokio::task::spawn_blocking(move || cache.write(&file_name, &bytes));
				Ok(picture)
			},
			_ => Err(PdfReaderError::Failed("unexpected render result".into())),
		}
	}

	fn note_foreground(&self, client: Option<ClientKey>, page: i32) {
		if client.is_none() {
			return;
		}
		if let Ok(mut scheduler) = self.shared.scheduler.lock() {
			scheduler.note_foreground(client, page);
		}
	}

	/// A page's text layer
	pub async fn text(
		&self,
		path: &Path,
		page: i32,
		priority: Priority,
	) -> Result<Arc<PageText>, PdfReaderError> {
		if page < 1 {
			return Err(PdfReaderError::PageNotFound);
		}
		let version = document_version(path)?;
		let cache_key = (version.clone(), page);
		if let Some(text) = self.texts.lock().ok().and_then(|mut t| t.get(&cache_key)) {
			return Ok(text);
		}
		let key = JobKey {
			path: path.to_path_buf(),
			version,
			kind: JobKind::Text { page },
		};
		match self.run(key, priority, None, page).await? {
			JobOutput::Text(text) => {
				if let Ok(mut texts) = self.texts.lock() {
					texts.put(cache_key, text.clone());
				}
				Ok(text)
			},
			_ => Err(PdfReaderError::Failed("unexpected render result".into())),
		}
	}

	/// The image reader's page (legacy route), rendered by this thread so it no longer fights
	/// the reader for PDFium
	pub async fn legacy_page(
		&self,
		path: &Path,
		page: i32,
		priority: Priority,
	) -> Result<(ContentType, Arc<Vec<u8>>), PdfReaderError> {
		if page < 1 {
			return Err(PdfReaderError::PageNotFound);
		}
		let version = document_version(path)?;
		let key = JobKey {
			path: path.to_path_buf(),
			version,
			kind: JobKind::Legacy { page },
		};
		match self.run(key, priority, None, page).await? {
			JobOutput::Legacy(content_type, bytes) => Ok((content_type, bytes)),
			_ => Err(PdfReaderError::Failed("unexpected render result".into())),
		}
	}
}

/// Documents opened during one burst, most recent first
struct OpenDocuments<'a> {
	documents: VecDeque<(PathBuf, String, PdfDocument<'a>)>,
}

impl<'a> OpenDocuments<'a> {
	fn get(
		&mut self,
		pdfium: &'a pdfium_render::prelude::Pdfium,
		path: &Path,
		version: &str,
	) -> Result<&PdfDocument<'a>, MediaProcessorError> {
		if let Some(position) = self
			.documents
			.iter()
			.position(|(p, v, _)| p == path && v == version)
		{
			let entry = self.documents.remove(position).expect("found above");
			self.documents.push_front(entry);
		} else {
			let document =
				pdfium.load_pdf_from_file(path.to_str().unwrap_or_default(), None)?;
			self.documents.push_front((
				path.to_path_buf(),
				version.to_string(),
				document,
			));
			self.documents.truncate(OPEN_DOCUMENTS);
		}
		Ok(&self.documents.front().expect("just pushed").2)
	}
}

fn worker_loop(shared: Arc<Shared>, config: Arc<StumpConfig>) {
	loop {
		// wait for work
		{
			let mut scheduler = shared
				.scheduler
				.lock()
				.unwrap_or_else(|poisoned| poisoned.into_inner());
			while scheduler.is_empty() {
				scheduler = shared
					.wake
					.wait(scheduler)
					.unwrap_or_else(|poisoned| poisoned.into_inner());
			}
		}

		let pdfium = match renderer(&config.pdfium_path) {
			Ok(pdfium) => pdfium,
			Err(error) => {
				// answer everything queued so callers don't hang
				let message = error.to_string();
				let mut scheduler = shared
					.scheduler
					.lock()
					.unwrap_or_else(|poisoned| poisoned.into_inner());
				while let Some(next) = scheduler.next() {
					let waiters = match next {
						Next::Run(key) => scheduler.complete(&key),
						Next::Drop(_, waiters) => waiters,
					};
					for waiter in waiters {
						let _ = waiter.send(Err(PdfReaderError::Failed(message.clone())));
					}
				}
				std::thread::sleep(Duration::from_secs(1));
				continue;
			},
		};

		let burst_started = Instant::now();
		let mut documents = OpenDocuments {
			documents: VecDeque::new(),
		};
		loop {
			let next = {
				let mut scheduler = shared
					.scheduler
					.lock()
					.unwrap_or_else(|poisoned| poisoned.into_inner());
				scheduler.next()
			};
			let key = match next {
				None => break,
				Some(Next::Drop(_, waiters)) => {
					for waiter in waiters {
						let _ = waiter.send(Err(PdfReaderError::Dropped));
					}
					continue;
				},
				Some(Next::Run(key)) => key,
			};

			// skip work nobody waits for any more (e.g. a search that was cancelled)
			let abandoned = {
				let mut scheduler = shared
					.scheduler
					.lock()
					.unwrap_or_else(|poisoned| poisoned.into_inner());
				scheduler.waiters_mut(&key).is_some_and(|waiters| {
					waiters.iter().all(|waiter| waiter.is_closed())
				})
			};
			let result = if abandoned {
				Err(PdfReaderError::Dropped)
			} else {
				let started = Instant::now();
				let result = run_job(&pdfium, &mut documents, &key, &config);
				tracing::trace!(?key, elapsed = ?started.elapsed(), ok = result.is_ok(), "PDF job done");
				result
			};

			let waiters = {
				let mut scheduler = shared
					.scheduler
					.lock()
					.unwrap_or_else(|poisoned| poisoned.into_inner());
				scheduler.complete(&key)
			};
			for waiter in waiters {
				let _ = waiter.send(result.clone());
			}

			if burst_started.elapsed() > BURST {
				break;
			}
		}
		drop(documents);
		drop(pdfium);
		// let other PDFium users (scans, thumbnails) take the lock between bursts
		std::thread::sleep(Duration::from_millis(2));
	}
}

fn run_job<'a>(
	pdfium: &'a pdfium_render::prelude::Pdfium,
	documents: &mut OpenDocuments<'a>,
	key: &JobKey,
	config: &StumpConfig,
) -> JobResult {
	let document = documents.get(pdfium, &key.path, &key.version)?;
	Ok(match key.kind {
		JobKind::Layout => JobOutput::Layout(Arc::new(engine::read_layout(
			document,
			&key.version,
			config.pdf_max_tier,
		)?)),
		JobKind::Picture { page, tier } => {
			let rendered = engine::render_tier(document, page, tier)?;
			JobOutput::Picture(PagePicture {
				bytes: Arc::new(rendered.bytes),
				tier,
				width: rendered.width,
				height: rendered.height,
			})
		},
		JobKind::Text { page } => {
			JobOutput::Text(Arc::new(engine::extract_text(document, page)?))
		},
		JobKind::Legacy { page } => {
			let (content_type, bytes) = engine::render_legacy(document, page, config)?;
			JobOutput::Legacy(content_type, Arc::new(bytes))
		},
	})
}

/// Search results for one call of `GET /api/v2/pdf/{id}/search`
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
	pub results: Vec<SearchHit>,
	pub next_cursor: Option<String>,
}

/// Searches pages from `start_page` on until `limit` hits, the end of the book or `budget`
/// time. Stops early when `cancelled` returns true.
pub async fn search_document(
	service: &PdfReaderService,
	path: &Path,
	page_count: i32,
	query: &str,
	start_page: i32,
	limit: usize,
	budget: Duration,
) -> Result<SearchPage, PdfReaderError> {
	let folded = fold_query(query);
	let started = Instant::now();
	let mut results = Vec::new();
	let mut page = start_page.max(1);
	// whole pages only, so the cursor never splits a page's hits (`limit` is a soft cap)
	while page <= page_count {
		let text = match service.text(path, page, Priority::Background).await {
			Ok(text) => text,
			Err(PdfReaderError::PageNotFound) => break,
			Err(error) => return Err(error),
		};
		results.extend(search_page(page, &text, &folded));
		page += 1;
		if results.len() >= limit || started.elapsed() > budget {
			break;
		}
	}
	let next_cursor = (page <= page_count).then(|| page.to_string());
	Ok(SearchPage {
		results,
		next_cursor,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lru_keeps_the_most_recent_entries() {
		let mut lru = Lru::new(2);
		lru.put("a", 1);
		lru.put("b", 2);
		assert_eq!(lru.get(&"a"), Some(1));
		lru.put("c", 3);
		assert_eq!(lru.get(&"b"), None, "b was the least recently used");
		assert_eq!(lru.get(&"a"), Some(1));
		assert_eq!(lru.get(&"c"), Some(3));
	}

	#[test]
	fn versions_change_with_the_file() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("a.pdf");
		std::fs::write(&path, b"one").unwrap();
		let first = document_version(&path).unwrap();
		assert_eq!(first.len(), 16);
		std::fs::write(&path, b"three").unwrap();
		assert_ne!(first, document_version(&path).unwrap());
		assert!(document_version(&dir.path().join("missing.pdf")).is_err());
	}
}
