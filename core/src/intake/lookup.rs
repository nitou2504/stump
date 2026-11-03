//! On-demand lookups on Open Library and Google Books. Short timeouts, a polite User-Agent
//! and a small in-memory cache. Google Books answers keyless requests from a quota shared by
//! every anonymous caller, which is often used up: an optional API key (a server setting, see
//! [`google_books_key`]) gives the server its own. Each source's answer is reported
//! ([`OnlineLookup`]: found with N subjects, no subjects, no match, rate-limited, error), so a
//! lookup that failed is never taken for one that found nothing.
//!
//! Matching (at most 3 requests per source and book, all cached): the title is searched
//! without its edition and format suffixes (`. Versión ilustrada`, `(Ed. Ilustrada)`,
//! `(Spanish Edition)`, `vNN` / `Vol. N` for series volumes), with its author; a manga volume is
//! searched by its series; then the main title (before `:`), or the original title of a
//! translated book when its file has one. Open Library is searched as a whole (`q=`), which
//! finds the work behind a translated edition (`El nombre de la rosa` → *Il nome della rosa*,
//! matched through its edition titles); among the works that agree, the one whose author
//! matches and with the most editions wins, and when it has no subjects in the search its work
//! record (or one of its editions) is asked (subjects, people, places, times).

use std::{
	collections::HashMap,
	sync::{LazyLock, Mutex},
	time::{Duration, Instant},
};

use models::{
	entity::server_config,
	shared::intake::{IntakeSource, OnlineLookup, OnlineLookupStatus},
};
use regex::Regex;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::Value;

use super::naming::{language_code, name_keys, name_tokens};
use crate::utils::encryption::decrypt_string;

const USER_AGENT: &str = "Stump-intake/1.0 (self-hosted library server)";
const TIMEOUT: Duration = Duration::from_secs(6);
/// Answers are kept a day; refusals and HTTP errors only a few minutes
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const ERROR_TTL: Duration = Duration::from_secs(10 * 60);
const CACHE_MAX: usize = 256;

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
	pub isbn: Option<String>,
	/// Open Library: the work (`/works/OL...W`) and one edition (`OL...M`) the record came from
	pub work_key: Option<String>,
	pub edition_key: Option<String>,
	/// Open Library: the titles of the work's editions that matched the search (a translated
	/// edition's title, when the work is known by its original one)
	pub edition_titles: Vec<String>,
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

/// Something that GETs JSON: the real HTTP client, or canned answers in tests
#[async_trait::async_trait]
pub trait Fetch: Send + Sync {
	async fn get(&self, url: &str) -> Fetched;
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

/// The HTTP client with the in-memory cache
pub struct HttpFetch;

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

#[async_trait::async_trait]
impl Fetch for HttpFetch {
	async fn get(&self, url: &str) -> Fetched {
		if let Ok(cache) = CACHE.lock() {
			if let Some((at, ttl, value)) = cache.get(url) {
				if at.elapsed() < *ttl {
					return value.clone();
				}
			}
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
		let ttl = match &fetched {
			Fetched::Json(_) => Some(CACHE_TTL),
			Fetched::Status(..) => Some(ERROR_TTL),
			// a network failure is tried again next time
			Fetched::Failed(_) => None,
		};
		if let (Some(ttl), Ok(mut cache)) = (ttl, CACHE.lock()) {
			if cache.len() >= CACHE_MAX {
				if let Some(oldest) = cache
					.iter()
					.min_by_key(|(_, (at, _, _))| *at)
					.map(|(k, _)| k.clone())
				{
					cache.remove(&oldest);
				}
			}
			cache.insert(url.to_string(), (Instant::now(), ttl, fetched.clone()));
		}
		fetched
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

/// Looks a book up on Open Library and Google Books at the same time
pub async fn lookup_with(
	fetch: &dyn Fetch,
	google_api_key: Option<&str>,
	query: &LookupQuery,
) -> LookupOutcome {
	let mut query = query.clone();
	query.isbn = query
		.isbn
		.map(|i| i.replace(['-', ' '], ""))
		.filter(|i| i.len() >= 10);
	let ((ol, ol_status), (gb, gb_status)) = tokio::join!(
		open_library(fetch, &query),
		google_books(fetch, google_api_key, &query)
	);
	LookupOutcome {
		records: [ol, gb].into_iter().flatten().collect(),
		sources: vec![ol_status, gb_status],
	}
}

/// Looks a book up online with the server's settings (its Google Books API key, if any)
pub async fn lookup(conn: &DatabaseConnection, query: &LookupQuery) -> LookupOutcome {
	let key = google_books_key(conn).await;
	lookup_with(&HttpFetch, key.as_deref(), query).await
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

	/// Canned answers by URL fragment; every request is recorded
	struct Canned {
		answers: Vec<(&'static str, Fetched)>,
		asked: Mutex<Vec<String>>,
	}

	#[async_trait::async_trait]
	impl Fetch for Canned {
		async fn get(&self, url: &str) -> Fetched {
			self.asked.lock().unwrap().push(url.to_string());
			self.answers
				.iter()
				.find(|(fragment, _)| url.contains(fragment))
				.map(|(_, answer)| answer.clone())
				.unwrap_or_else(|| Fetched::Status(404, None))
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
		let outcome = lookup_with(&canned, None, &query("Anything", vec![])).await;
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
		let outcome = lookup_with(&canned, None, &query("Anything", vec![])).await;
		assert!(outcome
			.sources
			.iter()
			.all(|s| s.status == OnlineLookupStatus::NoMatch));
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
			&LookupQuery {
				title: Some("Dune: House Harkonnen (Ed. Ilustrada)".into()),
				authors: vec!["Brian Herbert".into()],
				original_title: Some("Dune: House Harkonnen".into()),
				..Default::default()
			},
		)
		.await;
		assert!(outcome
			.sources
			.iter()
			.all(|s| s.status == OnlineLookupStatus::NoMatch));
		let asked = canned.asked.lock().unwrap().clone();
		let ol = asked.iter().filter(|u| u.contains("openlibrary")).count();
		let gb = asked.iter().filter(|u| u.contains("googleapis")).count();
		assert!(ol <= 3 && gb <= 3, "{asked:#?}");
		assert_eq!(ol, 2);
	}
}
