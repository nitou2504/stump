//! The web app's own sign-in screen on the public address: the shell and its files are served
//! with no pass (and only those), the sign-in and signup are JSON, and everything else stays
//! closed. The edge says it serves this screen with the capability header `X-Edge-Login: spa`.

use axum::http::{Method, StatusCode};
use axum_test::TestRequest;
use chrono::Utc;
use models::services::access as service;

use super::*;

/// What the fake web build says, so a test can tell the shell from any other answer
const SHELL: &str = "<div id=\"root\">THE-SHELL</div>";

struct SpaApp {
	env: EdgeApp,
	_client: TempDir,
}

/// An app with a small fake web build and the switch on or off
async fn spa_app(public_signin: bool) -> SpaApp {
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
	let mut edge = edge_settings(&state_dir, true);
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
