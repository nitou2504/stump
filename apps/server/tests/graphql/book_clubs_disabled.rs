//! Book clubs are off (package C1): their screens are gone, so their API is the server owner's
//! alone, and so are the custom emoji, which only the clubs used.
//!
//! The clubs' API was live and mostly unguarded: anybody with an account could read a club's
//! books, discussions and messages by ID, and ACCESS_BOOK_CLUB, CREATE_BOOK_CLUB or UPLOAD_FILE
//! were enough to make clubs, post and change emoji. And `readingSessionById` handed a reader's
//! session (with its bookmarks and annotations behind it) to anybody who shared a club with them.
//!
//! The table of fields below is the 36 club fields and the 4 custom emoji fields. Each call is
//! written from the schema itself (the arguments it needs, a selection that fits what it returns),
//! so the test can't be wrong about a field's shape, and a refusal can only be the guard's: every
//! call is made valid before it is made.

use std::collections::HashSet;

use models::{
	entity::{book_club, book_club_member},
	shared::{book_club::BookClubMemberRole, enums::UserPermission},
};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Value};

use crate::common::{
	account::{CreateTestUser, TestUser},
	visibility::{hide_library, VisibilityFixture},
	TestApp,
};

const FORBIDDEN: &str = "You do not have permission to perform this action";

const CLUB_QUERIES: [&str; 13] = [
	"bookClubs",
	"bookClubById",
	"bookClubBySlug",
	"bookClubBook",
	"bookClubDiscussion",
	"bookClubDiscussionByBook",
	"bookClubDiscussions",
	"bookClubDiscussionMessage",
	"bookClubDiscussionMessages",
	"previousBookClubDiscussions",
	"myBookClubInvitations",
	"bookClubSuggestions",
	"bookClubSuggestion",
];

const CLUB_MUTATIONS: [&str; 23] = [
	"createBookClub",
	"updateBookClub",
	"deleteBookClub",
	"sendMessage",
	"editMessage",
	"deleteMessage",
	"toggleReaction",
	"lockDiscussion",
	"pinMessage",
	"createDiscussion",
	"archiveDiscussion",
	"createBookClubInvitation",
	"respondToBookClubInvitation",
	"createBookClubMember",
	"removeBookClubMember",
	"leaveBookClub",
	"addBookToClub",
	"completeBook",
	"reorderBooks",
	"suggestBook",
	"removeSuggestion",
	"toggleSuggestionLike",
	"updateSuggestionStatus",
];

const EMOJI_QUERIES: [&str; 1] = ["customEmojis"];

/// `uploadCustomEmoji` takes a file, which a JSON request can't carry: `image_upload.rs` holds it
/// to the same rule with a multipart request
const EMOJI_MUTATIONS: [&str; 2] = ["deleteCustomEmoji", "updateCustomEmoji"];

/// A type, down to its name: `NON_NULL` and `LIST` wrap the type they are of
const TYPE_REF: &str =
	"kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name } } } }";

async fn schema(app: &TestApp) -> Value {
	let fields = format!(
		"fields {{ name args {{ name type {{ {TYPE_REF} }} }} type {{ {TYPE_REF} }} }}"
	);
	let query = format!(
		"{{ __schema {{ queryType {{ {fields} }} mutationType {{ {fields} }} \
		 types {{ kind name isOneOf enumValues {{ name }} inputFields {{ name type {{ {TYPE_REF} }} }} }} }} }}"
	);
	let response = app.execute_gql(&query, None).await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"]["__schema"].clone()
}

fn type_named<'a>(schema: &'a Value, name: &str) -> &'a Value {
	schema["types"]
		.as_array()
		.and_then(|types| types.iter().find(|t| t["name"] == name))
		.unwrap_or_else(|| panic!("a type called {name} in the schema"))
}

/// A literal of this type that the server accepts as a value: the one a required argument needs
fn literal(type_ref: &Value, schema: &Value) -> String {
	match type_ref["kind"].as_str() {
		Some("NON_NULL") => literal(&type_ref["ofType"], schema),
		// one item: some lists must not be empty
		Some("LIST") => format!("[{}]", literal(&type_ref["ofType"], schema)),
		Some("ENUM") => {
			let name = type_ref["name"].as_str().expect("an enum has a name");
			let first = type_named(schema, name)["enumValues"][0]["name"]
				.as_str()
				.unwrap_or_else(|| panic!("{name} has a value"));
			first.to_string()
		},
		Some("INPUT_OBJECT") => {
			let name = type_ref["name"].as_str().expect("an input has a name");
			let input = type_named(schema, name);
			let fields = input["inputFields"].as_array().expect("input fields");
			// a "oneof" input takes exactly one of its fields, none of them required: give it the first
			let required = fields
				.iter()
				.filter(|field| {
					if input["isOneOf"] == true {
						std::ptr::eq(*field, &fields[0])
					} else {
						field["type"]["kind"] == "NON_NULL"
					}
				})
				.map(|field| {
					format!(
						"{}: {}",
						field["name"].as_str().expect("a field name"),
						literal(&field["type"], schema)
					)
				})
				.collect::<Vec<_>>();
			format!("{{ {} }}", required.join(", "))
		},
		_ => match type_ref["name"].as_str() {
			Some("Int") => "1".to_string(),
			Some("Float") => "1.5".to_string(),
			Some("Boolean") => "false".to_string(),
			Some("DateTime") => "\"2024-01-01T00:00:00Z\"".to_string(),
			Some("JSON") => "{}".to_string(),
			_ => "\"x\"".to_string(),
		},
	}
}

/// The call of a root field: every required argument given a valid value, and a selection when
/// what it returns has fields
fn call(operation: &str, field: &Value, schema: &Value) -> String {
	let args = field["args"]
		.as_array()
		.expect("a field's arguments")
		.iter()
		.filter(|arg| arg["type"]["kind"] == "NON_NULL")
		.map(|arg| {
			format!(
				"{}: {}",
				arg["name"].as_str().expect("an argument name"),
				literal(&arg["type"], schema)
			)
		})
		.collect::<Vec<_>>();
	let args = if args.is_empty() {
		String::new()
	} else {
		format!("({})", args.join(", "))
	};

	let mut returned = &field["type"];
	while matches!(returned["kind"].as_str(), Some("NON_NULL" | "LIST")) {
		returned = &returned["ofType"];
	}
	let selection = match returned["kind"].as_str() {
		Some("OBJECT" | "INTERFACE" | "UNION") => " { __typename }",
		_ => "",
	};

	format!(
		"{operation} {{ {}{args}{selection} }}",
		field["name"].as_str().expect("a field name")
	)
}

fn field<'a>(schema: &'a Value, root: &str, name: &str) -> &'a Value {
	schema[root]["fields"]
		.as_array()
		.and_then(|fields| fields.iter().find(|f| f["name"] == name))
		.unwrap_or_else(|| panic!("{name} in {root}"))
}

/// Someone holding every permission that used to open the clubs and the emoji, and everything a
/// user is given to upload with
async fn club_member(app: &TestApp) -> TestUser {
	CreateTestUser {
		username: "club-member".to_string(),
		permissions: vec![
			UserPermission::AccessBookClub,
			UserPermission::CreateBookClub,
			UserPermission::UploadFile,
			UserPermission::EditLibrary,
		],
		..Default::default()
	}
	.insert(app)
	.await
}

/// Every one of the 40 fields refuses a user who holds the club permissions, and says why: the
/// guard's own answer, which only a valid call gets to
#[tokio::test]
async fn every_club_and_emoji_field_is_for_the_server_owner_only() {
	let app = TestApp::new_with_default_user().await;
	let member = club_member(&app).await;
	let schema = schema(&app).await;

	let fields: Vec<(&str, &str, &str)> = CLUB_QUERIES
		.iter()
		.chain(EMOJI_QUERIES.iter())
		.map(|name| ("queryType", "query", *name))
		.chain(
			CLUB_MUTATIONS
				.iter()
				.chain(EMOJI_MUTATIONS.iter())
				.map(|name| ("mutationType", "mutation", *name)),
		)
		.collect();
	assert_eq!(fields.len(), 13 + 1 + 23 + 2);

	let mut not_refused = Vec::new();
	for (root, operation, name) in fields {
		let query = call(operation, field(&schema, root, name), &schema);
		let response = app
			.execute_gql_with_token(&query, None, &member.token)
			.await;

		if !response["errors"].to_string().contains(FORBIDDEN) {
			not_refused.push(format!("{name}: {query} -> {response}"));
		}
		assert!(
			response["data"].is_null()
				|| response["data"][name].is_null() && response["errors"].is_array(),
			"{name} gave a user with the club permissions data: {response}"
		);
	}
	assert!(
		not_refused.is_empty(),
		"these fields let in a user who isn't the server owner:\n{}",
		not_refused.join("\n")
	);
}

/// A club or an emoji field added later, with no entry above, would be open: the ones named like
/// it are checked here, so that adding one fails this test until it is in the table (and guarded)
#[tokio::test]
async fn no_club_or_emoji_field_is_missing_from_the_table() {
	let app = TestApp::new_with_default_user().await;
	let schema = schema(&app).await;

	let known: HashSet<&str> = CLUB_QUERIES
		.iter()
		.chain(&EMOJI_QUERIES)
		.chain(&CLUB_MUTATIONS)
		.chain(&EMOJI_MUTATIONS)
		.chain(&["uploadCustomEmoji"])
		// an ordinary library setting: the icon of a library, guarded by EDIT_LIBRARY
		.chain(&["updateLibraryEmoji"])
		.copied()
		.collect();

	for root in ["queryType", "mutationType"] {
		for field in schema[root]["fields"].as_array().expect("fields") {
			let name = field["name"].as_str().expect("a field name");
			let lowered = name.to_lowercase();
			if (lowered.contains("club") || lowered.contains("emoji"))
				&& !known.contains(name)
			{
				panic!("{name} looks like a club or emoji field and is not in the table of owner-only fields");
			}
		}
	}
}

/// The owner is not locked out of what they keep: the clubs' API answers them
#[tokio::test]
async fn the_server_owner_still_reaches_the_club_and_emoji_fields() {
	let app = TestApp::new_with_default_user().await;

	let response = app.execute_gql("{ bookClubs { id } }", None).await;
	assert!(response.get("errors").is_none(), "{response}");
	assert_eq!(response["data"]["bookClubs"], json!([]));

	let response = app.execute_gql("{ customEmojis { id } }", None).await;
	assert!(response.get("errors").is_none(), "{response}");
	assert_eq!(response["data"]["customEmojis"], json!([]));

	let response = app
		.execute_gql(
			r#"mutation { createBookClub(input: { name: "Owner's club", isPrivate: true, creatorHideProgress: false }) { id name } }"#,
			None,
		)
		.await;
	assert!(
		!response["errors"].to_string().contains(FORBIDDEN),
		"the owner was refused: {response}"
	);
}

async fn read_a_page(app: &TestApp, token: &str, book_id: &str) -> i64 {
	let query = format!(
		r#"mutation {{ updateMediaProgress(id: "{book_id}", input: {{ paged: {{ page: 1 }} }}) {{ id }} }}"#
	);
	let response = app.execute_gql_with_token(&query, None, token).await;
	assert!(response.get("errors").is_none(), "{response}");
	response["data"]["updateMediaProgress"]["id"]
		.as_i64()
		.expect("the id of the reading session")
}

/// A club that shows its members' progress to each other: the old way to somebody else's session
async fn share_a_club(app: &TestApp, readers: &[&TestUser]) {
	let club = book_club::ActiveModel {
		name: Set("Reading together".to_string()),
		is_private: Set(false),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should insert the club");

	for reader in readers {
		book_club_member::ActiveModel {
			id: Set(format!("member-of-{}", reader.id)),
			hide_progress: Set(false),
			role: Set(BookClubMemberRole::Member),
			joined_at: Set(chrono::Utc::now().into()),
			user_id: Set(reader.id.clone()),
			book_club_id: Set(club.id.clone()),
			..Default::default()
		}
		.insert(app.conn())
		.await
		.expect("should insert the member");
	}
}

/// A reading session is its reader's: not another member's of a club that shows progress, not the
/// owner's, and not its own reader's once the book is hidden from them
#[tokio::test]
async fn a_reading_session_is_its_readers_alone() {
	let app = TestApp::new_with_default_user().await;
	let world = VisibilityFixture::build(&app).await;

	let anas = read_a_page(&app, &world.ana.token, &world.general.id).await;
	let bens = read_a_page(&app, &world.ben.token, &world.general.id).await;
	share_a_club(&app, &[&world.ana, &world.ben]).await;

	let by_id = |id: i64| format!("{{ readingSessionById(id: {id}) {{ id }} }}");

	// each gets their own: the question is not a wrong one
	for (token, id) in [(&world.ana.token, anas), (&world.ben.token, bens)] {
		let response = app.execute_gql_with_token(&by_id(id), None, token).await;
		assert!(response.get("errors").is_none(), "{response}");
		assert_eq!(response["data"]["readingSessionById"]["id"], id);
	}

	// nobody else gets hers: not ben, who is in a club with her that shows progress, and not the owner
	for token in [&world.ben.token, &world.owner_token] {
		let response = app.execute_gql_with_token(&by_id(anas), None, token).await;
		assert!(response.get("errors").is_none(), "{response}");
		assert!(
			response["data"]["readingSessionById"].is_null(),
			"{response}"
		);
	}
	// an ID that is no session at all answers the same, so the difference gives nothing away
	let response = app
		.execute_gql_with_token(&by_id(999_999), None, &world.ben.token)
		.await;
	assert!(
		response["data"]["readingSessionById"].is_null(),
		"{response}"
	);

	// the book goes out of her sight (its library is hidden from her): so does her session of it
	hide_library(&app, &world.ana, &world.open).await;
	let response = app
		.execute_gql_with_token(&by_id(anas), None, &world.ana.token)
		.await;
	assert!(response.get("errors").is_none(), "{response}");
	assert!(
		response["data"]["readingSessionById"].is_null(),
		"{response}"
	);

	// and ben, whose book is still in sight, keeps his
	let response = app
		.execute_gql_with_token(&by_id(bens), None, &world.ben.token)
		.await;
	assert_eq!(response["data"]["readingSessionById"]["id"], bens);
}
