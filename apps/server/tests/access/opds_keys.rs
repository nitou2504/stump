//! Reading-app keys: the address a reading app (KOReader, a Kobo…) is given, which works through
//! the public edge with no pass and no cookie, and nowhere but the OPDS feeds

use axum::http::StatusCode;
use chrono::Utc;

use super::*;

/// Makes a reading app for a friend through the edge. Returns its device id, the key in its
/// address, and the path of the catalogue
async fn reading_app(app: &TestApp, friend: &Friend) -> (String, String, String) {
	let response = friend_gql(
		app,
		&friend.pass,
		&friend.session,
		"mutation { createReadingApp(label: \"Kobo\") { id url } }",
		None,
	)
	.await
	.json::<Value>();
	assert!(response.get("errors").is_none(), "{response:#}");
	let created = &response["data"]["createReadingApp"];
	let url = created["url"].as_str().unwrap();
	let path = url.strip_prefix(PUBLIC_URL).expect("the public address").to_string();
	let key = path
		.strip_prefix("/opds/")
		.unwrap()
		.split('/')
		.next()
		.unwrap()
		.to_string();
	(created["id"].as_str().unwrap().to_string(), key, path)
}

/// A request as the edge forwards it for a reading app: the marker, and no pass, no cookie
async fn through_the_edge(app: &TestApp, path: &str) -> TestResponse {
	app.server.get(path).add_header("x-edge-public", "1").await
}

#[tokio::test]
async fn a_reading_app_reads_the_catalogue_with_only_its_key() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	let (id, key, catalogue) = reading_app(app, &ana).await;
	assert_eq!(catalogue, format!("/opds/{key}/v1.2/catalog"));
	assert_eq!(pass::verify_reading_key(KEY, &key).as_deref(), Some(id.as_str()));

	// no pass, no cookie: the key is the credential
	let feed = through_the_edge(app, &catalogue).await;
	assert_eq!(feed.status_code(), StatusCode::OK, "{}", feed.text());
	assert!(feed.header("content-type").to_str().unwrap().contains("xml"));
	let text = feed.text();
	assert!(text.contains(&format!("/opds/{key}/v1.2/")), "the feed's links lose the key");
	assert!(!text.contains("\"/opds/v1.2/"), "a link without the key");
	assert!(feed.headers().get("set-cookie").is_none());
	// at home it works the same (a reading app on the LAN)
	assert_eq!(app.server.get(&catalogue).await.status_code(), StatusCode::OK);

	// what the person sees is what the account sees: Biblioteca, and not the hidden Manga
	let libraries = through_the_edge(app, &format!("/opds/{key}/v1.2/libraries")).await;
	assert_eq!(libraries.status_code(), StatusCode::OK);
	let libraries = libraries.text();
	assert!(libraries.contains("Biblioteca"), "{libraries}");
	assert!(!libraries.contains("Manga"), "{libraries}");

	// read only: GET and nothing else
	for response in [
		app.server.post(&catalogue).add_header("x-edge-public", "1").await,
		app.server.put(&catalogue).add_header("x-edge-public", "1").await,
		app.server.delete(&catalogue).add_header("x-edge-public", "1").await,
		app.server.patch(&catalogue).add_header("x-edge-public", "1").await,
	] {
		assert_eq!(response.status_code(), StatusCode::METHOD_NOT_ALLOWED);
	}

	// the "last seen" of the key is noted, with where it came from
	assert!(eventually(|| async {
		device_row(app, &id).await.unwrap().last_ip == "127.0.0.1"
	})
	.await);
}

/// Anything that is not a key the app made is refused, without the database being asked about
/// somebody else's
#[tokio::test]
async fn a_forged_or_borrowed_key_is_refused() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	let (_, key, _) = reading_app(app, &ana).await;
	let browser = devices_of(app, &ana.id).await.into_iter().find(|d| d.kind == "browser").unwrap();

	let mut tampered = key.clone();
	tampered.pop();
	tampered.push(if key.ends_with('A') { 'B' } else { 'A' });
	// shaped like a key, but not one the app made: refused by the app
	for (what, bad) in [
		("a tampered mac", tampered),
		("an id nobody has", pass::mint_reading_key(KEY, "kobo-nobody")),
		// a key for a browser's device: the kind is checked too
		("a key made for a browser's pass", pass::mint_reading_key(KEY, &browser.id)),
		// another server's key
		("a key of another server", pass::mint_reading_key(&"f".repeat(64), &browser.id)),
	] {
		let response = through_the_edge(app, &format!("/opds/{bad}/v1.2/catalog")).await;
		assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED, "{what}");
	}
	// not shaped like a key at all: with no pass the app's own belt turns it away (the edge does
	// too), and it is never taken for an API key
	for (what, bad) in [
		("a pass", ana.pass.clone()),
		("a path segment of nonsense", "stump_abc_def".to_string()),
	] {
		let response = through_the_edge(app, &format!("/opds/{bad}/v1.2/catalog")).await;
		assert_eq!(response.status_code(), StatusCode::NOT_FOUND, "{what}");
		// even with a pass, an API key that is no key is refused
		let with_pass = app
			.server
			.get(&format!("/opds/{bad}/v1.2/catalog"))
			.add_header("x-edge-public", "1")
			.add_header("x-edge-pass", ana.pass.clone())
			.await;
		assert_eq!(with_pass.status_code(), StatusCode::UNAUTHORIZED, "{what} with a pass");
	}

	// a key is no bearer token and no pass for anything else
	for path in ["/api/graphql", "/api/v2/auth/me"] {
		let as_bearer = app
			.server
			.get(path)
			.add_header("x-edge-public", "1")
			.add_header("x-edge-pass", ana.pass.clone())
			.add_header("authorization", format!("Bearer {key}"))
			.await;
		assert_eq!(as_bearer.status_code(), StatusCode::UNAUTHORIZED, "{path}");
	}
	// and the key in a place that is not OPDS 1.2 finds nothing
	for path in [
		format!("/opds/{key}/v2.0/catalog"),
		format!("/koreader/{key}/syncs/progress"),
		format!("/api/v2/{key}/books"),
	] {
		let response = through_the_edge(app, &path).await;
		assert!(
			response.status_code() == StatusCode::NOT_FOUND
				|| response.status_code() == StatusCode::UNAUTHORIZED
				|| response.status_code() == StatusCode::FORBIDDEN
				|| response.status_code() == StatusCode::METHOD_NOT_ALLOWED,
			"{path}: {}",
			response.status_code()
		);
		assert!(!response.text().contains("<feed"), "{path} served a feed");
	}
}

/// Revoking a key stops it in the app at once, and the edge is told
#[tokio::test]
async fn a_revoked_key_stops_at_once() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	let (id, _, catalogue) = reading_app(app, &ana).await;
	assert_eq!(through_the_edge(app, &catalogue).await.status_code(), StatusCode::OK);

	let removed = friend_gql(
		app,
		&ana.pass,
		&ana.session,
		"mutation($id: ID!) { removeReadingApp(id: $id) }",
		Some(json!({"id": id})),
	)
	.await
	.json::<Value>();
	assert_eq!(removed["data"]["removeReadingApp"], true);
	assert_eq!(through_the_edge(app, &catalogue).await.status_code(), StatusCode::UNAUTHORIZED);
	assert_eq!(app.server.get(&catalogue).await.status_code(), StatusCode::UNAUTHORIZED);
	let state = std::fs::read_to_string(env.state_dir.path().join("state.json")).unwrap();
	assert!(state.contains(&id), "the edge is not told: {state}");

	// the owner can revoke it too, from Public access
	let (id2, _, catalogue2) = reading_app(app, &ana).await;
	assert_eq!(through_the_edge(app, &catalogue2).await.status_code(), StatusCode::OK);
	let revoked = owner_gql(
		app,
		"mutation($id: ID!) { revokeAccessDevice(id: $id) }",
		Some(json!({"id": id2})),
	)
	.await;
	assert_eq!(revoked["data"]["revokeAccessDevice"], true);
	assert_eq!(through_the_edge(app, &catalogue2).await.status_code(), StatusCode::UNAUTHORIZED);

	// a revoked key's row is never forgotten: it has no expiry, and forgetting it would revive it
	let mut row = {
		use sea_orm::IntoActiveModel;
		device_row(app, &id2).await.unwrap().into_active_model()
	};
	row.revoked_at = Set(Some(Utc::now() - chrono::Duration::days(400)));
	row.update(app.conn()).await.unwrap();
	models::services::access::prune(app.conn(), Utc::now()).await.unwrap();
	assert!(device_row(app, &id2).await.is_some(), "a revoked reading-app key was forgotten");
}

/// The person behind a key must be one the owner let in, whatever the key says
#[tokio::test]
async fn a_key_works_on_the_internet_only_for_somebody_who_may_be_there() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	let (_, _, catalogue) = reading_app(app, &ana).await;
	assert_eq!(through_the_edge(app, &catalogue).await.status_code(), StatusCode::OK);

	// given a permission that is not for the internet: the key stops there, and works at home
	set_permissions(app, "ana", "DOWNLOAD_FILE,MANAGE_USERS").await;
	assert_eq!(through_the_edge(app, &catalogue).await.status_code(), StatusCode::UNAUTHORIZED);
	assert_eq!(app.server.get(&catalogue).await.status_code(), StatusCode::OK);
	set_permissions(app, "ana", "DOWNLOAD_FILE,ADD_BOOKS").await;
	assert_eq!(through_the_edge(app, &catalogue).await.status_code(), StatusCode::OK);

	// switched off by the owner: the same
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { publicSignin } }",
		Some(json!({"i": {"personId": ana.id, "publicSignin": false}})),
	)
	.await;
	assert_eq!(through_the_edge(app, &catalogue).await.status_code(), StatusCode::UNAUTHORIZED);
	assert_eq!(app.server.get(&catalogue).await.status_code(), StatusCode::OK);

	// locked: refused everywhere
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { publicSignin } }",
		Some(json!({"i": {"personId": ana.id, "publicSignin": true}})),
	)
	.await;
	assert_eq!(through_the_edge(app, &catalogue).await.status_code(), StatusCode::OK);
	{
		use sea_orm::IntoActiveModel;
		let mut row = user_row(app, "ana").await.into_active_model();
		row.is_locked = Set(true);
		row.update(app.conn()).await.unwrap();
	}
	assert_eq!(through_the_edge(app, &catalogue).await.status_code(), StatusCode::FORBIDDEN);
	assert_eq!(app.server.get(&catalogue).await.status_code(), StatusCode::FORBIDDEN);
}

/// With public sign-in off the internet is as before: reading-app keys are refused through the
/// edge (switching it off ends them all at once), and still work at home
#[tokio::test]
async fn with_the_switch_off_a_key_is_refused_through_the_edge_and_works_at_home() {
	let env = edge_app(true).await;
	let app = &env.app;
	let ben = insert_user(app, "ben", "secret-pass-1", false).await;
	let device = models::services::access::create_reading_app(
		app.conn(),
		Utc::now(),
		&ben.id,
		"ben",
		"Kobo",
		None,
	)
	.await
	.unwrap()
	.expect("room for a device");
	let catalogue = format!("/opds/{}/v1.2/catalog", pass::mint_reading_key(KEY, &device.id));

	assert_eq!(through_the_edge(app, &catalogue).await.status_code(), StatusCode::NOT_FOUND);
	assert_eq!(app.server.get(&catalogue).await.status_code(), StatusCode::OK);
	// and the owner has no key of their own: the owner's account is never a reading app's
	let owner = user_row(app, "initial-server-admin").await;
	let owners_device = models::services::access::create_reading_app(
		app.conn(),
		Utc::now(),
		&owner.id,
		"owner",
		"Kobo",
		None,
	)
	.await
	.unwrap()
	.unwrap();
	let owners = format!("/opds/{}/v1.2/catalog", pass::mint_reading_key(KEY, &owners_device.id));
	assert_eq!(app.server.get(&owners).await.status_code(), StatusCode::UNAUTHORIZED);
}

/// The edge-side belt: with `require_pass` the app lets a key through without a pass, and only
/// OPDS 1.2 addresses shaped like a key
#[tokio::test]
async fn the_app_lets_only_key_shaped_opds_addresses_through_without_a_pass() {
	use stump_server::middleware::edge::{is_open_public_path, is_reading_key_path};

	let key = pass::mint_reading_key(KEY, "kobo-9x2mq");
	for path in [
		format!("/opds/{key}/v1.2/catalog"),
		format!("/opds/{key}/v1.2/books/abc/file/My%20Book.epub"),
	] {
		assert!(is_reading_key_path(&path), "{path}");
		assert!(is_open_public_path(&path), "{path}");
	}
	for path in [
		"/opds/v1.2/catalog".to_string(),
		format!("/opds/{key}/v2.0/catalog"),
		format!("/opds/{key}"),
		format!("/koreader/{key}/v1.2/catalog"),
		"/opds/stump_abc_def/v1.2/catalog".to_string(),
		format!("/api/{key}/v1.2/catalog"),
	] {
		assert!(!is_reading_key_path(&path), "{path}");
		assert!(!is_open_public_path(&path), "{path}");
	}
}
