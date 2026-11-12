//! What the owner should look at: each warning appears when it should, fades when it should, and
//! the badge counts them

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use models::services::{
	access::{self as service, device_kind, NewDevice},
	access_warnings::{self, WarningKind},
};
use sea_orm::{ActiveModelTrait, Set};

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
		let id = service::unused_device_id(app.conn(), &format!("phone{index}")).await.unwrap();
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
	assert!(kinds_of(app).await.is_empty(), "two of three is nothing to say");
	give_devices(app, &ana.id, 1).await;
	assert_eq!(kinds_of(app).await, vec![(WarningKind::AtDeviceLimit, Some(ana.id.clone()))]);
	give_devices(app, &ana.id, 1).await;
	assert_eq!(kinds_of(app).await, vec![(WarningKind::OverDeviceLimit, Some(ana.id.clone()))]);

	// a limit of 6 and four devices: four or more is worth a look whatever the limit
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { maxDevices } }",
		Some(json!({"i": {"personId": ana.id, "maxDevices": 6}})),
	)
	.await;
	assert_eq!(kinds_of(app).await, vec![(WarningKind::ManyDevices, Some(ana.id.clone()))]);
	// and a device that was not seen for 60 days no longer counts
	use sea_orm::IntoActiveModel;
	for device in devices_of(app, &ana.id).await {
		let mut row = device.into_active_model();
		row.last_seen_at = Set(Some(Utc::now() - Duration::days(61)));
		row.update(app.conn()).await.unwrap();
	}
	assert!(kinds_of(app).await.is_empty());
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
	assert!(kinds_of(app).await.contains(&(WarningKind::ManySessions, Some(ana.id.clone()))));

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
	assert!(!kinds_of(app).await.contains(&(WarningKind::SigninFailures, Some(ana.id.clone()))));
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
	assert!(kinds_of(app).await.contains(&(WarningKind::SigninFailures, Some(ana.id.clone()))));
}

#[tokio::test]
async fn events_fade_after_three_days_and_the_unknown_names_are_summed() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_with_row(app, "ana").await;
	let now = Utc::now();

	// old events are forgotten; recent ones are kept
	service::audit(app.conn(), now - Duration::days(4), "public", "signin.device_limit", &ana.id, "old").await.unwrap();
	assert!(kinds_of(app).await.is_empty());
	service::audit(app.conn(), now - Duration::days(2), "public", "signin.device_limit", &ana.id, "recent").await.unwrap();
	service::audit(app.conn(), now - Duration::hours(1), "public", "signin.device_limit", &ana.id, "again").await.unwrap();
	let found = access_warnings::warnings(app.conn(), now).await.unwrap();
	assert_eq!(found.len(), 1);
	assert_eq!(found[0].kind, WarningKind::DeviceLimitHit);
	assert_eq!(found[0].count, 2);

	// a name that was slowed down: the person if the name exists, "an unknown name" if not
	service::audit(app.conn(), now, "public", "signin.backoff", &ana.id, "slowed").await.unwrap();
	service::audit(app.conn(), now, "public", "signin.backoff", "unknown:ab12cd34", "slowed").await.unwrap();
	// failures on names that do not exist are summed from the aggregated rows
	service::audit(app.conn(), now, "public", "signin.unknown_names", "unknown", "12 failures on names that do not exist").await.unwrap();
	service::audit(app.conn(), now, "public", "signin.unknown_names", "unknown", "30 failures on names that do not exist").await.unwrap();
	let found = access_warnings::warnings(app.conn(), now).await.unwrap();
	let unknown = found
		.iter()
		.find(|w| w.kind == WarningKind::SigninFailures && w.person_id.is_none())
		.expect("a warning for names that do not exist");
	assert_eq!(unknown.count, 42);
	assert!(found.iter().any(|w| w.kind == WarningKind::NameBackoff && w.person_id.as_deref() == Some(ana.id.as_str())));
	assert!(found.iter().any(|w| w.kind == WarningKind::NameBackoff && w.person_id.is_none()));

	// the global budget is an alert and comes first
	service::audit(app.conn(), now, "public", "signin.global_budget", "all", "flood").await.unwrap();
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
	let (open_id, url) = make_signup_link(app, "OPEN", 3, &[&shelves.biblioteca.id]).await;
	for name in ["ana", "ben"] {
		join(app, token_of(&url), name, "tulip-Harbor-47", "tulip-Harbor-47", None, "203.0.113.5").await;
	}
	let found = access_warnings::warnings(app.conn(), Utc::now()).await.unwrap();
	let near = found.iter().find(|w| w.kind == WarningKind::LinkNearBudget).expect("a link near its budget");
	assert_eq!(near.link_id.as_deref(), Some(open_id.as_str()));
	assert_eq!(near.count, 1);

	// a link whose libraries are gone can't be used
	let (_, _) = make_signup_link(app, "OPEN", 5, &[&shelves.manga.id]).await;
	library::Entity::delete_by_id(shelves.manga.id.clone()).exec(app.conn()).await.unwrap();
	let found = access_warnings::warnings(app.conn(), Utc::now()).await.unwrap();
	assert!(found.iter().any(|w| w.kind == WarningKind::LinkNoLibraries));

	// used up: the warning about the budget goes away with the link
	join(app, token_of(&url), "cara", "tulip-Harbor-47", "tulip-Harbor-47", None, "203.0.113.5").await;
	let found = access_warnings::warnings(app.conn(), Utc::now()).await.unwrap();
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
