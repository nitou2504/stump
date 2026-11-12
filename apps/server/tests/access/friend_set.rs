//! The internet is only ever somebody the owner let in: whatever the credential, whatever the pass

use axum::http::StatusCode;

use super::*;

async fn ana_in_biblioteca(app: &TestApp) -> (Shelves, Friend) {
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	(shelves, ana)
}

async fn me_through_the_edge(app: &TestApp, friend: &Friend) -> TestResponse {
	friend_gql(app, &friend.pass, &friend.session, "{ me { username } }", None).await
}

/// A person who is given a permission a friend may not have is cut off from the internet at once,
/// and keeps working at home; taking it away brings them back
#[tokio::test]
async fn a_permission_a_friend_may_not_have_cuts_the_account_off_the_internet_and_not_home() {
	let env = signin_app().await;
	let app = &env.app;
	let (_, ana) = ana_in_biblioteca(app).await;
	assert_eq!(me_through_the_edge(app, &ana).await.json::<Value>()["data"]["me"]["username"], "ana");

	set_permissions(app, "ana", "CHANGE_PASSWORD,MANAGE_USERS").await;
	let cut = me_through_the_edge(app, &ana).await;
	assert_eq!(cut.status_code(), StatusCode::UNAUTHORIZED);
	// the refusal does not delete the session cookie: the cookie is not tied to a port, and a
	// refusal on the public address must not sign her out at home
	assert!(cut.headers().get("set-cookie").is_none());

	let at_home = app
		.server
		.post("/api/graphql")
		.add_header("cookie", format!("stump_session={}", ana.session))
		.json(&json!({"query": "{ me { username } }"}))
		.await;
	at_home.assert_status_ok();
	assert_eq!(at_home.json::<Value>()["data"]["me"]["username"], "ana");

	// the page and the web app's sign-in refuse her too, as a wrong password
	let page = page_signin(app, "ana", &ana.password, None, "198.51.100.5").await;
	assert_eq!(page.status_code(), StatusCode::UNAUTHORIZED);
	let login = public_login(app, "ana", &ana.password, Some(&ana.pass)).await;
	assert_eq!(login.status_code(), StatusCode::UNAUTHORIZED);

	// the owner sees why, in plain words
	assert!(warning_kinds(app)
		.await
		.contains(&("NOT_FRIEND_CLASS".to_string(), "ana".to_string())));
	let people = owner_gql(
		app,
		"{ accessPeople { person { username } publicSignin beyondFriendSet } }",
		None,
	)
	.await;
	let row = people["data"]["accessPeople"]
		.as_array()
		.unwrap()
		.iter()
		.find(|row| row["person"]["username"] == "ana")
		.unwrap();
	// what it implies is named too (managing users implies reading them)
	assert!(row["beyondFriendSet"].as_array().unwrap().contains(&json!("MANAGE_USERS")));
	assert_eq!(row["publicSignin"], true);

	set_permissions(app, "ana", "CHANGE_PASSWORD,ADD_BOOKS").await;
	assert_eq!(me_through_the_edge(app, &ana).await.status_code(), StatusCode::OK);
}

/// An account nobody enabled can't use the internet with any credential, whatever pass the
/// browser holds; the owner switching it on is what lets it in
#[tokio::test]
async fn an_account_nobody_enabled_is_refused_with_every_credential() {
	let env = signin_app().await;
	let app = &env.app;
	let ben = insert_user(app, "ben", "secret-pass-1", false).await;
	let token = app.login_as("ben", "secret-pass-1").await;
	let legacy = pass_for("deploy-old");
	let through_the_edge = |token: String, pass: String| {
		let app = &env.app;
		async move {
			app.server
				.post("/api/graphql")
				.add_header("x-edge-public", "1")
				.add_header("x-edge-pass", pass)
				.add_header("authorization", format!("Bearer {token}"))
				.json(&json!({"query": "{ me { username } }"}))
				.await
		}
	};

	assert_eq!(
		through_the_edge(token.clone(), legacy.clone()).await.status_code(),
		StatusCode::UNAUTHORIZED
	);
	// at home he is fine
	assert_eq!(app.get_with_token("/api/v2/auth/me", &token).await.status_code(), StatusCode::OK);

	// refused by the settings: not allowed for an account that holds more than a friend may
	set_permissions(app, "ben", "DOWNLOAD_FILE,SCAN_LIBRARY").await;
	let refused = owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { publicSignin } }",
		Some(json!({"i": {"personId": ben.id, "publicSignin": true}})),
	)
	.await;
	assert!(errors_of(&refused).contains("SCAN_LIBRARY"), "{refused:#}");
	assert!(errors_of(&refused).contains("Remove them first"), "{refused:#}");
	assert!(person_row_of(app, &ben.id).await.is_none());

	// enabled by the owner: let in
	set_permissions(app, "ben", "DOWNLOAD_FILE").await;
	let enabled = owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { publicSignin maxDevices } }",
		Some(json!({"i": {"personId": ben.id, "publicSignin": true}})),
	)
	.await;
	assert_eq!(enabled["data"]["setPersonAccess"]["publicSignin"], true, "{enabled:#}");
	assert_eq!(
		through_the_edge(token.clone(), legacy.clone()).await.status_code(),
		StatusCode::OK
	);
	// and switched off again, cut off at once
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { publicSignin } }",
		Some(json!({"i": {"personId": ben.id, "publicSignin": false}})),
	)
	.await;
	assert_eq!(
		through_the_edge(token, legacy).await.status_code(),
		StatusCode::UNAUTHORIZED
	);
}

/// What the owner can and can't do to a person's access
#[tokio::test]
async fn the_settings_refuse_what_cannot_be_allowed() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	// Dani as she is today: Upload files and Scan library keep her out
	let dani = crate::common::account::CreateTestUser {
		username: "dani".to_string(),
		permissions: ["CHANGE_PASSWORD", "CHANGE_USERNAME", "CHANGE_AVATAR", "DOWNLOAD_FILE", "UPLOAD_FILE", "SCAN_LIBRARY", "ADD_BOOKS"]
			.iter()
			.map(|name| name.parse().unwrap())
			.collect(),
		..Default::default()
	}
	.insert(app)
	.await;
	let enable = |input: Value| {
		let app = &env.app;
		async move {
			owner_gql(
				app,
				"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { publicSignin maxDevices newLibrariesHidden hiddenLibraries activeDevices } }",
				Some(json!({"i": input})),
			)
			.await
		}
	};

	let refused = enable(json!({"personId": dani.id, "publicSignin": true})).await;
	let text = errors_of(&refused);
	assert!(text.contains("UPLOAD_FILE") && text.contains("SCAN_LIBRARY"), "{refused:#}");
	// the owner removes both; now it works, narrowed to Biblioteca, with a new password later
	set_permissions(app, "dani", "CHANGE_PASSWORD,CHANGE_USERNAME,CHANGE_AVATAR,DOWNLOAD_FILE,ADD_BOOKS").await;
	let allowed = enable(json!({
		"personId": dani.id, "publicSignin": true, "onlyLibraryIds": [shelves.biblioteca.id]
	}))
	.await;
	assert_eq!(allowed["data"]["setPersonAccess"]["publicSignin"], true, "{allowed:#}");
	assert_eq!(allowed["data"]["setPersonAccess"]["hiddenLibraries"], 1);
	assert_eq!(hidden_libraries_of(app, &dani.id).await, vec![shelves.manga.id.clone()]);
	assert_eq!(decided_libraries_of(app, &dani.id).await, 2);

	// the other refusals
	let owner = user_row(app, "initial-server-admin").await;
	assert!(errors_of(&enable(json!({"personId": owner.id, "publicSignin": true})).await)
		.contains("home only"));
	for devices in [0, 11, -1] {
		let response = enable(json!({"personId": dani.id, "maxDevices": devices})).await;
		assert!(errors_of(&response).contains("number of devices"), "{devices}: {response:#}");
	}
	assert!(errors_of(&enable(json!({"personId": dani.id, "onlyLibraryIds": ["nope"]})).await)
		.contains("does not exist"));
	assert!(!errors_of(&enable(json!({"personId": "no-such-user", "maxDevices": 2})).await).is_empty());
	// an account with no password (an identity provider's) can't sign in with one
	let mut row = {
		use sea_orm::IntoActiveModel;
		user_row(app, "dani").await.into_active_model()
	};
	row.hashed_password = Set(String::new());
	row.update(app.conn()).await.unwrap();
	assert!(errors_of(&enable(json!({"personId": dani.id, "publicSignin": true})).await)
		.contains("no password"));

	// somebody who is not the owner can't change anybody's access, or their own
	let friend = insert_user(app, "gus", "secret-pass-3", false).await;
	let friend_token = app.login_as("gus", "secret-pass-3").await;
	let response = app
		.execute_gql_with_token(
			"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { publicSignin } }",
			Some(json!({"i": {"personId": friend.id, "publicSignin": true}})),
			&friend_token,
		)
		.await;
	assert!(response.get("errors").is_some(), "{response:#}");
	assert!(person_row_of(app, &friend.id).await.is_none());
}

/// A device link for an existing account also lets a friend-class account in from the internet
/// (the owner's decision, made by making the link); one that holds more is not let in
#[tokio::test]
async fn a_device_link_for_a_friend_class_account_lets_it_in() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let ana = insert_user(app, "ana", "secret-pass-1", false).await;
	let eve = insert_user(app, "eve", "secret-pass-2", false).await;
	set_permissions(app, "eve", "MANAGE_USERS").await;

	assert!(person_row_of(app, &ana.id).await.is_none());
	make_invite(app, &ana.id, "Ana's phone", 1).await;
	let row = person_row_of(app, &ana.id).await.expect("the link made a row");
	assert!(row.public_signin && row.new_libraries_hidden);
	// what she sees is as it was, and every library that exists is decided
	assert!(hidden_libraries_of(app, &ana.id).await.is_empty());
	assert_eq!(decided_libraries_of(app, &ana.id).await, 2);

	make_invite(app, &eve.id, "Eve's phone", 1).await;
	assert!(person_row_of(app, &eve.id).await.is_none(), "an account that holds more was let in");
	let _ = shelves;
}
