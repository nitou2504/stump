//! Uploads that wait for an approver: what somebody who may not file books themselves can do with
//! their inbox, what an approver sees and decides, and what a friend never learns about books
//! they can't read.
//!
//! The people are the ones of [`VisibilityFixture`]: the owner files directly; **ana** has
//! "Add books" (and limits: *Hidden* is hidden from her, nothing rated above 16, nothing unrated),
//! so what she uploads waits for the owner; **ben** has no permissions at all.

use std::{fs, path::PathBuf, time::Duration};

use models::shared::enums::UserPermission;
use models::{
	entity::{intake_item, library},
	shared::intake::{
		IntakeDecision, IntakeDestination, IntakeDestinationFile, IntakeDuplicate,
		IntakeFile, IntakeFiles, IntakeFolderCandidate, IntakeGenreSuggestion,
		IntakeKind, IntakeMetadata, IntakeProposal, IntakeReviewInput, IntakeSeriesKind,
		IntakeSimilarBook, IntakeState, IntakeUploaderView,
	},
};
use sea_orm::{prelude::*, ActiveValue::Set};
use serde_json::{json, Value};
use stump_core::intake::approval;
use tempfile::TempDir;
use tests::{fake_data, fixtures};

use crate::common::{
	account::CreateTestUser,
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
const REANALYSE: &str = r#"mutation Reanalyse($id: ID!) {
	intakeReanalyse(id: $id) { id state }
}"#;
const CHANGE_LIBRARY: &str = r#"mutation Change($id: ID!, $libraryId: ID!) {
	intakeChangeLibrary(id: $id, libraryId: $libraryId) { id state libraryId }
}"#;
const DISCARD: &str = r#"mutation Discard($ids: [ID!]!) { intakeDiscard(ids: $ids) }"#;
const ANA_VIEW: &str = r#"query View {
	intakeItems(states: [NEEDS_REVIEW, READY, FAILED]) {
		id name state problems error
		suggestedLibrary { name }
		proposal {
			warnings confidence seriesKind
			authorFolders { folder }
			seriesFolders { folder }
			genreSuggestions { value confidence libraryCount reasons selected kind }
			similarBooks { mediaId title authors reason }
			duplicates { mediaId path title reason fileName }
		}
		decision { seriesKind folder fileNames metadata { genres } }
	}
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
		userId quotaMb maxItems effective { quotaBytes maxItems maxFileBytes maxFilesPerRequest }
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

	/// ana uploads an EPUB over REST whose package carries `metadata` as it is (a creator, a
	/// series...), and the id of the item it makes
	async fn ana_uploads_epub(&self, title: &str, metadata: &str) -> String {
		self.ana_uploads_big_epub(title, metadata, 0).await
	}

	/// The same, with `noise` bytes of a stored entry added to the book (a book that takes a
	/// moment to copy: two of them are copied at once)
	async fn ana_uploads_big_epub(
		&self,
		title: &str,
		metadata: &str,
		noise: usize,
	) -> String {
		let path = self
			.tmp
			.path()
			.join(format!("{}.epub", title.replace(' ', "_")));
		write_unique_epub(&path, title, metadata);
		if noise > 0 {
			append_noise(&path, noise);
		}
		let bytes = fs::read(path).expect("epub bytes");
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

	/// a book of the library, at `<folder>/<name>.epub` under its root (the folder exists on the
	/// disk too, as one the library was filled from)
	async fn add_book_at(
		&self,
		folder: &str,
		name: &str,
		meta: Meta<'_>,
	) -> models::entity::media::Model {
		fs::create_dir_all(self.library_dir.join(folder)).expect("a folder");
		let series = add_series(&self.app, &self.library, name).await;
		let book = add_book(&self.app, &series, name, meta).await;
		let path = self.library_dir.join(folder).join(format!("{name}.epub"));
		models::entity::media::ActiveModel {
			id: sea_orm::ActiveValue::Unchanged(book.id.clone()),
			path: Set(path.to_string_lossy().into_owned()),
			..Default::default()
		}
		.update(self.app.conn())
		.await
		.expect("the book's path");
		book
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
		let row = self.row(id).await;
		panic!(
			"the item never became {state}: it is {} ({:?})",
			row.state, row.error
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

	/// the owner uploads a book of `noise` bytes more than an EPUB needs (a book that takes a
	/// moment to copy), waits for its analysis and makes it READY: the id of the item, and the
	/// owner as the approval functions take them
	async fn owner_has_ready_book(
		&self,
		title: &str,
		noise: usize,
	) -> (String, models::entity::user::AuthUser) {
		let path = self
			.tmp
			.path()
			.join(format!("{}.epub", title.replace(' ', "_")));
		fixtures::write_test_epub(&path, title, "<dc:creator>Eco, Umberto</dc:creator>");
		if noise > 0 {
			append_noise(&path, noise);
		}
		let owner_token = self.app.access_token.read().await.clone().unwrap();
		let response = post_upload(
			&self.app,
			Some(&owner_token),
			&self.library.id,
			None,
			&UploadBody::new().file(&format!("{title}.epub"), fs::read(&path).unwrap()),
		)
		.await;
		assert_eq!(response.status_code(), 200, "{}", response.text());
		let body: Value = response.json();
		let id = body["items"][0]["id"]
			.as_str()
			.expect("an item")
			.to_string();
		let row = self.wait_for_analysis(&id).await;
		let ready = intake_item::ActiveModel {
			state: Set(IntakeState::Ready),
			..row.into()
		};
		ready.update(self.app.conn()).await.expect("ready");
		let owner = approval::load_auth_user(self.app.conn(), &self.owner_id().await)
			.await
			.expect("the owner loads")
			.expect("the owner exists");
		(id, owner)
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

/// An EPUB like the fixture's whose first bytes are its own. The hash of a small book is made of
/// its first bytes, and the fixture's books all start alike: two of them would be the same file
fn write_unique_epub(path: &std::path::Path, title: &str, metadata: &str) {
	use std::io::{Cursor, Write};
	fixtures::write_test_epub(path, title, metadata);
	let mut input = zip::ZipArchive::new(Cursor::new(fs::read(path).expect("the EPUB")))
		.expect("a zip");
	let mut out = zip::ZipWriter::new(fs::File::create(path).expect("the EPUB"));
	out.start_file(
		"0-seed.txt",
		zip::write::SimpleFileOptions::default()
			.compression_method(zip::CompressionMethod::Stored),
	)
	.expect("an entry");
	out.write_all(format!("{title}|{metadata}|{}", "x".repeat(300)).as_bytes())
		.expect("the seed");
	for i in 0..input.len() {
		out.raw_copy_file(input.by_index_raw(i).expect("an entry"))
			.expect("a copy");
	}
	out.finish().expect("the zip");
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

	// she can't file it herself, even when it is as ready as can be, nor upload the way
	// somebody who files does
	let ready = world.row(&id).await;
	intake_item::ActiveModel {
		state: Set(IntakeState::Ready),
		..ready.into()
	}
	.update(world.app.conn())
	.await
	.expect("should make it ready");
	let response = world.gql(&ana.token, ACCEPT, json!({ "ids": [id] })).await;
	assert_refused(&response);
	assert!(
		errors(&response).contains("permission"),
		"refused for who she is, not for the state of the book: {response:#}"
	);
	assert_eq!(world.row(&id).await.state, IntakeState::Ready);
	assert!(!world
		.library_dir
		.join("Umberto_Eco/El_nombre_de_la_rosa.epub")
		.exists());
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
	// what the analysis found for her own view of the library. It can be older than what she
	// may read now (a book rated up since, a library hidden since): what she may not read is
	// taken out when she looks
	let proposal = IntakeProposal {
		uploader_view: Some(IntakeUploaderView {
			series_kind: None,
			genre_suggestions: proposal.genre_suggestions.clone(),
			similar_books: proposal.similar_books.clone(),
			warnings: proposal.warnings.clone(),
			confidence: proposal.confidence,
		}),
		..proposal
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
		uploader_view: Some(IntakeUploaderView {
			series_kind: None,
			genre_suggestions: proposal.genre_suggestions.clone(),
			similar_books: vec![similar(&vis.hidden)],
			warnings: vec![
				"This book may already be in the library".to_string(),
				"No title found".to_string(),
			],
			confidence: 0.5,
		}),
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

/// An item of two manga volumes, staged and in `FILING`, as a try that stopped leaves it. `recorded`:
/// whether that try wrote down the first volume's path in the library as placed. The test puts
/// (or doesn't put) the file there itself.
async fn item_filing_again(world: &World, recorded: bool) -> (String, PathBuf, PathBuf) {
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
	let placed = target_dir.join("Resume Series v01.cbz");

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
		result: Set(recorded.then(|| models::shared::intake::IntakeResult {
			paths: vec![placed.to_string_lossy().into_owned()],
			notes: vec![],
		})),
		submitted_at: Set(Some(chrono::Utc::now().into())),
		..Default::default()
	}
	.insert(world.app.conn())
	.await
	.unwrap();
	(id, staging, target_dir)
}

fn names_in(dir: &std::path::Path) -> Vec<String> {
	let mut names: Vec<String> = fs::read_dir(dir)
		.unwrap()
		.filter_map(Result::ok)
		.map(|e| e.file_name().to_string_lossy().into_owned())
		.collect();
	names.sort();
	names
}

/// filing again after a failure carries on where it stopped: the books already placed are not
/// copied a second time (`_2`)
#[tokio::test(flavor = "multi_thread")]
async fn test_filing_again_carries_on_where_it_stopped() {
	use stump_core::intake::file_claimed;

	let world = world().await;
	let ana = world.ana_id();
	let (id, staging, target_dir) = item_filing_again(&world, true).await;
	// the first volume was placed by the try that failed: a copy of its staged file
	let placed = target_dir.join("Resume Series v01.cbz");
	fs::copy(staging.join("000_a.cbz"), &placed).unwrap();
	let placed_bytes = fs::read(&placed).unwrap();

	let item = world.row(&id).await;
	let filed = file_claimed(&world.app.ctx, item, IntakeState::PendingApproval)
		.await
		.expect("filing should finish");
	assert_eq!(filed.state, IntakeState::Filed);
	assert_eq!(
		names_in(&target_dir),
		["Resume Series v01.cbz", "Resume Series v02.cbz"]
	);
	assert_eq!(
		fs::read(&placed).unwrap(),
		placed_bytes,
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

/// a path is written down before its file is put in the library, so the server stopping between
/// the two leaves a path with no file: trying again puts the book there, under the same name
#[tokio::test(flavor = "multi_thread")]
async fn test_filing_again_places_a_book_that_was_noted_but_never_put_there() {
	use stump_core::intake::file_claimed;

	let world = world().await;
	let (id, staging, target_dir) = item_filing_again(&world, true).await;
	let placed = target_dir.join("Resume Series v01.cbz");
	assert!(!placed.exists());

	let item = world.row(&id).await;
	let filed = file_claimed(&world.app.ctx, item, IntakeState::PendingApproval)
		.await
		.expect("filing should finish");
	assert_eq!(filed.state, IntakeState::Filed);
	assert_eq!(
		names_in(&target_dir),
		["Resume Series v01.cbz", "Resume Series v02.cbz"],
		"no `_2` copy of a book that was never there"
	);
	let paths = filed.result.unwrap().paths;
	assert_eq!(paths.len(), 2);
	assert_eq!(paths[0], placed.to_string_lossy());
	assert!(!staging.exists());
}

/// a path noted by an earlier try that now has somebody else's file at it is not the book's own:
/// the book goes in beside it, and the other file is left alone
#[tokio::test(flavor = "multi_thread")]
async fn test_filing_again_never_takes_somebody_elses_file_for_its_own() {
	use stump_core::intake::file_claimed;

	let world = world().await;
	let (id, _staging, target_dir) = item_filing_again(&world, true).await;
	let placed = target_dir.join("Resume Series v01.cbz");
	fs::write(&placed, b"another book that arrived since").unwrap();

	let item = world.row(&id).await;
	let filed = file_claimed(&world.app.ctx, item, IntakeState::PendingApproval)
		.await
		.expect("filing should finish");
	assert_eq!(filed.state, IntakeState::Filed);
	assert_eq!(
		fs::read(&placed).unwrap(),
		b"another book that arrived since",
		"the other file is not touched"
	);
	assert_eq!(
		names_in(&target_dir),
		[
			"Resume Series v01.cbz",
			"Resume Series v01_2.cbz",
			"Resume Series v02.cbz"
		]
	);
	let paths = filed.result.unwrap().paths;
	assert_eq!(paths.len(), 2);
	assert!(
		!paths.contains(&placed.to_string_lossy().into_owned()),
		"the book is not recorded as the other file"
	);
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

	// whoever files books themselves is not held to the quota, and a request of theirs holds as
	// many files as any request can (the upload route enforces what is told here)
	let response = world.as_owner(LIMITS, json!({})).await;
	assert_eq!(
		response["data"]["intakeUploadLimits"]["quotaApplies"],
		false
	);
	assert_eq!(response["data"]["intakeUploadLimits"]["maxFilesPerRequest"], 500);
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

	// the admin's view says the same of each: ana's request is the server's 50, the owner's 500
	let response = world
		.as_owner(USER_LIMITS, json!({ "userId": ana.id }))
		.await;
	assert_ok(&response);
	assert_eq!(
		response["data"]["userIntakeLimits"]["effective"]["maxFilesPerRequest"],
		50
	);
	let response = world
		.as_owner(USER_LIMITS, json!({ "userId": world.owner_id().await }))
		.await;
	assert_ok(&response);
	assert_eq!(
		response["data"]["userIntakeLimits"]["effective"]["maxFilesPerRequest"],
		500
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

/// every way to change an item is its uploader's; an approver may change one only while it waits
/// for them, and nobody finds out that an item they can't change exists
#[tokio::test(flavor = "multi_thread")]
async fn test_nobody_changes_somebody_elses_item() {
	let world = world().await;
	let id = world.ana_uploads("Not yours").await;
	world.ana_items().await;
	let carl = CreateTestUser {
		username: "carl".to_string(),
		permissions: vec![UserPermission::AddBooks],
		..Default::default()
	}
	.insert(&world.app)
	.await;

	let attempts = || {
		[
			(REVIEW, json!({ "id": id, "input": { "title": "x" } })),
			(REANALYSE, json!({ "id": id })),
			(
				CHANGE_LIBRARY,
				json!({ "id": id, "libraryId": world.library.id }),
			),
			(DISCARD, json!({ "ids": [id] })),
			(WITHDRAW, json!({ "ids": [id] })),
			(SUBMIT, json!({ "ids": [id] })),
		]
	};
	for who in [&world.vis.ben, &carl] {
		for (query, variables) in attempts() {
			let response = world.gql(&who.token, query, variables).await;
			assert_refused(&response);
			// somebody with "Add books" is told the same as for an item that isn't there
			if who.username == "carl" {
				assert!(
					errors(&response).contains("not found"),
					"{query}: {response:#}"
				);
			}
		}
	}
	// the owner approves books: but this one isn't waiting for approval, so it isn't theirs to
	// change, and isn't found
	for (query, variables) in [
		(REVIEW, json!({ "id": id, "input": { "title": "x" } })),
		(REANALYSE, json!({ "id": id })),
		(DISCARD, json!({ "ids": [id] })),
	] {
		let response = world.as_owner(query, variables).await;
		assert_refused(&response);
		assert!(
			errors(&response).contains("not found"),
			"{query}: {response:#}"
		);
	}
	let row = world.row(&id).await;
	assert_ne!(row.state, IntakeState::Discarded);
	assert_eq!(
		row.decision.unwrap().metadata.title.as_deref(),
		Some("Not yours")
	);

	// carl's inbox has nothing of ana's
	let response = world.gql(&carl.token, MY_ITEMS, json!({})).await;
	assert_eq!(response["data"]["intakeItems"], json!([]));

	// once ana sends it, the owner can look at it again, and move it to another library her
	// own account can use: never to one that is hidden from her
	assert_ok(
		&world
			.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": [id] }))
			.await,
	);
	let hidden = &world.vis.hidden_library;
	assert_refused(
		&world
			.as_owner(CHANGE_LIBRARY, json!({ "id": id, "libraryId": hidden.id }))
			.await,
	);
	assert_ok(&world.as_owner(REANALYSE, json!({ "id": id })).await);
	let row = world.wait_for(&id, IntakeState::PendingApproval).await;
	assert!(
		row.submitted_at.is_some(),
		"it is still in the queue after being read again"
	);
	// it is still not the owner's to throw away: the uploader discards, an approver sends back
	assert_refused(&world.as_owner(DISCARD, json!({ "ids": [id] })).await);

	// a list of ids is only so long
	let many: Vec<String> = (0..101).map(|i| format!("id-{i}")).collect();
	let response = world
		.gql(&world.vis.ana.token, DISCARD, json!({ "ids": many }))
		.await;
	assert_refused(&response);
	assert!(errors(&response).contains("At most 100"), "{response:#}");
}

/// a file name ends in the extension of the format it names: an EPUB isn't filed as a PDF
#[tokio::test(flavor = "multi_thread")]
async fn test_a_file_name_keeps_the_extension_of_its_format() {
	let world = world().await;
	let token = world.app.access_token.read().await.clone().unwrap();
	let response = post_upload(
		&world.app,
		Some(&token),
		&world.library.id,
		None,
		&UploadBody::new()
			.file("owner.epub", world.epub("Wrong extension", "Eco, Umberto")),
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
			json!({ "id": id, "input": { "fileNames": ["Wrong_extension.pdf"], "genres": ["History"] } }),
		)
		.await;
	assert_ok(&response);
	let item = &response["data"]["intakeReview"];
	assert_eq!(item["state"], "NEEDS_REVIEW", "{item:#}");
	assert!(
		item["problems"]
			.as_array()
			.unwrap()
			.iter()
			.any(|p| p == "The file name 'Wrong_extension.pdf' must end in .epub"),
		"{item:#}"
	);
	let response = world.as_owner(ACCEPT, json!({ "ids": [id] })).await;
	assert_ok(&response);
	assert_ne!(response["data"]["intakeAccept"][0]["state"], "FILED");
	assert!(
		!world
			.library_dir
			.join("Umberto_Eco/Wrong_extension.pdf")
			.exists(),
		"nothing was filed"
	);
	// with the right extension it is filed
	let response = world
		.as_owner(
			REVIEW,
			json!({ "id": id, "input": { "fileNames": ["Right.epub"] } }),
		)
		.await;
	assert_eq!(response["data"]["intakeReview"]["state"], "READY");
}

/// an approver sees the queue of the libraries they can use, and of no other
#[tokio::test(flavor = "multi_thread")]
async fn test_an_approver_only_sees_the_queue_of_libraries_they_can_use() {
	let world = world().await;
	let vis = &world.vis;
	// a library manager with limits: *Hidden* is hidden from them
	let manager = vis
		.limited_like_ana(
			&world.app,
			"manager",
			vec![UserPermission::ManageLibrary, UserPermission::AddBooks],
		)
		.await;
	let open = insert_item(
		&world,
		&vis.ana.id,
		IntakeState::PendingApproval,
		IntakeProposal::default(),
	)
	.await;
	let hidden = insert_item(
		&world,
		&vis.ana.id,
		IntakeState::PendingApproval,
		IntakeProposal::default(),
	)
	.await;
	intake_item::ActiveModel {
		library_id: Set(vis.hidden_library.id.clone()),
		..world.row(&hidden).await.into()
	}
	.update(world.app.conn())
	.await
	.unwrap();

	let response = world.gql(&manager.token, PENDING, json!({})).await;
	assert_ok(&response);
	let queue = &response["data"]["pendingIntakeItems"];
	assert_eq!(queue["totalCount"], 1, "{response:#}");
	assert_eq!(queue["nodes"][0]["id"], open);
	vis.assert_none_hidden(&response);
	let response = world.gql(&manager.token, PENDING_COUNT, json!({})).await;
	assert_eq!(response["data"]["pendingIntakeCount"], 1);
	// they can't approve, send back or change what they can't see: it isn't found
	for (query, variables) in [
		(APPROVE, json!({ "ids": [hidden] })),
		(REJECT, json!({ "id": hidden, "note": "no" })),
		(
			REVIEW,
			json!({ "id": hidden, "input": { "title": "mine now" } }),
		),
	] {
		let response = world.gql(&manager.token, query, variables).await;
		assert_refused(&response);
		assert!(
			errors(&response).contains("not found"),
			"{query}: {response:#}"
		);
	}
	assert_eq!(world.row(&hidden).await.state, IntakeState::PendingApproval);
	// the owner sees both
	let response = world.as_owner(PENDING, json!({})).await;
	assert_eq!(response["data"]["pendingIntakeItems"]["totalCount"], 2);
	// and what they can see, they approve (an item with nothing to file only fails, and goes
	// back to waiting, with the reason)
	let response = world
		.gql(&manager.token, APPROVE, json!({ "ids": [open] }))
		.await;
	assert_ok(&response);
}


/// what ana is shown of one item, without what is about the file itself (its id and name)
fn ana_view_of(items: &Value, name: &str) -> Value {
	let mut item = items
		.as_array()
		.expect("a list of items")
		.iter()
		.find(|item| item["name"] == name)
		.unwrap_or_else(|| panic!("no item named {name}: {items:#}"))
		.clone();
	item.as_object_mut().expect("an item").remove("id");
	item.as_object_mut().expect("an item").remove("name");
	item
}

/// A real analysis over a library whose author folder and series folder hold only books ana can't
/// read: what she is shown of her own upload is what she is shown for an author and a series
/// the library has never heard of (the confidence, the state, the kind of series, the tags the
/// books vote for, the warnings), while the approver's own view of the analysis knows both
#[tokio::test(flavor = "multi_thread")]
async fn test_an_analysis_does_not_tell_a_friend_what_a_folder_of_hidden_books_holds() {
	let world = world().await;
	// a book she can read, with a tag that the books she can't read share
	world
		.add_book_at(
			"Visible_Author",
			"Light",
			Meta {
				age: Some(0),
				writer: Some("Visible Author"),
				genre: Some("Horror: Gore"),
				..Default::default()
			},
		)
		.await;
	// an author's folder and a light novel series' folder that hold only books rated 18 (she
	// reads nothing above 16)
	for (folder, name, writer) in [
		("Hidden_Author", "Dark", "Hidden Author"),
		("_Novela ligera/Hidden Saga", "Saga 1", "Hidden Writer"),
	] {
		world
			.add_book_at(
				folder,
				name,
				Meta {
					age: Some(18),
					writer: Some(writer),
					genre: Some("Horror: Gore"),
					..Default::default()
				},
			)
			.await;
	}
	// the premise: she reads one book with the tag, the owner three
	let response = world
		.gql(
			&world.vis.ana.token,
			VOCABULARY,
			json!({ "libraryId": world.library.id }),
		)
		.await;
	assert_ok(&response);
	let count_of = |response: &Value| {
		response["data"]["intakeVocabulary"]
			.as_array()
			.expect("a list")
			.iter()
			.find(|tag| tag["value"] == "Horror: Gore")
			.map(|tag| tag["count"].clone())
	};
	assert_eq!(count_of(&response), Some(json!(1)), "{response:#}");
	let response = world
		.as_owner(VOCABULARY, json!({ "libraryId": world.library.id }))
		.await;
	assert_eq!(count_of(&response), Some(json!(3)), "{response:#}");

	// the same uploads, once for what the hidden books know and once for what nobody does
	let by_hidden_author = world
		.ana_uploads_epub(
			"Known Author",
			r#"<dc:creator>Hidden Author</dc:creator><dc:subject>Novela</dc:subject>"#,
		)
		.await;
	world
		.ana_uploads_epub(
			"Unknown Author",
			r#"<dc:creator>Nobody Known</dc:creator><dc:subject>Novela</dc:subject>"#,
		)
		.await;
	let in_hidden_series = world
		.ana_uploads_epub(
			"Known Series",
			r#"<dc:creator>Some Writer</dc:creator><meta name="calibre:series" content="Hidden Saga"/>"#,
		)
		.await;
	world
		.ana_uploads_epub(
			"Unknown Series",
			r#"<dc:creator>Some Writer</dc:creator><meta name="calibre:series" content="Never Heard Saga"/>"#,
		)
		.await;
	world.ana_items().await;

	let response = world
		.gql(&world.vis.ana.token, ANA_VIEW, json!({}))
		.await;
	assert_ok(&response);
	let items = &response["data"]["intakeItems"];
	assert_eq!(items.as_array().map(Vec::len), Some(4), "{response:#}");
	assert_eq!(
		ana_view_of(items, "Known Author.epub"),
		ana_view_of(items, "Unknown Author.epub"),
		"an author folder of books she can't read must answer as an author the library doesn't know"
	);
	assert_eq!(
		ana_view_of(items, "Known Series.epub"),
		ana_view_of(items, "Unknown Series.epub"),
		"a series folder of books she can't read must answer as a series the library doesn't know"
	);
	// nothing of what the hidden books say is in her view of the item, in any field
	for name in ["Known Author.epub", "Known Series.epub"] {
		let view = ana_view_of(items, name);
		assert!(view["proposal"]["seriesKind"].is_null(), "{view:#}");
		assert!(view["decision"]["seriesKind"].is_null(), "{view:#}");
		assert!(
			view["proposal"]["authorFolders"]
				.as_array()
				.is_some_and(Vec::is_empty),
			"{view:#}"
		);
		let text = view.to_string();
		for word in ["Hidden_Author", "Hidden Saga", "Hidden Author", "Hidden Writer", "Dark"] {
			assert!(!text.contains(word), "{word} reached her: {view:#}");
		}
	}

	// the approver's own analysis does know them: the folder, the kind of series, and her item has a
	// view of its own
	let row = world.row(&by_hidden_author).await;
	let proposal = row.proposal.expect("a proposal");
	assert_eq!(
		proposal.author_folders.first().map(|c| c.folder.as_str()),
		Some("Hidden_Author"),
		"{proposal:#?}"
	);
	assert!(proposal.uploader_view.is_some());
	assert_eq!(
		row.decision.expect("a decision").folder,
		"Hidden_Author",
		"the folder is worked out from the whole library, for the approver"
	);
	let row = world.row(&in_hidden_series).await;
	assert_eq!(
		row.proposal.expect("a proposal").series_kind,
		Some(IntakeSeriesKind::LightNovel)
	);
}


/// the text of a file in an EPUB
fn epub_entry(path: &std::path::Path, name: &str) -> String {
	use std::io::Read;
	let mut archive = zip::ZipArchive::new(fs::File::open(path).expect("the EPUB"))
		.expect("a zip archive");
	let mut text = String::new();
	archive
		.by_name(name)
		.expect("the entry")
		.read_to_string(&mut text)
		.expect("text");
	text
}

/// The description of a book is shown, as Markdown with the HTML in it, on the screens of whoever
/// reads the library. A friend's file carries its own, and an EPUB's long one is kept by a plain
/// filing: what is shown, and what is filed, is a plain text of its own
#[tokio::test(flavor = "multi_thread")]
async fn test_a_friends_description_is_plain_in_what_gets_filed() {
	let world = world().await;
	let hostile = concat!(
		"<dc:creator>Eco, Umberto</dc:creator>",
		r#"<dc:description>&lt;iframe srcdoc="&lt;script&gt;alert(1)&lt;/script&gt;"&gt;&lt;/iframe&gt;"#,
		"A description that is long enough to be kept by a plain filing: it has well over fifteen words in it, so the file's own would stay.",
		"</dc:description>"
	);
	let id = world.ana_uploads_epub("Hostile Description", hostile).await;
	let items = world.ana_items().await;
	let item = items
		.as_array()
		.expect("items")
		.iter()
		.find(|item| item["id"] == id)
		.expect("her item");
	let shown = item["proposal"]["metadata"]["description"]
		.as_str()
		.expect("a description");
	assert!(
		shown.contains("A description that is long enough") && !shown.contains('<'),
		"she is shown a plain text: {shown}"
	);
	// what the approver's own card holds is the plain text too
	let stored = world.row(&id).await;
	let decision = stored.decision.expect("a decision");
	assert!(
		decision
			.metadata
			.description
			.as_deref()
			.is_some_and(|d| !d.contains('<') && d.contains("A description that is long")),
		"{decision:#?}"
	);

	assert_ok(
		&world
			.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": [id] }))
			.await,
	);
	assert_ok(&world.as_owner(APPROVE, json!({ "ids": [id] })).await);
	world.wait_for(&id, IntakeState::Filed).await;
	let filed = world
		.library_dir
		.join("Umberto_Eco/Hostile_Description.epub");
	assert!(filed.exists(), "the book should be in its author folder");
	let opf = epub_entry(&filed, "OEBPS/content.opf");
	for markup in ["iframe", "<script", "srcdoc", "&lt;"] {
		assert!(!opf.contains(markup), "{markup} was filed:\n{opf}");
	}
	assert!(
		opf.contains("A description that is long enough"),
		"the plain description is what the file carries:\n{opf}"
	);
	assert_eq!(opf.matches("<dc:description>").count(), 1, "{opf}");

	// without a description of its own, the file's is taken out all the same (the owner's own
	// uploads are not changed: they are trusted)
	let id = world
		.ana_uploads_epub(
			"Only Markup",
			concat!(
				"<dc:creator>Eco, Umberto</dc:creator>",
				r#"<dc:description>&lt;iframe srcdoc="x"&gt;&lt;/iframe&gt;</dc:description>"#
			),
		)
		.await;
	world.ana_items().await;
	assert_ok(
		&world
			.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": [id] }))
			.await,
	);
	assert_ok(&world.as_owner(APPROVE, json!({ "ids": [id] })).await);
	world.wait_for(&id, IntakeState::Filed).await;
	let opf = epub_entry(
		&world.library_dir.join("Umberto_Eco/Only_Markup.epub"),
		"OEBPS/content.opf",
	);
	assert!(!opf.contains("iframe"), "{opf}");
	assert!(!opf.contains("dc:description"), "{opf}");
}

/// two books that file under one name are each filed whole, whichever copies first: the second
/// takes the next name, and neither holds the bytes of the other. The books are big, so that
/// their copies overlap: with one temporary name for both, the one that was linked first would
/// end up holding the bytes of the other (and both files would be the same book)
#[tokio::test(flavor = "multi_thread")]
async fn test_two_books_with_one_name_are_each_filed_whole() {
	let world = world().await;
	let mut ids = vec![];
	for edition in ["First", "Second"] {
		ids.push(
			world
				.ana_uploads_big_epub(
					"Twin",
					&format!(
						"<dc:creator>Eco, Umberto</dc:creator><dc:description>The {edition} edition of the twin book.</dc:description>"
					),
					32 << 20,
				)
				.await,
		);
	}
	world.ana_items().await;
	assert_ok(
		&world
			.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": ids }))
			.await,
	);
	assert_ok(&world.as_owner(APPROVE, json!({ "ids": ids })).await);
	for id in &ids {
		world.wait_for(id, IntakeState::Filed).await;
	}
	let mut editions = vec![];
	for name in ["Twin.epub", "Twin_2.epub"] {
		let opf = epub_entry(
			&world.library_dir.join("Umberto_Eco").join(name),
			"OEBPS/content.opf",
		);
		editions.push(if opf.contains("First edition") {
			"first"
		} else if opf.contains("Second edition") {
			"second"
		} else {
			"neither"
		});
	}
	editions.sort();
	assert_eq!(
		editions,
		["first", "second"],
		"each file holds its own book"
	);
	// no temporary copy is left in the folder
	assert!(
		names_in(&world.library_dir.join("Umberto_Eco"))
			.iter()
			.all(|name| !name.ends_with(".intake")),
		"{:?}",
		names_in(&world.library_dir.join("Umberto_Eco"))
	);
}

/// a link that points nowhere sits at the name a book files under. The name is taken all the
/// same (a file can't be linked over a link), so the book takes the next name: it used to look
/// the same name up again, for ever, copying the whole book and writing the database each time
#[tokio::test(flavor = "multi_thread")]
async fn test_a_link_to_nothing_at_the_name_is_a_taken_name() {
	let world = world().await;
	let author = world.library_dir.join("Umberto_Eco");
	let link = author.join("Dangling.epub");
	std::os::unix::fs::symlink(world.library_dir.join("nowhere.epub"), &link)
		.expect("a link");
	assert!(!link.exists(), "the link points nowhere");
	let id = world
		.ana_uploads_epub(
			"Dangling",
			"<dc:creator>Eco, Umberto</dc:creator><dc:description>A book that files next to a link to nothing.</dc:description>",
		)
		.await;
	world.ana_items().await;
	assert_ok(
		&world
			.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": [id] }))
			.await,
	);
	assert_ok(&world.as_owner(APPROVE, json!({ "ids": [id] })).await);
	world.wait_for(&id, IntakeState::Filed).await;
	assert!(
		author.join("Dangling_2.epub").is_file(),
		"the book takes the next name: {:?}",
		names_in(&author)
	);
	assert!(
		fs::symlink_metadata(&link)
			.expect("the link")
			.file_type()
			.is_symlink(),
		"nothing was put over the link"
	);
	assert!(
		names_in(&author)
			.iter()
			.all(|name| !name.ends_with(".intake")),
		"no temporary copy is left: {:?}",
		names_in(&author)
	);
}

/// the stored state of an item moves only from the state it was read in: a review, a move or an
/// analysis that was read while the item waited doesn't write waiting over an approval
#[tokio::test(flavor = "multi_thread")]
async fn test_what_was_read_before_an_approval_does_not_undo_it() {
	use stump_core::intake::{apply_review, change_library, Reviewer};

	let world = world().await;
	let id = world.ana_uploads("Stale Review").await;
	world.ana_items().await;
	assert_ok(
		&world
			.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": [id] }))
			.await,
	);
	// read while it waits for approval...
	let stale = world.row(&id).await;
	assert_eq!(stale.state, IntakeState::PendingApproval);
	// ...and approved and filed meanwhile
	assert_ok(&world.as_owner(APPROVE, json!({ "ids": [id] })).await);
	world.wait_for(&id, IntakeState::Filed).await;

	let approver = Reviewer {
		approver: true,
		owns_item: false,
	};
	let ctx = &world.app.ctx;
	let review = IntakeReviewInput {
		title: Some("Too late".to_string()),
		..Default::default()
	};
	let error = apply_review(ctx, stale.clone(), review, approver)
		.await
		.expect_err("a review of what was read before the approval is refused");
	assert!(error.to_string().contains("changed while"), "{error}");
	let error = change_library(ctx, stale.clone(), &world.library.id, approver)
		.await
		.expect_err("so is a move");
	assert!(error.to_string().contains("changed while"), "{error}");
	let row = world.row(&id).await;
	assert_eq!(
		row.state,
		IntakeState::Filed,
		"it is not waiting again, to be filed twice"
	);
	assert_ne!(
		row.decision.and_then(|d| d.metadata.title).as_deref(),
		Some("Too late")
	);
}

/// a zip with the given entries (name, text), in memory
fn zip_bytes(entries: &[(&str, &str)]) -> Vec<u8> {
	use std::io::Write;
	let mut buffer = std::io::Cursor::new(vec![]);
	{
		let mut zip = zip::ZipWriter::new(&mut buffer);
		for (name, text) in entries {
			zip.start_file(*name, zip::write::SimpleFileOptions::default())
				.expect("an entry");
			zip.write_all(text.as_bytes()).expect("its text");
		}
		zip.finish().expect("the zip");
	}
	buffer.into_inner()
}

/// every file under `dir`, with `extension`
fn files_with(dir: &std::path::Path, extension: &str) -> Vec<PathBuf> {
	let mut found = vec![];
	let mut pending = vec![dir.to_path_buf()];
	while let Some(next) = pending.pop() {
		for entry in fs::read_dir(&next).into_iter().flatten().flatten() {
			let path = entry.path();
			if path.is_dir() {
				pending.push(path);
			} else if path.extension().is_some_and(|e| e == extension) {
				found.push(path);
			}
		}
	}
	found
}

/// what the scan reads as the description of a file: what the library shows the book with
async fn scanned_summary(world: &World, path: &std::path::Path) -> Option<String> {
	let options = stump_core::media::processor::MediaProcessorOptions {
		process_metadata: true,
		organization_mode: models::shared::enums::OrganizationMode::Metadata,
		..Default::default()
	};
	stump_core::media::processor::process_file(path, options, &world.app.ctx.config)
		.await
		.expect("the filed book can be read")
		.metadata
		.and_then(|metadata| metadata.summary)
}

impl World {
	/// ana uploads a file over REST: the id of the item it makes
	async fn ana_uploads_file(&self, name: &str, bytes: Vec<u8>) -> String {
		let response = post_upload(
			&self.app,
			Some(&self.vis.ana.token),
			&self.library.id,
			None,
			&UploadBody::new().file(name, bytes),
		)
		.await;
		assert_eq!(response.status_code(), 200, "{}", response.text());
		let body: Value = response.json();
		body["items"][0]["id"]
			.as_str()
			.expect("an item id")
			.to_string()
	}

	/// ana fixes what she can (`review`, if any), submits the item and the owner approves it; what
	/// became of it
	async fn approve_ana_item(
		&self,
		id: &str,
		review: Option<Value>,
	) -> intake_item::Model {
		self.ana_items().await;
		if let Some(input) = review {
			assert_ok(
				&self
					.gql(
						&self.vis.ana.token,
						REVIEW,
						json!({ "id": id, "input": input }),
					)
					.await,
			);
		}
		assert_ok(
			&self
				.gql(&self.vis.ana.token, SUBMIT, json!({ "ids": [id] }))
				.await,
		);
		assert_ok(&self.as_owner(APPROVE, json!({ "ids": [id] })).await);
		// filed, or put back in the queue with the reason
		for _ in 0..150 {
			let row = self.row(id).await;
			if row.state != IntakeState::Filing {
				return row;
			}
			tokio::time::sleep(Duration::from_millis(100)).await;
		}
		panic!("the item stayed in FILING");
	}
}

/// A file's own description can say itself in more places than the one a filing writes: an OPF's
/// `meta` and `summary`, a `ComicInfo.xml` in a folder. A friend's book is filed with what the
/// scan reads as its description being the plain one, or none
#[tokio::test(flavor = "multi_thread")]
async fn test_a_friends_description_is_plain_wherever_a_file_says_it() {
	let world = world().await;

	// an EPUB whose own description is in a `meta` and in a `summary`, with a plain one to file
	let id = world
		.ana_uploads_epub(
			"Hostile Meta",
			concat!(
				"<dc:creator>Eco, Umberto</dc:creator>",
				r#"<meta name="description" content="&lt;iframe srcdoc=x&gt;&lt;/iframe&gt;"/>"#,
				"<summary>&lt;script&gt;x&lt;/script&gt;</summary>",
			),
		)
		.await;
	let row = world.approve_ana_item(&id, None).await;
	assert_eq!(row.state, IntakeState::Filed, "{:?}", row.error);
	let filed = world.library_dir.join("Umberto_Eco/Hostile_Meta.epub");
	let opf = epub_entry(&filed, "OEBPS/content.opf");
	for markup in ["iframe", "script", "srcdoc", "&lt;"] {
		assert!(!opf.contains(markup), "{markup} was filed:\n{opf}");
	}
	let summary = scanned_summary(&world, &filed).await;
	assert!(
		summary.as_deref().is_none_or(|s| !s.contains('<')),
		"{summary:?}"
	);

	// a comic whose `ComicInfo.xml` is in a folder (and another at the root)
	let evil = r#"<?xml version="1.0"?><ComicInfo><Series>Hostile</Series><Summary>&lt;iframe srcdoc=x&gt;&lt;/iframe&gt;</Summary></ComicInfo>"#;
	let id = world
		.ana_uploads_file(
			"Hostile Comic.cbz",
			zip_bytes(&[
				("01.png", "png"),
				("sub/ComicInfo.xml", evil),
				("ComicInfo.xml", evil),
				("02.png", "png"),
			]),
		)
		.await;
	let row = world.approve_ana_item(&id, None).await;
	assert_eq!(row.state, IntakeState::Filed, "{:?}", row.error);
	let filed = files_with(&world.library_dir, "cbz");
	assert_eq!(filed.len(), 1, "{filed:?}");
	let archive = zip::ZipArchive::new(fs::File::open(&filed[0]).unwrap()).unwrap();
	let infos: Vec<&str> = archive
		.file_names()
		.filter(|name| name.to_lowercase().ends_with("comicinfo.xml"))
		.collect();
	assert_eq!(
		infos,
		["ComicInfo.xml"],
		"only the one that was written stays"
	);
	let summary = scanned_summary(&world, &filed[0]).await;
	assert!(
		summary.as_deref().is_none_or(|s| !s.contains('<')),
		"{summary:?}"
	);
}

/// A format that can't be written (a RAR) is filed with the description it has when that is the
/// plain one, as the scan reads it: its `ComicInfo.xml` is in a folder
#[tokio::test(flavor = "multi_thread")]
async fn test_a_friends_rar_with_a_plain_description_is_filed_as_it_is() {
	let world = world().await;
	let bytes = fs::read(fixtures::get_test_complex_rar_path()).expect("the RAR fixture");
	let id = world.ana_uploads_file("Tree.cbr", bytes).await;
	let row = world
		.approve_ana_item(&id, Some(json!({ "series": "Tree" })))
		.await;
	assert_eq!(row.state, IntakeState::Filed, "{:?}", row.error);
	let filed = files_with(&world.library_dir, "cbr");
	assert_eq!(filed.len(), 1, "{filed:?}");
	let summary = scanned_summary(&world, &filed[0]).await;
	assert!(
		summary.as_deref().is_some_and(|s| !s.contains('<')),
		"{summary:?}"
	);
}

/// A book that can't be written and can't be read is not one that was shown to be plain: it is
/// not filed, it goes back to the approver's queue with the reason, and nothing is put in the
/// library (a PDF whose trailer says it is encrypted, with a description of its own)
#[tokio::test(flavor = "multi_thread")]
async fn test_a_friends_book_that_cannot_be_checked_is_not_filed() {
	let world = world().await;
	let pdf = concat!(
		"%PDF-1.4\n",
		"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
		"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
		"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n",
		"4 0 obj\n<< /Title (Locked) /Author (Eco, Umberto) /Subject (<iframe srcdoc=x>) >>\nendobj\n",
		"trailer\n<< /Size 5 /Root 1 0 R /Info 4 0 R /Encrypt 9 0 R >>\n",
		"startxref\n0\n%%EOF\n"
	);
	let id = world
		.ana_uploads_file("Locked.pdf", pdf.as_bytes().to_vec())
		.await;
	let row = world
		.approve_ana_item(&id, Some(json!({ "authors": ["Umberto Eco"] })))
		.await;
	assert_ne!(row.state, IntakeState::Filed, "it was filed: {row:?}");
	assert_eq!(row.state, IntakeState::PendingApproval, "{:?}", row.error);
	let error = row.error.expect("the reason");
	assert!(error.contains("description"), "{error}");
	assert!(
		files_with(&world.library_dir, "pdf").is_empty(),
		"nothing was put in the library"
	);
	// and the friend is told, in words that say nothing of the server
	let response = world.ana_items().await;
	let text = response.to_string();
	assert!(
		!text.contains(&*world.tmp.path().to_string_lossy()),
		"{text}"
	);
}

/// 40 MB of noise in an EPUB (a stored entry, so nothing makes it small): a book that takes a
/// moment to file
fn append_noise(path: &std::path::Path, bytes: usize) {
	use std::io::Write;
	let file = fs::OpenOptions::new()
		.read(true)
		.write(true)
		.open(path)
		.expect("the EPUB");
	let mut zip = zip::ZipWriter::new_append(file).expect("an appendable zip");
	zip.start_file(
		"OEBPS/noise.bin",
		zip::write::SimpleFileOptions::default()
			.compression_method(zip::CompressionMethod::Stored),
	)
	.expect("an entry");
	let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
	let mut chunk = vec![0u8; 1 << 20];
	for _ in 0..(bytes >> 20) {
		for piece in chunk.chunks_mut(8) {
			x ^= x << 13;
			x ^= x >> 7;
			x ^= x << 17;
			piece.copy_from_slice(&x.to_le_bytes()[..piece.len()]);
		}
		zip.write_all(&chunk).expect("noise");
	}
	zip.finish().expect("the zip");
}

/// a request that is dropped while the book is being filed (a closed tab, a proxy's timeout)
/// doesn't leave it in FILING: the filing goes on without it
#[tokio::test(flavor = "multi_thread")]
async fn test_an_accept_that_is_dropped_does_not_leave_the_book_filing() {
	let world = world().await;
	let (id, owner) = world.owner_has_ready_book("Big Book", 40 << 20).await;
	let item = world.row(&id).await;
	let ctx = world.app.ctx.clone();
	let request = tokio::spawn(async move { approval::accept(&ctx, &owner, item).await });
	// once it is being filed, the request is dropped on the floor
	for _ in 0..10_000 {
		if world.row(&id).await.state != IntakeState::Ready {
			break;
		}
		tokio::time::sleep(Duration::from_millis(1)).await;
	}
	request.abort();
	world.wait_for(&id, IntakeState::Filed).await;
	assert!(world.library_dir.join("Umberto_Eco/Big_Book.epub").exists());
}

/// the request is dropped while it waits for the database, before anything was claimed: the
/// claim, the read of what was claimed and the filing are the task's, so the book is filed all
/// the same once the database answers. When the request made the claim, a request that was
/// dropped there left the book in READY with nobody filing it, or claimed and in FILING with
/// nobody filing it.
#[tokio::test(flavor = "multi_thread")]
async fn test_an_accept_that_is_dropped_while_it_waits_for_the_database_still_files_the_book(
) {
	let world = world().await;
	let (id, owner) = world.owner_has_ready_book("Dropped Early", 0).await;
	let item = world.row(&id).await;
	let ctx = world.app.ctx.clone();
	// while the connections are held, everything that needs the database waits for them
	let pool = world.app.conn().get_sqlite_connection_pool();
	let mut held = vec![];
	for _ in 0..pool.options().get_max_connections() {
		held.push(pool.acquire().await.expect("a connection"));
	}
	let request = tokio::spawn(async move { approval::accept(&ctx, &owner, item).await });
	tokio::time::sleep(Duration::from_millis(300)).await;
	assert!(
		!request.is_finished(),
		"the request is waiting for the database"
	);
	request.abort();
	drop(held);
	world.wait_for(&id, IntakeState::Filed).await;
	assert!(world
		.library_dir
		.join("Umberto_Eco/Dropped_Early.epub")
		.exists());
}

/// an approval whose request is dropped once the book is claimed: the filing goes on without
/// it, the book is not left in FILING
#[tokio::test(flavor = "multi_thread")]
async fn test_an_approval_that_is_dropped_does_not_leave_the_book_filing() {
	let world = world().await;
	let id = world
		.ana_uploads_big_epub(
			"Big Approved",
			"<dc:creator>Eco, Umberto</dc:creator>",
			40 << 20,
		)
		.await;
	world.ana_items().await;
	assert_ok(
		&world
			.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": [id] }))
			.await,
	);
	let owner = approval::load_auth_user(world.app.conn(), &world.owner_id().await)
		.await
		.expect("the owner loads")
		.expect("the owner exists");
	let ctx = world.app.ctx.clone();
	let ids = vec![id.clone()];
	let request =
		tokio::spawn(async move { approval::approve(&ctx, &owner, &ids).await });
	for _ in 0..10_000 {
		if world.row(&id).await.state != IntakeState::PendingApproval {
			break;
		}
		tokio::time::sleep(Duration::from_millis(1)).await;
	}
	request.abort();
	world.wait_for(&id, IntakeState::Filed).await;
	assert!(world
		.library_dir
		.join("Umberto_Eco/Big_Approved.epub")
		.exists());
}

/// the same file twice: the one filed first is filed, the other waits for its approver, who is
/// told that a copy was added since (and who files it as well by approving once more). Of two
/// approved together only one goes on. A friend is told nothing of it: it is what the library
/// holds
#[tokio::test(flavor = "multi_thread")]
async fn test_a_copy_added_since_the_book_was_looked_at_is_not_filed_silently() {
	let world = world().await;
	let bytes = world.epub("Twin Copy", "Eco, Umberto");
	let mut ids = vec![];
	for name in ["Twin Copy.epub", "Twin Copy again.epub"] {
		let response = post_upload(
			&world.app,
			Some(&world.vis.ana.token),
			&world.library.id,
			None,
			&UploadBody::new().file(name, bytes.clone()),
		)
		.await;
		assert_eq!(response.status_code(), 200, "{}", response.text());
		let body: Value = response.json();
		ids.push(
			body["items"][0]["id"]
				.as_str()
				.expect("an item")
				.to_string(),
		);
	}
	world.ana_items().await;
	assert_ok(
		&world
			.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": ids }))
			.await,
	);
	// approved together: one is filed, the other is held back
	assert_ok(&world.as_owner(APPROVE, json!({ "ids": ids })).await);
	let mut states = vec![];
	for id in &ids {
		for _ in 0..150 {
			if world.row(id).await.state != IntakeState::Filing {
				break;
			}
			tokio::time::sleep(Duration::from_millis(100)).await;
		}
		states.push(world.row(id).await);
	}
	states.sort_by_key(|row| row.state.to_string());
	assert_eq!(
		states.iter().map(|row| row.state).collect::<Vec<_>>(),
		[IntakeState::Filed, IntakeState::PendingApproval],
		"{states:#?}"
	);
	let held = states.remove(1);
	assert!(
		held.error
			.as_deref()
			.is_some_and(|e| e.contains("added to the library after")),
		"the approver is told why: {:?}",
		held.error
	);
	assert!(
		held.proposal
			.as_ref()
			.is_some_and(|p| p.duplicates.iter().any(|d| d.reason == "SAME_FILE")),
		"and shown the copy"
	);
	assert_eq!(
		names_in(&world.library_dir.join("Umberto_Eco"))
			.iter()
			.filter(|name| name.starts_with("Twin_Copy"))
			.count(),
		1
	);
	// the friend is not told what the library holds
	let items = world.ana_items().await;
	let hers = items
		.as_array()
		.unwrap()
		.iter()
		.find(|item| item["id"] == held.id)
		.expect("her item");
	assert!(hers["error"].is_null(), "{hers:#}");
	assert_eq!(hers["state"], "PENDING_APPROVAL");

	// the approver has been told and looks at the copy: approving once more files it as well
	assert_ok(&world.as_owner(APPROVE, json!({ "ids": [held.id] })).await);
	world.wait_for(&held.id, IntakeState::Filed).await;
	assert_eq!(
		names_in(&world.library_dir.join("Umberto_Eco"))
			.iter()
			.filter(|name| name.starts_with("Twin_Copy"))
			.count(),
		2
	);
}

/// what only the library knows (a name that is taken) isn't in the reason a book wasn't filed,
/// and a friend's view of the reason has none of it either
#[tokio::test(flavor = "multi_thread")]
async fn test_the_reason_a_book_was_not_filed_says_nothing_of_the_library() {
	let world = world().await;
	let id = insert_item(
		&world,
		&world.ana_id(),
		IntakeState::PendingApproval,
		IntakeProposal::default(),
	)
	.await;
	// no title, and the name its file would get is taken
	let row = world.row(&id).await;
	let mut decision = row.decision.clone().expect("a decision");
	decision.metadata.title = None;
	intake_item::ActiveModel {
		decision: Set(Some(decision)),
		..row.into()
	}
	.update(world.app.conn())
	.await
	.expect("the decision");
	fs::write(world.library_dir.join("Umberto_Eco/crafted.epub"), b"taken").unwrap();

	assert_ok(&world.as_owner(APPROVE, json!({ "ids": [id] })).await);
	let row = world.wait_for(&id, IntakeState::PendingApproval).await;
	for _ in 0..100 {
		if world.row(&id).await.error.is_some() {
			break;
		}
		tokio::time::sleep(Duration::from_millis(50)).await;
	}
	let error = world.row(&id).await.error.expect("a reason");
	assert!(error.contains("Add a title"), "{error}");
	assert!(!error.contains("already in the library"), "{error}");
	drop(row);
	let items = world.ana_items().await;
	let hers = items
		.as_array()
		.unwrap()
		.iter()
		.find(|item| item["id"] == id)
		.expect("her item");
	assert!(
		!hers.to_string().contains("already in the library"),
		"{hers:#}"
	);
}

// ------------------------------------------------------------------------------------------
// The isolated checker, with the real binary: `stump intake-probe run` is started as a child of
// the test, so what is proved is the process boundary and not only the code on either side of it.
// ------------------------------------------------------------------------------------------

/// a server whose friends' books are read by the checker (a child process of the real binary)
async fn sandboxed_world() -> World {
	world_with(|c| {
		c.intake_probe_mode = "subprocess".to_string();
		c.intake_probe_exe = Some(env!("CARGO_BIN_EXE_stump_server").to_string());
		c.intake_probe_timeout_secs = 60;
	})
	.await
}

const CHECKED_CONTAINER: &str = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
const CHECKED_CHAPTER: &str = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>c</title></head><body><p>hello there</p><script>document.title = 'x'</script><img src="a.png" onerror="alert(1)"/></body></html>"#;

fn scripted_epub(title: &str) -> Vec<u8> {
	let opf = format!(
		r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:{title}</dc:identifier><dc:title>{title}</dc:title><dc:creator>Eco, Umberto</dc:creator><dc:language>en</dc:language><dc:description>&lt;img src=x onerror=alert(1)&gt; a plain sentence</dc:description></metadata><manifest><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/></spine></package>"#
	);
	zip_bytes(&[
		("mimetype", "application/epub+zip"),
		("META-INF/container.xml", CHECKED_CONTAINER),
		("OEBPS/content.opf", &opf),
		("OEBPS/c1.xhtml", CHECKED_CHAPTER),
	])
}

fn spool_out(world: &World) -> PathBuf {
	world.config_dir.join("workers/intake/out")
}

/// a friend's book is read by the checker, never by the server: what the server shows is what the
/// checker said, and what is filed is the file the checker made
#[tokio::test(flavor = "multi_thread")]
async fn test_a_friends_book_is_read_by_the_checker_and_filed_from_what_it_made() {
	let world = sandboxed_world().await;
	let id = world.ana_uploads("El nombre de la rosa").await;
	let items = world.ana_items().await;
	let item = &items[0];
	assert_eq!(item["id"], id);
	assert!(
		matches!(item["state"].as_str(), Some("NEEDS_REVIEW" | "READY")),
		"{item:#}"
	);
	// the same card as when the server reads the book itself
	assert_eq!(item["proposal"]["metadata"]["title"], "El nombre de la rosa", "{item:#}");
	assert!(item["destination"].is_null() && item["decision"]["folder"] == "");
	// what the checker said is kept in the file: the server asks once
	let row = world.row(&id).await;
	let screening = row.files.0[0].screening.clone().expect("a verdict is kept");
	assert_eq!(screening["verdict"]["status"], "ok");
	assert!(row.files.0[0].hash.is_some(), "the hash is the checker's");

	let response = world.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": [id] })).await;
	assert_ok(&response);
	let response = world.as_owner(APPROVE, json!({ "ids": [id] })).await;
	assert_ok(&response);
	let row = world.wait_for(&id, IntakeState::Filed).await;
	assert!(row.error.is_none(), "{:?}", row.error);
	let target = world
		.library_dir
		.join("Umberto_Eco/El_nombre_de_la_rosa.epub");
	assert!(target.exists(), "filed in its author folder");
	// what the checker made for it is gone from the spool
	let leftovers: Vec<_> = fs::read_dir(spool_out(&world))
		.into_iter()
		.flatten()
		.flatten()
		.collect();
	assert!(leftovers.is_empty(), "the spool is cleaned: {leftovers:?}");
}

/// the scripts of a friend's EPUB are taken out and nothing else, and the file that is filed is a
/// canonical zip with a plain description
#[tokio::test(flavor = "multi_thread")]
async fn test_a_friends_epub_is_filed_without_its_scripts() {
	let world = sandboxed_world().await;
	let sent = scripted_epub("A Scripted Book");
	let response = post_upload(
		&world.app,
		Some(&world.vis.ana.token),
		&world.library.id,
		None,
		&UploadBody::new().file("Scripted.epub", sent.clone()),
	)
	.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	let id = response.json::<Value>()["items"][0]["id"]
		.as_str()
		.unwrap()
		.to_string();
	let items = world.ana_items().await;
	let item = &items[0];
	assert!(
		matches!(item["state"].as_str(), Some("NEEDS_REVIEW" | "READY")),
		"{item:#}"
	);
	// she and the approver are told what will be taken out
	let warnings = item["proposal"]["warnings"].to_string();
	assert!(warnings.contains("will be removed"), "{warnings}");
	// the staged file is exactly what she sent: the server did not touch it
	let row = world.row(&id).await;
	let staged = PathBuf::from(&row.staging_dir).join(&row.files.0[0].staged_name);
	assert_eq!(fs::read(&staged).unwrap(), sent);

	assert_ok(&world.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": [id] })).await);
	assert_ok(&world.as_owner(APPROVE, json!({ "ids": [id] })).await);
	let row = world.wait_for(&id, IntakeState::Filed).await;
	assert!(row.error.is_none(), "{:?}", row.error);
	let filed = files_with(&world.library_dir, "epub")
		.into_iter()
		.find(|p| p.to_string_lossy().contains("Umberto_Eco"))
		.expect("the book is filed");
	let mut zip = zip::ZipArchive::new(fs::File::open(&filed).unwrap()).unwrap();
	let mut chapter = String::new();
	std::io::Read::read_to_string(&mut zip.by_name("OEBPS/c1.xhtml").unwrap(), &mut chapter).unwrap();
	assert!(chapter.contains("<p>hello there</p>"), "{chapter}");
	assert!(!chapter.contains("script") && !chapter.contains("onerror"), "{chapter}");
	let mut opf = String::new();
	std::io::Read::read_to_string(&mut zip.by_name("OEBPS/content.opf").unwrap(), &mut opf).unwrap();
	assert!(!opf.contains("onerror"), "the description is plain text now: {opf}");
}

/// a zip bomb is refused by the checker without the server reading it, and a book that was
/// refused is not checked again every time its item is analysed
#[tokio::test(flavor = "multi_thread")]
async fn test_a_friends_zip_bomb_is_refused_and_not_checked_again() {
	let world = sandboxed_world().await;
	let mut bomb = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
	std::io::Write::flush(&mut std::io::sink()).ok();
	bomb.start_file(
		"001.png",
		zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
	)
	.unwrap();
	std::io::Write::write_all(&mut bomb, &vec![0u8; 24 * 1024 * 1024]).unwrap();
	let bytes = bomb.finish().unwrap().into_inner();
	assert!(bytes.len() < 1024 * 1024, "a few KiB of file for 24 MB of nothing");
	let response = post_upload(
		&world.app,
		Some(&world.vis.ana.token),
		&world.library.id,
		None,
		&UploadBody::new().file("bomb.cbz", bytes),
	)
	.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	let id = response.json::<Value>()["items"][0]["id"]
		.as_str()
		.unwrap()
		.to_string();
	let row = world.wait_for(&id, IntakeState::Failed).await;
	let error = row.error.clone().unwrap_or_default();
	assert!(error.contains("not accepted"), "{error}");
	let first = row.files.0[0].screening.clone().expect("the refusal is kept");
	assert_eq!(first["verdict"]["status"], "reject");
	assert_eq!(first["verdict"]["reasons"][0]["code"], "RATIO");
	// analysing it again asks nobody: the refusal stands, and is the same one
	assert_ok(&world.gql(&world.vis.ana.token, REANALYSE, json!({ "id": id })).await);
	let again = world.wait_for(&id, IntakeState::Failed).await;
	assert_eq!(again.files.0[0].screening, row.files.0[0].screening);
	assert!(fs::read_dir(world.config_dir.join("workers/intake/done"))
		.map(|d| d.count() == 0)
		.unwrap_or(true));
}

/// when the checker is not there, a friend can't upload (and is told so), and a zip of books is not
/// opened for them: books are uploaded one by one
#[tokio::test(flavor = "multi_thread")]
async fn test_a_friend_cannot_upload_when_the_checker_is_offline_or_a_pack() {
	let queued = world_with(|c| c.intake_probe_mode = "queue".to_string()).await;
	let response = post_upload(
		&queued.app,
		Some(&queued.vis.ana.token),
		&queued.library.id,
		None,
		&UploadBody::new().file("a.epub", queued.epub("A", "B")),
	)
	.await;
	assert_eq!(response.status_code(), 503, "{}", response.text());
	assert!(message(&response).contains("offline"), "{}", message(&response));
	let limits = queued.gql(&queued.vis.ana.token, LIMITS, json!({})).await;
	assert_ok(&limits);
	assert_eq!(limits["data"]["intakeUploadLimits"]["uploadsOpen"], false, "{limits:#}");
	// a checker that is beating is there
	let spool = queued.config_dir.join("workers/intake");
	fs::create_dir_all(&spool).unwrap();
	let now = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap()
		.as_secs();
	fs::write(
		spool.join("worker.json"),
		json!({"version": stump_core::intake::probe::contract::probe_revision(), "updated": now, "busy": false})
			.to_string(),
	)
	.unwrap();
	let limits = queued.gql(&queued.vis.ana.token, LIMITS, json!({})).await;
	assert_eq!(limits["data"]["intakeUploadLimits"]["uploadsOpen"], true, "{limits:#}");

	let sandboxed = sandboxed_world().await;
	let pack = zip_bytes(&[("a.epub", "x"), ("b.epub", "y")]);
	let response = post_upload(
		&sandboxed.app,
		Some(&sandboxed.vis.ana.token),
		&sandboxed.library.id,
		None,
		&UploadBody::new().file("pack.zip", pack),
	)
	.await;
	assert_eq!(response.status_code(), 415, "{}", response.text());
	assert!(message(&response).contains("books themselves"), "{}", message(&response));
	// the formats Stump has no reader for are said as what they are
	for name in ["a.cb7", "a.cbt", "a.7z"] {
		let response = post_upload(
			&sandboxed.app,
			Some(&sandboxed.vis.ana.token),
			&sandboxed.library.id,
			None,
			&UploadBody::new().file(name, b"x".to_vec()),
		)
		.await;
		assert_eq!(response.status_code(), 415, "{name}: {}", response.text());
		assert!(
			message(&response).contains(&format!("can't read .{}", &name[2..])),
			"{}",
			message(&response)
		);
	}
}

/// kills the supervisor the test started
struct Supervisor(std::process::Child);

impl Drop for Supervisor {
	fn drop(&mut self) {
		let _ = self.0.kill();
		let _ = self.0.wait();
	}
}

/// the production arrangement: the server writes tickets to the spool, a supervisor (the real
/// binary, `stump intake-probe serve`) answers them, one child process for each file
#[tokio::test(flavor = "multi_thread")]
async fn test_the_queue_is_answered_by_a_real_supervisor() {
	let world = world_with(|c| {
		c.intake_probe_mode = "queue".to_string();
		c.intake_probe_timeout_secs = 60;
	})
	.await;
	let spool = world.config_dir.join("workers/intake");
	let inbox = world.config_dir.join("intake");
	fs::create_dir_all(&spool).unwrap();
	fs::create_dir_all(&inbox).unwrap();
	// offline until it beats
	let closed = world.gql(&world.vis.ana.token, LIMITS, json!({})).await;
	assert_eq!(closed["data"]["intakeUploadLimits"]["uploadsOpen"], false, "{closed:#}");
	let _supervisor = Supervisor(
		std::process::Command::new(env!("CARGO_BIN_EXE_stump_server"))
			.args(["intake-probe", "serve", "--spool"])
			.arg(&spool)
			.arg("--intake")
			.arg(&inbox)
			.env("PROBE_MIN_FREE_MB", "1")
			.stdout(std::process::Stdio::null())
			.stderr(std::process::Stdio::null())
			.spawn()
			.expect("the supervisor starts"),
	);
	let mut open = false;
	for _ in 0..100 {
		let limits = world.gql(&world.vis.ana.token, LIMITS, json!({})).await;
		if limits["data"]["intakeUploadLimits"]["uploadsOpen"] == true {
			open = true;
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	assert!(open, "the checker should say it is there");

	let id = world.ana_uploads("El nombre de la rosa").await;
	let items = world.ana_items().await;
	assert_eq!(items[0]["proposal"]["metadata"]["title"], "El nombre de la rosa", "{items:#}");
	assert_ok(&world.gql(&world.vis.ana.token, SUBMIT, json!({ "ids": [id] })).await);
	assert_ok(&world.as_owner(APPROVE, json!({ "ids": [id] })).await);
	let row = world.wait_for(&id, IntakeState::Filed).await;
	assert!(row.error.is_none(), "{:?}", row.error);
	assert!(world
		.library_dir
		.join("Umberto_Eco/El_nombre_de_la_rosa.epub")
		.exists());
	// the spool is left clean: nothing queued, running or made
	for folder in ["queue", "running", "out"] {
		let left: Vec<_> = fs::read_dir(spool.join(folder)).unwrap().flatten().collect();
		assert!(left.is_empty(), "{folder}: {left:?}");
	}
}

/// a checker that answers with a canned verdict for any file: what the server shows is what the
/// checker said, so the bytes of the book were never read here (they are not a book at all)
fn canned_probe(dir: &std::path::Path, verdict_fields: &str) -> String {
	use std::os::unix::fs::PermissionsExt;
	let revision = stump_core::intake::probe::contract::probe_revision();
	let script = format!(
		r#"#!/bin/sh
req="$3"
id=$(sed -n 's/.*"id":"\([^"]*\)".*/\1/p' "$req")
sha=$(sed -n 's/.*"sha256":"\([0-9a-f]*\)".*/\1/p' "$req")
size=$(sed -n 's/.*"size":\([0-9]*\).*/\1/p' "$req")
printf '{{"v":1,"id":"%s","kind":"screen","probe":"{revision}","status":"ok","reasons":[],"warnings":[],"book":{{"format":"epub","sha256":"%s","size":%s,{verdict_fields}}},"usage":{{}}}}' "$id" "$sha" "$size"
"#
	);
	let path = dir.join("canned-probe.sh");
	fs::write(&path, script).unwrap();
	fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
	path.to_string_lossy().into_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_the_server_uses_the_verdict_and_not_the_file() {
	let tmp = tempfile::tempdir().unwrap();
	let probe = canned_probe(
		tmp.path(),
		r#""stump_hash":"cafe0001","pages":3,"metadata":{"title":"From The Verdict","writers":["Eco, Umberto"]}"#,
	);
	let world = world_with(move |c| {
		c.intake_probe_mode = "subprocess".to_string();
		c.intake_probe_exe = Some(probe);
	})
	.await;
	// not a book: the first bytes of a zip, and rubbish. Any reader that opened it would fail
	let mut rubbish = b"PK\x03\x04".to_vec();
	rubbish.extend_from_slice(&[0xAB; 500]);
	let response = post_upload(
		&world.app,
		Some(&world.vis.ana.token),
		&world.library.id,
		None,
		&UploadBody::new().file("Not A Book.epub", rubbish.clone()),
	)
	.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	let items = world.ana_items().await;
	let item = &items[0];
	assert!(
		matches!(item["state"].as_str(), Some("NEEDS_REVIEW" | "READY")),
		"{item:#}"
	);
	assert_eq!(item["proposal"]["metadata"]["title"], "From The Verdict", "{item:#}");
	let id = item["id"].as_str().unwrap().to_string();
	assert_eq!(world.row(&id).await.files.0[0].hash.as_deref(), Some("cafe0001"));

	// an answer about another file than the one asked about is no answer
	let lying = canned_probe(tmp.path(), r#""stump_hash":"x","pages":1"#);
	let wrong = std::fs::read_to_string(&lying)
		.unwrap()
		.replace(r#"sha=$(sed -n 's/.*"sha256":"\([0-9a-f]*\)".*/\1/p' "$req")"#, r#"sha=$(printf '%064d' 0)"#);
	fs::write(&lying, wrong).unwrap();
	let response = post_upload(
		&world.app,
		Some(&world.vis.ana.token),
		&world.library.id,
		None,
		&UploadBody::new().file("Another.epub", rubbish),
	)
	.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	let second = response.json::<Value>()["items"][0]["id"].as_str().unwrap().to_string();
	let row = world.wait_for(&second, IntakeState::Failed).await;
	assert!(
		row.error.clone().unwrap_or_default().contains("could not be used"),
		"{:?}",
		row.error
	);
}
