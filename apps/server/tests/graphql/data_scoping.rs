//! What a user is allowed to see is decided by their account (hidden libraries, an age restriction)
//! and has to hold wherever it is listed, counted or written to, not only where a list is built:
//! counts, alphabets, stats, authors, tags, a series' cover or one-shot book, the last library
//! they visited, progress writes and club books.

use crate::common::{
	account::CreateTestUser,
	visibility::{
		add_book, add_library, add_series, errors, friend, hide_library, ids, kid,
		sorted, strings, Meta,
	},
	TestApp,
};
use models::{
	entity::{
		book_club, book_club_book, last_library_visit, library, media, media_tag,
		reading_session, series, tag,
	},
	shared::enums::UserPermission,
};
use sea_orm::{prelude::*, ActiveModelTrait, QueryFilter, Set};
use serde_json::Value;

/// A library "Shelf" with three series: Alpha (two general books), Xenon (two books rated 18) and
/// Mixed (a general book and two rated 18)
struct Shelf {
	library: library::Model,
	alpha: series::Model,
	xenon: series::Model,
	mixed: series::Model,
	adult_book: media::Model,
}

async fn setup_shelf(app: &TestApp) -> Shelf {
	let library = add_library(app, "Shelf").await;
	let alpha = add_series(app, &library, "Alpha").await;
	let xenon = add_series(app, &library, "Xenon").await;
	let mixed = add_series(app, &library, "Mixed").await;

	add_book(
		app,
		&alpha,
		"Alpha 1",
		Meta {
			writer: Some("Ann Writer"),
			publisher: Some("Open House"),
			genre: Some("Open Genre"),
			..Default::default()
		},
	)
	.await;
	add_book(app, &alpha, "Alpha 2", Meta::default()).await;

	let adult = Meta {
		age: Some(18),
		writer: Some("Zed Writer"),
		publisher: Some("Hidden House"),
		genre: Some("Hidden Genre"),
		..Default::default()
	};
	let adult_book = add_book(app, &xenon, "Xenon 1", adult).await;
	add_book(
		app,
		&xenon,
		"Xenon 2",
		Meta {
			age: Some(18),
			..Default::default()
		},
	)
	.await;

	add_book(app, &mixed, "Mixed 1", Meta::default()).await;
	for n in 2..=3 {
		add_book(
			app,
			&mixed,
			&format!("Mixed {n}"),
			Meta {
				age: Some(18),
				..Default::default()
			},
		)
		.await;
	}

	Shelf {
		library,
		alpha,
		xenon,
		mixed,
		adult_book,
	}
}

const OVERVIEW: &str = r#"
	query Overview($library: ID!, $xenon: ID!, $mixed: ID!) {
		series { nodes { id mediaCount } }
		xenon: seriesById(id: $xenon) { id }
		mixed: seriesById(id: $mixed) { id mediaCount unreadCount }
		numberOfSeries
		libraryById(id: $library) {
			series { id mediaCount }
			stats { bookCount seriesCount totalBytes }
			authors { name }
			publishers
			genres
			mediaAlphabet
			seriesAlphabet
		}
		librariesStats { bookCount seriesCount totalBytes }
		mediaDiskUsage
		mediaAlphabet
		seriesAlphabet
		librariesAlphabet
	}
"#;

async fn overview(app: &TestApp, shelf: &Shelf, token: &str) -> Value {
	let response = app
		.execute_gql_with_token(
			OVERVIEW,
			Some(serde_json::json!({
				"library": shelf.library.id,
				"xenon": shelf.xenon.id,
				"mixed": shelf.mixed.id,
			})),
			token,
		)
		.await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"].clone()
}

/// The series, books, counts, alphabets and the rest the owner sees are what they are; a user with an
/// age restriction gets the same questions answered about what they may read only
#[tokio::test]
async fn an_age_restricted_user_gets_counts_and_lists_of_what_they_may_read() {
	let app = TestApp::new_with_default_user().await;
	let shelf = setup_shelf(&app).await;
	let kid = kid(&app).await;
	let owner_token = app
		.access_token
		.read()
		.await
		.clone()
		.expect("the owner has a token");

	// the owner sees everything
	let all = overview(&app, &shelf, &owner_token).await;
	assert_eq!(
		ids(&all["series"]["nodes"]),
		sorted(vec![
			shelf.alpha.id.clone(),
			shelf.xenon.id.clone(),
			shelf.mixed.id.clone()
		])
	);
	assert_eq!(all["xenon"]["id"], shelf.xenon.id.as_str());
	assert_eq!(all["mixed"]["mediaCount"], 3);
	assert_eq!(all["numberOfSeries"], 3);
	assert_eq!(all["librariesStats"]["bookCount"], 7);
	assert_eq!(all["librariesStats"]["seriesCount"], 3);
	assert_eq!(all["mediaDiskUsage"], 7 * 1234);
	assert_eq!(all["libraryById"]["stats"]["bookCount"], 7);
	assert_eq!(
		strings(&all["libraryById"]["authors"], Some("name")),
		vec!["Ann Writer".to_string(), "Zed Writer".to_string()]
	);
	assert_eq!(
		strings(&all["libraryById"]["publishers"], None),
		vec!["Hidden House".to_string(), "Open House".to_string()]
	);
	assert_eq!(all["mediaAlphabet"]["X"], 2);
	assert_eq!(all["seriesAlphabet"]["X"], 1);
	assert_eq!(all["libraryById"]["mediaAlphabet"]["X"], 2);
	assert_eq!(all["libraryById"]["seriesAlphabet"]["X"], 1);

	// the kid sees Alpha and the one general book of Mixed, nothing of Xenon
	let kid_view = overview(&app, &shelf, &kid.token).await;
	assert_eq!(
		ids(&kid_view["series"]["nodes"]),
		sorted(vec![shelf.alpha.id.clone(), shelf.mixed.id.clone()])
	);
	assert!(kid_view["xenon"].is_null(), "{kid_view}");
	assert_eq!(kid_view["mixed"]["mediaCount"], 1);
	assert_eq!(kid_view["mixed"]["unreadCount"], 1);
	assert_eq!(kid_view["numberOfSeries"], 2);
	let library_series = kid_view["libraryById"]["series"].clone();
	assert_eq!(
		ids(&library_series),
		sorted(vec![shelf.alpha.id.clone(), shelf.mixed.id.clone()])
	);
	let count_of = |id: &str| {
		library_series
			.as_array()
			.unwrap()
			.iter()
			.find(|series| series["id"] == id)
			.map(|series| series["mediaCount"].clone())
	};
	assert_eq!(count_of(&shelf.alpha.id), Some(serde_json::json!(2)));
	assert_eq!(count_of(&shelf.mixed.id), Some(serde_json::json!(1)));

	// totals
	assert_eq!(kid_view["librariesStats"]["bookCount"], 3);
	assert_eq!(kid_view["librariesStats"]["seriesCount"], 2);
	assert_eq!(kid_view["librariesStats"]["totalBytes"], 3 * 1234);
	assert_eq!(kid_view["libraryById"]["stats"]["bookCount"], 3);
	assert_eq!(kid_view["libraryById"]["stats"]["seriesCount"], 2);
	assert_eq!(kid_view["mediaDiskUsage"], 3 * 1234);

	// who wrote, published and what genre the hidden books are
	assert_eq!(
		strings(&kid_view["libraryById"]["authors"], Some("name")),
		vec!["Ann Writer".to_string()]
	);
	assert_eq!(
		strings(&kid_view["libraryById"]["publishers"], None),
		vec!["Open House".to_string()]
	);
	assert_eq!(
		strings(&kid_view["libraryById"]["genres"], None),
		vec!["Open Genre".to_string()]
	);

	// the letters the hidden books and series start with
	assert_eq!(kid_view["mediaAlphabet"]["X"], 0);
	assert_eq!(kid_view["seriesAlphabet"]["X"], 0);
	assert_eq!(kid_view["libraryById"]["mediaAlphabet"]["X"], 0);
	assert_eq!(kid_view["libraryById"]["seriesAlphabet"]["X"], 0);
	assert_eq!(kid_view["mediaAlphabet"]["A"], 2);
	assert_eq!(kid_view["mediaAlphabet"]["M"], 1);
	assert_eq!(kid_view["seriesAlphabet"]["M"], 1);
}

/// A library hidden from a user is out of every total too, and its first letter
#[tokio::test]
async fn a_hidden_library_is_not_in_the_totals_or_the_alphabet() {
	let app = TestApp::new_with_default_user().await;
	setup_shelf(&app).await;
	let zine = add_library(&app, "Zine").await;
	let zine_series = add_series(&app, &zine, "Zzz").await;
	add_book(&app, &zine_series, "Zzz 1", Meta::default()).await;
	let friend = friend(&app).await;
	hide_library(&app, &friend, &zine).await;

	let totals = r#"{
		librariesStats { bookCount seriesCount }
		librariesAlphabet
		mediaAlphabet
		seriesAlphabet
		mediaDiskUsage
		numberOfLibraries
	}"#;

	let friend_view = app
		.execute_gql_with_token(totals, None, &friend.token)
		.await;
	assert!(friend_view.get("errors").is_none(), "{friend_view}");
	let data = &friend_view["data"];
	assert_eq!(data["librariesStats"]["bookCount"], 7);
	assert_eq!(data["librariesStats"]["seriesCount"], 3);
	assert_eq!(data["numberOfLibraries"], 1);
	assert_eq!(data["librariesAlphabet"]["Z"], 0);
	assert_eq!(data["librariesAlphabet"]["S"], 1);
	assert_eq!(data["mediaAlphabet"]["Z"], 0);
	assert_eq!(data["seriesAlphabet"]["Z"], 0);
	assert_eq!(data["mediaDiskUsage"], 7 * 1234);

	let owner_view = app.execute_gql(totals, None).await;
	let data = &owner_view["data"];
	assert_eq!(data["librariesStats"]["bookCount"], 8);
	assert_eq!(data["librariesAlphabet"]["Z"], 1);
	assert_eq!(data["mediaAlphabet"]["Z"], 1);
	assert_eq!(data["seriesAlphabet"]["Z"], 1);
}

/// A tag is shown to a user if it is on something they can see or on nothing yet, never when it is
/// only on what is hidden from them
#[tokio::test]
async fn tags_only_on_hidden_books_are_not_listed() {
	let app = TestApp::new_with_default_user().await;
	let shelf = setup_shelf(&app).await;
	let kid = kid(&app).await;

	let new_tag = |name: &str| {
		let name = name.to_string();
		let app = &app;
		async move {
			tag::ActiveModel {
				name: Set(name),
				..Default::default()
			}
			.insert(app.conn())
			.await
			.expect("should insert the tag")
		}
	};
	let general = new_tag("general").await;
	let adult_only = new_tag("adult-only").await;
	new_tag("orphan").await;

	let open_book = media::Entity::find()
		.filter(media::Column::Name.eq("Alpha 1"))
		.one(app.conn())
		.await
		.unwrap()
		.unwrap();
	for (book, tag) in [(&open_book, &general), (&shelf.adult_book, &adult_only)] {
		media_tag::ActiveModel {
			media_id: Set(book.id.clone()),
			tag_id: Set(tag.id),
			..Default::default()
		}
		.insert(app.conn())
		.await
		.expect("should tag the book");
	}

	let query = "{ tags { name } }";

	let kid_tags = app.execute_gql_with_token(query, None, &kid.token).await;
	assert_eq!(
		strings(&kid_tags["data"]["tags"], Some("name")),
		vec!["general".to_string(), "orphan".to_string()],
		"{kid_tags}"
	);

	let owner_tags = app.execute_gql(query, None).await;
	assert_eq!(
		strings(&owner_tags["data"]["tags"], Some("name")),
		vec![
			"adult-only".to_string(),
			"general".to_string(),
			"orphan".to_string()
		]
	);
}

/// The last library a user visited is gone from `lastVisitedLibrary` once it is hidden from them
#[tokio::test]
async fn the_last_visited_library_follows_what_is_hidden() {
	let app = TestApp::new_with_default_user().await;
	let library = add_library(&app, "Visited").await;
	let friend = friend(&app).await;

	last_library_visit::ActiveModel {
		user_id: Set(friend.id.clone()),
		library_id: Set(library.id.clone()),
		timestamp: Set(chrono::Utc::now().into()),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should record the visit");

	let query = "{ lastVisitedLibrary { id name } }";

	let before = app.execute_gql_with_token(query, None, &friend.token).await;
	assert_eq!(
		before["data"]["lastVisitedLibrary"]["id"],
		library.id.as_str(),
		"{before}"
	);

	hide_library(&app, &friend, &library).await;

	let after = app.execute_gql_with_token(query, None, &friend.token).await;
	assert!(after.get("errors").is_none(), "{after}");
	assert!(after["data"]["lastVisitedLibrary"].is_null(), "{after}");
}

/// How many reading sessions the book has, of anyone
async fn sessions_of(app: &TestApp, book_id: &str) -> u64 {
	reading_session::Entity::find()
		.filter(reading_session::Column::MediaId.eq(book_id))
		.count(app.conn())
		.await
		.expect("should count the sessions")
}

/// Progress can't be written to a book the user can't see: it answers like a read does, and leaves
/// nothing behind (it used to say how many pages a hidden book has)
#[tokio::test]
async fn progress_is_refused_for_books_the_user_cannot_see() {
	let app = TestApp::new_with_default_user().await;
	let open_library = add_library(&app, "Open").await;
	let open_series = add_series(&app, &open_library, "Open series").await;
	let open_book = add_book(&app, &open_series, "Open book", Meta::default()).await;
	let hidden_library = add_library(&app, "Hidden").await;
	let hidden_series = add_series(&app, &hidden_library, "Hidden series").await;
	let hidden_book =
		add_book(&app, &hidden_series, "Hidden book", Meta::default()).await;
	let adult_book = add_book(
		&app,
		&open_series,
		"Adult book",
		Meta {
			age: Some(18),
			..Default::default()
		},
	)
	.await;

	let friend = friend(&app).await;
	hide_library(&app, &friend, &hidden_library).await;
	let kid = kid(&app).await;

	let update = |id: &str, page: i32| {
		format!(
			r#"mutation {{ updateMediaProgress(id: "{id}", input: {{ paged: {{ page: {page} }} }}) {{ id status }} }}"#
		)
	};
	let accept = |id: &str| {
		format!(
			r#"mutation {{ acceptLocalProgress(id: "{id}", input: {{ paged: {{ page: 2 }} }}) {{ id }} }}"#
		)
	};
	let finish = |id: &str| format!(r#"mutation {{ finishMediaProgress(id: "{id}") }}"#);

	// a book in a hidden library, for the user it is hidden from
	for query in [
		update(&hidden_book.id, 10),
		accept(&hidden_book.id),
		finish(&hidden_book.id),
	] {
		let response = app
			.execute_gql_with_token(&query, None, &friend.token)
			.await;
		assert!(errors(&response).contains("Media not found"), "{response}");
	}
	assert_eq!(sessions_of(&app, &hidden_book.id).await, 0);

	// a book above the age restriction, for the user it is hidden from
	for query in [
		update(&adult_book.id, 10),
		accept(&adult_book.id),
		finish(&adult_book.id),
	] {
		let response = app.execute_gql_with_token(&query, None, &kid.token).await;
		assert!(errors(&response).contains("Media not found"), "{response}");
	}
	assert_eq!(sessions_of(&app, &adult_book.id).await, 0);

	// an ID that is no book at all gets the same answer, so the difference gives nothing away
	let response = app
		.execute_gql_with_token(&update("nothing", 1), None, &friend.token)
		.await;
	assert!(errors(&response).contains("Media not found"), "{response}");

	// the same books work for those who can see them, and what they can see works for everyone
	let response = app.execute_gql(&update(&hidden_book.id, 5), None).await;
	assert!(response.get("errors").is_none(), "{response}");
	assert_eq!(sessions_of(&app, &hidden_book.id).await, 1);
	let response = app
		.execute_gql_with_token(&update(&open_book.id, 5), None, &friend.token)
		.await;
	assert!(response.get("errors").is_none(), "{response}");
	assert_eq!(response["data"]["updateMediaProgress"]["status"], "READING");
	assert_eq!(sessions_of(&app, &open_book.id).await, 1);
	let response = app
		.execute_gql_with_token(&finish(&open_book.id), None, &kid.token)
		.await;
	assert!(response.get("errors").is_none(), "{response}");
	assert_eq!(response["data"]["finishMediaProgress"], true);
}

/// Everyone's reading is for those who may read users; what each can read is still all they get
#[tokio::test]
async fn stats_for_all_users_need_the_permission_to_read_users() {
	let app = TestApp::new_with_default_user().await;
	let shelf = setup_shelf(&app).await;
	let friend = friend(&app).await;
	let reader = CreateTestUser {
		username: "reader".to_string(),
		permissions: vec![UserPermission::ReadUsers],
		..Default::default()
	}
	.insert(&app)
	.await;

	// the owner finishes a book
	let finished = media::Entity::find()
		.filter(media::Column::Name.eq("Alpha 2"))
		.one(app.conn())
		.await
		.unwrap()
		.unwrap();
	let response = app
		.execute_gql(
			&format!(
				r#"mutation {{ finishMediaProgress(id: "{}") }}"#,
				finished.id
			),
			None,
		)
		.await;
	assert!(response.get("errors").is_none(), "{response}");

	let query = |all_users: bool| {
		format!(
			r#"{{
				libraryById(id: "{lib}") {{ stats(allUsers: {all_users}) {{ completedBooks bookCount }} }}
				seriesById(id: "{series}") {{ stats(allUsers: {all_users}) {{ completedBooks bookCount }} }}
			}}"#,
			lib = shelf.library.id,
			series = shelf.alpha.id,
		)
	};

	// their own stats, for everyone
	for token in [friend.token.as_str(), reader.token.as_str()] {
		let own = app.execute_gql_with_token(&query(false), None, token).await;
		assert!(own.get("errors").is_none(), "{own}");
		assert_eq!(own["data"]["libraryById"]["stats"]["completedBooks"], 0);
		assert_eq!(own["data"]["seriesById"]["stats"]["completedBooks"], 0);
	}

	// everyone's, not for a friend...
	let denied = app
		.execute_gql_with_token(&query(true), None, &friend.token)
		.await;
	assert!(
		errors(&denied).contains("do not have permission"),
		"{denied}"
	);
	assert!(denied["data"]["libraryById"].is_null(), "{denied}");

	// ...but for those who may read users, and the owner
	for token in [
		reader.token.clone(),
		app.access_token.read().await.clone().unwrap(),
	] {
		let all = app.execute_gql_with_token(&query(true), None, &token).await;
		assert!(all.get("errors").is_none(), "{all}");
		assert_eq!(all["data"]["libraryById"]["stats"]["completedBooks"], 1);
		assert_eq!(all["data"]["libraryById"]["stats"]["bookCount"], 7);
		assert_eq!(all["data"]["seriesById"]["stats"]["completedBooks"], 1);
		assert_eq!(all["data"]["seriesById"]["stats"]["bookCount"], 2);
	}
}

/// "no such user" and "no avatar" were two messages, which tells which IDs belong to a user
#[tokio::test]
async fn the_avatar_route_does_not_tell_which_ids_are_users() {
	let app = TestApp::new_with_default_user().await;
	let friend = friend(&app).await;

	let unknown = app.get("/api/v2/users/nobody-at-all/avatar").await;
	let without_avatar = app
		.get(&format!("/api/v2/users/{}/avatar", friend.id))
		.await;

	assert_eq!(unknown.status_code(), 404);
	assert_eq!(without_avatar.status_code(), 404);
	assert_eq!(unknown.text(), without_avatar.text());
	assert!(!without_avatar.text().contains(&friend.id));
}

/// A club book is read by the members of its club (anyone for a public club), not by whoever has the
/// ID, and its title doesn't show for a book the user can't see
#[tokio::test]
async fn a_club_book_is_for_the_members_of_its_club() {
	let app = TestApp::new_with_default_user().await;
	let shelf = setup_shelf(&app).await;
	let friend = friend(&app).await;

	let club = book_club::ActiveModel {
		name: Set("Secret club".to_string()),
		is_private: Set(true),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should insert the club");
	let club_book = book_club_book::ActiveModel {
		id: Set("club-book".to_string()),
		position: Set(1),
		book_club_id: Set(club.id.clone()),
		book_entity_id: Set(Some(shelf.adult_book.id.clone())),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should insert the club book");

	let query = format!(
		r#"{{ bookClubBook(id: "{}") {{ id bookClubId bookEntityId entity {{ id }} }} }}"#,
		club_book.id
	);

	// not a member of the private club: not found, like a book that doesn't exist
	let response = app
		.execute_gql_with_token(&query, None, &friend.token)
		.await;
	assert!(errors(&response).contains("Book not found"), "{response}");
	assert!(!response.to_string().contains(&club.id), "{response}");

	// the owner reads it, with the book
	let response = app.execute_gql(&query, None).await;
	assert!(response.get("errors").is_none(), "{response}");
	assert_eq!(
		response["data"]["bookClubBook"]["entity"]["id"],
		shelf.adult_book.id.as_str()
	);
}

async fn set_series_cover(app: &TestApp, series: &series::Model, path: &str) {
	let mut active: series::ActiveModel = series.clone().into();
	active.thumbnail_path = Set(Some(path.to_string()));
	active
		.update(app.conn())
		.await
		.expect("should set the cover of the series");
}

/// The cover the server makes for a series is the cover of its first book, whatever that book's age
/// rating (it gave the cover of a book rated 18 to a user who may not open the book). A user with an
/// age restriction gets the cover of a book they may read instead; a cover set by hand stays.
#[tokio::test]
async fn a_generated_series_cover_is_not_shown_to_an_age_restricted_user() {
	let app = TestApp::new_with_default_user().await;
	let shelf = setup_shelf(&app).await;
	let kid = kid(&app).await;

	let dir = std::env::temp_dir().join(format!("stump-data-scoping-{}", shelf.mixed.id));
	std::fs::create_dir_all(&dir).expect("should make the folder for the covers");
	let write = |name: &str, bytes: &[u8]| {
		let path = dir.join(name);
		std::fs::write(&path, bytes).expect("should write the cover");
		path.to_string_lossy().to_string()
	};
	let generated = write(
		&format!("{}.generated.webp", shelf.mixed.id),
		b"cover of an adult book",
	);
	let by_hand = write("by-hand.webp", b"cover set by hand");
	let readable = write("readable.webp", b"cover of a readable book");

	// the one book of "Mixed" the kid may read has a cover of its own
	let readable_book = media::Entity::find()
		.filter(media::Column::Name.eq("Mixed 1"))
		.one(app.conn())
		.await
		.expect("should look for the book")
		.expect("the book is there");
	let mut book: media::ActiveModel = readable_book.into();
	book.thumbnail_path = Set(Some(readable));
	book.update(app.conn())
		.await
		.expect("should set the cover of the book");

	let route = format!("/api/v2/series/{}/thumbnail", shelf.mixed.id);
	let cover_for_the_kid = || async {
		let response = app
			.server
			.get(&route)
			.add_header("Authorization", format!("Bearer {}", kid.token))
			.await;
		assert_eq!(response.status_code(), 200, "{}", response.text());
		response.as_bytes().to_vec()
	};
	let cover_for_the_owner = || async {
		let response = app.get(&route).await;
		assert_eq!(response.status_code(), 200, "{}", response.text());
		response.as_bytes().to_vec()
	};

	// a generated cover: the owner gets it, the kid gets the cover of the book they may read
	set_series_cover(&app, &shelf.mixed, &generated).await;
	assert_eq!(
		cover_for_the_owner().await,
		b"cover of an adult book".to_vec()
	);
	assert_eq!(
		cover_for_the_kid().await,
		b"cover of a readable book".to_vec()
	);

	// one set by hand is what it was set to, for everyone
	set_series_cover(&app, &shelf.mixed, &by_hand).await;
	assert_eq!(cover_for_the_owner().await, b"cover set by hand".to_vec());
	assert_eq!(cover_for_the_kid().await, b"cover set by hand".to_vec());

	let _ = std::fs::remove_dir_all(&dir);
}
