//! The web app's own sign-in screen on the public address: the shell and its files are served
//! with no pass (and only those), the sign-in and signup are JSON, and everything else stays
//! closed. The edge says it serves this screen with the capability header `X-Edge-Login: spa`.

use axum::http::{Method, StatusCode};
use axum_test::TestRequest;
use chrono::Utc;
use models::services::access as service;
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};

use super::*;

/// What the fake web build says, so a test can tell the shell from any other answer
const SHELL: &str = "<div id=\"root\">THE-SHELL</div>";

struct SpaApp {
	env: EdgeApp,
	_client: TempDir,
}

/// An app with a small fake web build and the switch on or off
async fn spa_app(public_signin: bool) -> SpaApp {
	spa_app_with(public_signin, true).await
}

/// Also with `require_pass` as given: `false` is how a rollout passes through, and the pass-less
/// sign-in must be safe then too
async fn spa_app_with(public_signin: bool, require_pass: bool) -> SpaApp {
	let client = tempfile::tempdir().expect("should create a temp dir");
	let root = client.path();
	std::fs::create_dir_all(root.join("assets/fonts")).unwrap();
	std::fs::write(
		root.join("index.html"),
		format!(
			"<!doctype html><head><script>window.theme=1</script><script type=\"module\" src=\"/assets/index-AAAA.js\"></script></head><body>{SHELL}</body>"
		),
	)
	.unwrap();
	std::fs::write(root.join("assets/x.js"), "console.log(1)").unwrap();
	std::fs::write(root.join("assets/x.js.map"), "{\"secret\":\"map\"}").unwrap();
	std::fs::write(root.join("assets/fonts/y.woff2"), "font").unwrap();
	std::fs::write(root.join("sw.js"), "// worker").unwrap();

	let state_dir = tempfile::tempdir().expect("should create a temp dir");
	let mut config = StumpConfig::debug();
	config.password_hash_cost = 4;
	config.trust_proxy_headers = true;
	config.client_dir = root.to_string_lossy().to_string();
	let mut edge = edge_settings(&state_dir, require_pass);
	edge.public_signin = public_signin;
	config.edge = Some(edge);

	let app = TestApp::with_config(config).await;
	let token = app.create_initial_account().await;
	*app.access_token.write().await = Some(token);
	SpaApp {
		env: EdgeApp { app, state_dir },
		_client: client,
	}
}

/// A request that came through the public edge, which serves the web app's sign-in screen
fn via_spa_edge(app: &TestApp, method: Method, path: &str) -> TestRequest {
	app.server
		.method(method, path)
		.add_header("x-edge-public", "1")
		.add_header("x-edge-login", "spa")
}

fn csp_of(response: &TestResponse) -> String {
	response
		.headers()
		.get("content-security-policy")
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default()
		.to_string()
}

/// With the switch on and the edge's header, a visitor with no pass gets the shell of the web
/// app and its files, under a policy that allows this site and the page's own scripts only
#[tokio::test]
async fn the_shell_and_its_files_open_without_a_pass_under_a_policy() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;

	for path in ["/", "/auth", "/join", "/?x=1", "/auth?redirect=%2Fbooks"] {
		let page = via_spa_edge(app, Method::GET, path).await;
		page.assert_status_ok();
		assert!(page.text().contains("THE-SHELL"), "{path}");
		let csp = csp_of(&page);
		assert!(csp.contains("default-src 'none'"), "{path}: {csp}");
		assert!(csp.contains("'sha256-"), "{path}: {csp}");
		assert!(csp.contains("frame-ancestors 'none'"), "{path}: {csp}");
		assert!(!csp.contains("unsafe-eval"), "{path}: {csp}");
		assert_eq!(
			page.headers()
				.get("cache-control")
				.and_then(|value| value.to_str().ok()),
			Some("no-store"),
			"{path}"
		);
		assert_eq!(header_of(&page, "x-robots-tag"), "noindex", "{path}");
	}
	let head = via_spa_edge(app, Method::HEAD, "/").await;
	head.assert_status_ok();
	assert!(csp_of(&head).contains("script-src 'self'"));

	// the build's own files, flat, of a known kind
	let script = via_spa_edge(app, Method::GET, "/assets/x.js").await;
	script.assert_status_ok();
	assert!(script.text().contains("console.log"));
	assert_eq!(csp_of(&script), "", "a file carries no policy of its own");

	// and nothing else of the build or the app
	for (method, path) in [
		(Method::GET, "/assets/x.js.map"),
		(Method::GET, "/assets/fonts/y.woff2"),
		(Method::GET, "/assets/x.js?v=1"),
		(Method::GET, "/assets/x.JS"),
		(Method::GET, "/assets/../index.html"),
		(Method::POST, "/assets/x.js"),
		(Method::POST, "/"),
		(Method::GET, "/sw.js"),
		(Method::GET, "/index.html"),
		(Method::GET, "/api/graphql"),
		(Method::GET, "/api/v2/auth/viewer"),
		(Method::GET, "/api/v2/ping"),
		(Method::GET, "/settings"),
		(Method::GET, "/auth/"),
		(Method::GET, "/Auth"),
	] {
		let refused = via_spa_edge(app, method.clone(), path).await;
		assert_eq!(
			refused.status_code(),
			StatusCode::NOT_FOUND,
			"{method} {path}"
		);
		assert!(!refused.text().contains("THE-SHELL"), "{method} {path}");
		assert!(!refused.text().contains("secret"), "{method} {path}");
	}
}

/// The edge's header and the switch are both needed: an older edge sends no header (the
/// server-made pages stay in charge), and with the switch off nothing of the screen opens, but
/// the front door still says what to do
#[tokio::test]
async fn without_the_header_or_the_switch_none_of_it_opens() {
	// the switch is off
	let off = spa_app(false).await;
	let app = &off.env.app;
	let front = via_spa_edge(app, Method::GET, "/").await;
	front.assert_status_ok();
	assert!(!front.text().contains("THE-SHELL"));
	assert!(front.text().contains("Private site"), "{}", front.text());
	assert_eq!(csp_of(&front).contains("script-src 'self' 'sha256-"), false);
	for path in ["/auth", "/join", "/assets/x.js"] {
		via_spa_edge(app, Method::GET, path)
			.await
			.assert_status(StatusCode::NOT_FOUND);
	}
	via_spa_edge(app, Method::POST, "/api/v2/auth/login")
		.json(&json!({"username": "a", "password": "b"}))
		.await
		.assert_status(StatusCode::NOT_FOUND);

	// the switch is on, but the edge is an older one (no header)
	let on = spa_app(true).await;
	let app = &on.env.app;
	for path in ["/auth", "/join", "/assets/x.js"] {
		app.server
			.get(path)
			.add_header("x-edge-public", "1")
			.await
			.assert_status(StatusCode::NOT_FOUND);
	}
	let front = app.server.get("/").add_header("x-edge-public", "1").await;
	assert!(!front.text().contains("THE-SHELL"));

	// a header that is not "spa" is no capability
	app.server
		.get("/auth")
		.add_header("x-edge-public", "1")
		.add_header("x-edge-login", "yes")
		.await
		.assert_status(StatusCode::NOT_FOUND);

	// not through the edge at all (home): the shell is just the shell, no policy
	let home = app.server.get("/auth").await;
	home.assert_status_ok();
	assert!(home.text().contains("THE-SHELL"));
	assert_eq!(csp_of(&home), "");
}

/// A visitor with a pass gets the shell as always: no policy, so the signed-in app runs free
#[tokio::test]
async fn a_valid_pass_gets_the_shell_without_the_pass_less_policy() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;

	let shell = via_spa_edge(app, Method::GET, "/")
		.add_header("x-edge-pass", ana.pass.clone())
		.await;
	shell.assert_status_ok();
	assert!(shell.text().contains("THE-SHELL"));
	assert_eq!(csp_of(&shell), "");
	assert_eq!(header_of(&shell, "x-robots-tag"), "");
	assert_ne!(
		shell
			.headers()
			.get("cache-control")
			.and_then(|value| value.to_str().ok()),
		Some("no-store")
	);
	// and its files, and the service worker, as a signed-in friend always could
	via_spa_edge(app, Method::GET, "/sw.js")
		.add_header("x-edge-pass", ana.pass.clone())
		.await
		.assert_status_ok();
}

/// A pass the app has just revoked is no pass at all on the sign-in screen: the person who removed
/// a device must be able to sign in again without a "turned off" page in between. Everywhere else
/// it is refused, as before
#[tokio::test]
async fn a_revoked_pass_is_no_pass_on_the_sign_in_screen() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	let device = devices_of(app, &ana.id).await.pop().expect("a device");
	service::revoke_device(app.conn(), Utc::now(), &device.id, "owner")
		.await
		.expect("revoked");

	let shell = via_spa_edge(app, Method::GET, "/auth")
		.add_header("x-edge-pass", ana.pass.clone())
		.await;
	shell.assert_status_ok();
	assert!(shell.text().contains("THE-SHELL"));
	assert!(
		csp_of(&shell).contains("default-src 'none'"),
		"treated as no pass: the policy applies"
	);
	via_spa_edge(app, Method::GET, "/assets/x.js")
		.add_header("x-edge-pass", ana.pass.clone())
		.await
		.assert_status_ok();

	let elsewhere = via_spa_edge(app, Method::GET, "/api/v2/ping")
		.add_header("x-edge-pass", ana.pass.clone())
		.await;
	assert_eq!(elsewhere.status_code(), StatusCode::FORBIDDEN);
}

/// The sign-in screen asks the server whether it is claimed: through the edge it is told the
/// request is public, at home it is not
#[tokio::test]
async fn the_claim_check_says_public_only_through_the_edge() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;

	let public = via_spa_edge(app, Method::GET, "/api/v2/claim")
		.add_header("x-edge-pass", ana.pass.clone())
		.await
		.json::<Value>();
	assert_eq!(public["isClaimed"], true, "{public:#}");
	assert_eq!(public["isPublic"], true, "{public:#}");

	let home = app.server.get("/api/v2/claim").await.json::<Value>();
	assert_eq!(home["isClaimed"], true, "{home:#}");
	assert_eq!(home["isPublic"], false, "{home:#}");
}

const WRONG: &str = "not-the-password-1";
const PASSWORD: &str = "tulip-Harbor-47";
/// What every refusal of a sign-in says, byte for byte (the edge says the same for the viewer)
const UNAUTHORIZED: &str = r#"{"status":401,"message":"Unauthorized"}"#;

/// What the web app's screen sends: a JSON `POST` from this site, through the edge that serves
/// it, from `ip`, with no pass
fn json_post(app: &TestApp, path: &str, ip: &str) -> TestRequest {
	app.server
		.post(path)
		.add_header("x-edge-public", "1")
		.add_header("x-edge-login", "spa")
		.add_header("x-real-ip", ip.to_string())
		.add_header("sec-fetch-site", "same-origin")
}

async fn json_login(
	app: &TestApp,
	username: &str,
	password: &str,
	ip: &str,
) -> TestResponse {
	json_post(app, "/api/v2/auth/login", ip)
		.json(&json!({ "username": username, "password": password }))
		.await
}

async fn json_join(
	app: &TestApp,
	token: &str,
	username: &str,
	password: &str,
	ip: &str,
) -> TestResponse {
	json_post(app, "/api/v2/auth/join", ip)
		.json(&json!({ "token": token, "username": username, "password": password }))
		.await
}

fn header_of(response: &TestResponse, name: &str) -> String {
	response
		.headers()
		.get(name)
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default()
		.to_string()
}

async fn friend(app: &TestApp, name: &str, ip: &str) -> Friend {
	let shelves = shelves(app).await;
	sign_up_friend(app, name, &[&shelves.biblioteca.id], ip).await
}

/// A friend with one device opens the address on another phone, signs in with the screen's JSON
/// call, and that phone gets a pass of its own, a session and the user
#[tokio::test]
async fn a_friend_signs_in_with_json_and_the_new_device_gets_a_pass() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let ana = friend(app, "ana", "203.0.113.5").await;
	let before: Vec<String> = devices_of(app, &ana.id)
		.await
		.into_iter()
		.map(|device| device.id)
		.collect();
	assert_eq!(before.len(), 1);

	let response = json_login(app, "ana", &ana.password, "198.51.100.1").await;
	response.assert_status_ok();
	let body = response.json::<Value>();
	assert_eq!(body["username"], "ana", "{body:#}");
	assert!(body.get("accessToken").is_none(), "{body:#}");
	assert_eq!(header_of(&response, "cache-control"), "no-store");

	// the pass verifies, and belongs to a NEW device of this person
	let pass = cookie_in(&response, COOKIE).expect("a pass");
	let verified =
		pass::verify_pass(KEY, &pass, Utc::now().timestamp()).expect("a valid pass");
	let devices = devices_of(app, &ana.id).await;
	assert_eq!(devices.len(), 2);
	let new = devices
		.iter()
		.find(|device| !before.contains(&device.id))
		.expect("a new device");
	assert_eq!(verified.id, new.id);
	assert!(cookie_in(&response, "stump_session").is_some());
	assert_eq!(sessions_of(app, &ana.id).await, 2);
	assert!(
		eventually(|| async {
			sign_in_log(app)
				.await
				.iter()
				.any(|(who, ok, _)| who == "ana" && *ok)
		})
		.await
	);

	// the sign-in takes no query (the screen sends none): `?generate_token=true` is not on the
	// open list, so without a pass it is a 404 like any other closed address
	let with_query = json_post(
		app,
		"/api/v2/auth/login?generate_token=true",
		"198.51.100.2",
	)
	.json(&json!({"username": "ana", "password": ana.password}))
	.await;
	with_query.assert_status(StatusCode::NOT_FOUND);
	assert!(with_query.headers().get("set-cookie").is_none());
}

/// Were a query ever to get through (a rollout where passes are not required), it is ignored: a
/// browser behind the edge gets a session and a pass, never a token
#[tokio::test]
async fn a_token_cannot_be_asked_for_by_a_pass_less_sign_in() {
	let spa = spa_app_with(true, false).await;
	let app = &spa.env.app;
	let ana = friend(app, "ana", "203.0.113.5").await;
	let response = json_post(
		app,
		"/api/v2/auth/login?generate_token=true&create_session=false",
		"198.51.100.2",
	)
	.json(&json!({"username": "ana", "password": ana.password}))
	.await;
	response.assert_status_ok();
	let body = response.json::<Value>();
	assert_eq!(body["username"], "ana", "{body:#}");
	assert!(body.get("accessToken").is_none(), "{body:#}");
	assert!(cookie_in(&response, "stump_session").is_some());
	assert!(cookie_in(&response, COOKIE).is_some());
}

/// Everything that is not a sign-in gets the same bytes, with no cookie: the owner's session
/// cookie (the browser sends cookies to every port of the name) is never touched
#[tokio::test]
async fn everything_that_is_not_a_sign_in_gets_the_same_bytes_and_no_cookie() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	let cara =
		sign_up_friend(app, "cara", &[&shelves.biblioteca.id], "203.0.113.6").await;
	let dora =
		sign_up_friend(app, "dora", &[&shelves.biblioteca.id], "203.0.113.7").await;
	let eve = sign_up_friend(app, "eve", &[&shelves.biblioteca.id], "203.0.113.8").await;
	let fay = sign_up_friend(app, "fay", &[&shelves.biblioteca.id], "203.0.113.9").await;
	insert_user(app, "ben", "secret-pass-1", false).await;

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

	// the owner's real session at home: the browser sends it to every port of the name
	let at_home = app
		.server
		.post("/api/v2/auth/login")
		.json(&json!({"username": "initial-server-admin", "password": "password"}))
		.await;
	at_home.assert_status_ok();
	let owners_session =
		cookie_in(&at_home, "stump_session").expect("the owner's session");

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
	for (n, (name, password)) in attempts.iter().enumerate() {
		let response =
			json_post(app, "/api/v2/auth/login", &format!("198.51.100.{}", 10 + n))
				.add_header("cookie", format!("stump_session={owners_session}"))
				.json(&json!({"username": name, "password": password}))
				.await;
		assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED, "{name}");
		assert_eq!(response.text(), UNAUTHORIZED, "{name}");
		assert!(response.headers().get("set-cookie").is_none(), "{name}");
		assert_eq!(header_of(&response, "cache-control"), "no-store", "{name}");
	}

	// the owner is still signed in at home: no refusal touched the session
	app.server
		.get("/api/v2/auth/viewer")
		.add_header("cookie", format!("stump_session={owners_session}"))
		.await
		.assert_status_ok();

	// none of it locked anything, and nothing was made
	for name in ["ana", "ben", "cara", "eve", "fay", "initial-server-admin"] {
		assert!(!user_row(app, name).await.is_locked, "{name} was locked");
	}
	assert_eq!(devices_of(app, &ana.id).await.len(), 1);
	// the owner can still tell why, from the sign-in log
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
				&& has("dora", "LOCKED")
				&& has("eve", "NOT_ALLOWED_PUBLICLY")
		})
		.await
	);
}

/// The page and the JSON sign-in are one budget: failures through either wait the same client,
/// and a friend on another client signs in at once
#[tokio::test]
async fn the_page_and_the_json_sign_in_share_one_budget() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let ana = friend(app, "ana", "203.0.113.5").await;
	let ip = "198.51.100.50";

	for attempt in 0..5 {
		let wrong = format!("{WRONG}{attempt}");
		let response = if attempt % 2 == 0 {
			json_login(app, "ana", &wrong, ip).await
		} else {
			page_signin(app, "ana", &wrong, None, ip).await
		};
		assert_eq!(
			response.status_code(),
			StatusCode::UNAUTHORIZED,
			"attempt {attempt}"
		);
	}
	// the next, even with the right password, waits: through either door
	let waiting = json_login(app, "ana", &ana.password, ip).await;
	assert_eq!(waiting.status_code(), StatusCode::TOO_MANY_REQUESTS);
	assert!(
		waiting.text().contains("Too many attempts"),
		"{}",
		waiting.text()
	);
	assert!(waiting.text().starts_with(r#"{"status":429,"message":""#));
	assert_eq!(header_of(&waiting, "retry-after"), "300");
	assert!(waiting.headers().get("set-cookie").is_none());
	let waiting_page = page_signin(app, "ana", &ana.password, None, ip).await;
	assert_eq!(waiting_page.status_code(), StatusCode::TOO_MANY_REQUESTS);
	// a friend on another client signs in at once
	json_login(app, "ana", &ana.password, "198.51.100.51")
		.await
		.assert_status_ok();
	// nothing was locked
	assert!(!user_row(app, "ana").await.is_locked);
}

/// A right password at the device limit is told clearly (in the browser's language), the owner is
/// told, and no pass or session is given
#[tokio::test]
async fn the_fourth_device_is_refused_in_json_and_the_owner_is_told() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let ana = friend(app, "ana", "203.0.113.5").await;
	for ip in ["198.51.100.1", "198.51.100.2"] {
		json_login(app, "ana", &ana.password, ip)
			.await
			.assert_status_ok();
	}
	assert_eq!(devices_of(app, &ana.id).await.len(), 3);

	let refused = json_login(app, "ana", &ana.password, "198.51.100.3").await;
	assert_eq!(refused.status_code(), StatusCode::FORBIDDEN);
	let message = refused.json::<Value>()["message"]
		.as_str()
		.unwrap_or_default()
		.to_string();
	assert!(message.contains("You already use 3 devices"), "{message}");
	assert!(message.contains("My devices"), "{message}");
	assert!(refused.headers().get("set-cookie").is_none());
	assert_eq!(devices_of(app, &ana.id).await.len(), 3);

	let spanish = json_post(app, "/api/v2/auth/login", "198.51.100.4")
		.add_header("accept-language", "es-EC,es;q=0.9")
		.json(&json!({"username": "ana", "password": ana.password}))
		.await;
	assert_eq!(spanish.status_code(), StatusCode::FORBIDDEN);
	assert!(
		spanish.text().contains("Ya usas 3 dispositivos"),
		"{}",
		spanish.text()
	);

	// a wrong password at the limit says nothing about the limit
	let wrong = json_login(app, "ana", WRONG, "198.51.100.5").await;
	assert_eq!(wrong.text(), UNAUTHORIZED);

	assert!(audit_actions(app)
		.await
		.contains(&"signin.device_limit".to_string()));
	assert!(warning_kinds(app)
		.await
		.contains(&("DEVICE_LIMIT_HIT".to_string(), "ana".to_string())));
}

/// A request another site made the browser send, and one too big to be a sign-in, get the generic
/// answer with no work: nothing is counted against the client
#[tokio::test]
async fn a_cross_site_or_oversized_sign_in_gets_the_generic_answer_with_no_work() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let ana = friend(app, "ana", "203.0.113.5").await;

	let cross = app
		.server
		.post("/api/v2/auth/login")
		.add_header("x-edge-public", "1")
		.add_header("x-edge-login", "spa")
		.add_header("x-real-ip", "198.51.100.80")
		.add_header("sec-fetch-site", "cross-site")
		.json(&json!({"username": "ana", "password": ana.password}))
		.await;
	assert_eq!(cross.text(), UNAUTHORIZED);
	assert!(cross.headers().get("set-cookie").is_none());

	let huge = "x".repeat(5_000);
	for (username, password) in [
		("ana", huge.as_str()),
		(huge.as_str(), "x"),
		("", "x"),
		("ana", ""),
		("   ", "x"),
	] {
		let response = json_login(app, username, password, "198.51.100.80").await;
		assert_eq!(response.text(), UNAUTHORIZED);
	}
	// none of that was counted: the same client signs in
	json_login(app, "ana", &ana.password, "198.51.100.80")
		.await
		.assert_status_ok();
	// not JSON at all: refused before any work too (and a form from another site can't send JSON)
	let not_json = json_post(app, "/api/v2/auth/login", "198.51.100.81")
		.text("username=ana&password=x")
		.await;
	assert!(
		not_json.status_code().is_client_error(),
		"{}",
		not_json.status_code()
	);
	assert!(not_json.headers().get("set-cookie").is_none());
}

/// A pass-less sign-in through the edge reaches the sign-in that counts failures towards a lock
/// only if nothing stops it. Without the edge's header or with the switch off it is a 404 before
/// any work, even when the server does not insist on passes (a rollout does that for a while)
#[tokio::test]
async fn without_the_header_or_the_switch_a_pass_less_sign_in_is_a_404_and_locks_nobody()
{
	for (public_signin, spa_header) in [(true, false), (false, true), (false, false)] {
		let spa = spa_app_with(public_signin, false).await;
		let app = &spa.env.app;
		// the account is made through the server-made form, which answers with the switch on only
		let ana = if public_signin {
			friend(app, "ana", "203.0.113.5").await
		} else {
			let id = insert_user(app, "ana", PASSWORD, false).await.id;
			Friend {
				id,
				password: PASSWORD.to_string(),
				pass: String::new(),
				session: String::new(),
			}
		};
		for n in 0..15 {
			let mut request = app
				.server
				.post("/api/v2/auth/login")
				.add_header("x-edge-public", "1")
				.add_header("x-real-ip", format!("198.51.100.{}", 100 + n))
				.add_header("sec-fetch-site", "same-origin");
			if spa_header {
				request = request.add_header("x-edge-login", "spa");
			}
			let response = request
				.json(&json!({"username": "ana", "password": format!("{WRONG}{n}")}))
				.await;
			assert_eq!(
				response.status_code(),
				StatusCode::NOT_FOUND,
				"switch {public_signin}, header {spa_header}, attempt {n}"
			);
		}
		assert!(!user_row(app, "ana").await.is_locked);
		let failures = sign_in_log(app)
			.await
			.into_iter()
			.filter(|(who, ok, _)| who == "ana" && !*ok)
			.count();
		assert_eq!(failures, 0, "nothing was done for a 404");
		// at home it is the sign-in it always was
		let home = app
			.server
			.post("/api/v2/auth/login")
			.json(&json!({"username": "ana", "password": ana.password}))
			.await;
		home.assert_status_ok();
	}
}

/// A request that holds a pass of its own is not the pass-less sign-in: it is the known device's
/// sign-in as before, and makes no device
#[tokio::test]
async fn a_known_device_signs_in_the_way_it_always_did() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let ana = friend(app, "ana", "203.0.113.5").await;
	let response = json_post(app, "/api/v2/auth/login", "198.51.100.1")
		.add_header("x-edge-pass", ana.pass.clone())
		.json(&json!({"username": "ana", "password": ana.password}))
		.await;
	response.assert_status_ok();
	assert!(cookie_in(&response, COOKIE).is_none());
	assert_eq!(devices_of(app, &ana.id).await.len(), 1);
}

/// A visitor makes their own account from a link with the screen's JSON call: the account, the
/// preset, the first device, the session and the pass, and one use of the link spent
#[tokio::test]
async fn a_visitor_makes_an_account_with_json() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let shelves = shelves(app).await;
	let (link_id, url) =
		make_signup_link(app, "SINGLE", 1, &[&shelves.biblioteca.id]).await;
	let token = token_of(&url);

	let response = json_join(app, token, "Ana", PASSWORD, "203.0.113.5").await;
	assert_eq!(
		response.status_code(),
		StatusCode::OK,
		"{}",
		response.text()
	);
	assert_eq!(response.json::<Value>()["username"], "ana");
	assert_eq!(header_of(&response, "cache-control"), "no-store");
	let pass = cookie_in(&response, COOKIE).expect("a pass");
	assert!(pass::verify_pass(KEY, &pass, Utc::now().timestamp()).is_some());
	assert!(cookie_in(&response, "stump_session").is_some());

	let account = user_row(app, "ana").await;
	assert_eq!(devices_of(app, &account.id).await.len(), 1);
	assert_eq!(link_row(app, &link_id).await.uses, 1);
	assert_eq!(
		hidden_libraries_of(app, &account.id).await,
		vec![shelves.manga.id.clone()],
		"only Biblioteca is shown"
	);
	let mut permissions: Vec<&str> = account
		.permissions
		.as_deref()
		.unwrap_or_default()
		.split(',')
		.collect();
	permissions.sort_unstable();
	assert_eq!(
		permissions,
		[
			"ADD_BOOKS",
			"CHANGE_AVATAR",
			"CHANGE_PASSWORD",
			"DOWNLOAD_FILE"
		]
	);
	assert_eq!(sessions_of(app, &account.id).await, 1);

	// the link was single-use
	let again = json_join(app, token, "bea", PASSWORD, "203.0.113.6").await;
	assert_eq!(again.status_code(), StatusCode::NOT_FOUND);
	assert!(again.headers().get("set-cookie").is_none());
}

/// Every dead link says the same, in bytes; a rule that is broken is told to the holder of a
/// valid link, in the browser's language
#[tokio::test]
async fn dead_links_say_the_same_and_the_rules_are_told_to_the_holder_of_a_link() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let shelves = shelves(app).await;
	let biblioteca = [shelves.biblioteca.id.as_str()];

	let (expired_id, expired_url) = make_signup_link(app, "SINGLE", 1, &biblioteca).await;
	let mut row = link_row(app, &expired_id).await.into_active_model();
	row.expires_at = Set(Utc::now() - chrono::Duration::hours(1));
	row.update(app.conn()).await.unwrap();
	let (used_id, used_url) = make_signup_link(app, "SINGLE", 1, &biblioteca).await;
	let mut row = link_row(app, &used_id).await.into_active_model();
	row.uses = Set(1);
	row.update(app.conn()).await.unwrap();
	let (revoked_id, revoked_url) = make_signup_link(app, "SINGLE", 1, &biblioteca).await;
	owner_gql(
		app,
		"mutation($id: ID!) { revokeAccessSignupLink(id: $id) }",
		Some(json!({"id": revoked_id})),
	)
	.await;

	let mut answers = Vec::new();
	for token in [
		token_of(&expired_url).to_string(),
		token_of(&used_url).to_string(),
		token_of(&revoked_url).to_string(),
		"x".repeat(60),
		"not a token".to_string(),
		String::new(),
		"y".repeat(500),
	] {
		let response = json_join(app, &token, "ana", PASSWORD, "203.0.113.5").await;
		assert_eq!(response.status_code(), StatusCode::NOT_FOUND, "{token}");
		assert!(response.headers().get("set-cookie").is_none());
		answers.push(response.text());
	}
	assert!(
		answers.windows(2).all(|pair| pair[0] == pair[1]),
		"the dead links answer differently: {answers:#?}"
	);
	assert!(
		answers[0].contains("This link doesn't work"),
		"{}",
		answers[0]
	);
	assert_eq!(user::Entity::find().all(app.conn()).await.unwrap().len(), 1);

	// a live link: the rules
	let (live_id, url) = make_signup_link(app, "OPEN", 5, &biblioteca).await;
	let token = token_of(&url);
	for (username, password, wanted) in [
		("a", PASSWORD, "Username"),
		("ana", "short", "Password"),
		("ana", "ana-ana-ana-1", "Password"),
	] {
		let response = json_join(app, token, username, password, "203.0.113.5").await;
		assert_eq!(
			response.status_code(),
			StatusCode::UNPROCESSABLE_ENTITY,
			"{username}/{password}: {}",
			response.text()
		);
		assert!(response.headers().get("set-cookie").is_none());
		let message = response.json::<Value>()["message"].to_string();
		assert!(!message.is_empty(), "{message}");
		let _ = wanted;
	}
	let spanish = json_post(app, "/api/v2/auth/join", "203.0.113.5")
		.add_header("accept-language", "es")
		.json(&json!({"token": token, "username": "a", "password": PASSWORD}))
		.await;
	assert_eq!(spanish.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
	assert!(spanish.text().contains("usuario"), "{}", spanish.text());
	// a name that is taken is told, and spends nothing
	json_join(app, token, "ana", PASSWORD, "203.0.113.6")
		.await
		.assert_status_ok();
	let taken = json_join(app, token, "Ana", PASSWORD, "203.0.113.7").await;
	assert_eq!(taken.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
	assert!(taken.text().contains("taken"), "{}", taken.text());
	// the refusals spent nothing: one use for the one account
	assert_eq!(link_row(app, &live_id).await.uses, 1);
}

/// The sum of the uses of every signup link of the app
async fn access_signup_link_uses(app: &TestApp) -> i32 {
	use models::entity::access_signup_link;
	access_signup_link::Entity::find()
		.all(app.conn())
		.await
		.unwrap()
		.iter()
		.map(|link| link.uses)
		.sum()
}

/// The budget of a link is spent once per account whichever door the visitors use, even all at
/// the same moment
#[tokio::test]
async fn parallel_uses_through_both_doors_make_exactly_the_budget() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let shelves = shelves(app).await;
	let (link_id, url) =
		make_signup_link(app, "OPEN", 3, &[&shelves.biblioteca.id]).await;
	let token = token_of(&url).to_string();

	let responses = futures_util::future::join_all((0..10).map(|n| {
		let token = token.clone();
		async move {
			let name = format!("person{n}");
			let ip = format!("203.0.113.{}", 10 + n);
			if n % 2 == 0 {
				json_join(app, &token, &name, PASSWORD, &ip)
					.await
					.status_code()
			} else {
				join(app, &token, &name, PASSWORD, PASSWORD, None, &ip)
					.await
					.status_code()
			}
		}
	}))
	.await;
	let made = responses
		.iter()
		.filter(|status| **status == StatusCode::OK || **status == StatusCode::SEE_OTHER)
		.count();
	assert_eq!(made, 3, "{responses:?}");
	assert_eq!(link_row(app, &link_id).await.uses, 3);
	assert_eq!(user::Entity::find().all(app.conn()).await.unwrap().len(), 4);
}

/// The JSON signup answers only through an edge that serves the screen, with the switch on, from
/// this site: otherwise 404 or the generic answer, before any work
#[tokio::test]
async fn the_json_signup_needs_the_edge_the_header_the_switch_and_this_site() {
	let on = spa_app(true).await;
	let app = &on.env.app;
	let shelves = shelves(app).await;
	let (_, url) = make_signup_link(app, "OPEN", 9, &[&shelves.biblioteca.id]).await;
	let token = token_of(&url);
	let body = json!({"token": token, "username": "ana", "password": PASSWORD});

	// no header, not through the edge, another site
	app.server
		.post("/api/v2/auth/join")
		.add_header("x-edge-public", "1")
		.add_header("sec-fetch-site", "same-origin")
		.json(&body)
		.await
		.assert_status(StatusCode::NOT_FOUND);
	app.server
		.post("/api/v2/auth/join")
		.json(&body)
		.await
		.assert_status(StatusCode::NOT_FOUND);
	let cross = app
		.server
		.post("/api/v2/auth/join")
		.add_header("x-edge-public", "1")
		.add_header("x-edge-login", "spa")
		.add_header("sec-fetch-site", "cross-site")
		.json(&body)
		.await;
	assert_eq!(cross.status_code(), StatusCode::NOT_FOUND);
	assert!(cross.text().contains("This link doesn't work"));
	// not JSON
	let form = json_post(app, "/api/v2/auth/join", "203.0.113.5")
		.text("token=x")
		.await;
	assert!(form.status_code().is_client_error());
	assert_eq!(user::Entity::find().all(app.conn()).await.unwrap().len(), 1);

	// the switch off
	let off = spa_app(false).await;
	json_join(&off.env.app, token, "ana", PASSWORD, "203.0.113.5")
		.await
		.assert_status(StatusCode::NOT_FOUND);
}

/// A signup link opened in a browser goes to the screen's join page with the token in the
/// address's fragment (never sent to any server); an edge that does not serve the screen keeps
/// the server-made form; a dead link and a device link are what they were
#[tokio::test]
async fn a_signup_link_opens_the_join_screen_with_the_token_in_the_fragment() {
	let spa = spa_app(true).await;
	let app = &spa.env.app;
	let shelves = shelves(app).await;
	let (_, url) = make_signup_link(app, "OPEN", 5, &[&shelves.biblioteca.id]).await;
	let token = token_of(&url);

	let open = via_spa_edge(app, Method::GET, &format!("/api/_i/{token}")).await;
	assert_eq!(open.status_code(), StatusCode::SEE_OTHER);
	assert_eq!(header_of(&open, "location"), format!("/join#{token}"));
	assert!(
		!open.text().contains(token),
		"the body must not repeat the token"
	);
	assert_eq!(header_of(&open, "cache-control"), "no-store");
	// nothing was spent by looking
	assert_eq!(access_signup_link_uses(app).await, 0);

	// an edge without the header: the server-made form, as before
	let old = open_link(app, token).await;
	old.assert_status_ok();
	assert!(old.text().contains("name=\"password2\""), "{}", old.text());

	// a dead link: the generic page, no redirect
	let dead =
		via_spa_edge(app, Method::GET, &format!("/api/_i/{}", "z".repeat(43))).await;
	assert_ne!(dead.status_code(), StatusCode::SEE_OTHER);

	// the old sign-in address sends a visitor to the screen, and only through such an edge
	let signin = via_spa_edge(app, Method::GET, "/api/_i/signin").await;
	assert_eq!(signin.status_code(), StatusCode::SEE_OTHER);
	assert_eq!(header_of(&signin, "location"), "/auth");
	assert_eq!(header_of(&signin, "cache-control"), "no-store");
	let page = public_get(app, "/api/_i/signin", None).await;
	page.assert_status_ok();
	assert!(page.text().contains("action=\"/api/_i/signin\""));
}
