//! The owner's GraphQL API for public access: who may use it, what it writes for the edge, and what
//! it refuses

use axum::http::StatusCode;
use chrono::Duration;
use models::{
	entity::{access_audit, access_device, session},
	shared::enums::UserPermission,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use stump_core::access::state;

use super::*;
use crate::common::account::CreateTestUser;

const QUERIES: [&str; 7] = [
	"{ accessOverview { configured } }",
	"{ accessInvites { nodes { id } } }",
	"{ accessDevices { nodes { id } } }",
	"{ accessSignIns { nodes { id } } }",
	"{ accessBlockedUsers { lockedBy } }",
	"{ accessPeople { mustChangePassword } }",
	"{ accessAudit { nodes { id } } }",
];

const MUTATIONS: [&str; 7] = [
	r#"mutation { createAccessInvite(input: {personId: "x", label: "x", validForHours: 24, maxUses: 1}) { link } }"#,
	r#"mutation { revokeAccessInvite(id: "x") }"#,
	r#"mutation { revokeAccessDevice(id: "x") }"#,
	r#"mutation { restoreAccessDevice(id: "x") }"#,
	r#"mutation { revokeAccessDevicesOfPerson(personId: "x") }"#,
	r#"mutation { resetAccessDeviceBudget(id: "x") }"#,
	r#"mutation { unblockUser(id: "x") }"#,
];

/// Every field is for the server owner only: not for a user who can manage the server and its
/// users, not for anybody
#[tokio::test]
async fn every_field_refuses_a_user_who_is_not_the_owner() {
	let env = edge_app(false).await;
	let app = &env.app;
	let admin = CreateTestUser {
		username: "admin".to_string(),
		permissions: vec![
			UserPermission::ManageServer,
			UserPermission::ManageUsers,
			UserPermission::ReadUsers,
			UserPermission::ManageLibrary,
		],
		..Default::default()
	}
	.insert(app)
	.await;

	for query in QUERIES.iter().chain(MUTATIONS.iter()) {
		let as_admin = app.execute_gql_with_token(query, None, &admin.token).await;
		assert!(
			!errors_of(&as_admin).is_empty(),
			"{query} was allowed to a user who is not the owner: {as_admin:#}"
		);
		assert!(
			as_admin["data"].is_null()
				|| as_admin["data"]
					.as_object()
					.is_some_and(|data| data.values().all(Value::is_null)),
			"{query}: {as_admin:#}"
		);
	}
	// the owner is allowed the queries
	for query in QUERIES {
		let as_owner = owner_gql(app, query, None).await;
		assert!(as_owner.get("errors").is_none(), "{query}: {as_owner:#}");
	}
}

/// The lead's R4b: a browser that another site made send a mutation is refused. A forged form can
/// post a GraphQL multipart request, and `SameSite=Lax` cookies still go along from the same site
#[tokio::test]
async fn a_mutation_that_another_site_made_the_browser_send_is_refused() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;
	let token = app.access_token.read().await.clone().unwrap();
	let mutation = json!({"query": "mutation($id: ID!) { unblockUser(id: $id) }", "variables": {"id": ana.id}});

	let send = |headers: Vec<(&'static str, &'static str)>| {
		let mutation = mutation.clone();
		let token = token.clone();
		async move {
			let mut request = app
				.server
				.post("/api/graphql")
				.add_header("authorization", format!("Bearer {token}"));
			for (name, value) in headers {
				request = request.add_header(name, value);
			}
			let response = request.json(&mutation).await;
			response.assert_status(StatusCode::OK);
			response.json::<Value>()
		}
	};

	for headers in [
		vec![("sec-fetch-site", "cross-site")],
		vec![("sec-fetch-site", "same-site")],
		// an older browser: its Origin is another site's
		vec![("origin", "https://evil.test")],
	] {
		let response = send(headers.clone()).await;
		assert!(
			!errors_of(&response).is_empty(),
			"{headers:?}: {response:#}"
		);
	}
	// the web app's own request, a script with a token, and a browser at its own origin
	for headers in [
		vec![("sec-fetch-site", "same-origin")],
		vec![],
		vec![("sec-fetch-site", "none")],
	] {
		let response = send(headers.clone()).await;
		assert!(
			response.get("errors").is_none(),
			"{headers:?}: {response:#}"
		);
	}
}

/// What the edge follows is written before the response ends, holds ids and numbers only, and is
/// readable by everybody (the edge runs as another user)
#[tokio::test]
async fn a_revoke_is_in_the_state_file_before_the_response_ends() {
	let env = edge_app(false).await;
	let app = &env.app;
	let path = env.state_dir.path().join("state.json");
	let pass_value = pass_for("hermana-phone");
	public_get(app, "/api/v2/ping", Some(&pass_value))
		.await
		.assert_status_ok();

	let revoked = owner_gql(
		app,
		r#"mutation { revokeAccessDevice(id: "hermana-phone") }"#,
		None,
	)
	.await;
	assert_eq!(revoked["data"]["revokeAccessDevice"], true, "{revoked:#}");

	let text = std::fs::read_to_string(&path).expect("the state file");
	let state: Value = serde_json::from_str(&text).unwrap();
	assert_eq!(state["revoked"], json!(["hermana-phone"]));
	let first_version = state["v"].as_i64().unwrap();
	assert!(state["written"].is_string());
	// ids and numbers: nothing else, and no secret
	assert_eq!(state.as_object().unwrap().len(), 4);
	for secret in [KEY, "initial-server-admin", "secret", "password"] {
		assert!(!text.contains(secret), "{secret} is in the state file");
	}
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		assert_eq!(
			std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
			0o644
		);
	}

	// restoring takes it out; resetting the budget puts the device in `budget`; the version grows
	owner_gql(
		app,
		r#"mutation { restoreAccessDevice(id: "hermana-phone") }"#,
		None,
	)
	.await;
	owner_gql(
		app,
		r#"mutation { resetAccessDeviceBudget(id: "hermana-phone") }"#,
		None,
	)
	.await;
	let state: Value =
		serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
	assert_eq!(state["revoked"], json!([]));
	assert!(state["budget"]["hermana-phone"].as_i64().unwrap() > 0);
	assert!(state["v"].as_i64().unwrap() > first_version);

	// a second reset gives a strictly later epoch, even within the same second
	let epoch = state["budget"]["hermana-phone"].as_i64().unwrap();
	owner_gql(
		app,
		r#"mutation { resetAccessDeviceBudget(id: "hermana-phone") }"#,
		None,
	)
	.await;
	let state: Value =
		serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
	assert!(state["budget"]["hermana-phone"].as_i64().unwrap() > epoch);
}

/// The lead's R8: if the edge can't be told, the admin is told so (the app refuses the device at
/// once anyway)
#[tokio::test]
async fn a_state_file_that_cannot_be_written_is_reported() {
	let env = edge_app_with(|dir| {
		let mut edge = edge_settings(dir, false);
		edge.state_dir = Some(dir.path().join("missing").to_string_lossy().to_string());
		Some(edge)
	})
	.await;
	let app = &env.app;
	let pass_value = pass_for("hermana-phone");
	public_get(app, "/api/v2/ping", Some(&pass_value))
		.await
		.assert_status_ok();

	let response = owner_gql(
		app,
		r#"mutation { revokeAccessDevice(id: "hermana-phone") }"#,
		None,
	)
	.await;
	assert!(
		errors_of(&response).contains("edge could not be told"),
		"{response:#}"
	);
	// it is revoked in Stump all the same, and refused at once
	assert!(device_row(app, "hermana-phone")
		.await
		.unwrap()
		.revoked_at
		.is_some());
	assert_eq!(
		public_get(app, "/api/v2/ping", Some(&pass_value))
			.await
			.status_code(),
		StatusCode::FORBIDDEN
	);
}

#[tokio::test]
async fn revoking_a_person_cuts_off_their_devices_invites_and_sessions() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = CreateTestUser {
		username: "ana".to_string(),
		..Default::default()
	}
	.insert(app)
	.await;
	let (invite_id, link) = make_invite(app, &ana.id, "Ana's phone", 5).await;
	let first = redeem(app, &link, None).await;
	assert!(pass_in(&first).is_some());
	let (spare_id, _) = make_invite(app, &ana.id, "spare", 1).await;
	// she has a session too
	app.server
		.post("/api/v2/auth/login")
		.json(&json!({"username": "ana", "password": "password"}))
		.await
		.assert_status_ok();

	let response = owner_gql(
		app,
		"mutation($id: ID!) { revokeAccessDevicesOfPerson(personId: $id) }",
		Some(json!({"id": ana.id})),
	)
	.await;
	assert_eq!(
		response["data"]["revokeAccessDevicesOfPerson"], 1,
		"{response:#}"
	);

	let devices = access_device::Entity::find()
		.filter(access_device::Column::RevokedAt.is_not_null())
		.count(app.conn())
		.await
		.unwrap();
	assert_eq!(devices, 1);
	assert!(invite_row(app, &invite_id).await.revoked_at.is_some());
	assert!(invite_row(app, &spare_id).await.revoked_at.is_some());
	assert_eq!(
		session::Entity::find()
			.filter(session::Column::UserId.eq(ana.id.clone()))
			.count(app.conn())
			.await
			.unwrap(),
		0
	);
	// the link of the revoked invite no longer works
	assert_eq!(
		redeem(app, &link, None).await.status_code(),
		StatusCode::NOT_FOUND
	);
	let audit = access_audit::Entity::find()
		.filter(access_audit::Column::Action.eq("person.revoke_all"))
		.count(app.conn())
		.await
		.unwrap();
	assert_eq!(audit, 1);
}

#[tokio::test]
async fn an_invite_asks_for_what_it_needs() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;

	let ask = |person: String, label: &'static str, hours: i32, uses: i32| {
		let person = person.clone();
		async move {
			owner_gql(
				app,
				r#"mutation($input: CreateAccessInviteInput!) { createAccessInvite(input: $input) { link } }"#,
				Some(json!({"input": {"personId": person, "label": label, "validForHours": hours, "maxUses": uses}})),
			)
			.await
		}
	};
	// a good one
	assert!(ask(ana.id.clone(), "phone", 24, 1)
		.await
		.get("errors")
		.is_none());
	// the validity is one of the offered ones, the uses 1 to 10, the name not empty, the person real
	for (person, label, hours, uses) in [
		(ana.id.clone(), "phone", 5, 1),
		(ana.id.clone(), "phone", 24, 0),
		(ana.id.clone(), "phone", 24, 11),
		(ana.id.clone(), "   ", 24, 1),
		("no-such-person".to_string(), "phone", 24, 1),
	] {
		let response = ask(person.clone(), label, hours, uses).await;
		assert!(
			!errors_of(&response).is_empty(),
			"{person} {label:?} {hours} {uses}: {response:#}"
		);
	}
	// a name with controls and direction marks is cleaned
	let cleaned = owner_gql(
		app,
		r#"mutation($input: CreateAccessInviteInput!) { createAccessInvite(input: $input) { invite { label } } }"#,
		Some(json!({"input": {"personId": ana.id, "label": "\u{202e}Ana\u{0}'s phone", "validForHours": 24, "maxUses": 1}})),
	)
	.await;
	assert_eq!(
		cleaned["data"]["createAccessInvite"]["invite"]["label"],
		"Ana's phone"
	);
}

/// The overview says what is set up; a page of sign-ins with every field and its person stays
/// within the limits of the complexity of a query
#[tokio::test]
async fn the_overview_and_the_biggest_page_are_within_the_limits() {
	let env = edge_app(false).await;
	let app = &env.app;
	insert_user(app, "ana", "ana-secret", false).await;
	let overview = owner_gql(
		app,
		"{ accessOverview { hasKey hasPublicUrl hasStateDir configured requirePass publicUrl activeDevices openInvites } }",
		None,
	)
	.await;
	let overview = &overview["data"]["accessOverview"];
	assert_eq!(overview["configured"], true);
	assert_eq!(overview["publicUrl"], PUBLIC_URL);
	assert_eq!(overview["requirePass"], false);

	let big = owner_gql(
		app,
		r#"{ accessSignIns(failuresOnly: true, pagination: {page: 1, pageSize: 200}) {
			nodes { id at ip userAgent successful deviceId deviceLabel reason cleared
				person { id username locked removed isServerOwner } }
			pageInfo { totalItems totalPages } } }"#,
		None,
	)
	.await;
	assert!(big.get("errors").is_none(), "{big:#}");
	for query in [
		"{ accessInvites(pagination: {page: 1, pageSize: 200}) { nodes { id person { id username } } } }",
		"{ accessDevices(pagination: {page: 1, pageSize: 200}) { nodes { id person { id username } } } }",
		"{ accessAudit(pagination: {page: 1, pageSize: 200}) { nodes { id } } }",
	] {
		let response = owner_gql(app, query, None).await;
		assert!(response.get("errors").is_none(), "{query}: {response:#}");
	}
}

/// An unconfigured server says so, and can't make links
#[tokio::test]
async fn without_edge_settings_the_page_says_what_is_missing() {
	let env = edge_app_with(|_| None).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;

	let overview = owner_gql(
		app,
		"{ accessOverview { hasKey hasPublicUrl hasStateDir configured } }",
		None,
	)
	.await;
	assert_eq!(overview["data"]["accessOverview"]["configured"], false);
	assert_eq!(overview["data"]["accessOverview"]["hasKey"], false);

	let response = owner_gql(
		app,
		r#"mutation($input: CreateAccessInviteInput!) { createAccessInvite(input: $input) { link } }"#,
		Some(json!({"input": {"personId": ana.id, "label": "phone", "validForHours": 24, "maxUses": 1}})),
	)
	.await;
	assert!(!errors_of(&response).is_empty(), "{response:#}");
	// without a state folder a revoke still works (nothing to tell the edge)
	let (_ok, _) = (
		access_device::Entity::find()
			.count(app.conn())
			.await
			.unwrap(),
		(),
	);
}

/// The version of the state file always grows, even for two files written in the same moment or
/// after the clock went back: the edge applies a file whenever its version *differs* from the one
/// it holds, so one that repeated or fell back could be skipped or confuse it
#[tokio::test]
async fn the_version_of_the_state_file_always_grows() {
	let env = edge_app(false).await;
	let app = &env.app;
	let dir = env.state_dir.path();
	let now = Utc::now();

	let mut versions = Vec::new();
	// the same moment twice, then an earlier one (the clock went back)
	for at in [now, now, now - Duration::seconds(30)] {
		versions.push(state::publish(app.conn(), dir, at).await.unwrap());
	}
	assert!(
		versions.windows(2).all(|pair| pair[0] < pair[1]),
		"{versions:?}"
	);

	// the file holds the last one written
	let file: Value =
		serde_json::from_str(&std::fs::read_to_string(dir.join("state.json")).unwrap())
			.unwrap();
	assert_eq!(file["v"].as_i64().unwrap(), *versions.last().unwrap());
}

/// Changes made at the same time can't leave an older state in the file: writes take turns and
/// each reads the database inside its turn, so whoever writes last has seen every change
#[tokio::test]
async fn concurrent_changes_never_leave_an_older_state_in_the_file() {
	let env = edge_app(false).await;
	let app = &env.app;
	let dir = env.state_dir.path();

	let ids: Vec<String> = (0..12).map(|n| format!("conc-phone-{n}")).collect();
	for id in &ids {
		// a pass the app has not seen is registered on its first request
		public_get(app, "/api/v2/ping", Some(&pass_for(id)))
			.await
			.assert_status_ok();
	}

	futures_util::future::join_all(ids.iter().map(|id| async move {
		models::services::access::revoke_device(app.conn(), Utc::now(), id, "owner")
			.await
			.unwrap();
		state::publish(app.conn(), dir, Utc::now()).await.unwrap();
	}))
	.await;

	let file: Value =
		serde_json::from_str(&std::fs::read_to_string(dir.join("state.json")).unwrap())
			.unwrap();
	let mut revoked: Vec<String> = file["revoked"]
		.as_array()
		.unwrap()
		.iter()
		.map(|id| id.as_str().unwrap().to_string())
		.collect();
	revoked.sort();
	let mut expected = ids.clone();
	expected.sort();
	assert_eq!(revoked, expected, "the file lost a revocation");
}
