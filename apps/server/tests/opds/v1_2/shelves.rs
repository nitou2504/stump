//! Shelves (smart lists) in the OPDS 1.2 catalog (package B4).
//!
//! `/opds/v1.2/smart-lists` holds the shelves the user made, then the public ones of other people
//! (`Name · Ana`, the person as the entry's author) that hold something the user may read. Every
//! test builds the world of [`VisibilityFixture`]: **ana** has limits (her hidden set H is `adult`,
//! `unrated`, `removed` and `hidden`) and may use smart lists, **ben** has no permissions (so no
//! shelves), **zoe** was deleted. What ana is given must never hold anything that comes from H,
//! and a description is text, not markup.

use models::shared::enums::UserPermission;
use serde_json::{json, Value};

use super::{answer, entries, facet_counts, feed, titles, Entry};
use crate::common::{
	account::{CreateTestUser, TestUser},
	visibility::VisibilityFixture,
	TestApp,
};

const OWNER: &str = "initial-server-admin";
const SHELVES: &str = "/opds/v1.2/smart-lists";

async fn world() -> (TestApp, VisibilityFixture) {
	let app = TestApp::new_with_default_user().await;
	let world = VisibilityFixture::build(&app).await;
	(app, world)
}

/// Another user who may use smart lists, without ana's limits
async fn friend_with_lists(app: &TestApp, username: &str) -> TestUser {
	CreateTestUser {
		username: username.to_string(),
		permissions: vec![UserPermission::AccessSmartList],
		..Default::default()
	}
	.insert(app)
	.await
}

/// The rules of a list of the hidden library's books and the general book: for anyone who may not
/// read the hidden library, only the general book is left
fn hidden_library_and_general(world: &VisibilityFixture) -> Value {
	json!([{
		"groups": [
			{ "media": { "libraryId": { "eq": world.hidden_library.id } } },
			{ "media": { "name": { "eq": world.general.name } } },
		],
		"joiner": "OR",
	}])
}

/// The rules of a list that holds the hidden library's books only
fn hidden_library_only(world: &VisibilityFixture) -> Value {
	json!([{
		"groups": [{ "media": { "libraryId": { "eq": world.hidden_library.id } } }],
		"joiner": "AND",
	}])
}

fn general_only(world: &VisibilityFixture) -> Value {
	json!([{
		"groups": [{ "media": { "name": { "eq": world.general.name } } }],
		"joiner": "AND",
	}])
}

/// Makes a shelf as `token` and returns its ID
async fn create(
	app: &TestApp,
	token: &str,
	name: &str,
	description: Option<&str>,
	visibility: &str,
	filters: Value,
) -> String {
	let response = app
		.execute_gql_with_token(
			r#"mutation Create($input: SaveSmartListInput!) {
				createSmartList(input: $input) { id }
			}"#,
			Some(json!({ "input": {
				"filters": filters,
				"name": name,
				"description": description,
				"joiner": "AND",
				"defaultGrouping": "BY_BOOKS",
				"visibility": visibility,
			} })),
			token,
		)
		.await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"]["createSmartList"]["id"]
		.as_str()
		.expect("an id")
		.to_string()
}

fn entry<'a>(entries: &'a [Entry], title: &str) -> &'a Entry {
	entries
		.iter()
		.find(|entry| entry.title == title)
		.unwrap_or_else(|| panic!("no entry titled {title:?} in {entries:?}"))
}

#[tokio::test]
async fn own_shelves_come_first_then_the_public_shelves_of_other_people() {
	let (app, world) = world().await;
	let cat = friend_with_lists(&app, "cat").await;
	let recs = create(
		&app,
		&world.owner_token,
		"Recs",
		Some("<b>Top</b> & more"),
		"PUBLIC",
		hidden_library_and_general(&world),
	)
	.await;
	create(
		&app,
		&world.owner_token,
		"Mine",
		None,
		"PRIVATE",
		general_only(&world),
	)
	.await;
	let alpha = create(
		&app,
		&cat.token,
		"Alpha",
		Some("for everyone"),
		"PUBLIC",
		general_only(&world),
	)
	.await;
	create(
		&app,
		&world.ana.token,
		"Zeta",
		None,
		"PRIVATE",
		general_only(&world),
	)
	.await;

	// ana: her own shelf, then the public ones of cat and the owner, by name; the owner's private
	// shelf is not there
	let text = feed(&app, &world.ana.token, SHELVES).await;
	assert_eq!(
		titles(&text),
		vec![
			"Zeta".to_string(),
			"Alpha · cat".to_string(),
			format!("Recs · {OWNER}"),
		]
	);
	world.assert_text_none_hidden(&text);
	assert!(!text.contains("Mine"), "{text}");
	let found = entries(&text);
	let recs_entry = entry(&found, &format!("Recs · {OWNER}"));
	assert_eq!(recs_entry.content, "&lt;b&gt;Top&lt;/b&gt; &amp; more");
	assert_eq!(recs_entry.author.as_deref(), Some(OWNER));
	assert_eq!(recs_entry.id, format!("smartList:{recs}"));
	assert!(
		recs_entry
			.href
			.ends_with(&format!("/opds/v1.2/smart-lists/{recs}")),
		"{recs_entry:?}"
	);
	let alpha_entry = entry(&found, "Alpha · cat");
	assert_eq!(alpha_entry.content, "for everyone");
	assert_eq!(alpha_entry.author.as_deref(), Some("cat"));
	assert_eq!(alpha_entry.id, format!("smartList:{alpha}"));
	assert_eq!(entry(&found, "Zeta").author, None);

	// the owner: their shelves by name, a description as it was written, then cat's. Ana's private
	// shelf is not there
	let text = feed(&app, &world.owner_token, SHELVES).await;
	assert_eq!(
		titles(&text),
		vec![
			"Mine".to_string(),
			"Recs".to_string(),
			"Alpha · cat".to_string()
		]
	);
	assert_eq!(entry(&entries(&text), "Recs").content, "<b>Top</b> & more");
	assert!(!text.contains("Zeta"), "{text}");
}

#[tokio::test]
async fn a_shelf_of_somebody_else_with_nothing_to_read_in_it_is_not_shown() {
	let (app, world) = world().await;
	let cat = friend_with_lists(&app, "cat").await;
	// all of it is H for ana
	let grown_up = create(
		&app,
		&world.owner_token,
		"Grown-up",
		Some("not for her"),
		"PUBLIC",
		hidden_library_only(&world),
	)
	.await;
	// the owner's own shelf stays whatever it holds
	let text = feed(&app, &world.owner_token, SHELVES).await;
	assert_eq!(titles(&text), vec!["Grown-up".to_string()]);

	let text = feed(&app, &world.ana.token, SHELVES).await;
	assert!(entries(&text).is_empty(), "{text}");
	for trace in ["Grown-up", "not for her", grown_up.as_str()] {
		assert!(!text.contains(trace), "{trace} in {text}");
	}
	world.assert_text_none_hidden(&text);

	// someone who may read the hidden library is shown it
	let text = feed(&app, &cat.token, SHELVES).await;
	assert_eq!(titles(&text), vec![format!("Grown-up · {OWNER}")]);
}

#[tokio::test]
async fn a_description_is_text_whoever_wrote_it() {
	let (app, world) = world().await;
	let cat = friend_with_lists(&app, "cat").await;
	let nasty =
		r#"<img src=x onerror="alert('1')"><a href="https://evil.example">click</a>"#;
	create(
		&app,
		&cat.token,
		"Cat's",
		Some(nasty),
		"PUBLIC",
		general_only(&world),
	)
	.await;

	let text = feed(&app, &world.owner_token, SHELVES).await;
	let found = entries(&text);
	let content = &entry(&found, "Cat's · cat").content;
	assert!(!content.contains('<'), "{content}");
	assert!(!content.contains('>'), "{content}");
	assert!(
		content.contains("&lt;img src=x onerror=&quot;"),
		"{content}"
	);
	assert!(!text.contains("<img"), "{text}");
	assert!(!text.contains("<a href"), "{text}");
}

#[tokio::test]
async fn the_books_of_a_shelf_are_the_ones_the_viewer_may_read() {
	let (app, world) = world().await;
	let recs = create(
		&app,
		&world.owner_token,
		"Recs",
		None,
		"PUBLIC",
		hidden_library_and_general(&world),
	)
	.await;
	let path = format!("{SHELVES}/{recs}");

	// ana is given the one book of the two she may read, and nothing of the other: no title, no
	// ID, no path, and the facets count what she may read
	let text = feed(&app, &world.ana.token, &path).await;
	assert_eq!(titles(&text), vec![world.general.name.clone()], "{text}");
	assert!(text.contains(&world.general.id), "{text}");
	world.assert_text_none_hidden(&text);
	assert!(
		facet_counts(&text).iter().all(|count| *count <= 1),
		"a facet counts what she may not read: {:?} in {text}",
		facet_counts(&text)
	);
	assert!(
		text.contains(&format!("<title>Recs · {OWNER}</title>")),
		"{text}"
	);

	// the owner is given two, under the shelf's own name
	let text = feed(&app, &world.owner_token, &path).await;
	assert!(text.contains(&world.general.id), "{text}");
	assert!(text.contains(&world.hidden.id), "{text}");
	assert!(text.contains("<title>Recs</title>"), "{text}");
}

#[tokio::test]
async fn a_private_shelf_is_not_found_like_one_that_does_not_exist() {
	let (app, world) = world().await;
	let private = create(
		&app,
		&world.owner_token,
		"Private",
		Some("secret"),
		"PRIVATE",
		general_only(&world),
	)
	.await;

	let unknown =
		answer(&app, &world.ana.token, &format!("{SHELVES}/no-such-list")).await;
	assert_eq!(unknown.0, 404);
	let hidden = answer(&app, &world.ana.token, &format!("{SHELVES}/{private}")).await;
	assert_eq!(hidden.0, 404);
	assert_eq!(
		hidden.1.replace(&private, "ID"),
		unknown.1.replace("no-such-list", "ID")
	);
}

/// Shelves are for people who may use smart lists, as before
#[tokio::test]
async fn without_the_permission_there_are_no_shelves() {
	let (app, world) = world().await;
	create(
		&app,
		&world.owner_token,
		"Recs",
		None,
		"PUBLIC",
		general_only(&world),
	)
	.await;
	let (status, _) = answer(&app, &world.ben.token, SHELVES).await;
	assert_eq!(status, 403);
}

/// The catalog says what the shelves are now
#[tokio::test]
async fn the_catalog_entry_is_about_shelves_not_only_yours() {
	let (app, world) = world().await;
	let text = feed(&app, &world.owner_token, "/opds/v1.2/catalog").await;
	let found = entries(&text);
	let shelves = entry(&found, "Shelves");
	assert_eq!(shelves.content, "Browse shelves");
	assert!(shelves.href.ends_with("/opds/v1.2/smart-lists"));
}
