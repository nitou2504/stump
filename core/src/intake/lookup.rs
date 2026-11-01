//! On-demand lookups on Open Library and Google Books (no API keys). Short timeouts, a
//! polite User-Agent and a small in-memory cache: a lookup that fails or times out only
//! leaves the item with less to suggest.

use std::{
	collections::HashMap,
	sync::{LazyLock, Mutex},
	time::{Duration, Instant},
};

use models::shared::intake::IntakeSource;
use serde_json::Value;

use super::naming::{language_code, name_keys, name_tokens};

const USER_AGENT: &str = "Stump-intake/1.0 (self-hosted library server)";
const TIMEOUT: Duration = Duration::from_secs(6);
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
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
}

static CACHE: LazyLock<Mutex<HashMap<String, (Instant, Option<Value>)>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

static CLIENT: LazyLock<Option<reqwest::Client>> = LazyLock::new(|| {
	reqwest::Client::builder()
		.user_agent(USER_AGENT)
		.timeout(TIMEOUT)
		.build()
		.map_err(|error| tracing::warn!(?error, "Failed to build the lookup HTTP client"))
		.ok()
});

async fn get_json(url: &str) -> Option<Value> {
	if let Ok(cache) = CACHE.lock() {
		if let Some((at, value)) = cache.get(url) {
			if at.elapsed() < CACHE_TTL {
				return value.clone();
			}
		}
	}
	let client = CLIENT.as_ref()?;
	let value = match client.get(url).send().await {
		Ok(response) if response.status().is_success() => response.json::<Value>().await.ok(),
		Ok(response) => {
			tracing::debug!(url, status = %response.status(), "Lookup answered with an error");
			None
		},
		Err(error) => {
			tracing::debug!(url, ?error, "Lookup failed");
			None
		},
	};
	if let Ok(mut cache) = CACHE.lock() {
		if cache.len() >= CACHE_MAX {
			if let Some(oldest) = cache
				.iter()
				.min_by_key(|(_, (at, _))| *at)
				.map(|(k, _)| k.clone())
			{
				cache.remove(&oldest);
			}
		}
		cache.insert(url.to_string(), (Instant::now(), value.clone()));
	}
	value
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
				.filter_map(Value::as_str)
				.map(|s| s.trim().to_string())
				.filter(|s| !s.is_empty())
				.collect()
		})
		.unwrap_or_default()
}

/// Whether an online title is the same book as ours (main titles compared, subtitles off)
pub fn titles_agree(ours: &str, theirs: &str) -> bool {
	let main = |t: &str| name_tokens(t.split([':', '(']).next().unwrap_or(t)).join(" ");
	let (a, b) = (main(ours), main(theirs));
	!a.is_empty() && !b.is_empty() && (a == b || strsim::jaro_winkler(&a, &b) >= 0.9)
}

fn authors_agree(ours: &[String], theirs: &[String]) -> bool {
	if ours.is_empty() || theirs.is_empty() {
		return true;
	}
	let theirs: Vec<String> = theirs.iter().map(|a| name_keys(a).sorted).collect();
	ours.iter()
		.map(|a| name_keys(a).sorted)
		.any(|k| theirs.iter().any(|t| *t == k || strsim::jaro_winkler(t, &k) >= 0.92))
}

const OPEN_LIBRARY_FIELDS: &str =
	"key,title,author_name,first_publish_year,subject,language,publisher,isbn";

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
	}
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
	}
}

fn strip_html(text: &str) -> String {
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

fn pick<'a>(
	candidates: impl Iterator<Item = &'a Value>,
	parse: fn(&Value) -> OnlineRecord,
	title: Option<&str>,
	authors: &[String],
	by_isbn: bool,
) -> Option<OnlineRecord> {
	candidates.map(parse).find(|record| {
		let title_ok = match (title, record.title.as_deref()) {
			(Some(ours), Some(theirs)) => titles_agree(ours, theirs),
			(None, Some(_)) => by_isbn,
			_ => false,
		};
		title_ok && authors_agree(authors, &record.authors)
	})
}

async fn open_library(
	isbn: Option<&str>,
	title: Option<&str>,
	authors: &[String],
) -> Option<OnlineRecord> {
	let enc = |s: &str| urlencoding::encode(s).into_owned();
	let mut record = None;
	if let Some(isbn) = isbn {
		let url = format!(
			"https://openlibrary.org/search.json?isbn={}&fields={OPEN_LIBRARY_FIELDS}&limit=3",
			enc(isbn)
		);
		if let Some(json) = get_json(&url).await {
			let docs = json.get("docs").and_then(Value::as_array).cloned().unwrap_or_default();
			record = pick(docs.iter(), parse_open_library_doc, title, authors, true);
		}
	}
	if record.is_none() {
		let title = title?;
		let mut url = format!(
			"https://openlibrary.org/search.json?title={}&fields={OPEN_LIBRARY_FIELDS}&limit=5",
			enc(title)
		);
		if let Some(author) = authors.first() {
			url.push_str(&format!("&author={}", enc(author)));
		}
		let json = get_json(&url).await?;
		let docs = json.get("docs").and_then(Value::as_array).cloned().unwrap_or_default();
		record = pick(docs.iter(), parse_open_library_doc, Some(title), authors, false);
	}
	record
}

async fn google_books(
	isbn: Option<&str>,
	title: Option<&str>,
	authors: &[String],
) -> Option<OnlineRecord> {
	let enc = |s: &str| urlencoding::encode(s).into_owned();
	let query = match (isbn, title) {
		(Some(isbn), _) => format!("isbn:{}", enc(isbn)),
		(None, Some(title)) => {
			let mut q = format!("intitle:{}", enc(title));
			if let Some(author) = authors.first() {
				q.push_str(&format!("+inauthor:{}", enc(author)));
			}
			q
		},
		_ => return None,
	};
	let url =
		format!("https://www.googleapis.com/books/v1/volumes?q={query}&maxResults=5&printType=books");
	let json = get_json(&url).await?;
	let items = json.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
	pick(items.iter(), parse_google_volume, title, authors, isbn.is_some())
}

/// Looks a book up on Open Library and Google Books at the same time
pub async fn lookup(
	isbn: Option<&str>,
	title: Option<&str>,
	authors: &[String],
) -> Vec<OnlineRecord> {
	let isbn = isbn.map(|i| i.replace(['-', ' '], "")).filter(|i| i.len() >= 10);
	let (ol, gb) = tokio::join!(
		open_library(isbn.as_deref(), title, authors),
		google_books(isbn.as_deref(), title, authors)
	);
	[ol, gb].into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn reads_open_library_and_google_records() {
		let doc = json!({
			"title": "Il nome della rosa",
			"author_name": ["Umberto Eco"],
			"first_publish_year": 1980,
			"language": ["ita", "spa"],
			"subject": ["Fiction", "Monasteries", "Historical fiction"],
			"publisher": ["Bompiani"]
		});
		let record = parse_open_library_doc(&doc);
		assert_eq!(record.first_publish_year, Some(1980));
		assert_eq!(record.language.as_deref(), Some("it"));
		assert_eq!(record.subjects.len(), 3);

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
		assert_eq!(record.description.as_deref(), Some("Una abadía benedictina..."));
		assert_eq!(record.isbn.as_deref(), Some("9788499080307"));
		assert_eq!(record.first_publish_year, Some(2011));
	}

	#[test]
	fn compares_titles_and_authors_loosely() {
		assert!(titles_agree("El nombre de la rosa", "El Nombre de la Rosa: Edición conmemorativa"));
		assert!(!titles_agree("El nombre de la rosa", "El péndulo de Foucault"));
		assert!(authors_agree(&["Eco, Umberto".into()], &["Umberto Eco".into()]));
		assert!(!authors_agree(&["Umberto Eco".into()], &["Dan Brown".into()]));
	}
}
