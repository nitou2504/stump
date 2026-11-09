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
