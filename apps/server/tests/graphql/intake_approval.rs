//! Uploads that wait for an approver: what somebody who may not file books themselves can do with
//! their inbox, what an approver sees and decides, and what a friend never learns about books
//! they can't read.
//!
//! The people are the ones of [`VisibilityFixture`]: the owner files directly; **ana** has
//! "Add books" (and limits: *Hidden* is hidden from her, nothing rated above 16, nothing unrated),
//! so what she uploads waits for the owner; **ben** has no permissions at all.

use std::{fs, path::PathBuf, time::Duration};

use models::{
	entity::{intake_item, library},
	shared::intake::{
		IntakeDecision, IntakeDestination, IntakeDestinationFile, IntakeDuplicate,
		IntakeFile, IntakeFiles, IntakeFolderCandidate, IntakeGenreSuggestion,
		IntakeKind, IntakeMetadata, IntakeProposal, IntakeSimilarBook, IntakeState,
	},
};
use sea_orm::{prelude::*, ActiveValue::Set};
use serde_json::{json, Value};
use stump_core::intake::approval;
use tempfile::TempDir;
use tests::{fake_data, fixtures};

use crate::common::{
	upload::{message, post_upload, UploadBody},
	visibility::{add_book, add_series, errors, Meta, VisibilityFixture},
	TestApp,
};

const SETTINGS: &str = r#"mutation Settings($libraryId: ID!) {
	updateLibraryIntakeSettings(libraryId: $libraryId, input: {
		mangaDir: "_Manga", lightNovelDir: "_Novela ligera", onlineLookup: false
	}) { onlineLookup }
}"#;
const MY_ITEMS: &str = r#"query Items {
	intakeItems(states: [ANALYSING, NEEDS_REVIEW, READY, PENDING_APPROVAL, FILING, FILED, REJECTED, FAILED]) {
		id state kind name error problems submittedAt reviewedAt reviewNote
		uploader { username }
		proposal {
			warnings confidence
			metadata { title authors description }
			authorFolders { folder }
			seriesFolders { folder }
			genreSuggestions { value libraryCount reasons selected kind }
			similarBooks { mediaId title authors reason }
			duplicates { mediaId path title reason fileName }
		}
		decision { folder fileNames metadata { title authors genres description ageRating } }
		destination { relativeDir dirExists files { relativePath exists } }
		result { paths notes }
	}
}"#;
const PENDING: &str = r#"query Pending($libraryId: ID) {
	pendingIntakeItems(libraryId: $libraryId) {
		totalCount
		nodes {
			id state name error problems submittedAt
			uploader { username }
			proposal {
				warnings
				similarBooks { mediaId title }
				duplicates { mediaId path title reason }
				authorFolders { folder }
			}
			decision { folder fileNames metadata { title authors genres } }
			destination { relativeDir dirExists files { relativePath exists } }
		}
	}
}"#;
const PENDING_COUNT: &str = r#"query Count { pendingIntakeCount }"#;
const SUBMIT: &str = r#"mutation Submit($ids: [ID!]!) {
	intakeSubmit(ids: $ids) { id state submittedAt reviewNote error }
}"#;
const WITHDRAW: &str = r#"mutation Withdraw($ids: [ID!]!) {
	intakeWithdraw(ids: $ids) { id state submittedAt }
}"#;
const APPROVE: &str = r#"mutation Approve($ids: [ID!]!) {
	intakeApprove(ids: $ids) { id state error }
}"#;
const REJECT: &str = r#"mutation Reject($id: ID!, $note: String!) {
	intakeReject(id: $id, note: $note) { id state reviewNote reviewedAt }
}"#;
const REVIEW: &str = r#"mutation Review($id: ID!, $input: IntakeReviewInput!) {
	intakeReview(id: $id, input: $input) {
		id state problems
		decision { folder fileNames metadata { title description genres ageRating year } }
	}
}"#;
const ACCEPT: &str = r#"mutation Accept($ids: [ID!]!) {
	intakeAccept(ids: $ids) { id state error }
}"#;
const GQL_UPLOAD: &str = r#"mutation Upload($libraryId: ID!, $files: [Upload!]!) {
	intakeUpload(libraryId: $libraryId, files: $files) { id state }
}"#;
const VOCABULARY: &str = r#"query Vocabulary($libraryId: ID!) {
	intakeVocabulary(libraryId: $libraryId) { value count kind }
}"#;
const FOLDERS: &str = r#"query Folders($libraryId: ID!) {
	intakeFolders(libraryId: $libraryId) { name books }
}"#;
const LIMITS: &str = r#"query Limits {
	intakeUploadLimits {
		maxRequestBytes maxFileBytes maxFilesPerRequest quotaBytes usedBytes maxItems
		openItems quotaApplies canFileDirectly uploadsOpen closedReason
	}
}"#;
const USER_LIMITS: &str = r#"query UserLimits($userId: ID!) {
	userIntakeLimits(userId: $userId) {
		userId quotaMb maxItems effective { quotaBytes maxItems maxFileBytes }
	}
}"#;
const UPDATE_USER_LIMITS: &str = r#"mutation Update($userId: ID!, $input: UserIntakeLimitsInput!) {
	updateUserIntakeLimits(userId: $userId, input: $input) {
		userId quotaMb maxItems effective { quotaBytes maxItems }
	}
}"#;

struct World {
	app: TestApp,
	vis: VisibilityFixture,
	library: library::Model,
	library_dir: PathBuf,
	config_dir: PathBuf,
	tmp: TempDir,
}

async fn world() -> World {
	world_with(|_| {}).await
}

async fn world_with(change: impl FnOnce(&mut stump_core::config::StumpConfig)) -> World {
	let tmp = tempfile::tempdir().expect("tempdir");
	let config_dir = tmp.path().join("config");
	fs::create_dir_all(&config_dir).expect("config folder");
	let config = config_dir.to_string_lossy().to_string();
	let app = TestApp::new_with_default_user_and_config(move |c| {
		c.config_dir = config;
		change(c);
	})
	.await;
	let vis = VisibilityFixture::build(&app).await;
	let library_dir = tmp.path().join("biblioteca");
	// an author already in the library, written the library's way
	fs::create_dir_all(library_dir.join("Umberto_Eco")).expect("author folder");
	let library = fake_data::Library {
		name: Some("Inbox".to_string()),
		path: Some(library_dir.to_string_lossy().to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let response = app
		.execute_gql(SETTINGS, Some(json!({ "libraryId": library.id })))
		.await;
	assert!(response.get("errors").is_none(), "{response:#}");
	World {
		app,
		vis,
		library,
		library_dir,
		config_dir,
		tmp,
	}
}

impl World {
	fn epub(&self, title: &str, author: &str) -> Vec<u8> {
		let path = self
			.tmp
			.path()
			.join(format!("{}.epub", title.replace(' ', "_")));
		fixtures::write_test_epub(
			&path,
			title,
			&format!(
				r#"<dc:creator>{author}</dc:creator><dc:subject>Novela</dc:subject>"#
			),
		);
		fs::read(path).expect("epub bytes")
	}

	/// ana uploads a book over REST, and the id of the item it makes
	async fn ana_uploads(&self, title: &str) -> String {
		let bytes = self.epub(title, "Eco, Umberto");
		let response = post_upload(
			&self.app,
			Some(&self.vis.ana.token),
			&self.library.id,
			None,
			&UploadBody::new().file(&format!("{title}.epub"), bytes),
		)
		.await;
		assert_eq!(response.status_code(), 200, "{}", response.text());
		let body: Value = response.json();
		body["items"][0]["id"]
			.as_str()
			.expect("an item id")
			.to_string()
	}

	async fn gql(&self, token: &str, query: &str, variables: Value) -> Value {
		self.app
			.execute_gql_with_token(query, Some(variables), token)
			.await
	}

	async fn as_owner(&self, query: &str, variables: Value) -> Value {
		self.app.execute_gql(query, Some(variables)).await
	}

	/// ana's inbox, once nothing is being analysed any more
	async fn ana_items(&self) -> Value {
		for _ in 0..100 {
			let response = self.gql(&self.vis.ana.token, MY_ITEMS, json!({})).await;
			assert!(response.get("errors").is_none(), "{response:#}");
			let items = response["data"]["intakeItems"].clone();
			if items
				.as_array()
				.is_some_and(|items| items.iter().all(|i| i["state"] != "ANALYSING"))
			{
				return items;
			}
			tokio::time::sleep(Duration::from_millis(100)).await;
		}
		panic!("analysis did not finish");
	}

	/// the item as the database has it
	async fn row(&self, id: &str) -> intake_item::Model {
		intake_item::Entity::find_by_id(id.to_string())
			.one(self.app.conn())
			.await
			.expect("the item should load")
			.expect("the item should exist")
	}

	/// waits until the item is in `state`
	async fn wait_for(&self, id: &str, state: IntakeState) -> intake_item::Model {
		for _ in 0..150 {
			let row = self.row(id).await;
			if row.state == state {
				return row;
			}
			tokio::time::sleep(Duration::from_millis(100)).await;
		}
		panic!(
			"the item never became {state}: it is {}",
			self.row(id).await.state
		);
	}

	fn ana_id(&self) -> String {
		self.vis.ana.id.clone()
	}

	/// the owner's id
	async fn owner_id(&self) -> String {
		models::entity::user::Entity::find()
			.filter(models::entity::user::Column::IsServerOwner.eq(true))
			.one(self.app.conn())
			.await
			.expect("the owner should load")
			.expect("there is an owner")
			.id
	}

	/// waits until the item is not being analysed any more
	async fn wait_for_analysis(&self, id: &str) -> intake_item::Model {
		for _ in 0..150 {
			let row = self.row(id).await;
			if row.state != IntakeState::Analysing {
				return row;
			}
			tokio::time::sleep(Duration::from_millis(100)).await;
		}
		panic!("analysis did not finish");
	}
}

fn assert_ok(response: &Value) {
	assert!(
		response.get("errors").is_none(),
		"unexpected errors: {response:#}"
	);
}

fn assert_refused(response: &Value) {
	assert!(
		response.get("errors").is_some(),
		"should be refused: {response:#}"
	);
}

/// upload -> review -> send -> the owner approves -> filed: what is written, by whom, and what
/// each of them could see on the way
#[tokio::test(flavor = "multi_thread")]
async fn test_a_friend_uploads_and_the_owner_approves() {
	let world = world().await;
	let ana = &world.vis.ana;
	let id = world.ana_uploads("El nombre de la rosa").await;

	let items = world.ana_items().await;
	let item = &items[0];
	assert_eq!(item["id"], id);
	// she may not file: nothing about where the book goes is hers
	assert!(item["destination"].is_null(), "{item:#}");
	assert_eq!(item["decision"]["folder"], "");
	assert_eq!(item["decision"]["fileNames"], json!([]));
	assert_eq!(item["proposal"]["authorFolders"], json!([]));
	assert_eq!(
		item["proposal"]["metadata"]["title"],
		"El nombre de la rosa"
	);
	assert_eq!(item["uploader"]["username"], "ana");
	world.vis.assert_none_hidden(&items);

	// she can't file it herself, nor upload the way somebody who files does
	assert_refused(&world.gql(&ana.token, ACCEPT, json!({ "ids": [id] })).await);
	assert_refused(
		&world
			.gql(
				&ana.token,
				GQL_UPLOAD,
				json!({ "libraryId": world.library.id, "files": [null] }),
			)
			.await,
	);

	let response = world.gql(&ana.token, SUBMIT, json!({ "ids": [id] })).await;
	assert_ok(&response);
	let sent = &response["data"]["intakeSubmit"][0];
	assert_eq!(sent["state"], "PENDING_APPROVAL", "{sent:#}");
	assert!(sent["submittedAt"].is_string());

	// ben has no permissions: the queue, its count and ana's item are not his
	let ben = &world.vis.ben;
	assert_refused(&world.gql(&ben.token, PENDING, json!({})).await);
	assert_refused(&world.gql(&ben.token, PENDING_COUNT, json!({})).await);
	assert_refused(&world.gql(&ben.token, APPROVE, json!({ "ids": [id] })).await);
	// and ana can't approve her own
	assert_refused(&world.gql(&ana.token, APPROVE, json!({ "ids": [id] })).await);
	assert_refused(&world.gql(&ana.token, PENDING, json!({})).await);

	// the owner sees the queue, with the whole card
	let response = world.as_owner(PENDING_COUNT, json!({})).await;
	assert_eq!(response["data"]["pendingIntakeCount"], 1, "{response:#}");
	let response = world.as_owner(PENDING, json!({})).await;
	assert_ok(&response);
	let queue = &response["data"]["pendingIntakeItems"];
	assert_eq!(queue["totalCount"], 1);
	let waiting = &queue["nodes"][0];
	assert_eq!(waiting["uploader"]["username"], "ana");
	assert_eq!(waiting["decision"]["folder"], "Umberto_Eco", "{waiting:#}");
	assert_eq!(
		waiting["destination"]["files"][0]["relativePath"],
		"Umberto_Eco/El_nombre_de_la_rosa.epub"
	);

	let response = world.as_owner(APPROVE, json!({ "ids": [id] })).await;
	assert_ok(&response);
	let approved = &response["data"]["intakeApprove"][0];
	assert!(
		matches!(approved["state"].as_str(), Some("FILING" | "FILED")),
		"{approved:#}"
	);
	let row = world.wait_for(&id, IntakeState::Filed).await;
	assert_eq!(
		row.reviewed_by.as_deref(),
		Some(world.owner_id().await.as_str()),
		"the record says who approved it"
	);
	assert!(row.reviewed_at.is_some());
	assert!(row.filed_at.is_some());
	assert!(row.submitted_at.is_none(), "it is not waiting any more");
	let target = world
		.library_dir
		.join("Umberto_Eco/El_nombre_de_la_rosa.epub");
	assert!(target.exists(), "the book should be in its author folder");
	assert!(!world
		.library_dir
		.join("Umberto_Eco/El_nombre_de_la_rosa_2.epub")
		.exists());

	// the queue is empty now
	let response = world.as_owner(PENDING_COUNT, json!({})).await;
	assert_eq!(response["data"]["pendingIntakeCount"], 0);
	assert_ne!(row.reviewed_by.as_deref(), Some(ana.id.as_str()));

	// ana sees it filed, with a path inside the library (not on the server's disk)
	let items = world.ana_items().await;
	assert_eq!(items[0]["state"], "FILED", "{items:#}");
	assert_eq!(
		items[0]["result"]["paths"][0],
		"Umberto_Eco/El_nombre_de_la_rosa.epub"
	);
	world.vis.assert_none_hidden(&items);
	let recent = world
		.as_owner(
			r#"query Recent { recentlyAddedBooks { title username paths } }"#,
			json!({}),
		)
		.await;
	assert_eq!(recent["data"]["recentlyAddedBooks"][0]["username"], "ana");
}

/// two approvers (or one who clicks twice) file a book once
#[tokio::test(flavor = "multi_thread")]
async fn test_two_approvals_file_a_book_once() {
	let world = world().await;
	let id = world.ana_uploads("Two approvals").await;
	world.ana_items().await;
	assert_ok(
		&world
			.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": [id] }))
			.await,
	);
	let a = world.as_owner(APPROVE, json!({ "ids": [id] }));
	let b = world.as_owner(APPROVE, json!({ "ids": [id] }));
	let (a, b) = tokio::join!(a, b);
	assert_ok(&a);
	assert_ok(&b);
	let row = world.wait_for(&id, IntakeState::Filed).await;
	assert_eq!(row.result.expect("a result").paths.len(), 1);
	let files: Vec<_> = fs::read_dir(world.library_dir.join("Umberto_Eco"))
		.unwrap()
		.filter_map(Result::ok)
		.map(|e| e.file_name().to_string_lossy().into_owned())
		.collect();
	assert_eq!(files, ["Two_approvals.epub"], "one copy, not two");
}

/// sent back with a note, changed and sent again; and taken back before anybody looked
#[tokio::test(flavor = "multi_thread")]
async fn test_a_book_can_be_sent_back_changed_and_sent_again() {
	let world = world().await;
	let ana = &world.vis.ana;
	let id = world.ana_uploads("Second thoughts").await;
	world.ana_items().await;
	assert_ok(&world.gql(&ana.token, SUBMIT, json!({ "ids": [id] })).await);

	// sending it back takes a reason
	assert_refused(
		&world
			.as_owner(REJECT, json!({ "id": id, "note": "  " }))
			.await,
	);
	let response = world
		.as_owner(
			REJECT,
			json!({ "id": id, "note": "The author is spelled Umberto Eco" }),
		)
		.await;
	assert_ok(&response);
	assert_eq!(response["data"]["intakeReject"]["state"], "REJECTED");
	let row = world.row(&id).await;
	assert!(row.submitted_at.is_none());
	assert!(row.reviewed_by.is_some());

	// it is hers again: she reads the note, changes the book and sends it again
	let items = world.ana_items().await;
	assert_eq!(items[0]["state"], "REJECTED");
	assert_eq!(items[0]["reviewNote"], "The author is spelled Umberto Eco");
	let response = world
		.gql(
			&ana.token,
			REVIEW,
			json!({ "id": id, "input": { "authors": ["Umberto Eco"] } }),
		)
		.await;
	assert_ok(&response);
	let state = response["data"]["intakeReview"]["state"].as_str().unwrap();
	assert!(matches!(state, "READY" | "NEEDS_REVIEW"), "{response:#}");
	let response = world.gql(&ana.token, SUBMIT, json!({ "ids": [id] })).await;
	assert_ok(&response);
	assert_eq!(
		response["data"]["intakeSubmit"][0]["state"],
		"PENDING_APPROVAL"
	);
	assert!(
		response["data"]["intakeSubmit"][0]["reviewNote"].is_null(),
		"the note was read: {response:#}"
	);

	// an item in the queue can't be changed by its uploader until she takes it back
	assert_refused(
		&world
			.gql(
				&ana.token,
				REVIEW,
				json!({ "id": id, "input": { "title": "Changed while waiting" } }),
			)
			.await,
	);
	let response = world
		.gql(&ana.token, WITHDRAW, json!({ "ids": [id] }))
		.await;
	assert_ok(&response);
	assert!(matches!(
		response["data"]["intakeWithdraw"][0]["state"].as_str(),
		Some("READY" | "NEEDS_REVIEW")
	));
	assert!(response["data"]["intakeWithdraw"][0]["submittedAt"].is_null());
	let response = world.as_owner(PENDING_COUNT, json!({})).await;
	assert_eq!(response["data"]["pendingIntakeCount"], 0);
	// nothing waiting to approve, or to send back
	assert_refused(&world.as_owner(APPROVE, json!({ "ids": [id] })).await);
	assert_refused(
		&world
			.as_owner(REJECT, json!({ "id": id, "note": "no" }))
			.await,
	);
	// she can't take back what isn't waiting
	assert_refused(
		&world
			.gql(&ana.token, WITHDRAW, json!({ "ids": [id] }))
			.await,
	);
}

/// the approver changes an item of the queue before approving it
#[tokio::test(flavor = "multi_thread")]
async fn test_an_approver_can_change_an_item_before_approving_it() {
	let world = world().await;
	let id = world.ana_uploads("Needs a fix").await;
	world.ana_items().await;
	assert_ok(
		&world
			.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": [id] }))
			.await,
	);

	let response = world
		.as_owner(
			REVIEW,
			json!({ "id": id, "input": {
				"title": "Fixed by the owner", "folder": "Umberto_Eco",
				"genres": ["Historical Fiction"]
			} }),
		)
		.await;
	assert_ok(&response);
	// still waiting: an approver's change doesn't send it anywhere
	assert_eq!(
		response["data"]["intakeReview"]["state"],
		"PENDING_APPROVAL"
	);
	assert_eq!(
		response["data"]["intakeReview"]["decision"]["metadata"]["title"],
		"Fixed by the owner"
	);
	// ben, who is no approver, can't change it
	assert_refused(
		&world
			.gql(
				&world.vis.ben.token,
				REVIEW,
				json!({ "id": id, "input": { "title": "ben was here" } }),
			)
			.await,
	);
	assert_ok(&world.as_owner(APPROVE, json!({ "ids": [id] })).await);
	world.wait_for(&id, IntakeState::Filed).await;
	assert!(world
		.library_dir
		.join("Umberto_Eco/Fixed_by_the_owner.epub")
		.exists());
}

/// somebody who needs approval is held to the rules of the item: no say in where it goes, texts
/// of a sensible length and without markup, an age rating that doesn't go down
#[tokio::test(flavor = "multi_thread")]
async fn test_a_friends_review_is_held_to_the_rules() {
	let world = world().await;
	let ana = &world.vis.ana;
	let id = world.ana_uploads("Held to rules").await;
	world.ana_items().await;
	let review = |input: Value| {
		let ana_token = ana.token.clone();
		let id = id.clone();
		let world = &world;
		async move {
			world
				.gql(&ana_token, REVIEW, json!({ "id": id, "input": input }))
				.await
		}
	};

	// not hers to choose
	for input in [
		json!({ "folder": "Elsewhere" }),
		json!({ "fileNames": ["x.epub"] }),
		json!({ "seriesKind": "MANGA" }),
		json!({ "clearSeriesKind": true }),
	] {
		let response = review(input.clone()).await;
		assert_refused(&response);
		assert!(
			errors(&response).contains("chosen by whoever approves"),
			"{input}: {response:#}"
		);
	}

	// lengths
	assert_refused(&review(json!({ "title": "x".repeat(301) })).await);
	assert_refused(&review(json!({ "description": "x".repeat(20_001) })).await);
	assert_refused(&review(json!({ "genres": vec!["g"; 51] })).await);
	assert_refused(&review(json!({ "authors": vec!["a"; 31] })).await);
	assert_refused(&review(json!({ "year": 999 })).await);
	assert_refused(&review(json!({ "year": 2101 })).await);
	assert_ok(&review(json!({ "year": 1980, "genres": vec!["g"; 50] })).await);

	// a description is plain text
	let response = review(json!({
		"description": "A <b>bold</b> claim<script>alert(1)</script>, <img src=x onerror=alert(1)>here."
	}))
	.await;
	assert_ok(&response);
	let description = response["data"]["intakeReview"]["decision"]["metadata"]
		["description"]
		.as_str()
		.unwrap()
		.to_string();
	assert!(
		!description.contains('<') && !description.contains('>'),
		"{description}"
	);
	assert!(description.contains("A bold claim"), "{description}");
	let stored = world
		.row(&id)
		.await
		.decision
		.unwrap()
		.metadata
		.description
		.unwrap();
	assert_eq!(stored, description);

	// the age rating goes up, never down
	assert_ok(&review(json!({ "ageRating": 18 })).await);
	let response = review(json!({ "ageRating": 0 })).await;
	assert_refused(&response);
	assert!(errors(&response).contains("not lowered"), "{response:#}");
	assert_ok(&review(json!({ "ageRating": 18 })).await);

	// the folder and the file names come from the authors, whatever she types
	let response =
		review(json!({ "authors": ["Umberto Eco"], "title": "Held to rules" })).await;
	assert_ok(&response);
	let decision = world.row(&id).await.decision.unwrap();
	assert_eq!(decision.folder, "Umberto_Eco");
	assert_eq!(decision.file_names, ["Held_to_rules.epub"]);
	// a new author gets a new folder, made from the name
	let response = review(json!({ "authors": ["Nobody Known"] })).await;
	assert_ok(&response);
	let decision = world.row(&id).await.decision.unwrap();
	assert_eq!(decision.folder, "Nobody_Known");
}

/// somebody who files books themselves chooses the folder and the file names, and writes what
/// they like: the rules are for the people who need approval
#[tokio::test(flavor = "multi_thread")]
async fn test_the_owner_is_not_held_to_a_friends_rules() {
	let world = world().await;
	let bytes = world.epub("Owner upload", "Eco, Umberto");
	let response = post_upload(
		&world.app,
		Some(&world.app.access_token.read().await.clone().unwrap()),
		&world.library.id,
		None,
		&UploadBody::new().file("owner.epub", bytes),
	)
	.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	let id = response.json::<Value>()["items"][0]["id"]
		.as_str()
		.unwrap()
		.to_string();
	world.wait_for_analysis(&id).await;
	let response = world
		.as_owner(
			REVIEW,
			json!({ "id": id, "input": {
				"title": "t".repeat(400), "folder": "Somewhere", "fileNames": ["chosen.epub"],
				"ageRating": 0, "description": "<b>kept as written</b>"
			} }),
		)
		.await;
	assert_ok(&response);
	let decision = &response["data"]["intakeReview"]["decision"];
	assert_eq!(decision["folder"], "Somewhere");
	assert_eq!(decision["fileNames"], json!(["chosen.epub"]));
	assert_eq!(
		decision["metadata"]["description"],
		"<b>kept as written</b>"
	);
	assert_eq!(decision["metadata"]["title"].as_str().unwrap().len(), 400);
}

fn suggestion(
	value: &str,
	count: i32,
	selected: bool,
	reasons: &[&str],
) -> IntakeGenreSuggestion {
	IntakeGenreSuggestion {
		value: value.to_string(),
		confidence: 0.8,
		reasons: reasons.iter().map(|r| String::from(*r)).collect(),
		library_count: count,
		selected,
		kind: "GENRE".to_string(),
	}
}

fn similar(book: &models::entity::media::Model) -> IntakeSimilarBook {
	IntakeSimilarBook {
		media_id: book.id.clone(),
		title: book.name.clone(),
		authors: vec!["Somebody".to_string()],
		genres: vec![],
		reason: "similar description".to_string(),
	}
}

fn duplicate(book: &models::entity::media::Model, reason: &str) -> IntakeDuplicate {
	IntakeDuplicate {
		media_id: book.id.clone(),
		path: book.path.clone(),
		title: book.name.clone(),
		reason: reason.to_string(),
		file_name: "crafted.epub".to_string(),
	}
}

/// an item of `user_id` as an analysis could have left it, with the proposal given
async fn insert_item(
	world: &World,
	user_id: &str,
	state: IntakeState,
	proposal: IntakeProposal,
) -> String {
	let id = uuid::Uuid::new_v4().to_string();
	let decision = IntakeDecision {
		metadata: IntakeMetadata {
			title: Some("Crafted".to_string()),
			authors: vec!["Eco, Umberto".to_string()],
			genres: vec![
				"Fantasy".to_string(),
				"Mature Themes".to_string(),
				"History".to_string(),
			],
			..Default::default()
		},
		folder: "Umberto_Eco".to_string(),
		file_names: vec!["crafted.epub".to_string()],
		volumes: vec![None],
		..Default::default()
	};
	let destination = IntakeDestination {
		relative_dir: "Umberto_Eco".to_string(),
		dir_exists: true,
		files: vec![IntakeDestinationFile {
			original_name: "crafted.epub".to_string(),
			relative_path: "Umberto_Eco/crafted.epub".to_string(),
			exists: true,
		}],
	};
	intake_item::ActiveModel {
		id: Set(id.clone()),
		user_id: Set(user_id.to_string()),
		library_id: Set(world.library.id.clone()),
		state: Set(state),
		kind: Set(IntakeKind::Book),
		name: Set("crafted.epub".to_string()),
		staging_dir: Set(world
			.config_dir
			.join("intake")
			.join(user_id)
			.join(&id)
			.to_string_lossy()
			.into_owned()),
		files: Set(IntakeFiles(vec![IntakeFile {
			original_name: "crafted.epub".to_string(),
			staged_name: "000_crafted.epub".to_string(),
			size: 10,
			format: "epub".to_string(),
			..Default::default()
		}])),
		proposal: Set(Some(proposal)),
		decision: Set(Some(decision)),
		destination: Set(Some(destination)),
		submitted_at: Set(
			(state == IntakeState::PendingApproval).then(|| chrono::Utc::now().into())
		),
		..Default::default()
	}
	.insert(world.app.conn())
	.await
	.expect("should insert the item");
	id
}

/// what an analysis finds in the library reaches somebody who can't read all of it only through
/// the books they can read
#[tokio::test(flavor = "multi_thread")]
async fn test_a_friend_never_learns_what_only_books_they_cannot_read_say() {
	let world = world().await;
	let vis = &world.vis;
	let shelf = add_series(&world.app, &world.library, "Inbox shelf").await;
	let readable = add_book(
		&world.app,
		&shelf,
		"Readable in the inbox",
		Meta {
			age: Some(0),
			genre: Some("Fantasy"),
			number: Some(1.0),
			..Default::default()
		},
	)
	.await;
	let adult = add_book(
		&world.app,
		&shelf,
		"Adult in the inbox",
		Meta {
			age: Some(18),
			genre: Some("Fantasy, Mature Themes"),
			number: Some(2.0),
			..Default::default()
		},
	)
	.await;

	let proposal = IntakeProposal {
		metadata: IntakeMetadata {
			title: Some("Crafted".to_string()),
			subjects: vec!["History".to_string()],
			..Default::default()
		},
		author_folders: vec![IntakeFolderCandidate {
			folder: "Umberto_Eco".to_string(),
			books: 7,
			confidence: 0.9,
			reason: "same author written 'Eco, Umberto'".to_string(),
			is_new: false,
		}],
		series_folders: vec![IntakeFolderCandidate {
			folder: "Inbox shelf".to_string(),
			books: 2,
			confidence: 0.9,
			reason: "this series is already in the library".to_string(),
			is_new: false,
		}],
		genre_suggestions: vec![
			suggestion(
				"Fantasy",
				2,
				true,
				&["3 other books by this author have it"],
			),
			suggestion("Mature Themes", 1, true, &["books by this author"]),
			// from her own file's subjects: hers to see
			suggestion("History", 0, false, &[]),
		],
		similar_books: vec![
			similar(&readable),
			similar(&adult),
			similar(&vis.hidden),
			similar(&vis.unrated),
		],
		duplicates: vec![
			duplicate(&readable, "SAME_TITLE"),
			duplicate(&adult, "SAME_FILE"),
			duplicate(&vis.hidden, "SAME_FILE"),
		],
		warnings: vec![
			"No author found: pick or type the author".to_string(),
			"This book may already be in the library".to_string(),
		],
		confidence: 0.5,
		..Default::default()
	};
	let first = insert_item(
		&world,
		&vis.ana.id,
		IntakeState::NeedsReview,
		proposal.clone(),
	)
	.await;
	// a second book whose only duplicates are books she can't read
	let hidden_only = IntakeProposal {
		duplicates: vec![
			duplicate(&adult, "SAME_FILE"),
			duplicate(&vis.hidden, "SAME_TITLE"),
		],
		similar_books: vec![similar(&vis.hidden)],
		warnings: vec![
			"This book may already be in the library".to_string(),
			"No title found".to_string(),
		],
		..proposal.clone()
	};
	let second =
		insert_item(&world, &vis.ana.id, IntakeState::NeedsReview, hidden_only).await;

	let response = world.gql(&vis.ana.token, MY_ITEMS, json!({})).await;
	assert_ok(&response);
	vis.assert_none_hidden(&response);
	let text = response.to_string();
	for hidden in [
		adult.id.as_str(),
		adult.name.as_str(),
		adult.path.as_str(),
		"Mature Themes",
		"books by this author",
	] {
		assert!(!text.contains(hidden), "she got {hidden} in:\n{text}");
	}
	let items = response["data"]["intakeItems"].as_array().unwrap();
	let find = |id: &str| items.iter().find(|i| i["id"] == id).expect("the item");
	let item = find(&first);
	let proposal = &item["proposal"];
	let titles: Vec<&str> = proposal["similarBooks"]
		.as_array()
		.unwrap()
		.iter()
		.map(|b| b["title"].as_str().unwrap())
		.collect();
	assert_eq!(titles, ["Readable in the inbox"], "{item:#}");
	let duplicates = proposal["duplicates"].as_array().unwrap();
	assert_eq!(duplicates.len(), 1, "{item:#}");
	assert_eq!(duplicates[0]["mediaId"], readable.id);
	assert_eq!(duplicates[0]["path"], "", "a path is the library's layout");
	assert!(
		proposal["warnings"]
			.as_array()
			.unwrap()
			.iter()
			.any(|w| w == "This book may already be in the library"),
		"a duplicate she can read is still told: {item:#}"
	);
	assert_eq!(proposal["authorFolders"], json!([]));
	assert_eq!(proposal["seriesFolders"], json!([]));
	let suggested: Vec<(&str, i64, usize)> = proposal["genreSuggestions"]
		.as_array()
		.unwrap()
		.iter()
		.map(|g| {
			(
				g["value"].as_str().unwrap(),
				g["libraryCount"].as_i64().unwrap(),
				g["reasons"].as_array().unwrap().len(),
			)
		})
		.collect();
	// Mature Themes is on a book she can't read: not hers; the counts are of the books she can
	// read; there are no reasons ("3 other books by this author have it" is about books)
	assert_eq!(
		suggested,
		[("Fantasy", 1, 0), ("History", 0, 0)],
		"{item:#}"
	);
	assert_eq!(
		item["decision"]["metadata"]["genres"],
		json!(["Fantasy", "History"])
	);
	// where it goes, and whether the name exists, are not hers
	assert!(item["destination"].is_null());
	assert_eq!(item["decision"]["folder"], "");
	assert!(item["problems"].as_array().unwrap().is_empty(), "{item:#}");

	let item = find(&second);
	assert_eq!(item["proposal"]["duplicates"], json!([]), "{item:#}");
	assert_eq!(item["proposal"]["similarBooks"], json!([]));
	assert_eq!(
		item["proposal"]["warnings"],
		json!(["No title found"]),
		"nothing is left to explain the warning that the book may be in the library: {item:#}"
	);

	// the owner, whose queue it is, sees all of it
	let row = world.row(&first).await;
	intake_item::ActiveModel {
		state: Set(IntakeState::PendingApproval),
		submitted_at: Set(Some(chrono::Utc::now().into())),
		..row.into()
	}
	.update(world.app.conn())
	.await
	.unwrap();
	let response = world.as_owner(PENDING, json!({})).await;
	assert_ok(&response);
	let queue = response["data"]["pendingIntakeItems"]["nodes"][0].clone();
	let text = queue.to_string();
	for seen in [
		vis.hidden.id.as_str(),
		vis.unrated.id.as_str(),
		adult.id.as_str(),
		vis.hidden.path.as_str(),
	] {
		assert!(
			text.contains(seen),
			"the owner should see {seen} in:\n{text}"
		);
	}
	assert_eq!(
		queue["proposal"]["authorFolders"][0]["folder"],
		"Umberto_Eco"
	);
	assert_eq!(queue["destination"]["files"][0]["exists"], true);
	assert!(queue["problems"]
		.as_array()
		.unwrap()
		.iter()
		.any(|p| p == "A file with that name is already in the library"));

	// the tags of the library are the tags of the books she can read, counted among those
	let vocabulary = |response: &Value| -> Vec<(String, i64)> {
		response["data"]["intakeVocabulary"]
			.as_array()
			.unwrap()
			.iter()
			.map(|t| {
				(
					t["value"].as_str().unwrap().to_string(),
					t["count"].as_i64().unwrap(),
				)
			})
			.collect()
	};
	let response = world
		.gql(
			&vis.ana.token,
			VOCABULARY,
			json!({ "libraryId": world.library.id }),
		)
		.await;
	assert_ok(&response);
	assert_eq!(vocabulary(&response), [("Fantasy".to_string(), 1)]);
	let response = world
		.as_owner(VOCABULARY, json!({ "libraryId": world.library.id }))
		.await;
	assert_ok(&response);
	assert_eq!(
		vocabulary(&response),
		[("Fantasy".to_string(), 2), ("Mature Themes".to_string(), 1)]
	);
	// the folders are the library's own layout: for somebody who chooses where books go
	assert_refused(
		&world
			.gql(
				&vis.ana.token,
				FOLDERS,
				json!({ "libraryId": world.library.id }),
			)
			.await,
	);
	let response = world
		.as_owner(FOLDERS, json!({ "libraryId": world.library.id }))
		.await;
	assert_ok(&response);
	assert_eq!(response["data"]["intakeFolders"][0]["name"], "Umberto_Eco");
	// and so are the folder settings of the library
	let response = world
		.gql(
			&vis.ana.token,
			r#"query S($id: ID!) { libraryIntakeSettings(libraryId: $id) { mangaDir lightNovelDir } }"#,
			json!({ "id": world.library.id }),
		)
		.await;
	assert_ok(&response);
	assert_eq!(response["data"]["libraryIntakeSettings"]["mangaDir"], "");
	let response = world
		.as_owner(
			r#"query S($id: ID!) { libraryIntakeSettings(libraryId: $id) { mangaDir } }"#,
			json!({ "id": world.library.id }),
		)
		.await;
	assert_eq!(
		response["data"]["libraryIntakeSettings"]["mangaDir"],
		"_Manga"
	);
}

/// sending a book that is the same file as one she can read is refused; that it is the same file
/// as one she can't read is not told (it is the approver's to see)
#[tokio::test(flavor = "multi_thread")]
async fn test_a_duplicate_of_a_hidden_book_is_not_an_answer() {
	let world = world().await;
	let vis = &world.vis;
	let proposal = |book: &models::entity::media::Model| IntakeProposal {
		duplicates: vec![duplicate(book, "SAME_FILE")],
		confidence: 0.9,
		..Default::default()
	};
	let visible = insert_item(
		&world,
		&vis.ana.id,
		IntakeState::Ready,
		proposal(&vis.general),
	)
	.await;
	let hidden = insert_item(
		&world,
		&vis.ana.id,
		IntakeState::Ready,
		proposal(&vis.hidden),
	)
	.await;
	let response = world
		.gql(&vis.ana.token, SUBMIT, json!({ "ids": [visible] }))
		.await;
	assert_refused(&response);
	assert!(
		errors(&response).contains("already in the library"),
		"{response:#}"
	);
	assert_eq!(world.row(&visible).await.state, IntakeState::Ready);

	let response = world
		.gql(&vis.ana.token, SUBMIT, json!({ "ids": [hidden] }))
		.await;
	assert_ok(&response);
	assert_eq!(
		response["data"]["intakeSubmit"][0]["state"],
		"PENDING_APPROVAL"
	);
	// and the owner sees the duplicate she was not told about
	let response = world.as_owner(PENDING, json!({})).await;
	let node = &response["data"]["pendingIntakeItems"]["nodes"][0];
	assert_eq!(node["proposal"]["duplicates"][0]["mediaId"], vis.hidden.id);
	// she can't send it from a library that is hidden from her
	let other = insert_item(
		&world,
		&vis.ana.id,
		IntakeState::Ready,
		IntakeProposal::default(),
	)
	.await;
	intake_item::ActiveModel {
		library_id: Set(vis.hidden_library.id.clone()),
		..world.row(&other).await.into()
	}
	.update(world.app.conn())
	.await
	.unwrap();
	let response = world
		.gql(&vis.ana.token, SUBMIT, json!({ "ids": [other] }))
		.await;
	assert_refused(&response);
	assert!(
		errors(&response).contains("Inbox item not found"),
		"{response:#}"
	);
}

/// a restart finishes nothing by itself, and puts back what it interrupted
#[tokio::test(flavor = "multi_thread")]
async fn test_a_restart_puts_back_what_it_interrupted() {
	let world = world().await;
	let ana = world.ana_id();
	let filing_for_approval =
		insert_item(&world, &ana, IntakeState::Filing, IntakeProposal::default()).await;
	// as if it had been sent: submitted
	intake_item::ActiveModel {
		submitted_at: Set(Some(chrono::Utc::now().into())),
		..world.row(&filing_for_approval).await.into()
	}
	.update(world.app.conn())
	.await
	.unwrap();
	let filing_directly =
		insert_item(&world, &ana, IntakeState::Filing, IntakeProposal::default()).await;
	let analysing = insert_item(
		&world,
		&ana,
		IntakeState::Analysing,
		IntakeProposal::default(),
	)
	.await;
	let filed =
		insert_item(&world, &ana, IntakeState::Filed, IntakeProposal::default()).await;
	let waiting = insert_item(
		&world,
		&ana,
		IntakeState::PendingApproval,
		IntakeProposal::default(),
	)
	.await;
	// an upload that was still arriving
	let incoming = world
		.config_dir
		.join("intake")
		.join(&ana)
		.join("_incoming/ticket");
	fs::create_dir_all(&incoming).unwrap();
	fs::write(incoming.join("001.upload"), b"half a book").unwrap();

	approval::recover_interrupted(&world.app.ctx)
		.await
		.expect("the recovery should run");

	let row = world.row(&filing_for_approval).await;
	assert_eq!(row.state, IntakeState::PendingApproval);
	assert!(row
		.error
		.as_deref()
		.unwrap_or_default()
		.contains("restarted"));
	assert_eq!(world.row(&filing_directly).await.state, IntakeState::Ready);
	let row = world.row(&analysing).await;
	assert_eq!(row.state, IntakeState::Failed);
	assert!(row
		.error
		.as_deref()
		.unwrap_or_default()
		.contains("analyse it again"));
	assert_eq!(world.row(&filed).await.state, IntakeState::Filed);
	assert_eq!(
		world.row(&waiting).await.state,
		IntakeState::PendingApproval
	);
	assert!(
		!incoming.exists(),
		"an upload that never finished is removed"
	);
}

/// filing again after a failure carries on where it stopped: the books already placed are not
/// copied a second time (`_2`)
#[tokio::test(flavor = "multi_thread")]
async fn test_filing_again_carries_on_where_it_stopped() {
	use stump_core::intake::file_claimed;

	let world = world().await;
	let ana = world.ana_id();
	let id = uuid::Uuid::new_v4().to_string();
	let staging = world.config_dir.join("intake").join(&ana).join(&id);
	fs::create_dir_all(&staging).unwrap();
	for name in ["000_a.cbz", "001_b.cbz"] {
		let mut zip = zip::ZipWriter::new(fs::File::create(staging.join(name)).unwrap());
		zip.start_file("page-1.png", zip::write::FileOptions::<()>::default())
			.unwrap();
		std::io::Write::write_all(&mut zip, b"not really a picture").unwrap();
		zip.finish().unwrap();
	}
	let target_dir = world.library_dir.join("_Manga/Resume Series");
	fs::create_dir_all(&target_dir).unwrap();
	// the first volume was placed by the try that failed
	let placed = target_dir.join("Resume Series v01.cbz");
	fs::write(&placed, b"already placed").unwrap();

	let file = |position: usize| IntakeFile {
		original_name: format!("v0{position}.cbz"),
		staged_name: format!("00{}_{}.cbz", position - 1, ["a", "b"][position - 1]),
		size: 100,
		format: "cbz".to_string(),
		volume: Some(position as f64),
		..Default::default()
	};
	intake_item::ActiveModel {
		id: Set(id.clone()),
		user_id: Set(ana.clone()),
		library_id: Set(world.library.id.clone()),
		state: Set(IntakeState::Filing),
		kind: Set(IntakeKind::Series),
		name: Set("Resume Series".to_string()),
		staging_dir: Set(staging.to_string_lossy().into_owned()),
		files: Set(IntakeFiles(vec![file(1), file(2)])),
		decision: Set(Some(IntakeDecision {
			metadata: IntakeMetadata {
				series: Some("Resume Series".to_string()),
				..Default::default()
			},
			series_kind: Some(models::shared::intake::IntakeSeriesKind::Manga),
			folder: "Resume Series".to_string(),
			file_names: vec![
				"Resume Series v01.cbz".to_string(),
				"Resume Series v02.cbz".to_string(),
			],
			volumes: vec![Some(1.0), Some(2.0)],
		})),
		result: Set(Some(models::shared::intake::IntakeResult {
			paths: vec![placed.to_string_lossy().into_owned()],
			notes: vec![],
		})),
		submitted_at: Set(Some(chrono::Utc::now().into())),
		..Default::default()
	}
	.insert(world.app.conn())
	.await
	.unwrap();

	let item = world.row(&id).await;
	let filed = file_claimed(&world.app.ctx, item, IntakeState::PendingApproval)
		.await
		.expect("filing should finish");
	assert_eq!(filed.state, IntakeState::Filed);
	let mut names: Vec<String> = fs::read_dir(&target_dir)
		.unwrap()
		.filter_map(Result::ok)
		.map(|e| e.file_name().to_string_lossy().into_owned())
		.collect();
	names.sort();
	assert_eq!(names, ["Resume Series v01.cbz", "Resume Series v02.cbz"]);
	assert_eq!(
		fs::read(&placed).unwrap(),
		b"already placed",
		"the first volume is left as it was"
	);
	assert_eq!(filed.result.unwrap().paths.len(), 2);
	assert!(
		!staging.exists(),
		"the staged files are removed once it is filed"
	);

	// a filing that fails puts the item back where it came from, with the reason
	let broken =
		insert_item(&world, &ana, IntakeState::Filing, IntakeProposal::default()).await;
	intake_item::ActiveModel {
		decision: Set(Some(IntakeDecision::default())),
		submitted_at: Set(Some(chrono::Utc::now().into())),
		..world.row(&broken).await.into()
	}
	.update(world.app.conn())
	.await
	.unwrap();
	let item = world.row(&broken).await;
	assert!(
		file_claimed(&world.app.ctx, item, IntakeState::PendingApproval)
			.await
			.is_err()
	);
	let row = world.row(&broken).await;
	assert_eq!(row.state, IntakeState::PendingApproval);
	assert!(row.error.is_some());
}

/// what each person may upload, and how much of it they have used
#[tokio::test(flavor = "multi_thread")]
async fn test_the_upload_limits_are_told_and_can_be_changed_for_a_user() {
	let world = world().await;
	let (ana, ben) = (&world.vis.ana, &world.vis.ben);
	let response = world.gql(&ana.token, LIMITS, json!({})).await;
	assert_ok(&response);
	let limits = &response["data"]["intakeUploadLimits"];
	assert_eq!(limits["maxRequestBytes"], 300 * 1024 * 1024);
	assert_eq!(limits["maxFilesPerRequest"], 50);
	assert_eq!(limits["quotaBytes"], 2048i64 * 1024 * 1024);
	assert_eq!(limits["maxItems"], 100);
	assert_eq!(limits["openItems"], 0);
	assert_eq!(limits["quotaApplies"], true);
	assert_eq!(limits["canFileDirectly"], false);
	assert_eq!(limits["uploadsOpen"], true);

	world.ana_uploads("Using the quota").await;
	world.ana_items().await;
	let response = world.gql(&ana.token, LIMITS, json!({})).await;
	assert_eq!(response["data"]["intakeUploadLimits"]["openItems"], 1);
	assert!(
		response["data"]["intakeUploadLimits"]["usedBytes"]
			.as_i64()
			.unwrap() > 0
	);

	// whoever files books themselves is not held to the quota
	let response = world.as_owner(LIMITS, json!({})).await;
	assert_eq!(
		response["data"]["intakeUploadLimits"]["quotaApplies"],
		false
	);
	assert_eq!(
		response["data"]["intakeUploadLimits"]["canFileDirectly"],
		true
	);

	// an admin gives ana limits of her own; the others follow the server
	let response = world
		.as_owner(
			UPDATE_USER_LIMITS,
			json!({ "userId": ana.id, "input": { "quotaMb": 5 } }),
		)
		.await;
	assert_ok(&response);
	let updated = &response["data"]["updateUserIntakeLimits"];
	assert_eq!(updated["quotaMb"], 5);
	assert_eq!(updated["effective"]["quotaBytes"], 5 * 1024 * 1024);
	assert_eq!(updated["effective"]["maxItems"], 100);
	let response = world.gql(&ana.token, LIMITS, json!({})).await;
	assert_eq!(
		response["data"]["intakeUploadLimits"]["quotaBytes"],
		5 * 1024 * 1024
	);

	// she can read her own, not another's; and nobody but an admin changes them
	assert_ok(
		&world
			.gql(&ana.token, USER_LIMITS, json!({ "userId": ana.id }))
			.await,
	);
	assert_refused(
		&world
			.gql(&ana.token, USER_LIMITS, json!({ "userId": ben.id }))
			.await,
	);
	assert_refused(
		&world
			.gql(&ben.token, USER_LIMITS, json!({ "userId": ana.id }))
			.await,
	);
	for token in [&ana.token, &ben.token] {
		assert_refused(
			&world
				.gql(
					token,
					UPDATE_USER_LIMITS,
					json!({ "userId": ana.id, "input": { "quotaMb": 5000 } }),
				)
				.await,
		);
	}
	// taking them away puts her back on the server's
	let response = world
		.as_owner(UPDATE_USER_LIMITS, json!({ "userId": ana.id, "input": {} }))
		.await;
	assert_ok(&response);
	assert!(response["data"]["updateUserIntakeLimits"]["quotaMb"].is_null());
	assert_eq!(
		response["data"]["updateUserIntakeLimits"]["effective"]["quotaBytes"],
		2048i64 * 1024 * 1024
	);
}

/// uploads that wait for approval are off until the server is told they may be read in-process
#[tokio::test(flavor = "multi_thread")]
async fn test_uploads_that_wait_for_approval_are_closed_by_default() {
	let world = world_with(|c| c.intake_probe_mode = "off".to_string()).await;
	let response = world.gql(&world.vis.ana.token, LIMITS, json!({})).await;
	assert_ok(&response);
	let limits = &response["data"]["intakeUploadLimits"];
	assert_eq!(limits["uploadsOpen"], false);
	assert!(limits["closedReason"]
		.as_str()
		.unwrap()
		.contains("approval"));
	let bytes = world.epub("Closed", "Eco, Umberto");
	let response = post_upload(
		&world.app,
		Some(&world.vis.ana.token),
		&world.library.id,
		None,
		&UploadBody::new().file("closed.epub", bytes.clone()),
	)
	.await;
	assert_eq!(response.status_code(), 503, "{}", response.text());
	assert!(message(&response).contains("approval"));
	// the owner files books themselves: open
	let response = world.as_owner(LIMITS, json!({})).await;
	assert_eq!(response["data"]["intakeUploadLimits"]["uploadsOpen"], true);
	let token = world.app.access_token.read().await.clone().unwrap();
	let response = post_upload(
		&world.app,
		Some(&token),
		&world.library.id,
		None,
		&UploadBody::new().file("closed.epub", bytes),
	)
	.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
}
