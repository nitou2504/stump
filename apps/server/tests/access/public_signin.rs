//! The public sign-in page: a friend opens the address on any phone or computer and signs in

use axum::http::StatusCode;
use chrono::Utc;

use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};

use super::*;

/// The most a test of the page can do from one client before the guard answers
const WRONG: &str = "not-the-password-1";

async fn friend_in_biblioteca(app: &TestApp, name: &str, ip: &str) -> Friend {
	let shelves = shelves(app).await;
	sign_up_friend(app, name, &[&shelves.biblioteca.id], ip).await
}

/// A friend with a device from signing up opens the address on another phone, signs in with the
/// name and the password, and that phone gets a pass of its own and a session
#[tokio::test]
async fn a_friend_signs_in_on_a_new_device_and_the_device_gets_a_pass() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_in_biblioteca(app, "ana", "203.0.113.5").await;

	let page = app
		.server
		.get("/api/_i/signin")
		.add_header("x-edge-public", "1")
		.await;
	page.assert_status_ok();
	let text = page.text();
	assert!(text.contains("action=\"/api/_i/signin\""), "{text}");
	assert!(text.contains("<span class=brand>Stump</span>"), "{text}");
	assert!(!text.contains("you chose"), "{text}");
	assert!(text.contains("autocomplete=\"current-password\""));
	assert!(text.contains("autocomplete=\"username\""));
	assert!(!text.contains("<script"), "the page has no script");
	for (name, value) in [
		("cache-control", "no-store"),
		("referrer-policy", "no-referrer"),
		("x-frame-options", "DENY"),
	] {
		assert_eq!(page.header(name), value, "{name}");
	}

	// the name is not case-sensitive for the person typing it
	let response = page_signin(app, "ANA", &ana.password, None, "198.51.100.7").await;
	assert_eq!(
		response.status_code(),
		StatusCode::SEE_OTHER,
		"{}",
		response.text()
	);
	assert_eq!(response.header("location"), "/");
	let new_pass = cookie_in(&response, COOKIE).expect("the phone gets a pass");
	let new_session = cookie_in(&response, "stump_session").expect("and a session");
	assert_ne!(new_pass, ana.pass);

	let devices = devices_of(app, &ana.id).await;
	assert_eq!(devices.len(), 2);
	let made =
		pass::verify_pass(KEY, &new_pass, Utc::now().timestamp()).expect("a valid pass");
	let device = devices
		.iter()
		.find(|device| device.id == made.id)
		.expect("the pass is for a new device");
	assert_eq!(device.kind, "browser");
	assert_eq!(device.person_id.as_deref(), Some(ana.id.as_str()));
	assert_eq!(device.last_ip, "198.51.100.7");

	// what they were given works through the edge
	let me = friend_gql(app, &new_pass, &new_session, "{ me { username } }", None).await;
	assert_eq!(me.json::<Value>()["data"]["me"]["username"], "ana");

	// the owner's log has the sign-in, with the device it made
	assert!(
		eventually(|| async {
			sign_in_log(app)
				.await
				.iter()
				.any(|(name, ok, _)| name == "ana" && *ok)
		})
		.await
	);
}

/// Three devices and then the page says so after a right password, tells the owner, and makes
/// nothing; a wrong password at the limit is just a wrong password
#[tokio::test]
async fn the_fourth_device_is_refused_with_a_clear_message_and_the_owner_is_told() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_in_biblioteca(app, "ana", "203.0.113.5").await;
	for ip in ["198.51.100.1", "198.51.100.2"] {
		let response = page_signin(app, "ana", &ana.password, None, ip).await;
		assert_eq!(response.status_code(), StatusCode::SEE_OTHER);
	}
	assert_eq!(devices_of(app, &ana.id).await.len(), 3);
	// no warning yet beyond "at the limit"
	assert_eq!(
		warning_kinds(app).await,
		vec![("AT_DEVICE_LIMIT".to_string(), "ana".to_string())]
	);

	let refused = page_signin(app, "ana", &ana.password, None, "198.51.100.3").await;
	assert_eq!(refused.status_code(), StatusCode::FORBIDDEN);
	let text = refused.text();
	assert!(text.contains("You already use 3 devices"), "{text}");
	assert!(text.contains("My devices"), "{text}");
	assert!(cookie_in(&refused, COOKIE).is_none());
	assert!(cookie_in(&refused, "stump_session").is_none());
	assert_eq!(devices_of(app, &ana.id).await.len(), 3);

	// in Spanish for a browser that asks for it
	let spanish = form_post(app, "/api/_i/signin", "198.51.100.4", None)
		.add_header("accept-language", "es-EC,es;q=0.9")
		.form(&[("username", "ana"), ("password", ana.password.as_str())])
		.await;
	assert_eq!(spanish.status_code(), StatusCode::FORBIDDEN);
	assert!(spanish.text().contains("Ya usas 3 dispositivos"));

	// a wrong password at the limit says nothing about the limit
	let wrong = page_signin(app, "ana", WRONG, None, "198.51.100.5").await;
	assert_eq!(wrong.status_code(), StatusCode::UNAUTHORIZED);
	assert!(!wrong.text().contains("devices"));

	// the owner is told, and the log says why
	assert!(audit_actions(app)
		.await
		.contains(&"signin.device_limit".to_string()));
	let kinds = warning_kinds(app).await;
	assert!(
		kinds.contains(&("DEVICE_LIMIT_HIT".to_string(), "ana".to_string())),
		"{kinds:?}"
	);
	assert!(
		kinds.contains(&("AT_DEVICE_LIMIT".to_string(), "ana".to_string())),
		"{kinds:?}"
	);
	assert!(
		eventually(|| async {
			sign_in_log(app).await.iter().any(|(name, ok, reason)| {
				name == "ana" && !*ok && reason.as_deref() == Some("DEVICE_LIMIT")
			})
		})
		.await
	);

	// raising the limit lets the fourth in; lowering it below what they use is a warning, not a
	// device lost
	let raised = owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { maxDevices activeDevices } }",
		Some(json!({"i": {"personId": ana.id, "maxDevices": 4}})),
	)
	.await;
	assert_eq!(
		raised["data"]["setPersonAccess"]["maxDevices"], 4,
		"{raised:#}"
	);
	assert_eq!(
		page_signin(app, "ana", &ana.password, None, "198.51.100.6")
			.await
			.status_code(),
		StatusCode::SEE_OTHER
	);
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { maxDevices } }",
		Some(json!({"i": {"personId": ana.id, "maxDevices": 2}})),
	)
	.await;
	assert_eq!(devices_of(app, &ana.id).await.len(), 4);
	assert!(warning_kinds(app)
		.await
		.contains(&("OVER_DEVICE_LIMIT".to_string(), "ana".to_string())));
}

/// An unknown name, a wrong password, the owner, an account nobody enabled, a locked one, one
/// that holds more than a friend may and one with no password: the same status and the same page,
/// so the page says nothing about which names exist
#[tokio::test]
async fn everything_that_is_not_a_sign_in_gets_the_same_answer() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_in_biblioteca(app, "ana", "203.0.113.5").await;
	let shelves = shelves_of(app).await;
	let cara = sign_up_friend(app, "cara", &[&shelves], "203.0.113.6").await;
	let dora = sign_up_friend(app, "dora", &[&shelves], "203.0.113.7").await;
	let eve = sign_up_friend(app, "eve", &[&shelves], "203.0.113.8").await;
	let fay = sign_up_friend(app, "fay", &[&shelves], "203.0.113.9").await;
	insert_user(app, "ben", "secret-pass-1", false).await;

	// cara: the owner switched her off. dora: locked. eve: holds a permission a friend may not.
	// fay: no password at all (an account that signs in some other way)
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { publicSignin } }",
		Some(json!({"i": {"personId": cara.id, "publicSignin": false}})),
	)
	.await;
	let mut row = user_row(app, "dora").await.into_active_model();
	row.is_locked = Set(true);
	row.update(app.conn()).await.unwrap();
	set_permissions(app, "eve", "CHANGE_PASSWORD,MANAGE_USERS").await;
	let mut row = user_row(app, "fay").await.into_active_model();
	row.hashed_password = Set(String::new());
	row.update(app.conn()).await.unwrap();

	let attempts: Vec<(&str, String)> = vec![
		("ana", WRONG.to_string()),         // wrong password
		("nobody-here", WRONG.to_string()), // a name that does not exist
		("initial-server-admin", "password".to_string()), // the owner, with the right password
		("ben", "secret-pass-1".to_string()), // right password, never enabled
		("cara", cara.password.clone()),    // right password, switched off
		("dora", dora.password.clone()),    // right password, locked
		("eve", eve.password.clone()),      // right password, beyond a friend
		("fay", fay.password.clone()),      // nothing to check
	];
	let mut answers = Vec::new();
	for (n, (name, password)) in attempts.iter().enumerate() {
		let response =
			page_signin(app, name, password, None, &format!("198.51.100.{}", 10 + n))
				.await;
		assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED, "{name}");
		assert!(response.headers().get("set-cookie").is_none(), "{name}");
		answers.push((name.to_string(), normalised(&response.text(), name)));
	}
	assert!(
		answers.windows(2).all(|pair| pair[0].1 == pair[1].1),
		"the answers differ: {answers:#?}"
	);
	assert!(answers[0].1.contains("Wrong username or password."));

	// none of it locked anything, and nothing was made
	for name in ["ana", "ben", "cara", "eve", "fay", "initial-server-admin"] {
		assert!(
			!user_row(app, name).await.is_locked,
			"{name} was locked by the page"
		);
	}
	assert!(user_row(app, "dora").await.is_locked);
	assert_eq!(devices_of(app, &ana.id).await.len(), 1);

	// the owner can tell the right password at an account that is not allowed from a wrong one,
	// and that the owner's own password was never checked
	assert!(
		eventually(|| async {
			let log = sign_in_log(app).await;
			let has = |name: &str, reason: &str| {
				log.iter().any(|(who, ok, why)| {
					who == name && !*ok && why.as_deref() == Some(reason)
				})
			};
			has("ana", "WRONG_PASSWORD")
				&& has("initial-server-admin", "OWNER_VIA_EDGE")
				&& has("ben", "NOT_ALLOWED_PUBLICLY")
				&& has("cara", "NOT_ALLOWED_PUBLICLY")
				&& has("dora", "LOCKED")
				&& has("eve", "NOT_ALLOWED_PUBLICLY")
				&& has("fay", "WRONG_PASSWORD")
		})
		.await
	);
	// and the warnings say who tried an account that is not enabled
	let kinds = warning_kinds(app).await;
	for name in ["ben", "cara", "eve"] {
		assert!(
			kinds.contains(&("NOT_ENABLED_TRIED".to_string(), name.to_string())),
			"{name}: {kinds:?}"
		);
	}
}

async fn shelves_of(app: &TestApp) -> String {
	library::Entity::find()
		.one(app.conn())
		.await
		.unwrap()
		.expect("a library")
		.id
}

/// Wrong passwords from the internet never lock an account, however many: it is only slowed down
#[tokio::test]
async fn a_hundred_wrong_passwords_never_lock_an_account() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_in_biblioteca(app, "ana", "203.0.113.5").await;

	let mut statuses = Vec::new();
	for client in 0..20 {
		for attempt in 0..5 {
			let response = page_signin(
				app,
				"ana",
				&format!("{WRONG}{attempt}"),
				None,
				&format!("198.51.100.{}", 30 + client),
			)
			.await;
			statuses.push(response.status_code());
		}
	}
	assert_eq!(statuses.len(), 100);
	assert!(statuses
		.iter()
		.all(|status| *status == StatusCode::UNAUTHORIZED
			|| *status == StatusCode::TOO_MANY_REQUESTS));
	assert!(
		statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
		"the guard never answered"
	);

	settled(app).await;
	let row = user_row(app, "ana").await;
	assert!(!row.is_locked, "the page locked the account");
	let counted = models::services::access::count_lock_failures(
		app.conn(),
		&ana.id,
		(Utc::now() - chrono::Duration::hours(24)).into(),
		Utc::now().into(),
	)
	.await
	.unwrap();
	assert_eq!(
		counted, 0,
		"a failure from the page counts towards the lock"
	);
	// their devices go on working: a known device is not asked
	let me = friend_gql(app, &ana.pass, &ana.session, "{ me { username } }", None).await;
	assert_eq!(me.json::<Value>()["data"]["me"]["username"], "ana");
}

/// From the fifth failure a client is made to wait for that name; another client is not, and the
/// wait is the same page as every other refusal
#[tokio::test]
async fn failures_make_one_client_wait_and_nobody_else() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_in_biblioteca(app, "ana", "203.0.113.5").await;

	for attempt in 0..5 {
		let response = page_signin(
			app,
			"ana",
			&format!("{WRONG}{attempt}"),
			None,
			"198.51.100.50",
		)
		.await;
		assert_eq!(
			response.status_code(),
			StatusCode::UNAUTHORIZED,
			"attempt {attempt}"
		);
	}
	// the sixth, even with the right password, waits
	let waiting = page_signin(app, "ana", &ana.password, None, "198.51.100.50").await;
	assert_eq!(waiting.status_code(), StatusCode::TOO_MANY_REQUESTS);
	assert!(waiting.text().contains("Too many attempts"));
	assert!(waiting.headers().get("retry-after").is_some());
	assert!(waiting.headers().get("set-cookie").is_none());
	// for any other name from the same client there is no wait yet
	let other = page_signin(app, "ben", WRONG, None, "198.51.100.50").await;
	assert_eq!(other.status_code(), StatusCode::UNAUTHORIZED);

	// a friend on another client signs in at once
	let friend = page_signin(app, "ana", &ana.password, None, "198.51.100.51").await;
	assert_eq!(friend.status_code(), StatusCode::SEE_OTHER);

	// the owner is told once that a name was slowed down
	assert!(
		eventually(|| async {
			audit_actions(app)
				.await
				.contains(&"signin.backoff".to_string())
		})
		.await
	);
	assert!(warning_kinds(app)
		.await
		.contains(&("NAME_BACKOFF".to_string(), "ana".to_string())));
}

/// Failures from enough clients take the whole page down for a few minutes, and the owner is
/// alerted; a client past its own cap cannot do it alone
#[tokio::test]
async fn a_flood_trips_the_global_budget_and_the_owner_is_alerted() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_in_biblioteca(app, "ana", "203.0.113.5").await;

	// one client alone is stopped at its own cap
	for attempt in 0..30 {
		page_signin(
			app,
			&format!("guess-{attempt}"),
			WRONG,
			None,
			"198.51.100.60",
		)
		.await;
	}
	assert_eq!(
		page_signin(app, "ana", &ana.password, None, "198.51.100.99")
			.await
			.status_code(),
		StatusCode::SEE_OTHER,
		"one client brought the page down"
	);

	// four more, ten failures each, do
	for client in 61..65 {
		for attempt in 0..10 {
			page_signin(
				app,
				&format!("guess-{client}-{attempt}"),
				WRONG,
				None,
				&format!("198.51.100.{client}"),
			)
			.await;
		}
	}
	let everybody = page_signin(app, "ana", &ana.password, None, "198.51.100.98").await;
	assert_eq!(everybody.status_code(), StatusCode::TOO_MANY_REQUESTS);

	assert!(
		eventually(|| async {
			audit_actions(app)
				.await
				.contains(&"signin.global_budget".to_string())
		})
		.await
	);
	let warnings = owner_gql(
		app,
		"{ accessWarnings { kind severity } accessWarningCount }",
		None,
	)
	.await;
	let found = warnings["data"]["accessWarnings"].as_array().unwrap();
	assert!(
		found
			.iter()
			.any(|w| w["kind"] == "GLOBAL_BUDGET" && w["severity"] == "ALERT"),
		"{warnings:#}"
	);
	assert!(warnings["data"]["accessWarningCount"].as_i64().unwrap() >= 1);
	// failures on names that do not exist are in the audit log too, as one aggregated row
	assert!(
		eventually(|| async {
			audit_actions(app)
				.await
				.contains(&"signin.unknown_names".to_string())
		})
		.await
	);
	// and the owner's page shows the counters, numbers only
	let overview = owner_gql(
		app,
		"{ accessOverview { publicSignin signinStats { failuresLast10Min namesWaiting globalBudgetTripped } } }",
		None,
	)
	.await;
	assert_eq!(overview["data"]["accessOverview"]["publicSignin"], true);
	assert_eq!(
		overview["data"]["accessOverview"]["signinStats"]["globalBudgetTripped"],
		true
	);
}

/// A pass that is already in the browser is never replaced by a sign-in, whoever's it is
#[tokio::test]
async fn a_sign_in_with_a_pass_makes_no_device_and_replaces_nothing() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_in_biblioteca(app, "ana", "203.0.113.5").await;
	let ben_pass = pass_for("deploy-old");

	for (pass, ip) in [
		(ana.pass.as_str(), "198.51.100.70"),
		(ben_pass.as_str(), "198.51.100.71"),
	] {
		let response = page_signin(app, "ana", &ana.password, Some(pass), ip).await;
		assert_eq!(response.status_code(), StatusCode::SEE_OTHER);
		assert!(
			cookie_in(&response, COOKIE).is_none(),
			"the pass was replaced"
		);
		assert!(cookie_in(&response, "stump_session").is_some());
	}
	assert_eq!(devices_of(app, &ana.id).await.len(), 1);
}

/// The page asks the same questions of a request that is not from this site
#[tokio::test]
async fn a_request_from_another_site_or_too_big_gets_the_generic_answer_with_no_work() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_in_biblioteca(app, "ana", "203.0.113.5").await;

	let cross = app
		.server
		.post("/api/_i/signin")
		.add_header("x-edge-public", "1")
		.add_header("sec-fetch-site", "cross-site")
		.form(&[("username", "ana"), ("password", ana.password.as_str())])
		.await;
	assert_eq!(cross.status_code(), StatusCode::UNAUTHORIZED);
	assert!(cross.headers().get("set-cookie").is_none());

	let huge = "x".repeat(5_000);
	for (username, password) in [
		("ana", huge.as_str()),
		(huge.as_str(), "x"),
		("", "x"),
		("ana", ""),
	] {
		let response = page_signin(app, username, password, None, "198.51.100.80").await;
		assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
	}
	// none of that was counted as a failure: the client can still sign in
	let ok = page_signin(app, "ana", &ana.password, None, "198.51.100.80").await;
	assert_eq!(ok.status_code(), StatusCode::SEE_OTHER);
	// not a form at all
	let not_a_form = app
		.server
		.post("/api/_i/signin")
		.add_header("x-edge-public", "1")
		.add_header("sec-fetch-site", "same-origin")
		.json(&json!({"username": "ana"}))
		.await;
	assert_eq!(not_a_form.status_code(), StatusCode::UNAUTHORIZED);
}

/// The web app's own sign-in, which a friend's known device uses, follows the same rules
#[tokio::test]
async fn the_web_apps_sign_in_follows_the_same_rules_with_public_sign_in_on() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_in_biblioteca(app, "ana", "203.0.113.5").await;
	let cara = {
		let shelf = shelves_of(app).await;
		sign_up_friend(app, "cara", &[&shelf], "203.0.113.6").await
	};
	insert_user(app, "ben", "secret-pass-1", false).await;
	let legacy = pass_for("deploy-old");

	// her own device: as before
	let own = public_login(app, "ana", &ana.password, Some(&ana.pass)).await;
	assert_eq!(own.status_code(), StatusCode::OK, "{}", own.text());

	// an account nobody enabled: the answer of a wrong password, recorded as such, never a lock
	let ben = public_login(app, "ben", "secret-pass-1", Some(&legacy)).await;
	assert_eq!(ben.status_code(), StatusCode::UNAUTHORIZED);
	assert!(!user_row(app, "ben").await.is_locked);

	// somebody's pass that is not hers, or one the owner made in the terminal: she signs in through
	// the gates, and no device is made, and nothing is replaced
	for pass in [&legacy, &cara.pass] {
		let response = public_login(app, "ana", &ana.password, Some(pass)).await;
		assert_eq!(
			response.status_code(),
			StatusCode::OK,
			"{}",
			response.text()
		);
		assert!(cookie_in(&response, COOKIE).is_none());
	}
	assert_eq!(devices_of(app, &ana.id).await.len(), 1);
	// …and a wrong password through such a pass is a failure of the gates, never of the lock
	for _ in 0..12 {
		let response = public_login(app, "ana", WRONG, Some(&legacy)).await;
		assert!(
			response.status_code() == StatusCode::UNAUTHORIZED,
			"{}",
			response.status_code()
		);
	}
	settled(app).await;
	assert!(!user_row(app, "ana").await.is_locked);
	let counted = models::services::access::count_lock_failures(
		app.conn(),
		&ana.id,
		(Utc::now() - chrono::Duration::hours(24)).into(),
		Utc::now().into(),
	)
	.await
	.unwrap();
	assert_eq!(counted, 0);

	// the owner is still refused here, whatever the pass
	let owner =
		public_login(app, "initial-server-admin", "password", Some(&ana.pass)).await;
	assert_eq!(owner.status_code(), StatusCode::UNAUTHORIZED);
}

/// A locked account that types its right password into the web app is recorded as locked, as the
/// sign-in page records it, and not as "not allowed from the internet": that would send the owner to
/// turn a locked account on in People
#[tokio::test]
async fn the_web_app_records_a_locked_account_as_locked() {
	let env = signin_app().await;
	let app = &env.app;
	let dora = friend_in_biblioteca(app, "dora", "203.0.113.7").await;
	let cara = {
		let shelf = shelves_of(app).await;
		sign_up_friend(app, "cara", &[&shelf], "203.0.113.6").await
	};
	// cara: the owner switched her off. dora: locked
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { publicSignin } }",
		Some(json!({"i": {"personId": cara.id, "publicSignin": false}})),
	)
	.await;
	let mut row = user_row(app, "dora").await.into_active_model();
	row.is_locked = Set(true);
	row.update(app.conn()).await.unwrap();

	// both typed the right password, from a device that has a pass: both are refused like a wrong one
	let legacy = pass_for("deploy-old");
	for (name, password) in [
		("dora", dora.password.as_str()),
		("cara", cara.password.as_str()),
	] {
		let response = public_login(app, name, password, Some(&legacy)).await;
		assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED, "{name}");
	}
	assert!(
		eventually(|| async {
			let log = sign_in_log(app).await;
			let has = |name: &str, reason: &str| {
				log.iter().any(|(who, ok, why)| {
					who == name && !*ok && why.as_deref() == Some(reason)
				})
			};
			has("dora", "LOCKED") && has("cara", "NOT_ALLOWED_PUBLICLY")
		})
		.await
	);

	// so the owner is told to look at cara, and not at dora
	let kinds = warning_kinds(app).await;
	assert!(
		kinds.contains(&("NOT_ENABLED_TRIED".to_string(), "cara".to_string())),
		"{kinds:?}"
	);
	assert!(
		!kinds.contains(&("NOT_ENABLED_TRIED".to_string(), "dora".to_string())),
		"{kinds:?}"
	);
}

/// The sign-ins of a person are in `user_login_activity`; nothing that was typed is stored
#[tokio::test]
async fn no_password_is_ever_stored() {
	let env = signin_app().await;
	let app = &env.app;
	let ana = friend_in_biblioteca(app, "ana", "203.0.113.5").await;
	page_signin(app, "ana", WRONG, None, "198.51.100.90").await;
	page_signin(app, "ana", &ana.password, None, "198.51.100.91").await;
	assert!(
		eventually(|| async {
			user_login_activity::Entity::find()
				.all(app.conn())
				.await
				.unwrap()
				.len() >= 3
		})
		.await
	);
	let everything = format!(
		"{:?}{:?}{:?}",
		user_login_activity::Entity::find()
			.all(app.conn())
			.await
			.unwrap(),
		access_audit::Entity::find().all(app.conn()).await.unwrap(),
		devices_of(app, &ana.id).await,
	);
	for secret in [WRONG, ana.password.as_str()] {
		assert!(!everything.contains(secret), "a typed password was stored");
	}
}
