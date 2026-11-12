//! What the owner should look at: each warning appears when it should, fades when it should, and
//! the badge counts them

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use models::services::{
	access::{self as service, device_kind, NewDevice},
	access_warnings::{self, WarningKind},
};
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};

use super::*;

async fn kinds_of(app: &TestApp) -> Vec<(WarningKind, Option<String>)> {
	access_warnings::warnings(app.conn(), Utc::now())
		.await
		.unwrap()
		.into_iter()
		.map(|warning| (warning.kind, warning.person_id))
		.collect()
}

/// Devices up to `n`, made the way sign-in makes them, with a limit that never refuses
async fn give_devices(app: &TestApp, user_id: &str, n: usize) {
	for index in 0..n {
		let id = service::unused_device_id(app.conn(), &format!("phone{index}"))
			.await
			.unwrap();
		service::insert_device_within_limit(
			app.conn(),
			Utc::now(),
			NewDevice {
				id,
				invite_id: None,
				person_id: user_id,
				label: "phone",
				kind: device_kind::BROWSER,
				ip: "198.51.100.1",
				agent: "test",
			},
			10,
		)
		.await
		.unwrap()
		.expect("room for a device");
	}
}

async fn friend_with_row(app: &TestApp, name: &str) -> user::Model {
	let user = insert_user(app, name, "secret-pass-1", false).await;
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { publicSignin } }",
		Some(json!({"i": {"personId": user.id, "publicSignin": true}})),
	)
	.await;
	user
}

#[tokio::test]
async fn the_device_warnings_follow_the_limit() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_with_row(app, "ana").await;
	assert!(kinds_of(app).await.is_empty());

	give_devices(app, &ana.id, 2).await;
	assert!(
		kinds_of(app).await.is_empty(),
		"two of three is nothing to say"
	);
	give_devices(app, &ana.id, 1).await;
	assert_eq!(
		kinds_of(app).await,
		vec![(WarningKind::AtDeviceLimit, Some(ana.id.clone()))]
	);
	give_devices(app, &ana.id, 1).await;
	assert_eq!(
		kinds_of(app).await,
		vec![(WarningKind::OverDeviceLimit, Some(ana.id.clone()))]
	);

	// a limit of 6 and four devices: four or more is worth a look whatever the limit
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { maxDevices } }",
		Some(json!({"i": {"personId": ana.id, "maxDevices": 6}})),
	)
	.await;
	assert_eq!(
		kinds_of(app).await,
		vec![(WarningKind::ManyDevices, Some(ana.id.clone()))]
	);
	// and a device that was not seen for 60 days no longer counts
	use sea_orm::IntoActiveModel;
	for device in devices_of(app, &ana.id).await {
		let mut row = device.into_active_model();
		row.last_seen_at = Set(Some(Utc::now() - Duration::days(61)));
		row.update(app.conn()).await.unwrap();
	}
	// the person has nothing to say about their devices any more; the old devices that still work
	// are a leftover worth a look, and that is one warning of its own
	assert_eq!(
		kinds_of(app).await,
		vec![(WarningKind::LeftoverDevices, None)]
	);
}

#[tokio::test]
async fn many_sessions_and_bursts_of_failures_are_flagged() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_with_row(app, "ana").await;

	// four sign-ins that were not signed out of: four sessions
	for _ in 0..4 {
		let response = app
			.server
			.post("/api/v2/auth/login")
			.json(&json!({"username": "ana", "password": "secret-pass-1"}))
			.await;
		assert_eq!(response.status_code(), StatusCode::OK);
	}
	assert_eq!(sessions_of(app, &ana.id).await, 4);
	assert!(kinds_of(app)
		.await
		.contains(&(WarningKind::ManySessions, Some(ana.id.clone()))));

	// ten failed sign-ins in the last hour
	for _ in 0..9 {
		models::entity::user_login_activity::ActiveModel {
			user_id: Set(ana.id.clone()),
			ip_address: Set("198.51.100.1".into()),
			user_agent: Set("test".into()),
			timestamp: Set(Utc::now().into()),
			authentication_successful: Set(false),
			..Default::default()
		}
		.insert(app.conn())
		.await
		.unwrap();
	}
	assert!(!kinds_of(app)
		.await
		.contains(&(WarningKind::SigninFailures, Some(ana.id.clone()))));
	models::entity::user_login_activity::ActiveModel {
		user_id: Set(ana.id.clone()),
		ip_address: Set("198.51.100.1".into()),
		user_agent: Set("test".into()),
		timestamp: Set(Utc::now().into()),
		authentication_successful: Set(false),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.unwrap();
	assert!(kinds_of(app)
		.await
		.contains(&(WarningKind::SigninFailures, Some(ana.id.clone()))));
}

#[tokio::test]
async fn events_fade_after_three_days_and_the_unknown_names_are_summed() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_with_row(app, "ana").await;
	let now = Utc::now();

	// old events are forgotten; recent ones are kept
	service::audit(
		app.conn(),
		now - Duration::days(4),
		"public",
		"signin.device_limit",
		&ana.id,
		"old",
	)
	.await
	.unwrap();
	assert!(kinds_of(app).await.is_empty());
	service::audit(
		app.conn(),
		now - Duration::days(2),
		"public",
		"signin.device_limit",
		&ana.id,
		"recent",
	)
	.await
	.unwrap();
	service::audit(
		app.conn(),
		now - Duration::hours(1),
		"public",
		"signin.device_limit",
		&ana.id,
		"again",
	)
	.await
	.unwrap();
	let found = access_warnings::warnings(app.conn(), now).await.unwrap();
	assert_eq!(found.len(), 1);
	assert_eq!(found[0].kind, WarningKind::DeviceLimitHit);
	assert_eq!(found[0].count, 2);

	// a name that was slowed down: the person if the name exists, "an unknown name" if not
	service::audit(
		app.conn(),
		now,
		"public",
		"signin.backoff",
		&ana.id,
		"slowed",
	)
	.await
	.unwrap();
	service::audit(
		app.conn(),
		now,
		"public",
		"signin.backoff",
		"unknown:ab12cd34",
		"slowed",
	)
	.await
	.unwrap();
	// failures on names that do not exist are summed from the aggregated rows
	service::audit(
		app.conn(),
		now,
		"public",
		"signin.unknown_names",
		"unknown",
		"12 failures on names that do not exist",
	)
	.await
	.unwrap();
	service::audit(
		app.conn(),
		now,
		"public",
		"signin.unknown_names",
		"unknown",
		"30 failures on names that do not exist",
	)
	.await
	.unwrap();
	let found = access_warnings::warnings(app.conn(), now).await.unwrap();
	let unknown = found
		.iter()
		.find(|w| w.kind == WarningKind::SigninFailures && w.person_id.is_none())
		.expect("a warning for names that do not exist");
	assert_eq!(unknown.count, 42);
	// a slow-down of a name that does not exist is not an item of its own: it is said in the one
	// about names that do not exist
	assert_eq!(unknown.detail.as_deref(), Some("slowed"));
	assert!(found.iter().any(|w| w.kind == WarningKind::NameBackoff
		&& w.person_id.as_deref() == Some(ana.id.as_str())));
	assert!(!found
		.iter()
		.any(|w| w.kind == WarningKind::NameBackoff && w.person_id.is_none()));
	assert_eq!(
		found
			.iter()
			.filter(|w| w.kind == WarningKind::SigninFailures && w.person_id.is_none())
			.count(),
		1
	);

	// the global budget is an alert and comes first
	service::audit(
		app.conn(),
		now,
		"public",
		"signin.global_budget",
		"all",
		"flood",
	)
	.await
	.unwrap();
	let found = access_warnings::warnings(app.conn(), now).await.unwrap();
	assert_eq!(found[0].kind, WarningKind::GlobalBudget);
	assert_eq!(found[0].severity, access_warnings::Severity::Alert);
}

#[tokio::test]
async fn links_near_their_budget_and_links_nobody_can_use_are_flagged() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;

	// a single link is always "one use left": not worth a warning
	make_signup_link(app, "SINGLE", 1, &[&shelves.biblioteca.id]).await;
	assert!(kinds_of(app).await.is_empty());

	// an open link of three with two used has one left
	let (open_id, url) =
		make_signup_link(app, "OPEN", 3, &[&shelves.biblioteca.id]).await;
	for name in ["ana", "ben"] {
		join(
			app,
			token_of(&url),
			name,
			"tulip-Harbor-47",
			"tulip-Harbor-47",
			None,
			"203.0.113.5",
		)
		.await;
	}
	let found = access_warnings::warnings(app.conn(), Utc::now())
		.await
		.unwrap();
	let near = found
		.iter()
		.find(|w| w.kind == WarningKind::LinkNearBudget)
		.expect("a link near its budget");
	assert_eq!(near.link_id.as_deref(), Some(open_id.as_str()));
	assert_eq!(near.count, 1);

	// a link whose libraries are gone can't be used
	let (_, _) = make_signup_link(app, "OPEN", 5, &[&shelves.manga.id]).await;
	library::Entity::delete_by_id(shelves.manga.id.clone())
		.exec(app.conn())
		.await
		.unwrap();
	let found = access_warnings::warnings(app.conn(), Utc::now())
		.await
		.unwrap();
	assert!(found.iter().any(|w| w.kind == WarningKind::LinkNoLibraries));

	// used up: the warning about the budget goes away with the link
	join(
		app,
		token_of(&url),
		"cara",
		"tulip-Harbor-47",
		"tulip-Harbor-47",
		None,
		"203.0.113.5",
	)
	.await;
	let found = access_warnings::warnings(app.conn(), Utc::now())
		.await
		.unwrap();
	assert!(!found.iter().any(|w| w.kind == WarningKind::LinkNearBudget));
}

/// The owner reads them at home, and nobody else does
#[tokio::test]
async fn the_warnings_are_for_the_owner_at_home() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_with_row(app, "ana").await;
	give_devices(app, &ana.id, 3).await;

	let all = owner_gql(app, "{ accessWarnings { id kind severity count since detail person { username } } accessWarningCount }", None).await;
	assert!(all.get("errors").is_none(), "{all:#}");
	let list = all["data"]["accessWarnings"].as_array().unwrap();
	assert_eq!(list.len(), 1);
	assert_eq!(list[0]["kind"], "AT_DEVICE_LIMIT");
	assert_eq!(list[0]["severity"], "WARN");
	assert_eq!(list[0]["person"]["username"], "ana");
	assert_eq!(list[0]["count"], 3);
	assert_eq!(all["data"]["accessWarningCount"], 1);
	// the same warning has the same id every time
	let again = owner_gql(app, "{ accessWarnings { id } }", None).await;
	assert_eq!(again["data"]["accessWarnings"][0]["id"], list[0]["id"]);

	// a friend, at home or through the edge, is refused
	let token = app.login_as("ana", "secret-pass-1").await;
	for query in ["{ accessWarnings { kind } }", "{ accessWarningCount }"] {
		let refused = app.execute_gql_with_token(query, None, &token).await;
		assert!(refused.get("errors").is_some(), "{query}: {refused:#}");
	}
	let owner_token = app.access_token.read().await.clone().unwrap();
	let via_edge = app
		.server
		.post("/api/graphql")
		.add_header("x-edge-public", "1")
		.add_header("x-edge-pass", pass_for("deploy-old"))
		.add_header("authorization", format!("Bearer {owner_token}"))
		.json(&json!({"query": "{ accessWarningCount }"}))
		.await;
	assert_eq!(via_edge.status_code(), StatusCode::UNAUTHORIZED);
}

// ---- marking warnings as seen -------------------------------------------------------------

const DISMISS: &str = "mutation($ids: [ID!]) { dismissAccessWarnings(ids: $ids) }";
const RESTORE: &str = "mutation($ids: [ID!]!) { restoreAccessWarnings(ids: $ids) }";

async fn visible_ids(app: &TestApp) -> Vec<String> {
	let list = owner_gql(app, "{ accessWarnings { id } }", None).await;
	list["data"]["accessWarnings"]
		.as_array()
		.unwrap()
		.iter()
		.map(|warning| warning["id"].as_str().unwrap().to_string())
		.collect()
}

async fn count_of(app: &TestApp) -> i64 {
	owner_gql(app, "{ accessWarningCount }", None).await["data"]["accessWarningCount"]
		.as_i64()
		.unwrap()
}

async fn audit_actions(app: &TestApp) -> Vec<String> {
	let page = owner_gql(app, "{ accessAudit { nodes { action target detail } } }", None).await;
	page["data"]["accessAudit"]["nodes"]
		.as_array()
		.unwrap()
		.iter()
		.map(|row| row["action"].as_str().unwrap().to_string())
		.collect()
}

#[tokio::test]
async fn dismissing_hides_a_warning_from_the_list_and_the_count_and_restoring_shows_it() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_with_row(app, "ana").await;
	give_devices(app, &ana.id, 3).await;
	let ids = visible_ids(app).await;
	assert_eq!(ids, vec![format!("AtDeviceLimit:{}", ana.id)]);
	assert_eq!(count_of(app).await, 1);

	// an id that is not a warning is ignored; the real one is marked
	let done = owner_gql(
		app,
		DISMISS,
		Some(json!({"ids": ["Nonsense:-", ids[0]]})),
	)
	.await;
	assert_eq!(done["data"]["dismissAccessWarnings"], 1, "{done:#}");
	assert!(visible_ids(app).await.is_empty());
	assert_eq!(count_of(app).await, 0);
	// it is still there for whoever asks for it, with when it was marked
	let all = owner_gql(
		app,
		"{ accessWarnings(includeDismissed: true) { id dismissedAt lastAt } }",
		None,
	)
	.await;
	let list = all["data"]["accessWarnings"].as_array().unwrap();
	assert_eq!(list.len(), 1);
	assert!(list[0]["dismissedAt"].is_string());
	assert!(list[0]["lastAt"].is_string());
	// nothing is deleted: the device rows are where they were
	assert_eq!(devices_of(app, &ana.id).await.len(), 3);
	// the audit log says so, with the id (one id) and the count
	assert!(audit_actions(app).await.contains(&"warning.dismiss".to_string()));

	// Undo shows it again
	let back = owner_gql(app, RESTORE, Some(json!({"ids": [ids[0]]}))).await;
	assert_eq!(back["data"]["restoreAccessWarnings"], 1, "{back:#}");
	assert_eq!(visible_ids(app).await, ids);
	assert_eq!(count_of(app).await, 1);
	assert!(audit_actions(app).await.contains(&"warning.restore".to_string()));

	// nothing to restore is not an error, and writes nothing
	let none = owner_gql(app, RESTORE, Some(json!({"ids": ["Nonsense:-"]}))).await;
	assert_eq!(none["data"]["restoreAccessWarnings"], 0);
	let before = audit_actions(app).await.len();
	let again = owner_gql(app, RESTORE, Some(json!({"ids": ["Nonsense:-"]}))).await;
	assert_eq!(again["data"]["restoreAccessWarnings"], 0);
	assert_eq!(audit_actions(app).await.len(), before);
}

#[tokio::test]
async fn clear_all_marks_every_visible_warning_and_a_new_event_brings_one_back() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_with_row(app, "ana").await;
	let ben = friend_with_row(app, "ben").await;
	give_devices(app, &ana.id, 3).await;
	let now = Utc::now();
	service::audit(
		app.conn(),
		now - Duration::hours(2),
		"public",
		"signin.device_limit",
		&ben.id,
		"refused",
	)
	.await
	.unwrap();
	assert_eq!(count_of(app).await, 2);

	let cleared = owner_gql(app, "mutation { dismissAccessWarnings }", None).await;
	assert_eq!(cleared["data"]["dismissAccessWarnings"], 2, "{cleared:#}");
	assert_eq!(count_of(app).await, 0);
	assert!(visible_ids(app).await.is_empty());
	// one row in the audit log for the call
	let actions = audit_actions(app).await;
	assert_eq!(
		actions.iter().filter(|a| a.as_str() == "warning.dismiss").count(),
		1
	);
	// clearing nothing writes nothing
	let nothing = owner_gql(app, "mutation { dismissAccessWarnings }", None).await;
	assert_eq!(nothing["data"]["dismissAccessWarnings"], 0);
	assert_eq!(audit_actions(app).await.len(), actions.len());

	// a new refusal at the limit brings that warning back, and only that one
	service::audit(
		app.conn(),
		Utc::now(),
		"public",
		"signin.device_limit",
		&ben.id,
		"refused again",
	)
	.await
	.unwrap();
	assert_eq!(visible_ids(app).await, vec![format!("DeviceLimitHit:{}", ben.id)]);
	assert_eq!(count_of(app).await, 1);
}

#[tokio::test]
async fn a_worse_state_shows_a_dismissed_warning_but_a_fading_event_does_not() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_with_row(app, "ana").await;
	give_devices(app, &ana.id, 3).await;
	owner_gql(app, "mutation { dismissAccessWarnings }", None).await;
	assert_eq!(count_of(app).await, 0);

	// one more device is worse: the same person is now over the limit
	give_devices(app, &ana.id, 1).await;
	assert_eq!(
		visible_ids(app).await,
		vec![format!("OverDeviceLimit:{}", ana.id)]
	);

	// an event that fades out of the window (its count goes down) stays hidden: two refusals
	// were seen, one of them is more than three days old tomorrow
	let now = Utc::now();
	for hours in [70, 20] {
		service::audit(
			app.conn(),
			now - Duration::hours(hours),
			"public",
			"signin.device_limit",
			&ana.id,
			"refused",
		)
		.await
		.unwrap();
	}
	owner_gql(app, "mutation { dismissAccessWarnings }", None).await;
	assert_eq!(count_of(app).await, 0);
	let later = now + Duration::days(1);
	let all = access_warnings::warnings(app.conn(), later).await.unwrap();
	let hit = all
		.iter()
		.find(|w| w.kind == WarningKind::DeviceLimitHit)
		.expect("the newer refusal is still inside the window");
	assert_eq!(hit.count, 1, "the older one fell out of it");
	assert!(
		hit.dismissed_at.is_some(),
		"fewer events is not something new: it stays hidden"
	);
}

#[tokio::test]
async fn the_warnings_of_a_person_who_is_gone_are_not_listed() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_with_row(app, "ana").await;
	service::audit(
		app.conn(),
		Utc::now(),
		"public",
		"signin.device_limit",
		&ana.id,
		"refused",
	)
	.await
	.unwrap();
	service::audit(
		app.conn(),
		Utc::now(),
		"public",
		"signin.device_limit",
		"a-person-that-was-removed",
		"refused",
	)
	.await
	.unwrap();
	assert_eq!(
		kinds_of(app).await,
		vec![(WarningKind::DeviceLimitHit, Some(ana.id.clone()))]
	);
}

#[tokio::test]
async fn locked_accounts_leftover_devices_and_books_waiting_are_warnings() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_with_row(app, "ana").await;
	assert!(kinds_of(app).await.is_empty());

	// a locked account
	app.conn()
		.execute_unprepared(&format!("UPDATE users SET is_locked = 1 WHERE id = '{}'", ana.id))
		.await
		.unwrap();
	assert_eq!(
		kinds_of(app).await,
		vec![(WarningKind::AccountLocked, Some(ana.id.clone()))]
	);
	app.conn()
		.execute_unprepared(&format!("UPDATE users SET is_locked = 0 WHERE id = '{}'", ana.id))
		.await
		.unwrap();

	// a pass made in the terminal still works: a leftover; a revoked one is not
	app.conn()
		.execute_unprepared(
			"INSERT INTO access_devices (id, label, created_at, kind) VALUES ('old-pass-k3f9x', 'old', CURRENT_TIMESTAMP, 'browser')",
		)
		.await
		.unwrap();
	app.conn()
		.execute_unprepared(
			"INSERT INTO access_devices (id, label, created_at, revoked_at, kind) VALUES ('gone-pass-k3f9x', 'gone', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 'browser')",
		)
		.await
		.unwrap();
	let found = access_warnings::warnings(app.conn(), Utc::now()).await.unwrap();
	let leftovers = found
		.iter()
		.find(|w| w.kind == WarningKind::LeftoverDevices)
		.expect("a leftover device");
	assert_eq!(leftovers.count, 1);
	assert!(leftovers.person_id.is_none());

	// books that wait for approval: one item, with how many
	let shelves = shelves(app).await;
	for id in ["b1", "b2"] {
		app.conn()
			.execute_unprepared(&format!(
				"INSERT INTO intake_items (id, user_id, library_id, state, kind, name, staging_dir, files, created_at, updated_at, submitted_at)
				 VALUES ('{id}', '{}', '{}', 'PENDING_APPROVAL', 'BOOK', 'a book', '/tmp/x', '[]', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
				ana.id, shelves.biblioteca.id
			))
			.await
			.unwrap();
	}
	let found = access_warnings::warnings(app.conn(), Utc::now()).await.unwrap();
	let waiting = found
		.iter()
		.find(|w| w.kind == WarningKind::BooksWaiting)
		.expect("books waiting");
	assert_eq!(waiting.count, 2);
	assert_eq!(count_of(app).await, 2);
	// they can be marked as seen like the rest, and one more book brings the item back
	owner_gql(app, "mutation { dismissAccessWarnings }", None).await;
	assert_eq!(count_of(app).await, 0);
	app.conn()
		.execute_unprepared(&format!(
			"INSERT INTO intake_items (id, user_id, library_id, state, kind, name, staging_dir, files, created_at, updated_at, submitted_at)
			 VALUES ('b3', '{}', '{}', 'PENDING_APPROVAL', 'BOOK', 'a book', '/tmp/x', '[]', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
			ana.id, shelves.biblioteca.id
		))
		.await
		.unwrap();
	assert_eq!(visible_ids(app).await, vec!["BooksWaiting:-".to_string()]);
}

#[tokio::test]
async fn marking_warnings_as_seen_is_for_the_owner_at_home() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_with_row(app, "ana").await;
	give_devices(app, &ana.id, 3).await;

	// a friend is refused for both mutations, and nothing changed
	let token = app.login_as("ana", "secret-pass-1").await;
	for (query, variables) in [
		("mutation { dismissAccessWarnings }", None),
		(RESTORE, Some(json!({"ids": ["AtDeviceLimit:x"]}))),
	] {
		let refused = app.execute_gql_with_token(query, variables, &token).await;
		assert!(refused.get("errors").is_some(), "{query}: {refused:#}");
	}
	assert_eq!(visible_ids(app).await, vec![format!("AtDeviceLimit:{}", ana.id)]);

	// the owner through the public edge is refused too
	let owner_token = app.access_token.read().await.clone().unwrap();
	for query in [
		"mutation { dismissAccessWarnings }",
		"mutation { restoreAccessWarnings(ids: []) }",
	] {
		let via_edge = app
			.server
			.post("/api/graphql")
			.add_header("x-edge-public", "1")
			.add_header("x-edge-pass", pass_for("deploy-old"))
			.add_header("authorization", format!("Bearer {owner_token}"))
			.json(&json!({ "query": query }))
			.await;
		assert_eq!(via_edge.status_code(), StatusCode::UNAUTHORIZED, "{query}");
	}
	// nothing was marked or shown again (the edge may have made a pass of its own known, which
	// is a warning of its own: a leftover)
	assert!(visible_ids(app)
		.await
		.contains(&format!("AtDeviceLimit:{}", ana.id)));
	assert!(!audit_actions(app).await.contains(&"warning.dismiss".to_string()));
	assert!(!audit_actions(app).await.contains(&"warning.restore".to_string()));
}
