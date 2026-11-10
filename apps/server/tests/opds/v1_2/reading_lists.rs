//! Reading lists in the OPDS 1.2 catalog (package B4).
//!
//! `/opds/v1.2/reading-lists` holds the lists the user made, then the public ones of other people
//! (`Name · Ana`, the person as the entry's author), each with the number of books the user may
//! read in it. Every test builds the world of [`VisibilityFixture`]: **ana** has limits (her hidden
//! set H is `adult`, `unrated`, `removed` and `hidden`), **ben** has none, **zoe** was deleted, and
//! the owner sees everything but `removed`. What ana is given must never hold anything that comes
//! from H, and what a person wrote in a description must arrive as text, not as markup.

use models::entity::{reading_list, reading_list_item};
use sea_orm::{prelude::*, ActiveModelTrait, Set};
use serde_json::json;

use super::{answer, entries, facet_counts, feed, titles, Entry};
use crate::common::{visibility::VisibilityFixture, TestApp};

const OWNER: &str = "initial-server-admin";
const LISTS: &str = "/opds/v1.2/reading-lists";

async fn world() -> (TestApp, VisibilityFixture) {
	let app = TestApp::new_with_default_user().await;
	let world = VisibilityFixture::build(&app).await;
	(app, world)
}

/// Makes a list as `token` and returns its ID
async fn create(
	app: &TestApp,
	token: &str,
	name: &str,
	description: Option<&str>,
	visibility: &str,
	media: &[&String],
) -> String {
	let response = app
		.execute_gql_with_token(
			r#"mutation Create($input: ReadingListInput!) {
				createReadingList(input: $input) { id }
			}"#,
			Some(json!({ "input": {
				"name": name,
				"description": description,
				"visibility": visibility,
				"mediaIds": media,
			} })),
			token,
		)
		.await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"]["createReadingList"]["id"]
		.as_str()
		.expect("the new list's id")
		.to_string()
}

fn entry<'a>(entries: &'a [Entry], title: &str) -> &'a Entry {
	entries
		.iter()
		.find(|entry| entry.title == title)
		.unwrap_or_else(|| panic!("no entry titled {title:?} in {entries:?}"))
}

/// The books in a feed of books: its entries that are not navigation
fn book_titles(feed: &str) -> Vec<String> {
	let mut titles = titles(feed);
	titles.sort();
	titles
}

#[tokio::test]
async fn own_lists_come_first_then_the_public_lists_of_other_people() {
	let (app, world) = world().await;
	let everything = world
		.all_books()
		.iter()
		.map(|book| &book.id)
		.collect::<Vec<_>>();
	// what the owner wrote is not what a reader should be given as markup
	let recs = create(
		&app,
		&world.owner_token,
		"Recs",
		Some("<b>Top</b> & more"),
		"PUBLIC",
		&everything,
	)
	.await;
	let _mine = create(
		&app,
		&world.owner_token,
		"Mine",
		None,
		"PRIVATE",
		&[&world.general.id],
	)
	.await;
	let alpha = create(
		&app,
		&world.ben.token,
		"Alpha",
		Some("for everyone"),
		"PUBLIC",
		&[&world.general.id],
	)
	.await;
	let _zeta = create(
		&app,
		&world.ana.token,
		"Zeta",
		None,
		"PRIVATE",
		&[&world.general.id],
	)
	.await;

	// ana: her own list, then the public ones of ben and the owner, by name. The owner's private
	// list is not there at all
	let text = feed(&app, &world.ana.token, LISTS).await;
	assert_eq!(
		titles(&text),
		vec![
			"Zeta".to_string(),
			"Alpha · ben".to_string(),
			format!("Recs · {OWNER}"),
		]
	);
	world.assert_text_none_hidden(&text);
	assert!(!text.contains("Mine"), "{text}");
	let found = entries(&text);
	let recs_entry = entry(&found, &format!("Recs · {OWNER}"));
	// she may read one of the five books of the list
	assert_eq!(
		recs_entry.content,
		"1 book · &lt;b&gt;Top&lt;/b&gt; &amp; more"
	);
	assert_eq!(recs_entry.author.as_deref(), Some(OWNER));
	assert_eq!(recs_entry.id, format!("readingList:{recs}"));
	assert!(
		recs_entry
			.href
			.ends_with(&format!("/opds/v1.2/reading-lists/{recs}")),
		"{recs_entry:?}"
	);
	let alpha_entry = entry(&found, "Alpha · ben");
	assert_eq!(alpha_entry.content, "1 book · for everyone");
	assert_eq!(alpha_entry.author.as_deref(), Some("ben"));
	assert_eq!(alpha_entry.id, format!("readingList:{alpha}"));
	// her own list has no author to name
	assert_eq!(entry(&found, "Zeta").author, None);
	assert_eq!(entry(&found, "Zeta").content, "1 book");

	// the owner: their lists, the private one included, each with the books the owner may read (the
	// removed book is out for everybody), a description as it was written, then ben's. Ana's
	// private list is not there
	let text = feed(&app, &world.owner_token, LISTS).await;
	assert_eq!(
		titles(&text),
		vec![
			"Mine".to_string(),
			"Recs".to_string(),
			"Alpha · ben".to_string()
		]
	);
	let found = entries(&text);
	assert_eq!(entry(&found, "Recs").content, "4 books · <b>Top</b> & more");
	assert_eq!(entry(&found, "Recs").author, None);
	assert_eq!(
		entry(&found, "Alpha · ben").content,
		"1 book · for everyone"
	);
	assert!(!text.contains("Zeta"), "{text}");
}

#[tokio::test]
async fn a_description_is_text_whoever_wrote_it_and_whoever_reads_it() {
	let (app, world) = world().await;
	let nasty =
		r#"<img src=x onerror="alert('1')"><a href="https://evil.example">click</a>"#;
	create(
		&app,
		&world.ben.token,
		"Ben's",
		Some(nasty),
		"PUBLIC",
		&[&world.general.id],
	)
	.await;

	// the owner is the one who is shown what a friend wrote
	let text = feed(&app, &world.owner_token, LISTS).await;
	let found = entries(&text);
	let content = &entry(&found, "Ben's · ben").content;
	assert!(!content.contains('<'), "{content}");
	assert!(!content.contains('>'), "{content}");
	assert!(!content.contains('"'), "{content}");
	assert!(
		content.contains("&lt;img src=x onerror=&quot;"),
		"{content}"
	);
	assert!(
		content
			.contains("&lt;a href=&quot;https://evil.example&quot;&gt;click&lt;/a&gt;"),
		"{content}"
	);
	// in the XML the escaped text was escaped once more: nothing in it can be taken for a tag
	assert!(!text.contains("<img"), "{text}");
	assert!(!text.contains("<a href"), "{text}");
}

#[tokio::test]
async fn a_list_of_somebody_else_with_nothing_to_read_in_it_is_not_shown() {
	let (app, world) = world().await;
	// all of it is H for ana
	let grown_up = create(
		&app,
		&world.owner_token,
		"Grown-up",
		Some("not for her"),
		"PUBLIC",
		&[&world.adult.id, &world.unrated.id, &world.hidden.id],
	)
	.await;
	// ana's own empty list is hers, and stays
	create(&app, &world.ana.token, "Empty", None, "PRIVATE", &[]).await;

	let text = feed(&app, &world.ana.token, LISTS).await;
	assert_eq!(titles(&text), vec!["Empty".to_string()]);
	assert_eq!(entries(&text)[0].content, "0 books");
	for trace in ["Grown-up", "not for her", grown_up.as_str()] {
		assert!(!text.contains(trace), "{trace} in {text}");
	}
	world.assert_text_none_hidden(&text);

	// the people it is for see it, with the three books
	let text = feed(&app, &world.ben.token, LISTS).await;
	let found = entries(&text);
	assert_eq!(
		entry(&found, &format!("Grown-up · {OWNER}")).content,
		"3 books · not for her"
	);
}

#[tokio::test]
async fn the_lists_of_people_who_are_gone_are_not_shown() {
	let (app, world) = world().await;
	let gone = reading_list::ActiveModel {
		id: Set(Uuid::new_v4().to_string()),
		name: Set("Gone".to_string()),
		description: Set(Some("zoe's".to_string())),
		updated_at: Set(chrono::Utc::now().into()),
		visibility: Set("PUBLIC".to_string()),
		ordering: Set("MANUAL".to_string()),
		creating_user_id: Set(world.zoe.id.clone()),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should insert zoe's list");
	reading_list_item::ActiveModel {
		reading_list_id: Set(gone.id.clone()),
		media_id: Set(world.general.id.clone()),
		display_order: Set(0),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should insert the item");

	// zoe was deleted: her list is in the database, and in no feed
	for token in [&world.owner_token, &world.ben.token, &world.ana.token] {
		let text = feed(&app, token, LISTS).await;
		assert!(entries(&text).is_empty(), "{text}");
		assert!(!text.contains("Gone"), "{text}");
		let (status, _) = answer(&app, token, &format!("{LISTS}/{}", gone.id)).await;
		assert_eq!(status, 404);
	}
}

#[tokio::test]
async fn the_books_of_a_list_are_the_ones_the_viewer_may_read() {
	let (app, world) = world().await;
	let everything = world
		.all_books()
		.iter()
		.map(|book| &book.id)
		.collect::<Vec<_>>();
	let recs = create(
		&app,
		&world.owner_token,
		"Recs",
		None,
		"PUBLIC",
		&everything,
	)
	.await;
	let path = format!("{LISTS}/{recs}");

	// ana is given the one book of the five she may read, and nothing of the others: no title, no
	// ID, no path, no count (the facets count what she may read)
	let text = feed(&app, &world.ana.token, &path).await;
	assert_eq!(book_titles(&text), vec![world.general.name.clone()]);
	assert!(text.contains(&world.general.id), "{text}");
	world.assert_text_none_hidden(&text);
	let counts = facet_counts(&text);
	assert!(
		counts.iter().all(|count| *count <= 1),
		"a facet counts what she may not read: {counts:?} in {text}"
	);
	// the feed is the list's, said as it is said in the list of lists
	assert!(
		text.contains(&format!("<title>Recs · {OWNER}</title>")),
		"{text}"
	);

	// the owner and ben are given four (the removed one is out for everybody), and their facets
	// count them: a feed with no counts at all would pass the check above for nothing
	for token in [&world.owner_token, &world.ben.token] {
		let text = feed(&app, token, &path).await;
		assert_eq!(book_titles(&text).len(), 4, "{text}");
		assert!(!text.contains(&world.removed.id), "{text}");
		assert!(
			facet_counts(&text).iter().any(|count| *count == 4),
			"{:?} in {text}",
			facet_counts(&text)
		);
	}
	// the owner reads their own list under its own name
	let text = feed(&app, &world.owner_token, &path).await;
	assert!(text.contains("<title>Recs</title>"), "{text}");
}

#[tokio::test]
async fn a_private_list_is_not_found_like_one_that_does_not_exist() {
	let (app, world) = world().await;
	let private = create(
		&app,
		&world.owner_token,
		"Private",
		Some("secret"),
		"PRIVATE",
		&[&world.general.id],
	)
	.await;

	let unknown = answer(&app, &world.ana.token, &format!("{LISTS}/no-such-list")).await;
	assert_eq!(unknown.0, 404);
	let hidden = answer(&app, &world.ana.token, &format!("{LISTS}/{private}")).await;
	assert_eq!(hidden.0, 404);
	// the answer says nothing more about a list that is there than about one that is not (but
	// for the id the request itself carried)
	assert_eq!(
		hidden.1.replace(&private, "ID"),
		unknown.1.replace("no-such-list", "ID")
	);

	// the creator reads it
	let text = feed(&app, &world.owner_token, &format!("{LISTS}/{private}")).await;
	assert_eq!(book_titles(&text), vec![world.general.name.clone()]);
}

#[tokio::test]
async fn making_a_list_public_or_private_changes_what_the_others_are_shown() {
	let (app, world) = world().await;
	let recs = create(
		&app,
		&world.owner_token,
		"Recs",
		None,
		"PRIVATE",
		&[&world.general.id],
	)
	.await;
	assert!(entries(&feed(&app, &world.ben.token, LISTS).await).is_empty());

	let set = |visibility: &str| {
		let app = &app;
		let token = world.owner_token.clone();
		let id = recs.clone();
		let visibility = visibility.to_string();
		async move {
			let response = app
				.execute_gql_with_token(
					r#"mutation Update($id: ID!, $input: UpdateReadingListInput!) {
						updateReadingList(id: $id, input: $input) { id }
					}"#,
					Some(
						json!({ "id": id, "input": { "name": "Recs", "visibility": visibility } }),
					),
					&token,
				)
				.await;
			assert!(response.get("errors").is_none(), "{response}");
		}
	};
	set("PUBLIC").await;
	assert_eq!(
		titles(&feed(&app, &world.ben.token, LISTS).await),
		vec![format!("Recs · {OWNER}")]
	);
	set("PRIVATE").await;
	assert!(entries(&feed(&app, &world.ben.token, LISTS).await).is_empty());
	let (status, _) = answer(&app, &world.ben.token, &format!("{LISTS}/{recs}")).await;
	assert_eq!(status, 404);
}

/// The catalog says what the lists are now
#[tokio::test]
async fn the_catalog_entry_is_about_reading_lists_not_only_yours() {
	let (app, world) = world().await;
	let text = feed(&app, &world.owner_token, "/opds/v1.2/catalog").await;
	let found = entries(&text);
	let reading_lists = entry(&found, "Reading lists");
	assert_eq!(reading_lists.content, "Browse reading lists");
	assert!(reading_lists.href.ends_with("/opds/v1.2/reading-lists"));
}
