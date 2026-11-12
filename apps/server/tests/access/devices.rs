//! A person's own devices: they see and manage theirs and nobody else's

use axum::http::StatusCode;

use super::*;

const MY_DEVICES: &str = "{ myDevices { active limit devices { id label kind current lastIp } } }";

async fn my_devices(app: &TestApp, friend: &Friend) -> Value {
	let response = friend_gql(app, &friend.pass, &friend.session, MY_DEVICES, None).await;
	let body: Value = response.json();
	assert!(body.get("errors").is_none(), "{body:#}");
	body["data"]["myDevices"].clone()
}

async fn two_friends(app: &TestApp) -> (Friend, Friend) {
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	let ben = sign_up_friend(app, "ben", &[&shelves.biblioteca.id], "203.0.113.6").await;
	(ana, ben)
}

/// Each person sees their own devices, with the one they are on marked, and can rename and remove
/// those; ids of somebody else's devices do nothing
#[tokio::test]
async fn a_person_sees_and_manages_only_their_own_devices() {
	let env = signin_app().await;
	let app = &env.app;
	let (ana, ben) = two_friends(app).await;
	// ana's second phone
	let second = page_signin(app, "ana", &ana.password, None, "198.51.100.5").await;
	assert_eq!(second.status_code(), StatusCode::SEE_OTHER);

	let mine = my_devices(app, &ana).await;
	assert_eq!(mine["active"], 2);
	assert_eq!(mine["limit"], 3);
	let devices = mine["devices"].as_array().unwrap();
	assert_eq!(devices.len(), 2);
	assert_eq!(devices.iter().filter(|device| device["current"] == true).count(), 1);
	let current = devices.iter().find(|device| device["current"] == true).unwrap();
	let other = devices.iter().find(|device| device["current"] == false).unwrap();
	assert_eq!(current["kind"], "BROWSER");
	assert_eq!(other["lastIp"], "198.51.100.5");

	let bens_device = devices_of(app, &ben.id).await.remove(0);
	assert!(!mine.to_string().contains(&bens_device.id), "ana can see ben's device");

	// rename her own; the name is cleaned of control characters
	let renamed = friend_gql(
		app,
		&ana.pass,
		&ana.session,
		"mutation($id: ID!, $label: String!) { renameMyDevice(id: $id, label: $label) { label } }",
		Some(json!({"id": other["id"], "label": "  Kitchen\u{0007} tablet  "})),
	)
	.await
	.json::<Value>();
	assert_eq!(renamed["data"]["renameMyDevice"]["label"], "Kitchen tablet", "{renamed:#}");

	// ben's device: not found, not renamed, not removed
	let rename_other = friend_gql(
		app,
		&ana.pass,
		&ana.session,
		"mutation($id: ID!) { renameMyDevice(id: $id, label: \"mine now\") { id } }",
		Some(json!({"id": bens_device.id})),
	)
	.await
	.json::<Value>();
	assert!(errors_of(&rename_other).contains("not found"), "{rename_other:#}");
	let remove_other = friend_gql(
		app,
		&ana.pass,
		&ana.session,
		"mutation($id: ID!) { removeMyDevice(id: $id) }",
		Some(json!({"id": bens_device.id})),
	)
	.await
	.json::<Value>();
	assert_eq!(remove_other["data"]["removeMyDevice"], false, "{remove_other:#}");
	let still = device_row(app, &bens_device.id).await.unwrap();
	assert!(still.revoked_at.is_none());
	assert_ne!(still.label, "mine now");

	// remove her own other device: revoked, the edge is told, her list shrinks
	let removed = friend_gql(
		app,
		&ana.pass,
		&ana.session,
		"mutation($id: ID!) { removeMyDevice(id: $id) }",
		Some(json!({"id": other["id"]})),
	)
	.await
	.json::<Value>();
	assert_eq!(removed["data"]["removeMyDevice"], true, "{removed:#}");
	let row = device_row(app, other["id"].as_str().unwrap()).await.unwrap();
	assert!(row.revoked_at.is_some());
	let state = std::fs::read_to_string(env.state_dir.path().join("state.json")).unwrap();
	assert!(state.contains(other["id"].as_str().unwrap()), "{state}");
	assert_eq!(my_devices(app, &ana).await["active"], 1);
	assert!(audit_actions(app).await.contains(&"device.removed_by_person".to_string()));

	// the owner sees the same devices with their kind, and the numbers on the People table
	let owner_view = owner_gql(
		app,
		"{ accessDevices(pagination: {page: 1, pageSize: 50}) { nodes { id kind status } } accessPeople { person { username } activeDevices maxDevices publicSignin signupLinkId } }",
		None,
	)
	.await;
	assert!(owner_view.get("errors").is_none(), "{owner_view:#}");
	assert!(owner_view["data"]["accessDevices"]["nodes"]
		.as_array()
		.unwrap()
		.iter()
		.all(|node| node["kind"] == "BROWSER"));
	let people = owner_view["data"]["accessPeople"].as_array().unwrap();
	let ana_row = people.iter().find(|row| row["person"]["username"] == "ana").unwrap();
	assert_eq!(ana_row["activeDevices"], 1);
	assert_eq!(ana_row["maxDevices"], 3);
	assert_eq!(ana_row["publicSignin"], true);
	assert!(ana_row["signupLinkId"].is_string());
}

/// Removing the device you are using signs you out of it: the pass is revoked and the session goes
#[tokio::test]
async fn removing_the_device_you_are_on_ends_the_session_and_the_pass() {
	let env = signin_app().await;
	let app = &env.app;
	let (ana, _) = two_friends(app).await;
	let mine = my_devices(app, &ana).await;
	let current = mine["devices"][0]["id"].as_str().unwrap().to_string();
	assert_eq!(sessions_of(app, &ana.id).await, 1);

	let removed = friend_gql(
		app,
		&ana.pass,
		&ana.session,
		"mutation($id: ID!) { removeMyDevice(id: $id) }",
		Some(json!({"id": current})),
	)
	.await
	.json::<Value>();
	assert_eq!(removed["data"]["removeMyDevice"], true, "{removed:#}");

	// the pass is refused at once, and the session is gone
	let after = me_after(app, &ana).await;
	assert!(after.status_code() == StatusCode::FORBIDDEN || after.status_code() == StatusCode::UNAUTHORIZED);
	assert_eq!(sessions_of(app, &ana.id).await, 0);
}

async fn me_after(app: &TestApp, friend: &Friend) -> TestResponse {
	friend_gql(app, &friend.pass, &friend.session, "{ me { username } }", None).await
}

/// A mutation another site steered the browser to send is refused
#[tokio::test]
async fn a_cross_site_request_cannot_remove_a_device() {
	let env = signin_app().await;
	let app = &env.app;
	let (ana, _) = two_friends(app).await;
	let device = devices_of(app, &ana.id).await.remove(0);
	let response = app
		.server
		.post("/api/graphql")
		.add_header("x-edge-public", "1")
		.add_header("x-edge-pass", ana.pass.clone())
		.add_header("cookie", format!("stump_session={}", ana.session))
		.add_header("sec-fetch-site", "cross-site")
		.json(&json!({
			"query": "mutation($id: ID!) { removeMyDevice(id: $id) }",
			"variables": {"id": device.id}
		}))
		.await
		.json::<Value>();
	assert!(response.get("errors").is_some(), "{response:#}");
	assert!(device_row(app, &device.id).await.unwrap().revoked_at.is_none());
}

/// Reading apps are devices: they count against the limit, and a person at the limit can replace
/// one but not add one
#[tokio::test]
async fn reading_apps_count_as_devices_and_the_limit_applies() {
	let env = signin_app().await;
	let app = &env.app;
	let (ana, _) = two_friends(app).await;

	let make = |label: &'static str| {
		let (app, ana) = (&env.app, &ana);
		async move {
			friend_gql(
				app,
				&ana.pass,
				&ana.session,
				"mutation($l: String!) { createReadingApp(label: $l) { id label url } }",
				Some(json!({"l": label})),
			)
			.await
			.json::<Value>()
		}
	};
	let kobo = make("Kobo").await;
	assert!(kobo.get("errors").is_none(), "{kobo:#}");
	let koreader = make("KOReader").await;
	assert!(koreader.get("errors").is_none(), "{koreader:#}");
	// the browser + two reading apps = three devices: at the limit
	let third = make("Another").await;
	assert!(errors_of(&third).contains("already use all your devices"), "{third:#}");
	let mine = my_devices(app, &ana).await;
	assert_eq!(mine["active"], 3);
	assert_eq!(
		mine["devices"].as_array().unwrap().iter().filter(|d| d["kind"] == "OPDS").count(),
		2
	);
	// the sign-in page counts them too
	let page = page_signin(app, "ana", &ana.password, None, "198.51.100.5").await;
	assert_eq!(page.status_code(), StatusCode::FORBIDDEN);

	// the address of a reading app is derived: asking again shows the same one
	let again = friend_gql(
		app,
		&ana.pass,
		&ana.session,
		"{ myReadingApps { id label url } }",
		None,
	)
	.await
	.json::<Value>();
	let apps = again["data"]["myReadingApps"].as_array().unwrap();
	assert_eq!(apps.len(), 2);
	let kobo_url = kobo["data"]["createReadingApp"]["url"].as_str().unwrap();
	assert!(apps.iter().any(|app| app["url"] == kobo_url), "{again:#}");
	assert!(kobo_url.starts_with(&format!("{PUBLIC_URL}/opds/")), "{kobo_url}");
	assert!(kobo_url.ends_with("/v1.2/catalog"), "{kobo_url}");

	// replacing one at the limit works: the old address stops, a new one starts
	let kobo_id = kobo["data"]["createReadingApp"]["id"].as_str().unwrap();
	let replaced = friend_gql(
		app,
		&ana.pass,
		&ana.session,
		"mutation($id: ID!) { replaceReadingApp(id: $id) { id label url } }",
		Some(json!({"id": kobo_id})),
	)
	.await
	.json::<Value>();
	assert!(replaced.get("errors").is_none(), "{replaced:#}");
	assert_eq!(replaced["data"]["replaceReadingApp"]["label"], "Kobo");
	assert_ne!(replaced["data"]["replaceReadingApp"]["url"], kobo_url);
	assert!(device_row(app, kobo_id).await.unwrap().revoked_at.is_some());
	assert_eq!(my_devices(app, &ana).await["active"], 3);

	// removing one makes room for another
	let removed = friend_gql(
		app,
		&ana.pass,
		&ana.session,
		"mutation($id: ID!) { removeReadingApp(id: $id) }",
		Some(json!({"id": koreader["data"]["createReadingApp"]["id"]})),
	)
	.await
	.json::<Value>();
	assert_eq!(removed["data"]["removeReadingApp"], true);
	assert!(make("Another").await.get("errors").is_none());
}

/// Who may make a reading app: a person who can download, who is not the owner
#[tokio::test]
async fn only_somebody_who_can_download_makes_a_reading_app() {
	let env = signin_app().await;
	let app = &env.app;
	let (ana, _) = two_friends(app).await;
	set_permissions(app, "ana", "CHANGE_PASSWORD").await;
	let refused = friend_gql(
		app,
		&ana.pass,
		&ana.session,
		"mutation { createReadingApp(label: \"Kobo\") { id } }",
		None,
	)
	.await
	.json::<Value>();
	assert!(errors_of(&refused).contains("can't download"), "{refused:#}");

	let owner = owner_gql(app, "mutation { createReadingApp(label: \"Kobo\") { id } }", None).await;
	assert!(errors_of(&owner).contains("server owner"), "{owner:#}");
}
