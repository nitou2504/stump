//! Signup links: the owner makes one, a visitor opens it and makes their own account

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};

use super::*;

const PASSWORD: &str = "tulip-Harbor-47";

fn permissions_of(row: &user::Model) -> Vec<String> {
	let mut names: Vec<String> = row
		.permissions
		.clone()
		.unwrap_or_default()
		.split(',')
		.filter(|name| !name.is_empty())
		.map(str::to_string)
		.collect();
	names.sort();
	names
}

/// The whole path: the owner makes a link, a visitor opens it and chooses a name and a password,
/// and ends up with an account made from the link's preset, a device with a pass, and a session
/// that works through the edge
#[tokio::test]
async fn a_visitor_makes_their_own_account_from_a_link() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let (link_id, url) =
		make_signup_link(app, "SINGLE", 1, &[&shelves.biblioteca.id]).await;
	let token = token_of(&url);
	assert!(url.starts_with(&format!("{PUBLIC_URL}/api/_i/")), "{url}");

	// opening it shows the form, spends nothing, and sets nothing
	for agent in [
		"Mozilla/5.0 (iPhone)",
		"WhatsApp/2.23 A",
		"Slackbot-LinkExpanding 1.0",
	] {
		let preview = app
			.server
			.get(&format!("/api/_i/{token}"))
			.add_header("x-edge-public", "1")
			.add_header("user-agent", agent)
			.await;
		preview.assert_status_ok();
		let text = preview.text();
		assert!(text.contains("Create your account"), "{text}");
		assert!(text.contains("autocomplete=\"new-password\""), "{text}");
		assert!(preview.headers().get("set-cookie").is_none());
	}
	assert_eq!(link_row(app, &link_id).await.uses, 0);
	assert_eq!(
		audit_actions(app)
			.await
			.iter()
			.filter(|a| *a == "signup.created")
			.count(),
		0
	);

	// the phone auto-capitalises: the name is kept in lowercase
	let response = join(app, token, "Ana", PASSWORD, PASSWORD, None, "203.0.113.5").await;
	assert_eq!(
		response.status_code(),
		StatusCode::SEE_OTHER,
		"{}",
		response.text()
	);
	assert_eq!(response.header("location"), "/");
	let pass = cookie_in(&response, COOKIE).expect("the device gets its pass");
	let session_cookie = cookie_in(&response, "stump_session").expect("and a session");

	// the account is what the preset says: exactly its permissions, its libraries and its limits
	let account = user_row(app, "ana").await;
	let mut expected: Vec<String> = [
		"CHANGE_PASSWORD",
		"CHANGE_AVATAR",
		"DOWNLOAD_FILE",
		"ADD_BOOKS",
	]
	.iter()
	.map(|name| name.to_string())
	.collect();
	expected.sort();
	assert_eq!(permissions_of(&account), expected);
	assert!(!account.is_server_owner && !account.is_locked);
	assert_eq!(account.max_sessions_allowed, Some(6));
	assert!(bcrypt::verify(PASSWORD, &account.hashed_password).unwrap());

	let person = person_row_of(app, &account.id)
		.await
		.expect("a row of its own");
	assert!(person.public_signin && person.new_libraries_hidden);
	assert_eq!(person.max_devices, 3);
	assert_eq!(person.signup_link_id.as_deref(), Some(link_id.as_str()));
	// only Biblioteca is visible: Manga is hidden, and both are decided
	assert_eq!(
		hidden_libraries_of(app, &account.id).await,
		vec![shelves.manga.id.clone()]
	);
	assert_eq!(decided_libraries_of(app, &account.id).await, 2);

	// one device, a browser's, and the pass is for it
	let devices = devices_of(app, &account.id).await;
	assert_eq!(devices.len(), 1);
	assert_eq!(devices[0].kind, "browser");
	assert_eq!(devices[0].invite_id, None);
	let verified =
		pass::verify_pass(KEY, &pass, Utc::now().timestamp()).expect("a valid pass");
	assert_eq!(verified.id, devices[0].id);
	assert!(response
		.headers()
		.get_all("set-cookie")
		.iter()
		.any(
			|value| value.to_str().unwrap().starts_with(&format!("{COOKIE}="))
				&& value.to_str().unwrap().contains("Secure")
				&& value.to_str().unwrap().contains("HttpOnly")
		));

	// the link spent one use, and said so
	assert_eq!(link_row(app, &link_id).await.uses, 1);
	assert!(audit_actions(app)
		.await
		.contains(&"signup.created".to_string()));
	assert_eq!(sessions_of(app, &account.id).await, 1);

	// the session and the pass work through the edge: they are who the app says they are
	let me = friend_gql(app, &pass, &session_cookie, "{ me { username } }", None).await;
	me.assert_status_ok();
	assert_eq!(me.json::<Value>()["data"]["me"]["username"], "ana");

	// the link is used up: the same link makes nothing more
	let again = join(app, token, "ben", PASSWORD, PASSWORD, None, "203.0.113.6").await;
	assert_eq!(again.status_code(), StatusCode::NOT_FOUND);
	assert!(again.text().contains("This link doesn't work"));
	assert_eq!(link_row(app, &link_id).await.uses, 1);
}

/// Unknown, expired, used up and revoked all show the same page, so it says nothing about which
/// link was real
#[tokio::test]
async fn every_dead_link_shows_the_same_page() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;

	let (expired_id, expired_url) =
		make_signup_link(app, "SINGLE", 1, &[&shelves.biblioteca.id]).await;
	let mut row = link_row(app, &expired_id).await.into_active_model();
	row.expires_at = Set(Utc::now() - Duration::hours(1));
	row.update(app.conn()).await.unwrap();

	let (used_id, used_url) =
		make_signup_link(app, "SINGLE", 1, &[&shelves.biblioteca.id]).await;
	let mut row = link_row(app, &used_id).await.into_active_model();
	row.uses = Set(1);
	row.update(app.conn()).await.unwrap();

	let (revoked_id, revoked_url) =
		make_signup_link(app, "SINGLE", 1, &[&shelves.biblioteca.id]).await;
	let revoked = owner_gql(
		app,
		"mutation($id: ID!) { revokeAccessSignupLink(id: $id) }",
		Some(json!({"id": revoked_id})),
	)
	.await;
	assert!(revoked.get("errors").is_none(), "{revoked:#}");

	let mut pages = Vec::new();
	for token in [
		token_of(&expired_url).to_string(),
		token_of(&used_url).to_string(),
		token_of(&revoked_url).to_string(),
	] {
		let page = open_link(app, &token).await;
		assert_eq!(page.status_code(), StatusCode::NOT_FOUND);
		pages.push(page.text());
		let posted =
			join(app, &token, "ana", PASSWORD, PASSWORD, None, "203.0.113.5").await;
		assert_eq!(posted.status_code(), StatusCode::NOT_FOUND);
		assert!(posted.headers().get("set-cookie").is_none());
		pages.push(posted.text());
	}
	assert!(
		pages.windows(2).all(|pair| pair[0] == pair[1]),
		"the dead links show different pages"
	);
	assert!(pages[0].contains("This link doesn't work"));
	// nothing was made
	assert_eq!(
		user::Entity::find().all(app.conn()).await.unwrap().len(),
		1,
		"only the owner exists"
	);
}

/// The budget of an open link is spent once per account; a name that is taken (whatever its
/// case) is told to the visitor and spends nothing
#[tokio::test]
async fn an_open_link_makes_as_many_accounts_as_its_budget_and_a_taken_name_spends_nothing(
) {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let (link_id, url) =
		make_signup_link(app, "OPEN", 3, &[&shelves.biblioteca.id]).await;
	let token = token_of(&url);

	let ana = join(app, token, "ana", PASSWORD, PASSWORD, None, "203.0.113.1").await;
	assert_eq!(ana.status_code(), StatusCode::SEE_OTHER);
	assert_eq!(link_row(app, &link_id).await.uses, 1);

	// the same name in capitals is the same name: told so, with the form again, nothing spent
	let taken = join(app, token, "ANA", PASSWORD, PASSWORD, None, "203.0.113.2").await;
	assert_eq!(taken.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
	assert!(taken.text().contains("That username is taken"));
	assert!(taken.headers().get("set-cookie").is_none());
	assert_eq!(link_row(app, &link_id).await.uses, 1);
	assert_eq!(user::Entity::find().all(app.conn()).await.unwrap().len(), 2);

	for (index, name) in ["ben", "cara"].iter().enumerate() {
		let response =
			join(app, token, name, PASSWORD, PASSWORD, None, "203.0.113.3").await;
		assert_eq!(response.status_code(), StatusCode::SEE_OTHER, "{name}");
		assert_eq!(link_row(app, &link_id).await.uses, index as i32 + 2);
	}
	// the budget is spent: the fourth person finds a link that doesn't work
	let dan = join(app, token, "dan", PASSWORD, PASSWORD, None, "203.0.113.4").await;
	assert_eq!(dan.status_code(), StatusCode::NOT_FOUND);
	assert_eq!(link_row(app, &link_id).await.uses, 3);
	assert_eq!(user::Entity::find().all(app.conn()).await.unwrap().len(), 4);

	// the people the link made are listed with it, with nothing about a password
	let list = owner_gql(
		app,
		"{ accessSignupLinks { nodes { id status uses maxUses kind people { username } preset { permissions maxDevices libraries { name } } } } }",
		None,
	)
	.await;
	assert!(list.get("errors").is_none(), "{list:#}");
	let node = &list["data"]["accessSignupLinks"]["nodes"][0];
	assert_eq!(node["status"], "USED_UP");
	assert_eq!(node["uses"], 3);
	assert_eq!(node["kind"], "OPEN");
	assert_eq!(node["people"].as_array().unwrap().len(), 3);
	assert_eq!(node["preset"]["libraries"][0]["name"], "Biblioteca");
	assert!(!list.to_string().contains(token));
}

/// Ten people use a link of three at once: exactly three accounts are made
#[tokio::test]
async fn parallel_uses_make_exactly_the_budget() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let (link_id, url) =
		make_signup_link(app, "OPEN", 3, &[&shelves.biblioteca.id]).await;
	let token = token_of(&url).to_string();

	let names: Vec<String> = (0..10).map(|n| format!("person{n}")).collect();
	let responses =
		futures_util::future::join_all(names.iter().enumerate().map(|(n, name)| {
			let token = token.clone();
			async move {
				join(
					app,
					&token,
					name,
					PASSWORD,
					PASSWORD,
					None,
					&format!("203.0.113.{}", 10 + n),
				)
				.await
			}
		}))
		.await;

	let made = responses
		.iter()
		.filter(|response| response.status_code() == StatusCode::SEE_OTHER)
		.count();
	assert_eq!(made, 3);
	assert_eq!(link_row(app, &link_id).await.uses, 3);
	assert_eq!(user::Entity::find().all(app.conn()).await.unwrap().len(), 4);
	assert_eq!(
		access_person::Entity::find()
			.all(app.conn())
			.await
			.unwrap()
			.len(),
		3
	);
}

/// The preset is looked at again when the link is used: a row that was changed (or a client that
/// skipped the owner's page) can't hand out more than a friend may have
#[tokio::test]
async fn a_preset_that_was_tampered_with_makes_no_account() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;

	for tampered in [
		// a permission that manages
		format!(
			r#"{{"permissions":["DOWNLOAD_FILE","MANAGE_USERS"],"libraryIds":["{}"],"ageRestriction":null,"maxDevices":3}}"#,
			shelves.biblioteca.id
		),
		// upload files and scan library: not for friends
		format!(
			r#"{{"permissions":["UPLOAD_FILE","SCAN_LIBRARY"],"libraryIds":["{}"],"ageRestriction":null,"maxDevices":3}}"#,
			shelves.biblioteca.id
		),
		// no libraries, too many devices, not even JSON
		r#"{"permissions":[],"libraryIds":[],"ageRestriction":null,"maxDevices":3}"#
			.to_string(),
		format!(
			r#"{{"permissions":[],"libraryIds":["{}"],"ageRestriction":null,"maxDevices":99}}"#,
			shelves.biblioteca.id
		),
		"not json".to_string(),
	] {
		let (link_id, url) =
			make_signup_link(app, "SINGLE", 1, &[&shelves.biblioteca.id]).await;
		let mut row = link_row(app, &link_id).await.into_active_model();
		row.preset = Set(tampered.clone());
		row.update(app.conn()).await.unwrap();

		// neither the form nor the form's answer works
		let page = open_link(app, token_of(&url)).await;
		assert_eq!(page.status_code(), StatusCode::NOT_FOUND, "{tampered}");
		let posted = join(
			app,
			token_of(&url),
			"ana",
			PASSWORD,
			PASSWORD,
			None,
			"203.0.113.5",
		)
		.await;
		assert_eq!(posted.status_code(), StatusCode::NOT_FOUND, "{tampered}");
		assert_eq!(link_row(app, &link_id).await.uses, 0);
	}
	assert_eq!(user::Entity::find().all(app.conn()).await.unwrap().len(), 1);
}

/// A link whose libraries were all deleted makes nothing and spends nothing; if some are left,
/// they are what the account sees
#[tokio::test]
async fn a_link_whose_libraries_are_gone_spends_nothing() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let (link_id, url) = make_signup_link(app, "OPEN", 5, &[&shelves.manga.id]).await;
	library::Entity::delete_by_id(shelves.manga.id.clone())
		.exec(app.conn())
		.await
		.unwrap();

	let posted = join(
		app,
		token_of(&url),
		"ana",
		PASSWORD,
		PASSWORD,
		None,
		"203.0.113.5",
	)
	.await;
	assert_eq!(posted.status_code(), StatusCode::NOT_FOUND);
	assert_eq!(link_row(app, &link_id).await.uses, 0);
	assert_eq!(user::Entity::find().all(app.conn()).await.unwrap().len(), 1);

	// a link for two libraries, one of which is gone, makes an account that sees the other
	let extra = crate::common::visibility::add_library(app, "Guides").await;
	let (link_id, url) =
		make_signup_link(app, "SINGLE", 1, &[&shelves.biblioteca.id, &extra.id]).await;
	library::Entity::delete_by_id(extra.id.clone())
		.exec(app.conn())
		.await
		.unwrap();
	let posted = join(
		app,
		token_of(&url),
		"ben",
		PASSWORD,
		PASSWORD,
		None,
		"203.0.113.5",
	)
	.await;
	assert_eq!(posted.status_code(), StatusCode::SEE_OTHER);
	assert_eq!(link_row(app, &link_id).await.uses, 1);
	let ben = user_row(app, "ben").await;
	assert!(hidden_libraries_of(app, &ben.id).await.is_empty());
}

/// The holder of a valid link is told what is wrong with the name or the password; a stranger
/// with no valid link is told nothing
#[tokio::test]
async fn the_rules_are_told_to_the_holder_of_a_link_and_nobody_else() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let (link_id, url) =
		make_signup_link(app, "OPEN", 5, &[&shelves.biblioteca.id]).await;
	let token = token_of(&url);

	for (username, password, again, wanted) in [
		("ab", PASSWORD, PASSWORD, "at least 3 characters"),
		("ana lopez", PASSWORD, PASSWORD, "only letters, numbers"),
		("-ana", PASSWORD, PASSWORD, "start with a letter"),
		("admin", PASSWORD, PASSWORD, "not available"),
		("ana", "short", "short", "at least 10 characters"),
		(
			"ana",
			PASSWORD,
			"another-Harbor-47",
			"The two passwords are different",
		),
		("ana", "1234567890", "1234567890", "too common"),
		(
			"ana",
			"aaaaaaaaaaaa",
			"aaaaaaaaaaaa",
			"repeats the same few characters",
		),
		(
			"ana",
			"my-ana-secret-1",
			"my-ana-secret-1",
			"contain the username",
		),
	] {
		let response =
			join(app, token, username, password, again, None, "203.0.113.5").await;
		assert_eq!(
			response.status_code(),
			StatusCode::UNPROCESSABLE_ENTITY,
			"{username} / {password}"
		);
		assert!(
			response.text().contains(wanted),
			"{username}/{password}: {}",
			response.text()
		);
		assert!(response.headers().get("set-cookie").is_none());
	}
	assert_eq!(link_row(app, &link_id).await.uses, 0);

	// in Spanish, for a browser that asks for it
	let spanish = form_post(app, "/api/_i/join", "203.0.113.5", None)
		.add_header("accept-language", "es-EC,es;q=0.9")
		.form(&[
			("t", token),
			("username", "ab"),
			("password", PASSWORD),
			("password2", PASSWORD),
		])
		.await;
	assert!(
		spanish.text().contains("al menos 3 caracteres"),
		"{}",
		spanish.text()
	);

	// no valid link: the same page whatever was typed, and no word about the name
	let stranger = join(
		app,
		"AAAAAAAAAAAAAAAAAAAAAA",
		"ab",
		"short",
		"other",
		None,
		"203.0.113.5",
	)
	.await;
	assert_eq!(stranger.status_code(), StatusCode::NOT_FOUND);
	assert!(!stranger.text().contains("at least"));
}

/// Public sign-in is off unless it is switched on: the new routes answer 404 as if they did not
/// exist, and a signup link's address behaves as an address that is not a link
#[tokio::test]
async fn nothing_works_with_the_switch_off() {
	let env = edge_app(true).await;
	let app = &env.app;

	for response in [
		join(
			app,
			"AAAAAAAAAAAAAAAAAAAAAA",
			"ana",
			PASSWORD,
			PASSWORD,
			None,
			"203.0.113.5",
		)
		.await,
		page_signin(app, "ana", PASSWORD, None, "203.0.113.5").await,
	] {
		assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
	}
	// the front door shows the old "private site, paste your invite link" page, with no form to sign in
	let door = app
		.server
		.get("/api/_i/signin")
		.add_header("x-edge-public", "1")
		.await;
	door.assert_status_ok();
	let text = door.text();
	assert!(text.contains("Private site"), "{text}");
	assert!(text.contains("action=\"/api/_i/\""), "{text}");
	assert!(!text.contains("current-password"), "{text}");
	// and not at home
	assert_eq!(
		app.server.get("/api/_i/signin").await.status_code(),
		StatusCode::NOT_FOUND
	);
	// making a link is refused too, with the reason
	let response = owner_gql(
		app,
		r#"mutation($input: CreateSignupLinkInput!) { createAccessSignupLink(input: $input) { url } }"#,
		Some(json!({"input": {
			"label": "x", "kind": "SINGLE",
			"preset": {"permissions": [], "libraryIds": ["x"], "maxDevices": 3}
		}})),
	)
	.await;
	assert!(
		errors_of(&response).contains("Public sign-in is switched off"),
		"{response:#}"
	);
}

/// The pages answer only through the edge
#[tokio::test]
async fn the_pages_answer_only_through_the_edge() {
	let env = signin_app().await;
	let app = &env.app;
	for path in ["/api/_i/signin"] {
		let home = app.server.get(path).await;
		assert_eq!(home.status_code(), StatusCode::NOT_FOUND, "{path}");
	}
	let home_post = app
		.server
		.post("/api/_i/join")
		.add_header("sec-fetch-site", "same-origin")
		.form(&[
			("t", "AAAAAAAAAAAAAAAAAAAAAA"),
			("username", "ana"),
			("password", PASSWORD),
			("password2", PASSWORD),
		])
		.await;
	assert_eq!(home_post.status_code(), StatusCode::NOT_FOUND);
}

/// A pass that is already in the browser is never replaced: the account is made, and the browser
/// keeps the pass it has
#[tokio::test]
async fn a_pass_already_in_the_browser_is_not_replaced() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let (_, url) = make_signup_link(app, "SINGLE", 1, &[&shelves.biblioteca.id]).await;
	let existing = pass_for("deploy-old");

	let response = join(
		app,
		token_of(&url),
		"ana",
		PASSWORD,
		PASSWORD,
		Some(&existing),
		"203.0.113.5",
	)
	.await;
	assert_eq!(response.status_code(), StatusCode::SEE_OTHER);
	assert!(
		cookie_in(&response, COOKIE).is_none(),
		"the pass in the browser was replaced"
	);
	assert!(cookie_in(&response, "stump_session").is_some());
	let ana = user_row(app, "ana").await;
	assert!(devices_of(app, &ana.id).await.is_empty());
	// the account exists and has its row, ready to sign in on a device of its own
	assert!(person_row_of(app, &ana.id).await.is_some());
}

/// A form posted from another site makes nothing and spends nothing
#[tokio::test]
async fn a_post_from_another_site_is_refused() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let (link_id, url) =
		make_signup_link(app, "SINGLE", 1, &[&shelves.biblioteca.id]).await;

	let response = app
		.server
		.post("/api/_i/join")
		.add_header("x-edge-public", "1")
		.add_header("sec-fetch-site", "cross-site")
		.form(&[
			("t", token_of(&url)),
			("username", "ana"),
			("password", PASSWORD),
			("password2", PASSWORD),
		])
		.await;
	assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
	assert_eq!(link_row(app, &link_id).await.uses, 0);

	// and a body that is far too big for a form is refused before anything is hashed
	let huge = "x".repeat(10_000);
	let response = join(
		app,
		token_of(&url),
		"ana",
		&huge,
		&huge,
		None,
		"203.0.113.5",
	)
	.await;
	assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
	assert_eq!(link_row(app, &link_id).await.uses, 0);
}

/// The owner's API refuses a preset that holds anything outside the friend set, naming it
#[tokio::test]
async fn the_owner_cannot_make_a_link_that_gives_more_than_a_friend_may_have() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;

	for (permissions, named) in [
		(vec!["DOWNLOAD_FILE", "UPLOAD_FILE"], "UPLOAD_FILE"),
		(vec!["SCAN_LIBRARY"], "SCAN_LIBRARY"),
		(vec!["MANAGE_USERS", "ADD_BOOKS"], "MANAGE_USERS"),
		(vec!["MANAGE_LIBRARY"], "MANAGE_LIBRARY"),
	] {
		let response = make_signup_link_with(
			app,
			json!({
				"label": "x", "kind": "SINGLE",
				"preset": {"permissions": permissions, "libraryIds": [shelves.biblioteca.id], "maxDevices": 3}
			}),
		)
		.await;
		assert!(
			errors_of(&response).contains(named),
			"{permissions:?}: {response:#}"
		);
	}
	assert_eq!(
		access_signup_link::Entity::find()
			.all(app.conn())
			.await
			.unwrap()
			.len(),
		0
	);

	// the others the owner got wrong
	let base = |change: &dyn Fn(&mut Value)| {
		let mut input = json!({
			"label": "club", "kind": "OPEN", "validForHours": 72, "maxUses": 5,
			"preset": {"permissions": ["ADD_BOOKS"], "libraryIds": [shelves.biblioteca.id], "maxDevices": 3}
		});
		change(&mut input);
		input
	};
	for (change, wanted) in [
		(
			Box::new(|v: &mut Value| v["preset"]["libraryIds"] = json!([]))
				as Box<dyn Fn(&mut Value)>,
			"Choose at least one library",
		),
		(
			Box::new(|v: &mut Value| v["preset"]["libraryIds"] = json!(["nope"])),
			"does not exist",
		),
		(
			Box::new(|v: &mut Value| v["preset"]["maxDevices"] = json!(11)),
			"number of devices",
		),
		(
			Box::new(|v: &mut Value| v["validForHours"] = json!(0)),
			"validity",
		),
		(
			Box::new(|v: &mut Value| v["validForHours"] = json!(721)),
			"validity",
		),
		(Box::new(|v: &mut Value| v["maxUses"] = json!(1)), "2 to 25"),
		(
			Box::new(|v: &mut Value| v["maxUses"] = json!(26)),
			"2 to 25",
		),
		(Box::new(|v: &mut Value| v["label"] = json!("  ")), "note"),
		(
			Box::new(|v: &mut Value| {
				v["preset"]["ageRestriction"] =
					json!({"age": 0, "restrictOnUnset": false})
			}),
			"age limit",
		),
	] {
		let response = make_signup_link_with(app, base(&*change)).await;
		assert!(
			errors_of(&response).contains(wanted),
			"{wanted}: {response:#}"
		);
	}

	// a single link is always one account, whatever budget was typed
	let response = make_signup_link_with(app, {
		let mut input = base(&|_| {});
		input["kind"] = json!("SINGLE");
		input["maxUses"] = json!(9);
		input
	})
	.await;
	assert_eq!(
		response["data"]["createAccessSignupLink"]["link"]["maxUses"],
		1
	);
}

/// The owner's tools stay at home, and nobody but the owner has them
#[tokio::test]
async fn making_links_is_for_the_owner_at_home() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let friend =
		sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	let input = json!({
		"label": "x", "kind": "SINGLE",
		"preset": {"permissions": ["ADD_BOOKS"], "libraryIds": [shelves.biblioteca.id], "maxDevices": 3}
	});
	let query = r#"mutation($input: CreateSignupLinkInput!) { createAccessSignupLink(input: $input) { url } }"#;

	// a friend through the edge
	let response = friend_gql(
		app,
		&friend.pass,
		&friend.session,
		query,
		Some(json!({"input": input})),
	)
	.await;
	assert!(response.json::<Value>().get("errors").is_some());
	// a friend's own token at home
	let token = app.login_as("ana", &friend.password).await;
	let at_home = app
		.execute_gql_with_token(query, Some(json!({"input": input})), &token)
		.await;
	assert!(at_home.get("errors").is_some(), "{at_home:#}");
	// the owner's token through the edge
	let owner_token = app.access_token.read().await.clone().unwrap();
	let owner_via_edge = app
		.server
		.post("/api/graphql")
		.add_header("x-edge-public", "1")
		.add_header("x-edge-pass", pass_for("deploy-old"))
		.add_header("authorization", format!("Bearer {owner_token}"))
		.json(&json!({"query": query, "variables": {"input": input}}))
		.await;
	assert_eq!(owner_via_edge.status_code(), StatusCode::UNAUTHORIZED);
}

/// Revoking a link stops it; with the switch the accounts it made are turned off too
#[tokio::test]
async fn revoking_a_link_can_turn_off_the_accounts_it_made() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let (link_id, url) =
		make_signup_link(app, "OPEN", 3, &[&shelves.biblioteca.id]).await;
	let token = token_of(&url);
	let ana = join(app, token, "ana", PASSWORD, PASSWORD, None, "203.0.113.1").await;
	let ben = join(app, token, "ben", PASSWORD, PASSWORD, None, "203.0.113.2").await;
	assert_eq!(ana.status_code(), StatusCode::SEE_OTHER);
	assert_eq!(ben.status_code(), StatusCode::SEE_OTHER);
	let ana_row = user_row(app, "ana").await;
	assert_eq!(sessions_of(app, &ana_row.id).await, 1);

	// revoked without the switch: nobody new, the accounts stay on
	let revoked = owner_gql(
		app,
		"mutation($id: ID!) { revokeAccessSignupLink(id: $id) }",
		Some(json!({"id": link_id})),
	)
	.await;
	assert_eq!(revoked["data"]["revokeAccessSignupLink"], 0);
	assert_eq!(
		join(app, token, "cara", PASSWORD, PASSWORD, None, "203.0.113.3")
			.await
			.status_code(),
		StatusCode::NOT_FOUND
	);
	assert!(person_row_of(app, &ana_row.id).await.unwrap().public_signin);

	// later, "and turn off the accounts it made"
	let turned_off = owner_gql(
		app,
		"mutation($id: ID!) { revokeAccessSignupLink(id: $id, disableAccounts: true) }",
		Some(json!({"id": link_id})),
	)
	.await;
	assert_eq!(
		turned_off["data"]["revokeAccessSignupLink"], 2,
		"{turned_off:#}"
	);
	for name in ["ana", "ben"] {
		let row = user_row(app, name).await;
		assert!(
			!person_row_of(app, &row.id).await.unwrap().public_signin,
			"{name}"
		);
		assert!(devices_of(app, &row.id)
			.await
			.iter()
			.all(|device| device.revoked_at.is_some()));
		assert_eq!(sessions_of(app, &row.id).await, 0);
		// the accounts are still there for the owner to keep or remove
		assert!(!row.is_locked);
	}
}

/// What a new link offers by default: the friend permissions, the library called Biblioteca
#[tokio::test]
async fn the_defaults_are_the_friend_preset_and_the_biblioteca_library() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let defaults = owner_gql(
		app,
		"{ signupPresetDefaults { permissions maxDevices ageRestriction { age } libraries { id name } } }",
		None,
	)
	.await;
	assert!(defaults.get("errors").is_none(), "{defaults:#}");
	let preset = &defaults["data"]["signupPresetDefaults"];
	let mut permissions: Vec<String> = preset["permissions"]
		.as_array()
		.unwrap()
		.iter()
		.map(|name| name.as_str().unwrap().to_string())
		.collect();
	permissions.sort();
	let mut expected: Vec<String> = [
		"ADD_BOOKS",
		"CHANGE_AVATAR",
		"CHANGE_PASSWORD",
		"DOWNLOAD_FILE",
	]
	.iter()
	.map(|name| name.to_string())
	.collect();
	expected.sort();
	assert_eq!(permissions, expected);
	assert!(permissions
		.iter()
		.all(|name| !["UPLOAD_FILE", "SCAN_LIBRARY"].contains(&name.as_str())));
	assert_eq!(preset["maxDevices"], 3);
	assert_eq!(preset["libraries"].as_array().unwrap().len(), 1);
	assert_eq!(preset["libraries"][0]["id"], shelves.biblioteca.id);
}
