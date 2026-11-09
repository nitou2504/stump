use crate::common::{
	book::create_nth_readthrough, series::setup_single_series_with_n_books, TestApp,
};
use models::entity::media;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};
use serde_json::{json, Value};
use tests::fake_data;

const QUERY: &str = r#"
query($seed: String, $count: Int!, $filter: MediaFilterInput!) {
    randomMedia(seed: $seed, count: $count, filter: $filter) { id }
}
"#;

async fn pick(
	app: &TestApp,
	seed: Option<&str>,
	count: i32,
	filter: Value,
) -> Vec<String> {
	let response = app
		.execute_gql(
			QUERY,
			Some(json!({"seed": seed, "count": count, "filter": filter})),
		)
		.await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"]["randomMedia"]
		.as_array()
		.unwrap()
		.iter()
		.map(|book| book["id"].as_str().unwrap().to_string())
		.collect()
}

/// A seed always picks the same books, another seed others; unread-only leaves out books the
/// user has read, and removed books are never picked
#[tokio::test]
async fn random_media_is_stable_per_seed_and_respects_the_filter() {
	let app = TestApp::new_with_default_user().await;
	let (_, books) =
		setup_single_series_with_n_books(&app, fake_data::Series::default(), 30).await;
	let unread = json!({"readingStatus": {"is": "NOT_STARTED"}});

	let week = pick(&app, Some("2026-W46"), 5, unread.clone()).await;
	assert_eq!(week.len(), 5);
	assert_eq!(pick(&app, Some("2026-W46"), 5, unread.clone()).await, week);
	assert_ne!(
		pick(&app, Some("2026-W46#1"), 5, unread.clone()).await,
		week
	);

	create_nth_readthrough(&app, &week[0], 1).await;
	let mut removed = books
		.iter()
		.find(|book| book.id == week[1])
		.unwrap()
		.clone()
		.into_active_model();
	removed.deleted_at = Set(Some(chrono::Utc::now().into()));
	let _: media::Model = removed.update(app.conn()).await.unwrap();

	let all = pick(&app, Some("x"), 50, unread).await;
	assert_eq!(all.len(), 28);
	assert!(!all.contains(&week[0]) && !all.contains(&week[1]));
	// the rest of the week's picks stay where they were
	let after = pick(
		&app,
		Some("2026-W46"),
		3,
		json!({"readingStatus": {"is": "NOT_STARTED"}}),
	)
	.await;
	assert_eq!(after, week[2..5]);

	let surprise = pick(&app, None, 1, json!({})).await;
	assert_eq!(surprise.len(), 1);
}

async fn set_tags(app: &TestApp, mutation: &str, id: &str, tags: Value) {
	let query = format!(
		"mutation($id: ID!, $tags: [String!]!) {{ {mutation}(id: $id, tags: $tags) {{ id }} }}"
	);
	let response = app
		.execute_gql(&query, Some(json!({"id": id, "tags": tags})))
		.await;
	assert!(response.get("errors").is_none(), "{response}");
}

/// The `No picks` tag (any case) on a book, its series or its library keeps it out
#[tokio::test]
async fn books_tagged_no_picks_are_never_picked() {
	let app = TestApp::new_with_default_user().await;
	let (_, books) =
		setup_single_series_with_n_books(&app, fake_data::Series::default(), 4).await;
	let (series, others) =
		setup_single_series_with_n_books(&app, fake_data::Series::default(), 3).await;

	set_tags(
		&app,
		"setMediaTags",
		&books[0].id,
		json!(["Favourite", "No Picks"]),
	)
	.await;
	let picked = pick(&app, Some("s"), 50, json!({})).await;
	assert_eq!(picked.len(), 6);
	assert!(!picked.contains(&books[0].id));

	set_tags(&app, "setSeriesTags", &series.id, json!(["no picks"])).await;
	let picked = pick(&app, Some("s"), 50, json!({})).await;
	assert_eq!(picked.len(), 3);
	assert!(others.iter().all(|book| !picked.contains(&book.id)));
}

async fn add_metadata(
	app: &TestApp,
	id: &str,
	genres: Option<&str>,
	rating: Option<f64>,
) {
	models::entity::media_metadata::ActiveModel {
		media_id: Set(Some(id.to_string())),
		genres: Set(genres.map(str::to_string)),
		rating: Set(rating.map(|r| r.to_string().parse().unwrap())),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.unwrap();
}

async fn pool(app: &TestApp, rules: Option<Value>) -> u64 {
	let response = app
		.execute_gql(
			"query($r: DiscoverRulesInput) { discoverPoolSize(rules: $r) }",
			Some(json!({"r": rules})),
		)
		.await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"]["discoverPoolSize"].as_u64().unwrap()
}

/// The saved rules decide what can be picked: genres (whole or `Family: *`), libraries,
/// a minimum rating (unrated books stay) and unread only; the pool size previews them
#[tokio::test]
async fn discover_rules_shape_the_picks() {
	let app = TestApp::new_with_default_user().await;
	let (_, books) =
		setup_single_series_with_n_books(&app, fake_data::Series::default(), 6).await;
	let (_, elsewhere) =
		setup_single_series_with_n_books(&app, fake_data::Series::default(), 2).await;
	add_metadata(
		&app,
		&books[0].id,
		Some("History, Level: High School"),
		Some(4.5),
	)
	.await;
	add_metadata(&app, &books[1].id, Some("Reference: Atlases & Maps"), None).await;
	add_metadata(&app, &books[2].id, Some("Science Fiction"), Some(2.0)).await;
	add_metadata(&app, &books[3].id, Some("Level: High Schoolish"), Some(4.0)).await;
	create_nth_readthrough(&app, &books[4].id, 1).await;

	let defaults = app
		.execute_gql(
			"{ myDiscoverRules { unreadOnly excludedTags excludedGenres excludedLibraryIds minRating } }",
			None,
		)
		.await;
	assert_eq!(
		defaults["data"]["myDiscoverRules"],
		json!({"unreadOnly": true, "excludedTags": ["No picks"], "excludedGenres": [],
			"excludedLibraryIds": [], "minRating": null})
	);
	// 8 books, one read
	assert_eq!(pool(&app, None).await, 7);

	let other_library = elsewhere[0].library_id.clone();
	assert_ne!(other_library, books[0].library_id);
	let rules = json!({
		"unreadOnly": false,
		"excludedGenres": ["Level: High School", "reference: *", " "],
		"excludedLibraryIds": [other_library],
		"minRating": 3,
	});
	// previewed, not saved
	assert_eq!(pool(&app, Some(rules.clone())).await, 3);
	assert_eq!(pool(&app, None).await, 7);

	let saved = app
		.execute_gql(
			"mutation($r: DiscoverRulesInput!) { updateMyDiscoverRules(input: $r) { excludedGenres minRating } }",
			Some(json!({"r": rules})),
		)
		.await;
	assert_eq!(
		saved["data"]["updateMyDiscoverRules"],
		json!({"excludedGenres": ["Level: High School", "reference: *"], "minRating": 3.0})
	);
	let mut picked = pick(&app, Some("s"), 50, json!({})).await;
	picked.sort();
	// the High Schoolish one, the read one and the unrated one stay
	let mut expected = vec![
		books[3].id.clone(),
		books[4].id.clone(),
		books[5].id.clone(),
	];
	expected.sort();
	assert_eq!(picked, expected);
}
