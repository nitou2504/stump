//! `/api/v2/content`: the text index of the books. The world: an owner, a friend with a library
//! hidden from them and a kid with an age limit, and four EPUBs (copies of two fixtures, so each
//! has a path of its own) tagged `Estante: …`, indexed the way the job does it.
//!
//! The scope filters (`tags_any`, `tags_none`, `exclude`) and the user's own limits are checked on
//! EVERY endpoint, by id too: `read` and `grep` can't reach what `search` hides.

use std::path::PathBuf;

use models::entity::{library, media};
use sea_orm::{ActiveModelTrait, ActiveValue, IntoActiveModel};
use serde_json::Value;
use stump_core::content::job::{index_book, ContentIndexTask};

use crate::common::{
	account::TestUser,
	visibility::{
		add_book, add_library, add_series, friend, hide_library, kid, Meta,
	},
	TestApp,
};

fn fixture(name: &str) -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("../../core/integration-tests/data")
		.join(name)
		.canonicalize()
		.expect("fixture")
}

struct World {
	app: TestApp,
	_dir: tempfile::TempDir,
	friend: TestUser,
	kid: TestUser,
	#[allow(dead_code)]
	hidden_library: library::Model,
	/// Alice, `Estante: Novela`
	alice: media::Model,
	/// Whitman, `Estante: Poesía`
	whitman: media::Model,
	/// Alice, `Estante: Novela`, rated 18
	adult: media::Model,
	/// Whitman, `Estante: Poesía`, in the library hidden from the friend
	secret: media::Model,
}

async fn put_file(
	app: &TestApp,
	dir: &std::path::Path,
	book: media::Model,
	fixture_name: &str,
	copy_as: &str,
) -> media::Model {
	let target = dir.join(copy_as);
	std::fs::copy(fixture(fixture_name), &target).expect("copy the fixture");
	let size = std::fs::metadata(&target).unwrap().len() as i64;
	let mut active = book.into_active_model();
	active.path = ActiveValue::Set(target.to_string_lossy().to_string());
	active.extension = ActiveValue::Set("epub".to_string());
	active.size = ActiveValue::Set(size);
	let book = active.update(app.conn()).await.expect("update the book");
	index_book(
		app.conn(),
		app.ctx.config.as_ref(),
		&ContentIndexTask {
			media_id: book.id.clone(),
			path: book.path.clone(),
			kind: "epub".to_string(),
			size,
			mtime: 1,
		},
	)
	.await
	.expect("index the book");
	book
}

async fn world() -> World {
	let app = TestApp::new_with_default_user().await;
	let dir = tempfile::tempdir().unwrap();
	let open = add_library(&app, "Open").await;
	let hidden_library = add_library(&app, "Secret").await;
	let open_series = add_series(&app, &open, "Open series").await;
	let secret_series = add_series(&app, &hidden_library, "Secret series").await;

	let alice = add_book(
		&app,
		&open_series,
		"Alice",
		Meta {
			writer: Some("Lewis Carroll"),
			genre: Some("Estante: Novela"),
			age: Some(8),
			..Default::default()
		},
	)
	.await;
	let whitman = add_book(
		&app,
		&open_series,
		"Hojas de hierba",
		Meta {
			writer: Some("Walt Whitman"),
			genre: Some("Estante: Poesía, Clásicos"),
			age: Some(8),
			..Default::default()
		},
	)
	.await;
	let adult = add_book(
		&app,
		&open_series,
		"Alice para adultos",
		Meta {
			genre: Some("Estante: Novela"),
			age: Some(18),
			..Default::default()
		},
	)
	.await;
	let secret = add_book(
		&app,
		&secret_series,
		"Hojas secretas",
		Meta {
			genre: Some("Estante: Poesía"),
			age: Some(8),
			..Default::default()
		},
	)
	.await;
	let alice = put_file(&app, dir.path(), alice, "book.epub", "alice.epub").await;
	let whitman = put_file(&app, dir.path(), whitman, "leaves.epub", "leaves.epub").await;
	let adult = put_file(&app, dir.path(), adult, "book.epub", "adult.epub").await;
	let secret = put_file(&app, dir.path(), secret, "leaves.epub", "secret.epub").await;

	let friend = friend(&app).await;
	hide_library(&app, &friend, &hidden_library).await;
	let kid = kid(&app).await;
	World {
		app,
		_dir: dir,
		friend,
		kid,
		hidden_library,
		alice,
		whitman,
		adult,
		secret,
	}
}

fn ids(items: &Value) -> Vec<String> {
	let mut ids: Vec<String> = items
		.as_array()
		.expect("an array")
		.iter()
		.map(|item| item["media_id"].as_str().expect("media_id").to_string())
		.collect();
	ids.sort();
	ids
}

fn sorted(mut list: Vec<String>) -> Vec<String> {
	list.sort();
	list
}

/// Every endpoint that takes a book's id, for that book, with `extra` added to the query
fn by_id_paths(id: &str, extra: &str) -> Vec<String> {
	let base = "/api/v2/content";
	vec![
		format!("{base}/media/{id}/sections?page=1{extra}"),
		format!("{base}/media/{id}/sections/0?pos=0{extra}"),
		format!("{base}/media/{id}/grep?pattern=a{extra}"),
		format!("{base}/search?q=alice&media_id={id}{extra}"),
	]
}

#[tokio::test]
async fn the_routes_need_a_signed_in_user() {
	let w = world().await;
	for path in [
		"/api/v2/content/status",
		"/api/v2/content/search?q=alice",
		"/api/v2/content/tags",
		"/api/v2/content/lookup?path=/x",
		&format!("/api/v2/content/media/{}/sections", w.alice.id),
	] {
		let response = w.app.server.get(path).await;
		assert!(
			response.status_code().is_client_error(),
			"{path}: {}",
			response.status_code()
		);
	}
	let response = w.app.server.post("/api/v2/content/reindex").await;
	assert!(response.status_code().is_client_error());
}

#[tokio::test]
async fn status_counts_what_the_user_may_see() {
	let w = world().await;
	let owner: Value = w.app.get("/api/v2/content/status").await.json();
	assert_eq!(owner["books"], 4);
	assert_eq!(owner["indexed"], 4);
	assert_eq!(owner["pending"], 0);
	assert_eq!(owner["failed"], 0);
	assert!(owner["last_run"].is_null());

	let response = w
		.app
		.get_with_token("/api/v2/content/status", &w.friend.token)
		.await;
	let friend: Value = response.json();
	assert_eq!(friend["books"], 3, "the library hidden from the friend isn't counted");
	let response = w
		.app
		.get_with_token("/api/v2/content/status", &w.kid.token)
		.await;
	let kid: Value = response.json();
	assert_eq!(kid["books"], 3, "nor the book above the kid's age");

	let scoped: Value = w
		.app
		.get("/api/v2/content/status?tags_none=estante:%20novela")
		.await
		.json();
	assert_eq!(scoped["books"], 2);
}

#[tokio::test]
async fn books_that_are_not_epubs_or_pdfs_are_not_counted() {
	let w = world().await;
	let series = add_series(&w.app, &library_of(&w).await, "Another series").await;
	let comic = add_book(&w.app, &series, "Comic", Meta::default()).await;
	let mut active = comic.into_active_model();
	active.extension = ActiveValue::Set("cbz".to_string());
	active.path = ActiveValue::Set("/books/comic.cbz".to_string());
	active.update(w.app.conn()).await.unwrap();
	let status: Value = w.app.get("/api/v2/content/status").await.json();
	assert_eq!(status["books"], 4);
}

async fn library_of(w: &World) -> library::Model {
	use sea_orm::EntityTrait;
	library::Entity::find()
		.one(w.app.conn())
		.await
		.unwrap()
		.expect("a library")
}

#[tokio::test]
async fn reindex_is_for_the_owner_and_library_managers() {
	let w = world().await;
	let response = w
		.app
		.server
		.post("/api/v2/content/reindex")
		.add_header("Authorization", format!("Bearer {}", w.friend.token))
		.await;
	response.assert_status_forbidden();

	let response = w
		.app
		.server
		.post("/api/v2/content/reindex")
		.add_header("Authorization", w.app.auth_header().await)
		.await;
	response.assert_status_ok();
	let body: Value = response.json();
	assert!(body["job_id"].as_str().is_some_and(|id| id.len() >= 32));
}

#[tokio::test]
async fn text_search_finds_the_best_passage_of_each_book_without_case_or_accents() {
	let w = world().await;
	for q in ["Alice", "ALICE", "alicé", "rabbit"] {
		let body: Value = w
			.app
			.get(&format!("/api/v2/content/search?q={q}"))
			.await
			.json();
		assert_eq!(
			ids(&body["items"]),
			sorted(vec![w.alice.id.clone(), w.adult.id.clone()]),
			"{q}"
		);
		assert_eq!(body["total"], 2);
		assert_eq!(body["page"], 1);
		assert_eq!(body["page_size"], 10);
		let item = &body["items"][0];
		assert!(item["snippet"].as_str().unwrap().to_lowercase().contains(q.to_lowercase().trim_end_matches('é')));
		assert!(item["section"].is_number());
		assert!(item["pos"].is_number());
		assert!(item["score"].as_f64().unwrap() > 0.0);
		assert!(item["authors"].is_array());
		assert!(item["tags"].is_array());
		assert_eq!(item["library"], "Open");
	}
	// phrases and match=all
	let body: Value = w
		.app
		.get("/api/v2/content/search?q=alice%20whitman&match=all")
		.await
		.json();
	assert_eq!(body["total"], 0);
	let body: Value = w
		.app
		.get("/api/v2/content/search?q=alice%20whitman&match=any")
		.await
		.json();
	assert_eq!(body["total"], 4);
}

#[tokio::test]
async fn text_search_in_one_book_lists_passages_and_pages() {
	let w = world().await;
	let first: Value = w
		.app
		.get(&format!(
			"/api/v2/content/search?q=alice&media_id={}&page_size=2",
			w.alice.id
		))
		.await
		.json();
	assert!(first["total"].as_u64().unwrap() > 2);
	assert_eq!(first["items"].as_array().unwrap().len(), 2);
	let sections: Vec<i64> = first["items"]
		.as_array()
		.unwrap()
		.iter()
		.map(|i| i["section"].as_i64().unwrap())
		.collect();
	assert_ne!(sections[0], sections[1]);
	let second: Value = w
		.app
		.get(&format!(
			"/api/v2/content/search?q=alice&media_id={}&page_size=2&page=2",
			w.alice.id
		))
		.await
		.json();
	assert_ne!(first["items"][0]["section"], second["items"][0]["section"]);
}

#[tokio::test]
async fn catalog_search_looks_at_title_authors_tags_and_contents() {
	let w = world().await;
	let by_author: Value = w
		.app
		.get("/api/v2/content/search?mode=catalog&q=carroll")
		.await
		.json();
	assert_eq!(ids(&by_author["items"]), vec![w.alice.id.clone()]);
	assert!(by_author["items"][0]["section"].is_null());
	let by_tag: Value = w
		.app
		.get("/api/v2/content/search?mode=catalog&q=poesia")
		.await
		.json();
	assert_eq!(
		ids(&by_tag["items"]),
		sorted(vec![w.whitman.id.clone(), w.secret.id.clone()])
	);
	// the text of the book is not the catalog
	let by_text: Value = w
		.app
		.get("/api/v2/content/search?mode=catalog&q=cheshire")
		.await
		.json();
	assert_eq!(by_text["total"], 0);
	// but a chapter title is
	let by_toc: Value = w
		.app
		.get("/api/v2/content/search?mode=catalog&q=rabbit-hole")
		.await
		.json();
	assert_eq!(
		ids(&by_toc["items"]),
		sorted(vec![w.alice.id.clone(), w.adult.id.clone()])
	);
	let by_title_all: Value = w
		.app
		.get("/api/v2/content/search?mode=catalog&q=hojas%20hierba&match=all")
		.await
		.json();
	assert_eq!(ids(&by_title_all["items"]), vec![w.whitman.id.clone()]);
}

#[tokio::test]
async fn search_filters_by_library_language_and_author() {
	let w = world().await;
	let library = library_of(&w).await;
	let body: Value = w
		.app
		.get(&format!(
			"/api/v2/content/search?q=whitman&library_id={}",
			library.id
		))
		.await
		.json();
	assert_eq!(ids(&body["items"]), vec![w.whitman.id.clone()]);
	let body: Value = w
		.app
		.get("/api/v2/content/search?q=whitman&author=whitman")
		.await
		.json();
	assert_eq!(ids(&body["items"]), vec![w.whitman.id.clone()]);
	let body: Value = w
		.app
		.get("/api/v2/content/search?q=whitman&language=fr")
		.await
		.json();
	assert_eq!(body["total"], 0);
}

#[tokio::test]
async fn the_users_own_limits_hold_on_every_endpoint() {
	let w = world().await;
	// the kid can't reach the 18+ book, by id on any endpoint, nor in search, tags or lookup
	for path in by_id_paths(&w.adult.id, "") {
		w.app
			.get_with_token(&path, &w.kid.token)
			.await
			.assert_status_not_found();
		w.app.get(&path).await.assert_status_ok();
	}
	let search: Value = w
		.app
		.get_with_token("/api/v2/content/search?q=alice", &w.kid.token)
		.await
		.json();
	assert_eq!(ids(&search["items"]), vec![w.alice.id.clone()]);
	w.app
		.get_with_token(
			&format!("/api/v2/content/lookup?path={}", w.adult.path),
			&w.kid.token,
		)
		.await
		.assert_status_not_found();

	// the friend can't reach the book of the hidden library
	for path in by_id_paths(&w.secret.id, "") {
		w.app
			.get_with_token(&path, &w.friend.token)
			.await
			.assert_status_not_found();
		w.app.get(&path).await.assert_status_ok();
	}
	let search: Value = w
		.app
		.get_with_token("/api/v2/content/search?q=whitman", &w.friend.token)
		.await
		.json();
	assert_eq!(ids(&search["items"]), vec![w.whitman.id.clone()]);
	let catalog: Value = w
		.app
		.get_with_token("/api/v2/content/search?mode=catalog&q=hojas", &w.friend.token)
		.await
		.json();
	assert_eq!(ids(&catalog["items"]), vec![w.whitman.id.clone()]);
	w.app
		.get_with_token(
			&format!("/api/v2/content/lookup?path={}", w.secret.path),
			&w.friend.token,
		)
		.await
		.assert_status_not_found();
	let tags: Value = w
		.app
		.get_with_token("/api/v2/content/tags", &w.friend.token)
		.await
		.json();
	let poesia = tags["tags"]
		.as_array()
		.unwrap()
		.iter()
		.find(|t| t["name"] == "Estante: Poesía")
		.expect("the tag");
	assert_eq!(poesia["count"], 1, "counted over what the friend sees only");
}

/// The scope filters, on every endpoint, for the book each one hides
#[tokio::test]
async fn the_scope_filters_hold_on_every_endpoint() {
	let w = world().await;
	let novel_hidden = [
		// blocked tags, in another case and without the accent
		"&tags_none=ESTANTE:%20novela".to_string(),
		// only the poetry shelf
		"&tags_any=estante:%20poesia".to_string(),
		// the book itself, in a list
		format!("&exclude=x1,{},x2", w.alice.id),
	];
	for scope in &novel_hidden {
		for path in by_id_paths(&w.alice.id, scope) {
			w.app.get(&path).await.assert_status_not_found();
		}
		let search: Value = w
			.app
			.get(&format!("/api/v2/content/search?q=alice{scope}"))
			.await
			.json();
		assert!(
			!ids(&search["items"]).contains(&w.alice.id),
			"search {scope}"
		);
		let catalog: Value = w
			.app
			.get(&format!("/api/v2/content/search?mode=catalog&q=alice{scope}"))
			.await
			.json();
		assert!(
			!ids(&catalog["items"]).contains(&w.alice.id),
			"catalog {scope}"
		);
	}
	// and the book that passes still answers on the same endpoints
	for scope in [
		"&tags_none=estante:%20novela",
		"&tags_any=estante:%20poesia,otra",
		"&tags_any=clasicos&tags_none=Policial",
		"&exclude=nobody",
	] {
		let ok = &w.whitman.id;
		for path in [
			format!("/api/v2/content/media/{ok}/sections?page=1{scope}"),
			format!("/api/v2/content/media/{ok}/sections/0?pos=0{scope}"),
			format!("/api/v2/content/media/{ok}/grep?pattern=whitman{scope}"),
			format!("/api/v2/content/search?q=whitman&media_id={ok}{scope}"),
		] {
			let response = w.app.get(&path).await;
			assert!(response.status_code().is_success(), "{path}");
		}
	}
	// the status and the tags follow the scope too
	let tags: Value = w
		.app
		.get("/api/v2/content/tags?tags_none=estante:%20novela")
		.await
		.json();
	let names: Vec<&str> = tags["tags"]
		.as_array()
		.unwrap()
		.iter()
		.map(|t| t["name"].as_str().unwrap())
		.collect();
	assert!(!names.contains(&"Estante: Novela"));
	assert!(names.contains(&"Estante: Poesía"));
	let tags: Value = w
		.app
		.get(&format!("/api/v2/content/tags?exclude={},{}", w.whitman.id, w.secret.id))
		.await
		.json();
	assert!(tags["tags"]
		.as_array()
		.unwrap()
		.iter()
		.all(|t| t["name"] != "Estante: Poesía"));
	// scope and limits together: blocked tag AND hidden library
	let search: Value = w
		.app
		.get_with_token(
			"/api/v2/content/search?q=whitman&tags_none=clasicos",
			&w.friend.token,
		)
		.await
		.json();
	assert_eq!(search["total"], 0);
}

#[tokio::test]
async fn tags_count_books_once_each_over_the_visible_ones() {
	let w = world().await;
	let tags: Value = w.app.get("/api/v2/content/tags").await.json();
	let count = |name: &str| {
		tags["tags"]
			.as_array()
			.unwrap()
			.iter()
			.find(|t| t["name"] == name)
			.map(|t| t["count"].as_u64().unwrap())
	};
	assert_eq!(count("Estante: Novela"), Some(2));
	assert_eq!(count("Estante: Poesía"), Some(2));
	assert_eq!(count("Clásicos"), Some(1));
}

#[tokio::test]
async fn lookup_resolves_a_path_to_its_media_id() {
	let w = world().await;
	let body: Value = w
		.app
		.get(&format!("/api/v2/content/lookup?path={}", w.alice.path))
		.await
		.json();
	assert_eq!(body["media_id"], w.alice.id);
	w.app
		.get("/api/v2/content/lookup?path=/nowhere/x.epub")
		.await
		.assert_status_not_found();
	w.app
		.get("/api/v2/content/lookup")
		.await
		.assert_status_bad_request();
}

#[tokio::test]
async fn sections_and_reading_page_through_a_book() {
	let w = world().await;
	let list: Value = w
		.app
		.get(&format!(
			"/api/v2/content/media/{}/sections?page_size=500",
			w.alice.id
		))
		.await
		.json();
	assert_eq!(list["media_id"], w.alice.id);
	assert_eq!(list["title"], "Alice");
	assert_eq!(list["authors"][0], "Lewis Carroll");
	let total = list["total"].as_u64().unwrap();
	assert!(total > 2);
	assert_eq!(
		list["sections"].as_array().unwrap().len(),
		50.min(total as usize),
		"page_size is capped at 50"
	);
	let first = &list["sections"][0];
	assert_eq!(first["n"], 0);
	assert!(first["chars"].as_u64().unwrap() > 0);

	// read a long section in pieces: they join to its whole text
	let (n, chars_total) = list["sections"]
		.as_array()
		.unwrap()
		.iter()
		.map(|s| (s["n"].as_i64().unwrap(), s["chars"].as_u64().unwrap()))
		.max_by_key(|(_, chars)| *chars)
		.unwrap();
	assert!(chars_total > 600, "the fixture has a long section");
	let mut pos = 0u64;
	let mut text = String::new();
	let mut pieces = 0;
	loop {
		let piece: Value = w
			.app
			.get(&format!(
				"/api/v2/content/media/{}/sections/{n}?pos={pos}&chars=500",
				w.alice.id
			))
			.await
			.json();
		assert_eq!(piece["chars_total"].as_u64().unwrap(), chars_total);
		assert_eq!(piece["pos"].as_u64().unwrap(), pos);
		text.push_str(piece["text"].as_str().unwrap());
		pieces += 1;
		match piece["next_pos"].as_u64() {
			Some(next) => {
				assert!(next > pos);
				// the pieces end at a space: the next one starts there
				pos = next;
			},
			None => break,
		}
		assert!(pieces < 500);
	}
	assert!(pieces >= 2);
	assert_eq!(text.chars().count() as u64, chars_total);

	// caps and errors
	let big: Value = w
		.app
		.get(&format!(
			"/api/v2/content/media/{}/sections/{n}?chars=99999999",
			w.alice.id
		))
		.await
		.json();
	assert!(big["text"].as_str().unwrap().chars().count() <= 20_000);
	w.app
		.get(&format!("/api/v2/content/media/{}/sections/{n}?chars=0", w.alice.id))
		.await
		.assert_status_bad_request();
	w.app
		.get(&format!(
			"/api/v2/content/media/{}/sections/{n}?pos={}",
			w.alice.id,
			chars_total + 1
		))
		.await
		.assert_status_bad_request();
	w.app
		.get(&format!("/api/v2/content/media/{}/sections/99999", w.alice.id))
		.await
		.assert_status_not_found();
	w.app
		.get("/api/v2/content/media/nope/sections")
		.await
		.assert_status_not_found();
}

#[tokio::test]
async fn grep_finds_literals_and_regexes_and_refuses_runaway_patterns() {
	let w = world().await;
	let url = |extra: &str| format!("/api/v2/content/media/{}/grep?{extra}", w.alice.id);

	let literal: Value = w.app.get(&url("pattern=white%20rabbit")).await.json();
	assert!(literal["total"].as_u64().unwrap() > 0);
	assert_eq!(literal["page"], 1);
	let hit = &literal["hits"][0];
	assert!(hit["snippet"].as_str().unwrap().to_lowercase().contains("white rabbit"));
	assert!(hit["n"].is_number() && hit["pos"].is_number());

	// case: matters only when asked
	let exact: Value = w.app.get(&url("pattern=WHITE%20RABBIT&case=true")).await.json();
	assert_eq!(exact["total"], 0);
	let loose: Value = w.app.get(&url("pattern=WHITE%20RABBIT")).await.json();
	assert_eq!(loose["total"], literal["total"]);

	// a regex; a literal never is one
	let re: Value = w
		.app
		.get(&url("pattern=white%5Cs%2Brab%7B2%7Dit&regex=true"))
		.await
		.json();
	assert_eq!(re["total"], literal["total"]);
	let not_a_regex: Value = w.app.get(&url("pattern=r.bbit")).await.json();
	assert_eq!(not_a_regex["total"], 0);

	// one section, and pages
	let n = hit["n"].as_i64().unwrap();
	let in_section: Value = w
		.app
		.get(&url(&format!("pattern=rabbit&section={n}")))
		.await
		.json();
	assert!(in_section["hits"].as_array().unwrap().iter().all(|h| h["n"] == n));
	let page: Value = w.app.get(&url("pattern=the&page_size=3&page=2")).await.json();
	assert_eq!(page["hits"].as_array().unwrap().len(), 3);
	assert_eq!(page["page"], 2);
	let capped: Value = w.app.get(&url("pattern=the&page_size=9999")).await.json();
	assert_eq!(capped["hits"].as_array().unwrap().len(), 50);

	// bad input
	w.app.get(&url("pattern=")).await.assert_status_bad_request();
	w.app.get(&url("")).await.assert_status_bad_request();
	w.app
		.get(&url(&format!("pattern={}", "a".repeat(201))))
		.await
		.assert_status_bad_request();
	w.app
		.get(&url("pattern=(unclosed&regex=true"))
		.await
		.assert_status_bad_request();
	w.app
		.get(&url("pattern=((a%7B100%7D)%7B100%7D)%7B100%7D&regex=true"))
		.await
		.assert_status_bad_request();
	// the classic catastrophic pattern answers, and quickly
	let started = std::time::Instant::now();
	let response = w.app.get(&url("pattern=(a%2B)%2B%24&regex=true")).await;
	response.assert_status_ok();
	assert!(started.elapsed() < std::time::Duration::from_secs(10));
}

#[tokio::test]
async fn inputs_are_validated_and_capped() {
	let w = world().await;
	let get = |path: String| {
		let app = &w.app;
		async move { app.get(&path).await }
	};
	get("/api/v2/content/search".to_string())
		.await
		.assert_status_bad_request();
	get("/api/v2/content/search?q=%22%22%20%3F".to_string())
		.await
		.assert_status_bad_request();
	get(format!("/api/v2/content/search?q={}", "a".repeat(257)))
		.await
		.assert_status_bad_request();
	get(format!(
		"/api/v2/content/search?q={}",
		(0..13).map(|i| format!("w{i}")).collect::<Vec<_>>().join("%20")
	))
	.await
	.assert_status_bad_request();
	get("/api/v2/content/search?q=a&mode=vector".to_string())
		.await
		.assert_status_bad_request();
	get("/api/v2/content/search?q=a&match=some".to_string())
		.await
		.assert_status_bad_request();
	get("/api/v2/content/search?q=a&page=0".to_string())
		.await
		.assert_status_bad_request();
	get("/api/v2/content/search?q=a&page_size=0".to_string())
		.await
		.assert_status_bad_request();
	get("/api/v2/content/search?q=a&page=abc".to_string())
		.await
		.assert_status_bad_request();
	let capped: Value = get("/api/v2/content/search?q=alice&page_size=500".to_string())
		.await
		.json();
	assert_eq!(capped["page_size"], 50);
	// the scope's lists are capped, on every endpoint
	let many = (0..501).map(|i| format!("id{i}")).collect::<Vec<_>>().join(",");
	let fine = (0..500).map(|i| format!("id{i}")).collect::<Vec<_>>().join(",");
	for path in [
		"/api/v2/content/status".to_string(),
		"/api/v2/content/tags".to_string(),
		"/api/v2/content/search?q=alice".to_string(),
		format!("/api/v2/content/media/{}/sections", w.alice.id),
		format!("/api/v2/content/media/{}/sections/0", w.alice.id),
		format!("/api/v2/content/media/{}/grep?pattern=a", w.alice.id),
	] {
		let join = if path.contains('?') { '&' } else { '?' };
		get(format!("{path}{join}exclude={many}"))
			.await
			.assert_status_bad_request();
		let ok = get(format!("{path}{join}exclude={fine}")).await;
		assert!(ok.status_code().is_success(), "{path}");
	}
}
