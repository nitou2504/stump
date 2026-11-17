//! Discover (this week's random picks) in the OPDS 1.2 catalog: the same picks as the web home
//! (the reader's saved rules, the books they may see, seeded with the ISO week). Built on
//! [`VisibilityFixture`]: ana's hidden set H (`adult`, `unrated`, `removed`, `hidden`) must never
//! be picked for her, and a removed book is never picked for anybody.

use super::{entries, feed, titles};
use crate::common::{visibility::VisibilityFixture, TestApp};

const DISCOVER: &str = "/opds/v1.2/discover";

async fn world() -> (TestApp, VisibilityFixture) {
	let app = TestApp::new_with_default_user().await;
	let world = VisibilityFixture::build(&app).await;
	(app, world)
}

#[tokio::test]
async fn the_catalog_has_discover() {
	let (app, world) = world().await;
	let found = entries(&feed(&app, &world.owner_token, "/opds/v1.2/catalog").await);
	let discover = found
		.iter()
		.find(|entry| entry.title == "Discover")
		.expect("a Discover entry in the catalog");
	assert!(discover.href.ends_with(DISCOVER), "{}", discover.href);
}

#[tokio::test]
async fn the_picks_are_the_same_all_week_and_never_a_removed_book() {
	let (app, world) = world().await;
	let first = titles(&feed(&app, &world.owner_token, DISCOVER).await);
	let again = titles(&feed(&app, &world.owner_token, DISCOVER).await);
	assert!(!first.is_empty(), "the owner has books to pick from");
	assert!(first.len() <= 16);
	assert_eq!(
		first, again,
		"a week's picks do not change between two visits"
	);
	assert!(!first.contains(&world.removed.name), "{first:?}");
}

#[tokio::test]
async fn a_friend_is_only_given_books_they_may_read() {
	let (app, world) = world().await;
	let picked = titles(&feed(&app, &world.ana.token, DISCOVER).await);
	for book in world.hidden_from_ana() {
		assert!(
			!picked.contains(&book.name),
			"{} was picked: {picked:?}",
			book.name
		);
	}
	assert!(picked.contains(&world.general.name), "{picked:?}");
}
