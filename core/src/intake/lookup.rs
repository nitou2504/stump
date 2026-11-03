//! On-demand lookups on Open Library, Google Books and Hardcover. Short timeouts, a polite
//! User-Agent and a small in-memory cache. Google Books answers keyless requests from a quota
//! shared by every anonymous caller, which is often used up: an optional API key (a server
//! setting, see [`google_books_key`]) gives the server its own. Hardcover is asked only when its
//! metadata provider (Settings → Metadata) is enabled with an API token, through that
//! provider's client and within its rate limit. Each source's answer is reported
//! ([`OnlineLookup`]: found with N subjects, no subjects, no match, rate-limited, error, not
//! configured), so a lookup that failed is never taken for one that found nothing.
//!
//! Matching (at most 3 requests per source and book, all cached): the title is searched
//! without its edition and format suffixes (`. Versión ilustrada`, `(Ed. Ilustrada)`,
//! `(Spanish Edition)`, `vNN` / `Vol. N` for series volumes), with its author; a manga volume is
//! searched by its series; then the main title (before `:`), or the original title of a
//! translated book when its file has one. Open Library is searched as a whole (`q=`), which
//! finds the work behind a translated edition (`El nombre de la rosa` → *Il nome della rosa*,
//! matched through its edition titles); among the works that agree, the one whose author
//! matches and with the most editions wins, and when it has no subjects in the search its work
//! record (or one of its editions) is asked (subjects, people, places, times). Hardcover is
//! searched by ISBN, else by title and author; the book that agrees (its title or one of its
//! alternative titles) with the most readers wins, and its record gives the genres and tags its
//! readers gave it, weighted by their votes (see [`hardcover_subjects`]), its description and
//! its series.

use std::{
	collections::{hash_map::DefaultHasher, HashMap},
	hash::{Hash, Hasher},
	sync::{Arc, LazyLock, Mutex},
	time::{Duration, Instant},
};

use metadata_integrations::{HardcoverClient, MetadataProviderError};
use models::{
	entity::{metadata_provider_config, server_config},
	shared::{
		enums::MetadataProvider,
		intake::{IntakeSource, OnlineLookup, OnlineLookupStatus},
	},
};
use regex::Regex;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{json, Value};

use super::naming::{language_code, name_keys, name_tokens};
use crate::utils::encryption::decrypt_string;

const USER_AGENT: &str = "Stump-intake/1.0 (self-hosted library server)";
const TIMEOUT: Duration = Duration::from_secs(6);
/// Answers are kept a day; refusals and HTTP errors only a few minutes
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const ERROR_TTL: Duration = Duration::from_secs(10 * 60);
const CACHE_MAX: usize = 256;
/// How long Hardcover may take to answer a request, and the most a request may wait for the
/// rate limiter and its answer together
const HARDCOVER_TIMEOUT: Duration = Duration::from_secs(8);
const HARDCOVER_DEADLINE: Duration = Duration::from_secs(20);
/// After a refusal that doesn't say how long to wait, Hardcover isn't asked for a minute
const HARDCOVER_DEFAULT_WAIT: Duration = Duration::from_secs(60);

/// What an online source knows about a book
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OnlineRecord {
	pub source: Option<IntakeSource>,
	pub title: Option<String>,
	pub authors: Vec<String>,
	pub first_publish_year: Option<i32>,
	pub language: Option<String>,
	pub publisher: Option<String>,
	pub description: Option<String>,
	pub subjects: Vec<String>,
	/// How much a subject counts (0-1) when the source weighs it (Hardcover: by how many of
	/// its readers gave it); the others count fully
	pub subject_weights: HashMap<String, f64>,
	pub isbn: Option<String>,
	/// Hardcover: the book's series and its place in it
	pub series: Option<String>,
	pub series_index: Option<f64>,
	/// Open Library: the work (`/works/OL...W`) and one edition (`OL...M`) the record came from
	pub work_key: Option<String>,
	pub edition_key: Option<String>,
	/// Hardcover: the book's id
	pub hardcover_id: Option<i64>,
	/// The titles the book also goes by: Open Library's matched edition titles (a translated
	/// edition's title, when the work is known by its original one), Hardcover's alternative
	/// titles
	pub edition_titles: Vec<String>,
	/// Open Library: its editions; Hardcover: its readers (the canonical book has the most)
	pub edition_count: i64,
}

/// What to look a book up by
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LookupQuery {
	pub isbn: Option<String>,
	pub title: Option<String>,
	pub authors: Vec<String>,
	/// A translated book's original title (from its file), tried when its title finds nothing
	pub original_title: Option<String>,
	/// The series of a volume (manga, light novels): searched instead of "Series vNN"
	pub series: Option<String>,
	/// Search the series rather than the title (a comic or manga volume)
	pub by_series: bool,
}

/// A title without the edition and format words around it (`. Versión ilustrada`,
/// `(Ed. Ilustrada)`, `(Spanish Edition)`, `Illustrated edition`) and without a trailing volume
/// number (`v03`, `Vol. 3`, `Tomo 3`, `#3`)
pub fn normalize_title(title: &str) -> String {
	static RULES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
		let words = r"ed\.?|edici[oó]n|edition|editions|versi[oó]n|version|illustrated|ilustrad[oa]|annotated|anotad[oa]|spanish|english|espa[nñ]ol|unabridged|abridged|biling[uü]e|bilingual|kindle|ebook|epub|tapa blanda|paperback|hardcover|novela gr[aá]fica";
		[
			// a bracketed part that names an edition or a format: (Ed. Ilustrada), [Spanish Edition]
			format!(r"(?i)\s*[\(\[][^\)\]]*\b(?:{words})\b[^\)\]]*[\)\]]\s*$"),
			// . Versión ilustrada, - Illustrated edition, : edición anotada
			format!(r"(?i)\s*[\.\-–—:,;/]\s*(?:{words})(?:\s+(?:{words}|\w+)){{0,2}}\s*$"),
			// a bare trailing edition phrase: ed ilustrada, illustrated edition
			r"(?i)\s+(?:ed\.?|edici[oó]n|versi[oó]n)\s+(?:ilustrad[oa]|anotad[oa]|completa|definitiva|especial|revisada)\s*$".to_string(),
			r"(?i)\s+(?:illustrated|annotated|unabridged)\s+edition\s*$".to_string(),
			// a volume number
			r"(?i)[\s,:\-–]*\b(?:v|vol\.?|volume|volumen|tomo|t\.)\s*\d+(?:\.\d+)?\s*$".to_string(),
			r"(?i)\s*#\s*\d+(?:\.\d+)?\s*$".to_string(),
		]
		.iter()
		.map(|rule| Regex::new(rule).expect("valid regex"))
		.collect()
	});
	let mut out = title.trim().to_string();
	loop {
		let before = out.clone();
		for rule in RULES.iter() {
			out = rule.replace(&out, "").trim().to_string();
		}
		if out == before || out.is_empty() {
			break;
		}
	}
	if out.is_empty() {
		title.trim().to_string()
	} else {
		out
	}
}

/// The titles a book is searched by, best first (at most two): its normalised title (or its
/// series, for a volume), then its original title (a translation), else its main title
/// (before `:`)
pub fn query_titles(query: &LookupQuery) -> Vec<String> {
	let mut titles: Vec<String> = vec![];
	let mut push = |title: Option<String>| {
		if let Some(title) = title
			.map(|t| t.trim().to_string())
			.filter(|t| !t.is_empty())
		{
			if !titles.iter().any(|t| name_tokens(t) == name_tokens(&title)) {
				titles.push(title);
			}
		}
	};
	let series = query
		.series
		.as_deref()
		.map(normalize_title)
		.filter(|s| !s.is_empty());
	if query.by_series {
		push(series.clone());
	}
	let title = query.title.as_deref().map(normalize_title);
	push(title.clone());
	push(query.original_title.as_deref().map(normalize_title));
	// the main title: before a subtitle (`:`) or a closing parenthetical (a series note)
	push(
		title
			.as_deref()
			.and_then(|t| {
				t.split_once(':').map(|(main, _)| main).or_else(|| {
					t.strip_suffix(')')
						.and_then(|t| t.rsplit_once(" ("))
						.map(|(main, _)| main)
				})
			})
			.map(str::to_string)
			.filter(|main| main.trim().chars().count() >= 4),
	);
	titles.truncate(2);
	titles
}

/// The original title a translated EPUB names in its OPF, if any: a calibre custom column
/// whose name says "original" (`#titulo_original`, `#original_title`), or an
/// `original-title` meta
pub fn original_title_from_opf(opf: &str) -> Option<String> {
	static META: LazyLock<Regex> = LazyLock::new(|| {
		Regex::new(r#"(?is)<meta\b[^>]*\bname=["']([^"']*original[^"']*)["'][^>]*\bcontent=["']([^"']*)["']"#)
			.expect("valid regex")
	});
	static PROPERTY: LazyLock<Regex> = LazyLock::new(|| {
		Regex::new(
			r#"(?is)<meta\b[^>]*\bproperty=["'][^"']*original[-_ ]?title["'][^>]*>([^<]+)</meta>"#,
		)
		.expect("valid regex")
	});
	let unescape = |s: &str| super::write::html_unescape(s).trim().to_string();
	for caps in META.captures_iter(opf) {
		let content = unescape(&caps[2]);
		// calibre keeps custom columns as JSON with the value in "#value#"
		let value = if content.starts_with('{') {
			serde_json::from_str::<Value>(&content).ok().and_then(|v| {
				v.get("#value#").and_then(Value::as_str).map(str::to_string)
			})
		} else {
			Some(content)
		};
		if let Some(value) = value
			.map(|v| v.trim().to_string())
			.filter(|v| !v.is_empty())
		{
			return Some(value);
		}
	}
	PROPERTY
		.captures(opf)
		.map(|caps| unescape(&caps[1]))
		.filter(|v| !v.is_empty())
}

/// The original title in an EPUB file's OPF (see [`original_title_from_opf`])
pub fn original_title_of(path: &std::path::Path) -> Option<String> {
	let mut archive = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
	let name = super::write::opf_path(&mut archive).ok()?;
	let mut opf = String::new();
	std::io::Read::read_to_string(&mut archive.by_name(&name).ok()?, &mut opf).ok()?;
	original_title_from_opf(&opf)
}

/// The records found and how each source answered
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LookupOutcome {
	pub records: Vec<OnlineRecord>,
	pub sources: Vec<OnlineLookup>,
}

impl LookupOutcome {
	/// Whether a source found the book (a failed lookup, or one with no match, isn't one)
	pub fn found(&self) -> bool {
		self.sources.iter().any(|s| {
			matches!(
				s.status,
				OnlineLookupStatus::Ok | OnlineLookupStatus::NoSubjects
			)
		})
	}

	/// One line per source that couldn't help, for the review screens' notes
	pub fn notes(&self) -> Vec<String> {
		self.sources
			.iter()
			.filter(|s| {
				matches!(
					s.status,
					OnlineLookupStatus::RateLimited | OnlineLookupStatus::Error
				)
			})
			.map(|s| format!("{}: {}", source_name(s.source), s.message))
			.collect()
	}
}

pub fn source_name(source: IntakeSource) -> &'static str {
	match source {
		IntakeSource::OpenLibrary => "Open Library",
		IntakeSource::GoogleBooks => "Google Books",
		IntakeSource::Hardcover => "Hardcover",
		_ => "Online",
	}
}

/// How a request went
#[derive(Debug, Clone, PartialEq)]
pub enum Fetched {
	Json(Value),
	/// An HTTP error, with its body when it was JSON
	Status(u16, Option<Value>),
	/// No answer (network, timeout, unreadable body)
	Failed(String),
}

/// Something that GETs JSON and asks Hardcover: the real HTTP clients, or canned answers in
/// tests
#[async_trait::async_trait]
pub trait Fetch: Send + Sync {
	async fn get(&self, url: &str) -> Fetched;

	/// Asks Hardcover's GraphQL API with the provider's token: the body when it answered
	/// with data, else its refusal (HTTP status and body, GraphQL errors included)
	async fn hardcover_graphql(&self, query: &str, variables: &Value) -> Fetched;
}

/// The URL without its API key, for logs
fn redacted(url: &str) -> String {
	match url.find("&key=") {
		Some(at) => {
			let rest = &url[at + 5..];
			let end = rest.find('&').map(|i| at + 5 + i).unwrap_or(url.len());
			format!("{}&key=…{}", &url[..at], &url[end..])
		},
		None => url.to_string(),
	}
}

/// The HTTP client with the in-memory cache, and the Hardcover provider's client when it is
/// set up
#[derive(Default)]
pub struct HttpFetch {
	hardcover: Option<HardcoverHandle>,
}

/// The Hardcover client for lookups and a fingerprint of its token, which keys its cached
/// answers (an answer to one token is never reused for another)
#[derive(Clone)]
struct HardcoverHandle {
	fingerprint: u64,
	client: Arc<HardcoverClient>,
}

/// When an answer was kept, for how long, and the answer
type Cached = (Instant, Duration, Fetched);

static CACHE: LazyLock<Mutex<HashMap<String, Cached>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

static CLIENT: LazyLock<Option<reqwest::Client>> = LazyLock::new(|| {
	reqwest::Client::builder()
		.user_agent(USER_AGENT)
		.timeout(TIMEOUT)
		.build()
		.map_err(|error| tracing::warn!(?error, "Failed to build the lookup HTTP client"))
		.ok()
});

fn cached(key: &str) -> Option<Fetched> {
	let cache = CACHE.lock().ok()?;
	let (at, ttl, value) = cache.get(key)?;
	(at.elapsed() < *ttl).then(|| value.clone())
}

fn keep(key: &str, ttl: Duration, fetched: &Fetched) {
	if let Ok(mut cache) = CACHE.lock() {
		if cache.len() >= CACHE_MAX {
			if let Some(oldest) = cache
				.iter()
				.min_by_key(|(_, (at, _, _))| *at)
				.map(|(k, _)| k.clone())
			{
				cache.remove(&oldest);
			}
		}
		cache.insert(key.to_string(), (Instant::now(), ttl, fetched.clone()));
	}
}

/// Until when Hardcover asked not to be called with a token (its `Retry-After` on a
/// refusal): lookups until then report the refusal without asking
static HARDCOVER_WAIT: Mutex<Option<(u64, Instant)>> = Mutex::new(None);

fn hardcover_wait(fingerprint: u64) -> Option<Duration> {
	let (token, until) = (*HARDCOVER_WAIT.lock().ok()?)?;
	(token == fingerprint)
		.then(|| until.checked_duration_since(Instant::now()))
		.flatten()
		.filter(|wait| !wait.is_zero())
}

fn wait_for_hardcover(fingerprint: u64, wait: Duration) {
	if let Ok(mut until) = HARDCOVER_WAIT.lock() {
		*until = Some((fingerprint, Instant::now() + wait.min(CACHE_TTL)));
	}
}

/// Hardcover's refusal, in its words, for a request not sent while it asks to wait
fn wait_answer(wait: Duration) -> Fetched {
	Fetched::Status(
		429,
		Some(json!({
			"error": "Too Many Requests",
			"message": format!("Try again in {} seconds.", wait.as_secs().max(1)),
		})),
	)
}

#[async_trait::async_trait]
impl Fetch for HttpFetch {
	async fn get(&self, url: &str) -> Fetched {
		if let Some(answer) = cached(url) {
			return answer;
		}
		let Some(client) = CLIENT.as_ref() else {
			return Fetched::Failed("no HTTP client".to_string());
		};
		let fetched = match client.get(url).send().await {
			Ok(response) if response.status().is_success() => {
				match response.json::<Value>().await {
					Ok(json) => Fetched::Json(json),
					Err(error) => Fetched::Failed(format!("unreadable answer: {error}")),
				}
			},
			Ok(response) => {
				let status = response.status().as_u16();
				let body = response.json::<Value>().await.ok();
				tracing::debug!(url = %redacted(url), status, "Lookup answered with an error");
				Fetched::Status(status, body)
			},
			Err(error) => {
				tracing::debug!(url = %redacted(url), ?error, "Lookup failed");
				Fetched::Failed(if error.is_timeout() {
					"no answer in time".to_string()
				} else {
					"unreachable".to_string()
				})
			},
		};
		match &fetched {
			Fetched::Json(_) => keep(url, CACHE_TTL, &fetched),
			Fetched::Status(..) => keep(url, ERROR_TTL, &fetched),
			// a network failure is tried again next time
			Fetched::Failed(_) => {},
		}
		fetched
	}

	async fn hardcover_graphql(&self, query: &str, variables: &Value) -> Fetched {
		let Some(handle) = &self.hardcover else {
			return Fetched::Failed("no HTTP client".to_string());
		};
		let key = format!("hardcover:{:x}\n{query}\n{variables}", handle.fingerprint);
		if let Some(answer) = cached(&key) {
			return answer;
		}
		if let Some(wait) = hardcover_wait(handle.fingerprint) {
			return wait_answer(wait);
		}
		let reply = tokio::time::timeout(
			HARDCOVER_DEADLINE,
			handle.client.post_graphql(query, variables.clone()),
		)
		.await;
		let fetched = match reply {
			Err(_) => Fetched::Failed("no answer in time".to_string()),
			Ok(Err(error)) => {
				tracing::debug!(%error, "Hardcover lookup failed");
				let timeout = matches!(
					&error,
					MetadataProviderError::MiddlewareReqwestError(e) if e.is_timeout()
				);
				Fetched::Failed(
					if timeout {
						"no answer in time"
					} else {
						"unreachable"
					}
					.to_string(),
				)
			},
			Ok(Ok(reply)) if reply.status == 429 => {
				let wait = reply.retry_after.unwrap_or(HARDCOVER_DEFAULT_WAIT);
				tracing::debug!(seconds = wait.as_secs(), "Hardcover asked to wait");
				wait_for_hardcover(handle.fingerprint, wait);
				wait_answer(wait)
			},
			Ok(Ok(reply)) => {
				let success = (200..300).contains(&reply.status);
				match reply.body {
					Some(body)
						if success
							&& body.get("errors").is_none()
							&& body.get("data").is_some_and(|d| !d.is_null()) =>
					{
						Fetched::Json(body)
					},
					None if success => Fetched::Failed("unreadable answer".to_string()),
					// an HTTP error, or GraphQL errors instead of data
					body => {
						tracing::debug!(
							status = reply.status,
							"Hardcover answered with an error"
						);
						Fetched::Status(reply.status, body)
					},
				}
			},
		};
		match &fetched {
			Fetched::Json(_) => keep(&key, CACHE_TTL, &fetched),
			// a rate limit is kept as the wait above, for every request
			Fetched::Status(429, _) => {},
			Fetched::Status(..) => keep(&key, ERROR_TTL, &fetched),
			Fetched::Failed(_) => {},
		}
		fetched
	}
}

/// Whether Hardcover can be asked: its metadata provider (Settings → Metadata) is enabled
/// with an API token
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardcoverAccess {
	Ready,
	/// No Hardcover provider, or it is turned off
	NotConfigured,
	/// Enabled, without an API token (or with one that can't be decrypted)
	TokenMissing,
}

/// The Hardcover provider's client for lookups when it is enabled with an API token (decrypted
/// with the server's key, as the provider's own client does), else why not. One client per
/// token, so its rate limiter sees every lookup
async fn hardcover_client(
	conn: &DatabaseConnection,
) -> (HardcoverAccess, Option<HardcoverHandle>) {
	static LOOKUP_CLIENT: Mutex<Option<HardcoverHandle>> = Mutex::new(None);
	let config = metadata_provider_config::Entity::find()
		.all(conn)
		.await
		.unwrap_or_default()
		.into_iter()
		.find(|config| config.provider_type == MetadataProvider::Hardcover);
	let Some(config) = config.filter(|config| config.enabled) else {
		return (HardcoverAccess::NotConfigured, None);
	};
	let Some(encrypted) = config
		.encrypted_api_token
		.filter(|token| !token.trim().is_empty())
	else {
		return (HardcoverAccess::TokenMissing, None);
	};
	let key = server_config::Entity::find()
		.one(conn)
		.await
		.ok()
		.flatten()
		.and_then(|config| config.encryption_key);
	let token = match key.map(|key| decrypt_string(&encrypted, &key)) {
		Some(Ok(token)) => token,
		Some(Err(error)) => {
			tracing::warn!(?error, "The Hardcover API token couldn't be decrypted");
			return (HardcoverAccess::TokenMissing, None);
		},
		None => return (HardcoverAccess::TokenMissing, None),
	};
	// a token pasted with its scheme works too
	let token = token.trim();
	let token = token.strip_prefix("Bearer ").unwrap_or(token).trim();
	if token.is_empty() {
		return (HardcoverAccess::TokenMissing, None);
	}
	let fingerprint = {
		let mut hasher = DefaultHasher::new();
		token.hash(&mut hasher);
		hasher.finish()
	};
	let Ok(mut shared) = LOOKUP_CLIENT.lock() else {
		return (HardcoverAccess::Ready, None);
	};
	if let Some(handle) = shared.as_ref().filter(|h| h.fingerprint == fingerprint) {
		return (HardcoverAccess::Ready, Some(handle.clone()));
	}
	match HardcoverClient::for_lookups(token.to_string(), USER_AGENT, HARDCOVER_TIMEOUT) {
		Ok(client) => {
			let handle = HardcoverHandle {
				fingerprint,
				client: Arc::new(client),
			};
			*shared = Some(handle.clone());
			(HardcoverAccess::Ready, Some(handle))
		},
		Err(error) => {
			tracing::warn!(?error, "Failed to build the Hardcover lookup client");
			(HardcoverAccess::Ready, None)
		},
	}
}

/// The server's Google Books API key, if one is set (decrypted with the server's key)
pub async fn google_books_key(conn: &DatabaseConnection) -> Option<String> {
	let config = server_config::Entity::find()
		.one(conn)
		.await
		.ok()
		.flatten()?;
	let encrypted = config.google_books_api_key.filter(|k| !k.is_empty())?;
	let key = config.encryption_key?;
	match decrypt_string(&encrypted, &key) {
		Ok(api_key) => Some(api_key).filter(|k| !k.trim().is_empty()),
		Err(error) => {
			tracing::warn!(?error, "The Google Books API key couldn't be decrypted");
			None
		},
	}
}

fn str_field(value: &Value, key: &str) -> Option<String> {
	value
		.get(key)
		.and_then(Value::as_str)
		.map(str::trim)
		.filter(|s| !s.is_empty())
		.map(str::to_string)
}

fn str_list(value: &Value, key: &str) -> Vec<String> {
	value
		.get(key)
		.and_then(Value::as_array)
		.map(|items| {
			items
				.iter()
				.filter_map(|item| {
					// work records list some subjects as {"name": ...}
					item.as_str()
						.or_else(|| item.get("name").and_then(Value::as_str))
				})
				.map(|s| s.trim().to_string())
				.filter(|s| !s.is_empty())
				.collect()
		})
		.unwrap_or_default()
}

/// Words a title comparison skips: articles and conjunctions (`El crimen y el castigo` is
/// `Crimen y castigo`)
const TITLE_STOPWORDS: &[&str] = &[
	"a", "an", "and", "the", "of", "el", "la", "los", "las", "lo", "un", "una", "y", "e",
	"de", "del", "le", "les", "il", "l", "et", "der", "die", "das", "und",
];

/// Whether an online title is the same book as ours: main titles compared (subtitles off),
/// without articles, loosely; a bilingual title (`El Principito / The Little Prince`) agrees
/// when one of its sides does
pub fn titles_agree(ours: &str, theirs: &str) -> bool {
	let main = |t: &str| -> String {
		name_tokens(t.split([':', '(']).next().unwrap_or(t))
			.into_iter()
			.filter(|w| !TITLE_STOPWORDS.contains(&w.as_str()))
			.collect::<Vec<_>>()
			.join(" ")
	};
	let a = main(ours);
	if a.is_empty() {
		return false;
	}
	theirs.split(" / ").any(|side| {
		let b = main(side);
		!b.is_empty() && (a == b || strsim::jaro_winkler(&a, &b) >= 0.9)
	})
}

/// Whether two names are the same person: the same name keys (in any order, loosely), or
/// the same surname with compatible first initials (`Michael Cook` and `M. A. Cook`)
fn same_person(ours: &str, theirs: &str) -> bool {
	let (a, b) = (name_keys(ours).sorted, name_keys(theirs).sorted);
	if a == b || strsim::jaro_winkler(&a, &b) >= 0.92 {
		return true;
	}
	let tokens = |n: &str| name_tokens(&super::naming::reorder_last_first(n));
	let (ta, tb) = (tokens(ours), tokens(theirs));
	match (ta.split_last(), tb.split_last()) {
		(Some((sa, ra)), Some((sb, rb))) if sa == sb && sa.len() > 2 => {
			match (ra.first(), rb.first()) {
				(Some(fa), Some(fb)) => fa.chars().next() == fb.chars().next(),
				_ => false,
			}
		},
		_ => false,
	}
}

fn authors_agree(ours: &[String], theirs: &[String]) -> bool {
	if ours.is_empty() || theirs.is_empty() {
		return true;
	}
	ours.iter()
		.any(|a| theirs.iter().any(|t| same_person(a, t)))
}

const OPEN_LIBRARY_FIELDS: &str = "key,title,author_name,first_publish_year,subject,language,publisher,isbn,edition_key,cover_edition_key,edition_count,editions";

/// Reads one Open Library search document
pub fn parse_open_library_doc(doc: &Value) -> OnlineRecord {
	OnlineRecord {
		source: Some(IntakeSource::OpenLibrary),
		title: str_field(doc, "title"),
		authors: str_list(doc, "author_name"),
		first_publish_year: doc
			.get("first_publish_year")
			.and_then(Value::as_i64)
			.map(|y| y as i32),
		language: str_list(doc, "language")
			.first()
			.and_then(|l| language_code(l)),
		publisher: str_list(doc, "publisher").into_iter().next(),
		description: None,
		subjects: str_list(doc, "subject").into_iter().take(25).collect(),
		isbn: str_list(doc, "isbn").into_iter().next(),
		work_key: str_field(doc, "key").filter(|k| k.starts_with("/works/")),
		edition_key: str_field(doc, "cover_edition_key")
			.or_else(|| str_list(doc, "edition_key").into_iter().next()),
		edition_titles: doc
			.get("editions")
			.and_then(|e| e.get("docs"))
			.and_then(Value::as_array)
			.map(|docs| docs.iter().filter_map(|d| str_field(d, "title")).collect())
			.unwrap_or_default(),
		edition_count: doc
			.get("edition_count")
			.and_then(Value::as_i64)
			.unwrap_or(0),
		..Default::default()
	}
}

/// The subjects of an Open Library work or edition record: its subjects, then the people,
/// places and times it is about
pub fn open_library_record_subjects(record: &Value) -> Vec<String> {
	let mut subjects = vec![];
	for key in [
		"subjects",
		"subject_people",
		"subject_places",
		"subject_times",
	] {
		for subject in str_list(record, key) {
			if !subjects.contains(&subject) {
				subjects.push(subject);
			}
		}
	}
	subjects.into_iter().take(25).collect()
}

/// An Open Library description: a string, or `{"type": ..., "value": ...}`
fn open_library_description(record: &Value) -> Option<String> {
	let description = record.get("description")?;
	description
		.as_str()
		.or_else(|| description.get("value").and_then(Value::as_str))
		.map(strip_html)
		.filter(|d| !d.is_empty())
}

/// Reads one Google Books volume
pub fn parse_google_volume(item: &Value) -> OnlineRecord {
	let info = item.get("volumeInfo").cloned().unwrap_or(Value::Null);
	let isbn = info
		.get("industryIdentifiers")
		.and_then(Value::as_array)
		.and_then(|ids| {
			ids.iter()
				.find(|id| id.get("type").and_then(Value::as_str) == Some("ISBN_13"))
				.or_else(|| ids.first())
		})
		.and_then(|id| str_field(id, "identifier"));
	OnlineRecord {
		source: Some(IntakeSource::GoogleBooks),
		title: str_field(&info, "title"),
		authors: str_list(&info, "authors"),
		first_publish_year: str_field(&info, "publishedDate")
			.and_then(|d| d.get(..4).and_then(|y| y.parse().ok())),
		language: str_field(&info, "language").and_then(|l| language_code(&l)),
		publisher: str_field(&info, "publisher"),
		description: str_field(&info, "description").map(|d| strip_html(&d)),
		subjects: str_list(&info, "categories"),
		isbn,
		..Default::default()
	}
}

pub(crate) fn strip_html(text: &str) -> String {
	let mut out = String::with_capacity(text.len());
	let mut in_tag = false;
	for c in text.chars() {
		match c {
			'<' => in_tag = true,
			'>' => {
				in_tag = false;
				out.push(' ');
			},
			c if !in_tag => out.push(c),
			_ => {},
		}
	}
	out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Reads one Hardcover search hit (`results.hits[].document`): its alternative titles match
/// like Open Library's edition titles, its readers rank it like editions do, and its genres
/// stand for its subjects until the book's own tags are read (how many readers gave each
/// isn't in the search, so they count a little less)
pub fn parse_hardcover_document(doc: &Value) -> OnlineRecord {
	let genres: Vec<String> = str_list(doc, "genres").into_iter().take(12).collect();
	OnlineRecord {
		source: Some(IntakeSource::Hardcover),
		title: str_field(doc, "title"),
		authors: str_list(doc, "author_names"),
		description: str_field(doc, "description")
			.map(|d| strip_html(&d))
			.filter(|d| !d.is_empty()),
		subject_weights: genres.iter().map(|g| (g.clone(), 0.8)).collect(),
		subjects: genres,
		edition_titles: str_list(doc, "alternative_titles"),
		edition_count: doc.get("users_count").and_then(Value::as_i64).unwrap_or(0),
		hardcover_id: doc.get("id").and_then(|id| {
			id.as_i64()
				.or_else(|| id.as_str().and_then(|s| s.trim().parse().ok()))
		}),
		..Default::default()
	}
}

/// Hardcover tags about where a reader keeps a book or how they read it, not what it is
const HARDCOVER_SHELF_TAGS: &[&str] = &[
	"owned",
	"own",
	"to read",
	"tbr",
	"want to read",
	"currently reading",
	"read",
	"unread",
	"reread",
	"dnf",
	"did not finish",
	"favorite",
	"favorites",
	"favourites",
	"wishlist",
	"library",
	"kindle",
	"kindle unlimited",
	"ebook",
	"e book",
	"audiobook",
	"audiobooks",
	"audible",
	"paperback",
	"hardcover",
	"physical",
	"book club",
	"netgalley",
	"arc",
];

/// The genres and tags Hardcover's readers gave a book (`cached_tags`), as subjects with a
/// weight from their votes: the genres, then the other tags (moods and content warnings are
/// left out, and so are shelf tags like `Owned` or `Kindle`). Each counts by its votes against
/// the most voted of its kind, from 0.5 to 1 for a genre and 0.4 to 0.8 for a tag; a genre
/// with under a tenth of the top genre's votes, or a tag with under a quarter of the top
/// tag's, is left out
pub fn hardcover_subjects(cached_tags: &Value) -> Vec<(String, f64)> {
	// a jsonb column may come as text
	let parsed;
	let tags = match cached_tags {
		Value::String(text) => {
			parsed = serde_json::from_str::<Value>(text).unwrap_or(Value::Null);
			&parsed
		},
		other => other,
	};
	let mut out: Vec<(String, f64)> = vec![];
	for (category, floor, scale, most) in [("Genre", 0.1, 1.0, 12), ("Tag", 0.25, 0.8, 8)]
	{
		let mut items: Vec<(String, f64)> = tags
			.get(category)
			.and_then(Value::as_array)
			.map(|items| {
				items
					.iter()
					.filter_map(|item| {
						let tag = item.get("tag").and_then(Value::as_str)?.trim();
						let count =
							item.get("count").and_then(Value::as_f64).unwrap_or(1.0);
						(!tag.is_empty()).then(|| (tag.to_string(), count.max(1.0)))
					})
					.collect()
			})
			.unwrap_or_default();
		if category == "Tag" {
			items.retain(|(tag, _)| {
				let tag = super::library_index::norm(tag);
				!HARDCOVER_SHELF_TAGS.contains(&tag.as_str())
					&& !tag.chars().all(|c| c.is_ascii_digit() || c == ' ')
			});
		}
		// most votes first (ties keep Hardcover's order)
		items.sort_by(|a, b| b.1.total_cmp(&a.1));
		let top = items.first().map(|(_, count)| *count).unwrap_or(1.0);
		for (tag, count) in items.into_iter().take(most) {
			let share = count / top;
			if share < floor {
				break;
			}
			if !out.iter().any(|(t, _)| t.eq_ignore_ascii_case(&tag)) {
				out.push((tag, scale * (0.5 + 0.5 * share)));
			}
		}
	}
	out
}

/// Reads Hardcover's book record (`books[0]`): its readers' weighted tags replace the search's
/// genres, and it brings its description and featured series
fn apply_hardcover_book(record: &mut OnlineRecord, book: &Value) {
	let subjects = book
		.get("cached_tags")
		.map(hardcover_subjects)
		.unwrap_or_default();
	if !subjects.is_empty() {
		record.subjects = subjects.iter().map(|(s, _)| s.clone()).collect();
		record.subject_weights = subjects.into_iter().filter(|(_, w)| *w < 1.0).collect();
	}
	if let Some(description) = str_field(book, "description")
		.map(|d| strip_html(&d))
		.filter(|d| !d.is_empty())
	{
		record.description = Some(description);
	}
	if let Some(featured) = book.get("featured_book_series") {
		record.series = featured
			.get("series")
			.and_then(|series| str_field(series, "name"));
		record.series_index = record
			.series
			.as_ref()
			.and(featured.get("position").and_then(Value::as_f64));
	}
}

/// Why Hardcover refused: its rate limit (and how long it asks to wait), a refused token, or a
/// GraphQL error
fn hardcover_refusal(status: u16, body: Option<&Value>) -> (OnlineLookupStatus, String) {
	static WAIT: LazyLock<Regex> = LazyLock::new(|| {
		Regex::new(r"(?i)try again in (\d+) seconds?").expect("valid regex")
	});
	let text = |key: &str| {
		body.and_then(|b| b.get(key))
			.and_then(Value::as_str)
			.unwrap_or_default()
			.to_string()
	};
	let errors: Vec<String> = body
		.and_then(|b| b.get("errors"))
		.and_then(Value::as_array)
		.map(|errors| {
			errors
				.iter()
				.filter_map(|e| e.get("message").and_then(Value::as_str))
				.map(str::to_string)
				.collect()
		})
		.unwrap_or_default();
	let said = format!(
		"{} {} {} {}",
		text("error"),
		text("message"),
		text("error_description"),
		errors.join(" ")
	)
	.to_lowercase();
	if status == 429
		|| said.contains("rate limit")
		|| said.contains("too many requests")
		|| said.contains("throttl")
	{
		let wait = WAIT
			.captures(&said)
			.and_then(|caps| caps[1].parse::<u64>().ok());
		let message = match wait {
			Some(seconds) if seconds >= 60 * 60 => format!(
				"daily limit reached: try again in about {} h",
				(seconds + 30 * 60) / (60 * 60)
			),
			Some(seconds) if seconds > 90 => format!(
				"rate limit reached: try again in {} min",
				seconds.div_ceil(60)
			),
			_ => "too many requests: try again in a minute".to_string(),
		};
		return (OnlineLookupStatus::RateLimited, message);
	}
	if status == 401
		|| status == 403
		|| said.contains("invalid_token")
		|| said.contains("jwt")
		|| said.contains("unauthorized")
		|| said.contains("not associated with a user")
	{
		return (
			OnlineLookupStatus::Error,
			"the API token was refused: check it in Settings → Metadata".to_string(),
		);
	}
	if let Some(first) = errors.first() {
		return (
			OnlineLookupStatus::Error,
			format!(
				"unexpected answer: {}",
				first.chars().take(80).collect::<String>()
			),
		);
	}
	(
		OnlineLookupStatus::Error,
		format!("answered with HTTP {status}"),
	)
}

/// A source that isn't set up
fn not_configured(source: IntakeSource, message: &str) -> OnlineLookup {
	OnlineLookup {
		source,
		status: OnlineLookupStatus::NotConfigured,
		subjects: 0,
		message: message.to_string(),
	}
}

/// The best candidate for a book: its title (or one of its matching edition titles) agrees
/// with ours and its authors don't disagree; a candidate whose author matches beats one with no
/// author, then one with subjects beats one without, then the one with most editions (the
/// canonical work) wins
fn pick<'a>(
	candidates: impl Iterator<Item = &'a Value>,
	parse: fn(&Value) -> OnlineRecord,
	title: Option<&str>,
	authors: &[String],
	by_isbn: bool,
) -> Option<OnlineRecord> {
	candidates
		.map(parse)
		.filter(|record| {
			let title_ok = match title {
				Some(ours) => record
					.title
					.iter()
					.chain(record.edition_titles.iter())
					.any(|theirs| titles_agree(ours, theirs)),
				None => by_isbn && record.title.is_some(),
			};
			title_ok && authors_agree(authors, &record.authors)
		})
		.enumerate()
		.max_by_key(|(position, record)| {
			let author_named = !authors.is_empty() && !record.authors.is_empty();
			(
				author_named,
				!record.subjects.is_empty(),
				record.edition_count,
				std::cmp::Reverse(*position),
			)
		})
		.map(|(_, record)| record)
}

/// Why a source refused, from its HTTP status and error body
fn refusal(
	source: IntakeSource,
	status: u16,
	body: Option<&Value>,
) -> (OnlineLookupStatus, String) {
	let error = body.and_then(|b| b.get("error"));
	let message = error
		.and_then(|e| e.get("message"))
		.and_then(Value::as_str)
		.unwrap_or_default()
		.to_lowercase();
	let reasons: Vec<String> = error
		.and_then(|e| e.get("errors"))
		.and_then(Value::as_array)
		.map(|errors| {
			errors
				.iter()
				.filter_map(|e| e.get("reason").and_then(Value::as_str))
				.map(str::to_lowercase)
				.collect()
		})
		.unwrap_or_default();
	let rpc_status = error
		.and_then(|e| e.get("status"))
		.and_then(Value::as_str)
		.unwrap_or_default();
	let quota = status == 429
		|| rpc_status == "RESOURCE_EXHAUSTED"
		|| message.contains("quota exceeded")
		|| reasons.iter().any(|r| {
			r.contains("ratelimit") || r.contains("limitexceeded") || r.contains("quota")
		});
	if quota {
		let text = match source {
			IntakeSource::GoogleBooks => {
				"daily limit reached: add a Google Books API key in Settings → Metadata"
			},
			_ => "too many requests: try again later",
		};
		return (OnlineLookupStatus::RateLimited, text.to_string());
	}
	if reasons.iter().any(|r| r.contains("keyinvalid"))
		|| message.contains("api key not valid")
	{
		return (
			OnlineLookupStatus::Error,
			"the API key was refused: check it in Settings → Metadata".to_string(),
		);
	}
	(
		OnlineLookupStatus::Error,
		format!("answered with HTTP {status}"),
	)
}

/// A source's answer as a lookup row
fn answered(source: IntakeSource, record: Option<&OnlineRecord>) -> OnlineLookup {
	let (status, subjects, message) = match record {
		Some(record) if !record.subjects.is_empty() => (
			OnlineLookupStatus::Ok,
			record.subjects.len(),
			format!(
				"found, {} {}",
				record.subjects.len(),
				if record.subjects.len() == 1 {
					"subject"
				} else {
					"subjects"
				}
			),
		),
		Some(_) => (
			OnlineLookupStatus::NoSubjects,
			0,
			"found, but it has no subjects for this book".to_string(),
		),
		None => (
			OnlineLookupStatus::NoMatch,
			0,
			"no matching book".to_string(),
		),
	};
	OnlineLookup {
		source,
		status,
		subjects: subjects as i32,
		message,
	}
}

fn failed(source: IntakeSource, fetched: &Fetched) -> OnlineLookup {
	let (status, message) = match fetched {
		Fetched::Status(code, body) if source == IntakeSource::Hardcover => {
			hardcover_refusal(*code, body.as_ref())
		},
		Fetched::Status(code, body) => refusal(source, *code, body.as_ref()),
		Fetched::Failed(why) => (OnlineLookupStatus::Error, why.clone()),
		Fetched::Json(_) => (OnlineLookupStatus::Error, "unexpected answer".to_string()),
	};
	OnlineLookup {
		source,
		status,
		subjects: 0,
		message,
	}
}

/// At most this many requests per source and book
const REQUESTS_PER_SOURCE: usize = 3;

async fn open_library(
	fetch: &dyn Fetch,
	query: &LookupQuery,
) -> (Option<OnlineRecord>, OnlineLookup) {
	let source = IntakeSource::OpenLibrary;
	let enc = |s: &str| urlencoding::encode(s).into_owned();
	let mut budget = REQUESTS_PER_SOURCE;
	let mut record = None;
	let mut last_failure = None;
	let mut answered_once = false;
	let titles = query_titles(query);
	let author = query.authors.first().cloned();
	let mut searches: Vec<(String, Option<String>, bool)> = vec![];
	if let Some(isbn) = &query.isbn {
		searches.push((format!("isbn={}", enc(isbn)), titles.first().cloned(), true));
	}
	for title in &titles {
		// title and author words together: also finds the work behind a translated edition
		let q = match &author {
			Some(author) => format!("{title} {author}"),
			None => title.clone(),
		};
		searches.push((format!("q={}", enc(&q)), Some(title.clone()), false));
	}
	if author.is_some() {
		if let Some(title) = titles.first() {
			// the title alone, for an author written differently there
			searches.push((format!("title={}", enc(title)), Some(title.clone()), false));
		}
	}
	for (search, title, by_isbn) in searches {
		// keep a request for the work's subjects
		if budget <= 1 {
			break;
		}
		budget -= 1;
		let url = format!(
			"https://openlibrary.org/search.json?{search}&fields={OPEN_LIBRARY_FIELDS}&limit=8"
		);
		match fetch.get(&url).await {
			Fetched::Json(json) => {
				answered_once = true;
				let docs = json
					.get("docs")
					.and_then(Value::as_array)
					.cloned()
					.unwrap_or_default();
				record = pick(
					docs.iter(),
					parse_open_library_doc,
					title.as_deref(),
					&query.authors,
					by_isbn,
				);
			},
			other => last_failure = Some(other),
		}
		if record.is_some() {
			break;
		}
	}
	let Some(mut record) = record else {
		return match (answered_once, last_failure) {
			(false, Some(failure)) => (None, failed(source, &failure)),
			_ => (None, answered(source, None)),
		};
	};
	// a search result often has no subjects: its work record, then one of its editions
	for key in [
		record.work_key.clone(),
		record.edition_key.clone().map(|e| format!("/books/{e}")),
	]
	.into_iter()
	.flatten()
	{
		if !record.subjects.is_empty() || budget == 0 {
			break;
		}
		budget -= 1;
		if let Fetched::Json(json) = fetch
			.get(&format!("https://openlibrary.org{key}.json"))
			.await
		{
			record.subjects = open_library_record_subjects(&json);
			if record.description.is_none() {
				record.description = open_library_description(&json);
			}
		}
	}
	let lookup = answered(source, Some(&record));
	(Some(record), lookup)
}

async fn google_books(
	fetch: &dyn Fetch,
	api_key: Option<&str>,
	query: &LookupQuery,
) -> (Option<OnlineRecord>, OnlineLookup) {
	let source = IntakeSource::GoogleBooks;
	let enc = |s: &str| urlencoding::encode(s).into_owned();
	let titles = query_titles(query);
	let mut searches: Vec<(String, Option<String>, bool)> = vec![];
	if let Some(isbn) = &query.isbn {
		searches.push((format!("isbn:{}", enc(isbn)), titles.first().cloned(), true));
	}
	for title in &titles {
		let mut q = format!("intitle:{}", enc(title));
		if let Some(author) = query.authors.first() {
			q.push_str(&format!("+inauthor:{}", enc(author)));
		}
		searches.push((q, Some(title.clone()), false));
	}
	if searches.is_empty() {
		return (None, answered(source, None));
	}
	let mut last = None;
	for (q, title, by_isbn) in searches.into_iter().take(REQUESTS_PER_SOURCE) {
		let mut url = format!(
			"https://www.googleapis.com/books/v1/volumes?q={q}&maxResults=5&printType=books"
		);
		if let Some(key) = api_key.map(str::trim).filter(|k| !k.is_empty()) {
			url.push_str(&format!("&key={}", enc(key)));
		}
		match fetch.get(&url).await {
			Fetched::Json(json) => {
				let items = json
					.get("items")
					.and_then(Value::as_array)
					.cloned()
					.unwrap_or_default();
				let record = pick(
					items.iter(),
					parse_google_volume,
					title.as_deref(),
					&query.authors,
					by_isbn,
				);
				if record.is_some() {
					let lookup = answered(source, record.as_ref());
					return (record, lookup);
				}
				last = Some(answered(source, None));
			},
			// a refusal (the quota) won't change with another title
			failure => return (None, failed(source, &failure)),
		}
	}
	(None, last.unwrap_or_else(|| answered(source, None)))
}

/// Hardcover's book search; `$q` is an ISBN or a title with its author
const HARDCOVER_SEARCH: &str = r#"query StumpLookupSearch($q: String!) { search(query: $q, query_type: "Book", per_page: 10, page: 1) { results } }"#;

/// A Hardcover book's record: its readers' tags with their votes, description and series
fn hardcover_book_query(id: i64) -> String {
	format!(
		"query StumpLookupBook {{ books(where: {{id: {{_eq: {id}}}}}, limit: 1) {{ id title description cached_tags featured_book_series {{ position series {{ name }} }} }} }}"
	)
}

/// Hardcover, when its provider is set up: the search (the ISBN, else the title with the
/// author; with nothing at all, the title alone, else the next title), the agreeing book with
/// the most readers, then its record for the tags with their votes (at most 3 requests)
async fn hardcover(
	fetch: &dyn Fetch,
	access: HardcoverAccess,
	query: &LookupQuery,
) -> (Option<OnlineRecord>, OnlineLookup) {
	let source = IntakeSource::Hardcover;
	match access {
		HardcoverAccess::Ready => {},
		HardcoverAccess::NotConfigured => {
			return (None, not_configured(source, "not configured"));
		},
		HardcoverAccess::TokenMissing => {
			return (
				None,
				not_configured(source, "token missing: add it in Settings → Metadata"),
			);
		},
	}
	let titles = query_titles(query);
	let author = query
		.authors
		.first()
		.map(|a| a.trim())
		.filter(|a| !a.is_empty());
	// (what to search, the title it must agree with, whether it is the ISBN)
	let mut searches: Vec<(String, Option<String>, bool)> = vec![];
	if let Some(isbn) = &query.isbn {
		searches.push((isbn.clone(), titles.first().cloned(), true));
	}
	for title in &titles {
		let q = match author {
			Some(author) => format!("{title} {author}"),
			None => title.clone(),
		};
		searches.push((q, Some(title.clone()), false));
	}
	let mut budget = REQUESTS_PER_SOURCE;
	let mut record = None;
	let mut next = 0;
	while record.is_none() && next < searches.len() {
		// keep a request for the book's record
		if budget <= 1 {
			break;
		}
		budget -= 1;
		let (q, title, by_isbn) = searches[next].clone();
		next += 1;
		let documents: Vec<Value> = match fetch
			.hardcover_graphql(HARDCOVER_SEARCH, &json!({ "q": q }))
			.await
		{
			Fetched::Json(json) => json
				.pointer("/data/search/results/hits")
				.and_then(Value::as_array)
				.map(|hits| {
					hits.iter()
						.filter_map(|h| h.get("document"))
						.cloned()
						.collect()
				})
				.unwrap_or_default(),
			// a refusal (the rate limit, the token) won't change with another search
			failure => return (None, failed(source, &failure)),
		};
		record = pick(
			documents.iter(),
			parse_hardcover_document,
			title.as_deref(),
			&query.authors,
			by_isbn,
		);
		// nothing at all for the title with its author: the title alone next
		if documents.is_empty() && !by_isbn {
			if let Some(title) = title.filter(|t| *t != q) {
				if !searches.iter().any(|(s, _, _)| *s == title) {
					searches.insert(next, (title.clone(), Some(title), false));
				}
			}
		}
	}
	let Some(mut record) = record else {
		return (None, answered(source, None));
	};
	if let Some(id) = record.hardcover_id {
		match fetch
			.hardcover_graphql(&hardcover_book_query(id), &json!({}))
			.await
		{
			Fetched::Json(json) => {
				if let Some(book) = json.pointer("/data/books/0") {
					apply_hardcover_book(&mut record, book);
				}
			},
			// the search's genres stay
			failure => {
				tracing::debug!(?failure, "Hardcover's book record could not be read");
			},
		}
	}
	let lookup = answered(source, Some(&record));
	(Some(record), lookup)
}

/// Looks a book up on Open Library, Google Books and Hardcover at the same time
pub async fn lookup_with(
	fetch: &dyn Fetch,
	google_api_key: Option<&str>,
	hardcover_access: HardcoverAccess,
	query: &LookupQuery,
) -> LookupOutcome {
	let mut query = query.clone();
	query.isbn = query
		.isbn
		.map(|i| i.replace(['-', ' '], ""))
		.filter(|i| i.len() >= 10);
	let ((ol, ol_status), (gb, gb_status), (hc, hc_status)) = tokio::join!(
		open_library(fetch, &query),
		google_books(fetch, google_api_key, &query),
		hardcover(fetch, hardcover_access, &query)
	);
	LookupOutcome {
		records: [ol, gb, hc].into_iter().flatten().collect(),
		sources: vec![ol_status, gb_status, hc_status],
	}
}

/// Looks a book up online with the server's settings: its Google Books API key, if any, and
/// the Hardcover metadata provider, when it is enabled with a token
pub async fn lookup(conn: &DatabaseConnection, query: &LookupQuery) -> LookupOutcome {
	let key = google_books_key(conn).await;
	let (access, hardcover) = hardcover_client(conn).await;
	lookup_with(&HttpFetch { hardcover }, key.as_deref(), access, query).await
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn reads_open_library_and_google_records() {
		let doc = json!({
			"key": "/works/OL42379976W",
			"title": "Il nome della rosa",
			"author_name": ["Umberto Eco"],
			"first_publish_year": 1980,
			"language": ["ita", "spa"],
			"subject": ["Fiction", "Monasteries", "Historical fiction"],
			"publisher": ["Bompiani"],
			"edition_key": ["OL1M", "OL2M"]
		});
		let record = parse_open_library_doc(&doc);
		assert_eq!(record.first_publish_year, Some(1980));
		assert_eq!(record.language.as_deref(), Some("it"));
		assert_eq!(record.subjects.len(), 3);
		assert_eq!(record.work_key.as_deref(), Some("/works/OL42379976W"));
		assert_eq!(record.edition_key.as_deref(), Some("OL1M"));

		let item = json!({ "volumeInfo": {
			"title": "El nombre de la rosa",
			"authors": ["Umberto Eco"],
			"publishedDate": "2011-05-01",
			"language": "es",
			"description": "<p>Una <b>abadía</b> benedictina...</p>",
			"categories": ["Fiction / Historical"],
			"industryIdentifiers": [{"type": "ISBN_10", "identifier": "8499080308"}, {"type": "ISBN_13", "identifier": "9788499080307"}]
		}});
		let record = parse_google_volume(&item);
		assert_eq!(
			record.description.as_deref(),
			Some("Una abadía benedictina...")
		);
		assert_eq!(record.isbn.as_deref(), Some("9788499080307"));
		assert_eq!(record.first_publish_year, Some(2011));
	}

	#[test]
	fn compares_translated_and_bilingual_titles_and_initials() {
		assert!(titles_agree("Crimen y castigo", "El crimen y el castigo"));
		assert!(titles_agree(
			"El principito",
			"El Principito / The Little Prince"
		));
		assert!(!titles_agree("Dune", "Dune Messiah"));
		assert!(authors_agree(
			&["Michael Cook".into()],
			&["M. A. Cook".into()]
		));
		assert!(authors_agree(
			&["Michael Cook".into()],
			&["Michael A. Cook".into()]
		));
		assert!(authors_agree(
			&["Fiódor Dostoyevski".into()],
			&["Fiódor Dostoievski".into()]
		));
		assert!(!authors_agree(
			&["Brian Herbert".into()],
			&["Frank Herbert".into()]
		));
		assert!(!authors_agree(&["Ana Cook".into()], &["M. A. Cook".into()]));
	}

	#[test]
	fn compares_titles_and_authors_loosely() {
		assert!(titles_agree(
			"El nombre de la rosa",
			"El Nombre de la Rosa: Edición conmemorativa"
		));
		assert!(!titles_agree(
			"El nombre de la rosa",
			"El péndulo de Foucault"
		));
		assert!(authors_agree(
			&["Eco, Umberto".into()],
			&["Umberto Eco".into()]
		));
		assert!(!authors_agree(
			&["Umberto Eco".into()],
			&["Dan Brown".into()]
		));
	}

	/// Canned answers by URL fragment (for Hardcover, by a fragment of `hardcover <query>
	/// <variables>`); every request is recorded
	struct Canned {
		answers: Vec<(&'static str, Fetched)>,
		asked: Mutex<Vec<String>>,
	}

	impl Canned {
		fn answer(&self, asked: String) -> Fetched {
			let answer = self
				.answers
				.iter()
				.find(|(fragment, _)| asked.contains(fragment))
				.map(|(_, answer)| answer.clone())
				.unwrap_or_else(|| Fetched::Status(404, None));
			self.asked.lock().unwrap().push(asked);
			answer
		}

		fn asked_hardcover(&self) -> Vec<String> {
			self.asked
				.lock()
				.unwrap()
				.iter()
				.filter(|a| a.starts_with("hardcover "))
				.cloned()
				.collect()
		}
	}

	#[async_trait::async_trait]
	impl Fetch for Canned {
		async fn get(&self, url: &str) -> Fetched {
			self.answer(url.to_string())
		}

		async fn hardcover_graphql(&self, query: &str, variables: &Value) -> Fetched {
			self.answer(format!("hardcover {query} {variables}"))
		}
	}

	/// Google's answer when the shared anonymous quota is used up
	fn quota_error() -> Fetched {
		Fetched::Status(
			429,
			Some(json!({
				"error": {
					"code": 429,
					"message": "Quota exceeded for quota metric 'Queries' and limit 'Queries per day' of service 'books.googleapis.com' for consumer 'project_number:624717413613'.",
					"errors": [{
						"message": "Quota exceeded for quota metric 'Queries' and limit 'Queries per day' of service 'books.googleapis.com' for consumer 'project_number:624717413613'.",
						"domain": "global",
						"reason": "rateLimitExceeded"
					}],
					"status": "RESOURCE_EXHAUSTED"
				}
			})),
		)
	}

	fn pantano() -> Vec<String> {
		vec!["Robert Pantano".to_string()]
	}

	fn query(title: &str, authors: Vec<String>) -> LookupQuery {
		LookupQuery {
			title: Some(title.to_string()),
			authors,
			..Default::default()
		}
	}

	/// 'Notes from the End of Everything': Google's quota is used up, Open Library's search
	/// result and work have no subjects, its edition has some
	#[tokio::test]
	async fn reports_each_source_and_falls_back_to_the_edition_subjects() {
		let canned = Canned {
			answers: vec![
				("googleapis.com", quota_error()),
				(
					"openlibrary.org/search.json",
					Fetched::Json(json!({ "docs": [{
						"key": "/works/OL42379976W",
						"title": "Notes from the End of Everything",
						"author_name": ["Robert Pantano"],
						"edition_key": ["OL57552455M"]
					}]})),
				),
				(
					"openlibrary.org/works/OL42379976W.json",
					Fetched::Json(json!({
						"title": "Notes from the End of Everything",
						"description": {"type": "/type/text", "value": "Essays on <i>meaning</i>."}
					})),
				),
				(
					"openlibrary.org/books/OL57552455M.json",
					Fetched::Json(json!({
						"subjects": ["Philosophy", "Meaning (Philosophy)"],
						"subject_times": ["21st century"]
					})),
				),
			],
			asked: Mutex::new(vec![]),
		};
		let outcome = lookup_with(
			&canned,
			None,
			HardcoverAccess::NotConfigured,
			&query("Notes from the End of Everything", pantano()),
		)
		.await;
		assert_eq!(outcome.records.len(), 1);
		let record = &outcome.records[0];
		assert_eq!(
			record.subjects,
			vec!["Philosophy", "Meaning (Philosophy)", "21st century"]
		);
		assert_eq!(record.description.as_deref(), Some("Essays on meaning ."));
		let ol = &outcome.sources[0];
		assert_eq!(
			(ol.source, ol.status, ol.subjects),
			(IntakeSource::OpenLibrary, OnlineLookupStatus::Ok, 3)
		);
		let gb = &outcome.sources[1];
		assert_eq!(
			(gb.source, gb.status),
			(IntakeSource::GoogleBooks, OnlineLookupStatus::RateLimited)
		);
		assert!(gb.message.contains("API key"), "{}", gb.message);
		assert!(outcome.found());
		assert_eq!(
			outcome.notes(),
			vec!["Google Books: daily limit reached: add a Google Books API key in Settings → Metadata"]
		);
		// no key: none sent
		assert!(canned
			.asked
			.lock()
			.unwrap()
			.iter()
			.all(|url| !url.contains("key=")));
	}

	#[tokio::test]
	async fn a_found_book_without_subjects_and_failed_sources_are_not_a_successful_lookup(
	) {
		let canned = Canned {
			answers: vec![
				(
					"googleapis.com",
					Fetched::Failed("no answer in time".into()),
				),
				(
					"openlibrary.org/search.json",
					Fetched::Json(json!({ "docs": [{
						"key": "/works/OL1W",
						"title": "Notes from the End of Everything",
						"author_name": ["Robert Pantano"]
					}]})),
				),
				(
					"openlibrary.org/works/OL1W.json",
					Fetched::Json(json!({"title": "x"})),
				),
			],
			asked: Mutex::new(vec![]),
		};
		let outcome = lookup_with(
			&canned,
			None,
			HardcoverAccess::NotConfigured,
			&query("Notes from the End of Everything", pantano()),
		)
		.await;
		assert_eq!(outcome.sources[0].status, OnlineLookupStatus::NoSubjects);
		assert_eq!(
			outcome.sources[0].message,
			"found, but it has no subjects for this book"
		);
		assert_eq!(outcome.sources[1].status, OnlineLookupStatus::Error);
		// found, with nothing to add
		assert!(outcome.found());

		// everything failed: not looked up
		let canned = Canned {
			answers: vec![("googleapis.com", quota_error())],
			asked: Mutex::new(vec![]),
		};
		let outcome = lookup_with(
			&canned,
			None,
			HardcoverAccess::NotConfigured,
			&query("Anything", vec![]),
		)
		.await;
		assert_eq!(
			outcome.sources[0].status,
			OnlineLookupStatus::Error,
			"Open Library 404"
		);
		assert_eq!(outcome.sources[1].status, OnlineLookupStatus::RateLimited);
		assert!(!outcome.found());
		assert!(outcome.records.is_empty());

		// nothing matches: answered, but not found
		let canned = Canned {
			answers: vec![
				("googleapis.com", Fetched::Json(json!({"totalItems": 0}))),
				(
					"openlibrary.org/search.json",
					Fetched::Json(json!({"docs": []})),
				),
			],
			asked: Mutex::new(vec![]),
		};
		let outcome = lookup_with(
			&canned,
			None,
			HardcoverAccess::NotConfigured,
			&query("Anything", vec![]),
		)
		.await;
		assert!(outcome.sources[..2]
			.iter()
			.all(|s| s.status == OnlineLookupStatus::NoMatch));
		assert_eq!(outcome.sources[2].status, OnlineLookupStatus::NotConfigured);
		assert!(!outcome.found());
	}

	#[tokio::test]
	async fn the_google_books_key_is_sent_and_never_logged() {
		let canned = Canned {
			answers: vec![(
				"googleapis.com",
				Fetched::Json(json!({"items": [{"volumeInfo": {
					"title": "Notes from the End of Everything",
					"authors": ["Robert Pantano"],
					"categories": ["Philosophy"]
				}}]})),
			)],
			asked: Mutex::new(vec![]),
		};
		let outcome = lookup_with(
			&canned,
			Some("AIza-secret"),
			HardcoverAccess::NotConfigured,
			&query("Notes from the End of Everything", pantano()),
		)
		.await;
		let asked = canned.asked.lock().unwrap().clone();
		let google = asked.iter().find(|u| u.contains("googleapis")).unwrap();
		assert!(google.ends_with("&key=AIza-secret"), "{google}");
		assert!(!redacted(google).contains("secret"));
		assert_eq!(outcome.sources[1].status, OnlineLookupStatus::Ok);
		assert_eq!(outcome.records[0].subjects, vec!["Philosophy"]);

		// a refused key says so
		let (status, message) = refusal(
			IntakeSource::GoogleBooks,
			400,
			Some(
				&json!({"error": {"code": 400, "message": "API key not valid. Please pass a valid API key.", "errors": [{"reason": "badRequest"}]}}),
			),
		);
		assert_eq!(status, OnlineLookupStatus::Error);
		assert!(message.contains("refused"), "{message}");
	}

	#[test]
	fn normalizes_edition_format_and_volume_suffixes() {
		for (title, expected) in [
			("Moby Dick. Versión ilustrada", "Moby Dick"),
			(
				"Cien años de soledad (Ed. Ilustrada)",
				"Cien años de soledad",
			),
			("Crimen y castigo (ilustrado)", "Crimen y castigo"),
			("El principito (Spanish Edition)", "El principito"),
			("Cita con Rama ed ilustrada", "Cita con Rama"),
			("The Hobbit: Illustrated edition", "The Hobbit"),
			("Yotsuba&! v03", "Yotsuba&!"),
			("Spice and Wolf, Vol. 3", "Spice and Wolf"),
			("Dr. Stone #232", "Dr. Stone"),
			// left alone
			("Dune: House Harkonnen", "Dune: House Harkonnen"),
			("Bartleby, el escribiente", "Bartleby, el escribiente"),
			(
				"The Koran: A Very Short Introduction",
				"The Koran: A Very Short Introduction",
			),
			(
				"Chemical Secret (Oxford Bookworms Library, Stage 3)",
				"Chemical Secret (Oxford Bookworms Library, Stage 3)",
			),
			("Apollo 13", "Apollo 13"),
		] {
			assert_eq!(normalize_title(title), expected, "{title}");
		}
	}

	#[test]
	fn searches_series_original_and_main_titles() {
		let manga = LookupQuery {
			title: Some("Yotsuba&! v03".into()),
			series: Some("Yotsuba&!".into()),
			by_series: true,
			..Default::default()
		};
		assert_eq!(query_titles(&manga), vec!["Yotsuba&!"]);
		let translated = LookupQuery {
			title: Some("El nombre de la rosa".into()),
			original_title: Some("Il nome della rosa".into()),
			..Default::default()
		};
		assert_eq!(
			query_titles(&translated),
			vec!["El nombre de la rosa", "Il nome della rosa"]
		);
		let subtitled = LookupQuery {
			title: Some("Dune: House Harkonnen".into()),
			..Default::default()
		};
		assert_eq!(
			query_titles(&subtitled),
			vec!["Dune: House Harkonnen", "Dune"]
		);
		let reader = LookupQuery {
			title: Some("Chemical Secret (Oxford Bookworms Library, Stage 3)".into()),
			..Default::default()
		};
		assert_eq!(query_titles(&reader)[1], "Chemical Secret");
	}

	#[test]
	fn reads_an_original_title_from_the_opf() {
		let calibre = r#"<metadata><meta name="calibre:user_metadata:#titulo_original" content="{&quot;table&quot;: &quot;custom_column_9&quot;, &quot;#value#&quot;: &quot;Il nome della rosa&quot;, &quot;datatype&quot;: &quot;text&quot;}"/></metadata>"#;
		assert_eq!(
			original_title_from_opf(calibre).as_deref(),
			Some("Il nome della rosa")
		);
		let plain = r#"<meta name="original-title" content="Moby-Dick; or, The Whale"/>"#;
		assert_eq!(
			original_title_from_opf(plain).as_deref(),
			Some("Moby-Dick; or, The Whale")
		);
		assert_eq!(
			original_title_from_opf(r#"<meta name="calibre:title_sort" content="x"/>"#),
			None
		);
	}

	/// A Spanish edition is matched to its Italian work through the edition titles, and the
	/// canonical work (most editions, with subjects) beats an orphan edition's work
	#[tokio::test]
	async fn finds_the_work_behind_a_translated_edition() {
		let canned = Canned {
			answers: vec![
				("googleapis.com", quota_error()),
				(
					"openlibrary.org/search.json",
					Fetched::Json(json!({ "docs": [
						{
							"key": "/works/OL38059713W",
							"title": "El Nombre de la Rosa",
							"author_name": ["Umberto Eco"],
							"edition_count": 1,
							"editions": {"docs": [{"key": "/books/OL51699004M", "title": "El Nombre de la Rosa"}]}
						},
						{
							"key": "/works/OL8996439W",
							"title": "Il nome della rosa",
							"author_name": ["Umberto Eco"],
							"edition_count": 151,
							"subject": ["Fiction", "Monasteries", "Historical fiction"],
							"editions": {"docs": [{"key": "/books/OL9199745M", "title": "El nombre de la rosa"}]}
						}
					]})),
				),
			],
			asked: Mutex::new(vec![]),
		};
		let outcome = lookup_with(
			&canned,
			None,
			HardcoverAccess::NotConfigured,
			&query("El nombre de la rosa", vec!["Umberto Eco".to_string()]),
		)
		.await;
		let record = &outcome.records[0];
		assert_eq!(record.work_key.as_deref(), Some("/works/OL8996439W"));
		assert_eq!(record.subjects.len(), 3);
		let asked = canned.asked.lock().unwrap().clone();
		let ol: Vec<&String> =
			asked.iter().filter(|u| u.contains("openlibrary")).collect();
		assert_eq!(ol.len(), 1, "one search was enough: {ol:?}");
		assert!(
			ol[0].contains("q=El%20nombre%20de%20la%20rosa%20Umberto%20Eco"),
			"{}",
			ol[0]
		);
	}

	/// Nothing agrees: at most 3 requests per source (here two Open Library searches)
	#[tokio::test]
	async fn keeps_to_three_requests_per_source() {
		let canned = Canned {
			answers: vec![
				("googleapis.com", Fetched::Json(json!({"items": []}))),
				(
					"openlibrary.org/search.json",
					Fetched::Json(
						json!({ "docs": [{"key": "/works/OL1W", "title": "Something else", "author_name": ["Someone"]}]}),
					),
				),
			],
			asked: Mutex::new(vec![]),
		};
		let outcome = lookup_with(
			&canned,
			None,
			HardcoverAccess::NotConfigured,
			&LookupQuery {
				title: Some("Dune: House Harkonnen (Ed. Ilustrada)".into()),
				authors: vec!["Brian Herbert".into()],
				original_title: Some("Dune: House Harkonnen".into()),
				..Default::default()
			},
		)
		.await;
		assert!(outcome.sources[..2]
			.iter()
			.all(|s| s.status == OnlineLookupStatus::NoMatch));
		let asked = canned.asked.lock().unwrap().clone();
		let ol = asked.iter().filter(|u| u.contains("openlibrary")).count();
		let gb = asked.iter().filter(|u| u.contains("googleapis")).count();
		assert!(ol <= 3 && gb <= 3, "{asked:#?}");
		assert_eq!(ol, 2);
	}

	/// A Hardcover search answer with these documents
	fn hardcover_hits(documents: Value) -> Fetched {
		let hits: Vec<Value> = documents
			.as_array()
			.unwrap()
			.iter()
			.map(|document| json!({ "document": document, "text_match": 1 }))
			.collect();
		Fetched::Json(json!({ "data": { "search": { "results": {
			"found": hits.len(), "hits": hits, "page": 1
		}}}}))
	}

	fn koran() -> LookupQuery {
		LookupQuery {
			title: Some("The Koran: A Very Short Introduction".into()),
			authors: vec!["Michael Cook".into()],
			..Default::default()
		}
	}

	/// The book (searched by title and author) is the agreeing hit with most readers; its
	/// record's genres and tags become subjects weighted by their votes, with its description
	/// and series
	#[tokio::test]
	async fn hardcover_finds_a_book_and_weighs_its_readers_tags() {
		let canned = Canned {
			answers: vec![
				("googleapis.com", Fetched::Json(json!({"items": []}))),
				(
					"openlibrary.org/search.json",
					Fetched::Json(json!({"docs": []})),
				),
				(
					"StumpLookupSearch",
					hardcover_hits(json!([
						{"id": "11", "title": "The Koran", "author_names": ["Anonymous"], "users_count": 900, "genres": ["Religion"]},
						{"id": "435107", "title": "The Koran: A Very Short Introduction", "author_names": ["Michael Cook"],
						 "alternative_titles": ["El Corán: una breve introducción"], "users_count": 52,
						 "genres": ["Religion", "History"], "description": "From the search"},
						{"id": 999, "title": "The Koran: A Very Short Introduction", "author_names": ["Michael Cook"], "users_count": 1}
					])),
				),
				(
					"StumpLookupBook { books(where: {id: {_eq: 435107}}",
					Fetched::Json(json!({ "data": { "books": [{
						"id": 435107,
						"title": "The Koran: A Very Short Introduction",
						"description": "<p>The Koran is the <b>sacred</b> book of Islam.</p>",
						"cached_tags": {
							"Genre": [
								{"tag": "Religion", "count": 20, "category": "Genre"},
								{"tag": "History", "count": 10},
								{"tag": "Nonfiction", "count": 20},
								{"tag": "Philosophy", "count": 1}
							],
							"Mood": [{"tag": "informative", "count": 15}],
							"Content Warning": [{"tag": "Violence", "count": 3}],
							"Tag": [
								{"tag": "Owned", "count": 9},
								{"tag": "Islam", "count": 6},
								{"tag": "2023", "count": 5},
								{"tag": "Theology", "count": 1}
							]
						},
						"featured_book_series": {"position": 143, "series": {"name": "Very Short Introductions"}}
					}]}})),
				),
			],
			asked: Mutex::new(vec![]),
		};
		let outcome = lookup_with(&canned, None, HardcoverAccess::Ready, &koran()).await;
		let hc = &outcome.sources[2];
		assert_eq!(
			(hc.source, hc.status, hc.subjects, hc.message.as_str()),
			(
				IntakeSource::Hardcover,
				OnlineLookupStatus::Ok,
				4,
				"found, 4 subjects"
			)
		);
		let record = outcome
			.records
			.iter()
			.find(|r| r.source == Some(IntakeSource::Hardcover))
			.unwrap();
		assert_eq!(record.hardcover_id, Some(435107));
		// most votes first; Philosophy (1 of 20) is the long tail, moods, content warnings,
		// shelf tags and years are left out, a tag needs a quarter of the top tag's votes
		assert_eq!(
			record.subjects,
			vec!["Religion", "Nonfiction", "History", "Islam"]
		);
		assert_eq!(record.subject_weights.get("Religion"), None, "counts fully");
		assert_eq!(record.subject_weights.get("History"), Some(&0.75));
		assert_eq!(record.subject_weights.get("Islam"), Some(&0.8));
		assert_eq!(
			record.description.as_deref(),
			Some("The Koran is the sacred book of Islam.")
		);
		assert_eq!(record.series.as_deref(), Some("Very Short Introductions"));
		assert_eq!(record.series_index, Some(143.0));
		// one search (title and author), then the book: 2 requests
		let asked = canned.asked_hardcover();
		assert_eq!(asked.len(), 2, "{asked:#?}");
		assert!(
			asked[0]
				.contains(r#""q":"The Koran: A Very Short Introduction Michael Cook""#),
			"{}",
			asked[0]
		);
		assert!(outcome.found());
		assert!(outcome.notes().is_empty(), "{:?}", outcome.notes());
	}

	/// A translated edition is found through the alternative titles, and an ISBN search comes
	/// first
	#[tokio::test]
	async fn hardcover_searches_the_isbn_first_and_reads_alternative_titles() {
		let canned = Canned {
			answers: vec![
				(
					"9788426418999",
					hardcover_hits(json!([{
						"id": 77, "title": "Ur-Fascism", "author_names": ["Umberto Eco"],
						"alternative_titles": ["Il fascismo eterno", "Contra el fascismo"],
						"genres": ["Politics", "Essays"]
					}])),
				),
				(
					"StumpLookupBook",
					Fetched::Json(
						json!({ "data": { "books": [{ "id": 77, "cached_tags": [] }] }}),
					),
				),
			],
			asked: Mutex::new(vec![]),
		};
		let query = LookupQuery {
			isbn: Some("978-84-264-1899-9".into()),
			title: Some("Contra el fascismo".into()),
			authors: vec!["Umberto Eco".into()],
			..Default::default()
		};
		let outcome = lookup_with(&canned, None, HardcoverAccess::Ready, &query).await;
		let hc = &outcome.sources[2];
		assert_eq!(hc.status, OnlineLookupStatus::Ok);
		let record = outcome.records.last().unwrap();
		// no tags in the record: the search's genres stay, a little weaker
		assert_eq!(record.subjects, vec!["Politics", "Essays"]);
		assert_eq!(record.subject_weights.get("Politics"), Some(&0.8));
		let asked = canned.asked_hardcover();
		assert_eq!(asked.len(), 2, "{asked:#?}");
		assert!(asked[0].contains(r#""q":"9788426418999""#), "{}", asked[0]);
	}

	/// Nothing agrees: no match, and never more than 3 requests (here: the title with its
	/// author finds nothing at all, so the title alone; no book record)
	#[tokio::test]
	async fn hardcover_reports_no_match() {
		let canned = Canned {
			answers: vec![
				(
					r#""q":"Notes from the End of Everything Robert Pantano""#,
					hardcover_hits(json!([])),
				),
				(
					r#""q":"Notes from the End of Everything""#,
					hardcover_hits(json!([
						{"id": 5, "title": "Notes from the End of the World", "author_names": ["Someone Else"]}
					])),
				),
			],
			asked: Mutex::new(vec![]),
		};
		let outcome = lookup_with(
			&canned,
			None,
			HardcoverAccess::Ready,
			&query("Notes from the End of Everything", pantano()),
		)
		.await;
		let hc = &outcome.sources[2];
		assert_eq!(
			(hc.status, hc.message.as_str()),
			(OnlineLookupStatus::NoMatch, "no matching book")
		);
		let asked = canned.asked_hardcover();
		assert_eq!(asked.len(), 2, "{asked:#?}");
		assert!(asked.iter().all(|a| a.contains("StumpLookupSearch")));
		assert!(outcome
			.records
			.iter()
			.all(|r| r.source != Some(IntakeSource::Hardcover)));
	}

	/// A 429 is reported with how long Hardcover asks to wait, and no other search is tried
	#[tokio::test]
	async fn hardcover_reports_its_rate_limit() {
		let canned = Canned {
			answers: vec![(
				"StumpLookupSearch",
				Fetched::Status(
					429,
					Some(json!({
						"error": "Too Many Requests",
						"message": "API rate limit exceeded for tier 'Free'. Try again in 6856 seconds."
					})),
				),
			)],
			asked: Mutex::new(vec![]),
		};
		let outcome = lookup_with(&canned, None, HardcoverAccess::Ready, &koran()).await;
		let hc = &outcome.sources[2];
		assert_eq!(
			(hc.status, hc.message.as_str()),
			(
				OnlineLookupStatus::RateLimited,
				"daily limit reached: try again in about 2 h"
			)
		);
		assert_eq!(canned.asked_hardcover().len(), 1);
		assert!(outcome.notes().contains(
			&"Hardcover: daily limit reached: try again in about 2 h".to_string()
		));

		assert_eq!(
			hardcover_refusal(429, Some(&json!({"message": "Try again in 20 seconds."}))),
			(
				OnlineLookupStatus::RateLimited,
				"too many requests: try again in a minute".to_string()
			)
		);
		assert_eq!(
			hardcover_refusal(
				429,
				Some(&json!({"message": "Try again in 600 seconds."}))
			)
			.1,
			"rate limit reached: try again in 10 min"
		);
		assert_eq!(
			hardcover_refusal(429, None).0,
			OnlineLookupStatus::RateLimited
		);
	}

	/// A refused token (Hardcover's 401) and GraphQL errors are errors that say so
	#[tokio::test]
	async fn hardcover_reports_a_refused_token() {
		let canned = Canned {
			answers: vec![(
				"StumpLookupSearch",
				Fetched::Status(
					401,
					Some(json!({
						"error": "invalid_token",
						"error_description": "Token is not associated with a user"
					})),
				),
			)],
			asked: Mutex::new(vec![]),
		};
		let outcome = lookup_with(&canned, None, HardcoverAccess::Ready, &koran()).await;
		let hc = &outcome.sources[2];
		assert_eq!(
			(hc.status, hc.message.as_str()),
			(
				OnlineLookupStatus::Error,
				"the API token was refused: check it in Settings → Metadata"
			)
		);
		assert_eq!(canned.asked_hardcover().len(), 1);
		assert!(!outcome.found());

		let (status, message) = hardcover_refusal(
			200,
			Some(
				&json!({"errors": [{"message": "field 'nope' not found in type: 'books'"}]}),
			),
		);
		assert_eq!(status, OnlineLookupStatus::Error);
		assert_eq!(
			message,
			"unexpected answer: field 'nope' not found in type: 'books'"
		);
	}

	/// Without an enabled provider and a token, Hardcover isn't asked and says why, without a
	/// warning
	#[tokio::test]
	async fn hardcover_not_configured_is_not_asked() {
		for (access, message) in [
			(HardcoverAccess::NotConfigured, "not configured"),
			(
				HardcoverAccess::TokenMissing,
				"token missing: add it in Settings → Metadata",
			),
		] {
			let canned = Canned {
				answers: vec![],
				asked: Mutex::new(vec![]),
			};
			let outcome = lookup_with(&canned, None, access, &koran()).await;
			let hc = &outcome.sources[2];
			assert_eq!(
				(hc.source, hc.status, hc.message.as_str()),
				(
					IntakeSource::Hardcover,
					OnlineLookupStatus::NotConfigured,
					message
				)
			);
			assert!(canned.asked_hardcover().is_empty());
			assert!(outcome.notes().iter().all(|n| !n.starts_with("Hardcover")));
		}
	}

	#[test]
	fn hardcover_tags_weigh_by_votes() {
		// no counts: every genre counts fully, in Hardcover's order
		let subjects = hardcover_subjects(&json!({
			"Genre": [{"tag": "Manga"}, {"tag": "Comedy"}]
		}));
		assert_eq!(
			subjects,
			vec![("Manga".to_string(), 1.0), ("Comedy".to_string(), 1.0)]
		);
		// a jsonb column given as text
		let subjects = hardcover_subjects(&json!(
			r#"{"Genre": [{"tag": "Fantasy", "count": 40}, {"tag": "Romance", "count": 4}, {"tag": "Horror", "count": 3}]}"#
		));
		let tags: Vec<&str> = subjects.iter().map(|(t, _)| t.as_str()).collect();
		assert_eq!(tags, vec!["Fantasy", "Romance"], "Horror is the long tail");
		assert_eq!(subjects[0].1, 1.0);
		assert!((subjects[1].1 - 0.55).abs() < 1e-9, "{subjects:?}");
		assert!(hardcover_subjects(&Value::Null).is_empty());
	}

	#[test]
	fn a_hardcover_refusal_holds_further_requests_for_its_token() {
		let (token, other) = (0x5eed_u64, 0x0dd_u64);
		assert_eq!(hardcover_wait(token), None);
		wait_for_hardcover(token, Duration::from_secs(120));
		let wait = hardcover_wait(token).unwrap();
		assert!(wait > Duration::from_secs(100), "{wait:?}");
		assert_eq!(hardcover_wait(other), None);
		match wait_answer(wait) {
			Fetched::Status(429, body) => assert_eq!(
				hardcover_refusal(429, body.as_ref()).1,
				"rate limit reached: try again in 2 min"
			),
			other => panic!("{other:?}"),
		}
	}
}
