use crate::common::{
	account::CreateTestUser, series::setup_single_series_with_n_books, TestApp,
};
use graphql::input::user::AgeRestrictionInput;
use models::entity::media;
use sea_orm::{prelude::*, sea_query::Expr, QueryFilter};
use serde_json::{json, Value};
use tests::fake_data;

fn errors(response: &Value) -> String {
	response["errors"].to_string()
}

/// `userById` with an ID nobody has used to `unwrap()` the lookup: with `panic = "abort"` one
/// request with an unknown ID (an old link to a deleted user) took the whole server down
#[tokio::test]
async fn user_by_id_with_an_unknown_id_is_an_error() {
	let app = TestApp::new_with_default_user().await;

	let response = app
		.execute_gql(r#"{ userById(id: "nobody") { id } }"#, None)
		.await;
	assert!(errors(&response).contains("User not found"), "{response}");

	// the server is still there
	let me = app.execute_gql("{ me { username } }", None).await;
	assert_eq!(me["data"]["me"]["username"], "initial-server-admin");
}

/// Series don't track an abandoned status, and the status is in the schema: any user sending
/// it hit an `unimplemented!()`, which aborts the whole server
#[tokio::test]
async fn filtering_series_by_abandoned_status_does_not_crash() {
	let app = TestApp::new_with_default_user().await;
	setup_single_series_with_n_books(&app, fake_data::Series::default(), 2).await;

	let query = r#"
		query($filter: SeriesFilterInput!) {
			series(filter: $filter) { nodes { id } }
		}
	"#;
	let ids = |response: &Value| response["data"]["series"]["nodes"].clone();

	// none is abandoned...
	let abandoned = app
		.execute_gql(
			query,
			Some(json!({ "filter": { "readingStatus": { "is": "ABANDONED" } } })),
		)
		.await;
	assert!(abandoned.get("errors").is_none(), "{abandoned}");
	assert_eq!(ids(&abandoned), json!([]));

	// ...so all are not abandoned
	let not_abandoned = app
		.execute_gql(
			query,
			Some(json!({ "filter": { "readingStatus": { "isNot": "ABANDONED" } } })),
		)
		.await;
	assert!(not_abandoned.get("errors").is_none(), "{not_abandoned}");
	assert_eq!(ids(&not_abandoned).as_array().unwrap().len(), 1);
}

/// The query that took the server down: the books of a series, their series, its books again
#[tokio::test]
async fn nested_lists_are_refused_before_they_run() {
	let app = TestApp::new_with_default_user().await;
	let (series, _) =
		setup_single_series_with_n_books(&app, fake_data::Series::default(), 3).await;

	let nested = format!(
		r#"{{ seriesById(id: "{}") {{ media {{ id series {{ media {{ id }} }} }} }} }}"#,
		series.id
	);
	let response = app.execute_gql(&nested, None).await;
	assert!(errors(&response).contains("too complex"), "{response}");
	assert!(response["data"].is_null(), "{response}");

	// one level of the same is what the clients do, and still works
	let flat = format!(
		r#"{{ seriesById(id: "{}") {{ media(take: 50) {{ id series {{ id }} }} }} }}"#,
		series.id
	);
	let response = app.execute_gql(&flat, None).await;
	assert!(response.get("errors").is_none(), "{response}");
	assert_eq!(
		response["data"]["seriesById"]["media"]
			.as_array()
			.unwrap()
			.len(),
		3
	);
}

/// A page is at most 200 books; the page info says how big it really was. Everything at once is
/// refused for the big tables, and a cursor limit is bounded
#[tokio::test]
async fn pages_and_lists_are_bounded() {
	let app = TestApp::new_with_default_user().await;
	let (series, _) =
		setup_single_series_with_n_books(&app, fake_data::Series::default(), 3).await;

	let page = app
		.execute_gql(
			r#"{ media(pagination: { offset: { page: 1, pageSize: 5000 } }) {
				nodes { id }
				pageInfo { ... on OffsetPaginationInfo { pageSize totalItems } }
			} }"#,
			None,
		)
		.await;
	assert!(page.get("errors").is_none(), "{page}");
	assert_eq!(page["data"]["media"]["pageInfo"]["pageSize"], 200);
	assert_eq!(page["data"]["media"]["pageInfo"]["totalItems"], 3);

	let everything = app
		.execute_gql(
			r#"{ media(pagination: { none: { unpaginated: true } }) { nodes { id } } }"#,
			None,
		)
		.await;
	assert!(
		errors(&everything).contains("Unpaginated"),
		"unpaginated media should be refused: {everything}"
	);

	let cursor = app
		.execute_gql(
			r#"{ media(pagination: { cursor: { limit: 201 } }) { nodes { id } } }"#,
			None,
		)
		.await;
	assert!(errors(&cursor).contains("at most 200"), "{cursor}");

	// ...but the tables that are small by nature can still be listed whole, as the web app does
	let libraries = app
		.execute_gql(
			r#"{ libraries(pagination: { none: { unpaginated: true } }) { nodes { id } } }"#,
			None,
		)
		.await;
	assert!(libraries.get("errors").is_none(), "{libraries}");

	// a huge `take` is cut, not refused
	let books = app
		.execute_gql(
			&format!(
				r#"{{ seriesById(id: "{}") {{ media(take: 100000) {{ id }} }} }}"#,
				series.id
			),
			None,
		)
		.await;
	assert!(books.get("errors").is_none(), "{books}");
	assert_eq!(
		books["data"]["seriesById"]["media"].as_array().unwrap().len(),
		3
	);
}

/// An OPDS `page_size` of 0 is the client's mistake (a 400, not a 500), a huge one is cut
#[tokio::test]
async fn opds_page_sizes_are_validated() {
	let app = TestApp::new_with_default_user().await;
	setup_single_series_with_n_books(&app, fake_data::Series::default(), 2).await;

	for path in [
		"/opds/v1.2/series?page_size=0",
		"/opds/v1.2/books?page_size=0",
		"/opds/v1.2/authors?page_size=0",
		"/opds/v2.0/libraries?page_size=0",
	] {
		app.get(path).await.assert_status_bad_request();
	}

	let huge = app.get("/opds/v1.2/series?page_size=100000").await;
	huge.assert_status_ok();
	assert!(huge.text().contains("page_size=200"), "{}", huge.text());
}

/// Searching series as an age-restricted user joined the series metadata twice: "ambiguous column
/// name" and a 500 for every such search (the series' own metadata is already joined to apply the
/// restriction)
#[tokio::test]
async fn opds_series_search_works_for_age_restricted_users() {
	let app = TestApp::new_with_default_user().await;
	let (series, _) = setup_single_series_with_n_books(
		&app,
		fake_data::Series {
			name: Some("Wanderer".to_string()),
			..Default::default()
		},
		1,
	)
	.await;
	assert_eq!(series.name, "Wanderer");

	let restricted = CreateTestUser {
		username: "restricted".to_string(),
		age_restriction: Some(AgeRestrictionInput {
			age: 13,
			restrict_on_unset: false,
		}),
		..Default::default()
	}
	.insert(&app)
	.await;

	for path in [
		"/opds/v1.2/series?search=Wanderer",
		"/opds/v1.2/search/feed?search=Wanderer",
		"/opds/v2.0/search?query=Wanderer",
	] {
		for (who, token) in [
			("the owner", app.access_token.read().await.clone().unwrap()),
			("the restricted user", restricted.token.clone()),
		] {
			let response = app
				.server
				.get(path)
				.add_header("Authorization", format!("Bearer {token}"))
				.await;
			assert_eq!(
				response.status_code(),
				200,
				"{path} as {who}: {}",
				response.text()
			);
		}
	}
}

/// `duplicateMedia` is a list like the others: at most 200 books, in a fixed order
#[tokio::test]
async fn duplicate_books_come_at_most_200_at_a_time() {
	let app = TestApp::new_with_default_user().await;
	setup_single_series_with_n_books(&app, fake_data::Series::default(), 230).await;
	// one file, 230 times
	media::Entity::update_many()
		.col_expr(media::Column::Hash, Expr::value("one-file"))
		.filter(media::Column::DeletedAt.is_null())
		.exec(app.conn())
		.await
		.expect("should set the hashes");

	let query = "{ duplicateMedia { id } }";
	let first = app.execute_gql(query, None).await;
	assert!(first.get("errors").is_none(), "{first:#}");
	let ids = |response: &Value| -> Vec<String> {
		response["data"]["duplicateMedia"]
			.as_array()
			.unwrap()
			.iter()
			.map(|book| book["id"].as_str().unwrap().to_string())
			.collect()
	};
	assert_eq!(ids(&first).len(), 200);
	// the same 200 every time
	assert_eq!(ids(&app.execute_gql(query, None).await), ids(&first));
}
