use std::{path::Path, sync::Arc};

use axum::{
	body::Body,
	extract::State,
	http::{header, HeaderMap, HeaderValue, Method, Request},
	middleware::{from_fn_with_state, Next},
	response::IntoResponse,
	response::Response,
	routing::get,
	Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use graphql::data::EdgeRequest;
use sha2::{Digest, Sha256};
use tower::ServiceBuilder;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::{
	config::state::AppState,
	errors::{APIError, APIResult},
};

pub const FAVICON: &str = "/favicon.ico";
const SW: &str = "/sw.js";
const INDEX: &str = "/";
const INDEX_HTML: &str = "/index.html";
const MANIFEST: &str = "/assets/manifest.webmanifest";
const ASSETS: &str = "/assets";
const DIST: &str = "/dist";

pub(crate) fn mount(app_state: AppState) -> Router<AppState> {
	let dist_path = Path::new(&app_state.config.client_dir);
	let static_assets = ServiceBuilder::new()
		.layer(SetResponseHeaderLayer::if_not_present(
			header::VARY,
			HeaderValue::from_static("Accept-Encoding"),
		))
		.layer(SetResponseHeaderLayer::overriding(
			header::CACHE_CONTROL,
			HeaderValue::from_static("public, max-age=31536000, immutable, no-transform"),
		))
		.service(
			ServeDir::new(dist_path.join("assets"))
				.precompressed_br()
				.precompressed_gzip(),
		);

	let dist_files = ServiceBuilder::new()
		.layer(SetResponseHeaderLayer::if_not_present(
			header::VARY,
			HeaderValue::from_static("Accept-Encoding"),
		))
		.layer(SetResponseHeaderLayer::if_not_present(
			header::CACHE_CONTROL,
			HeaderValue::from_static("no-cache"),
		))
		.service(
			ServeDir::new(dist_path)
				.precompressed_br()
				.precompressed_gzip(),
		);

	let spa_fallback = ServiceBuilder::new()
		.layer(SetResponseHeaderLayer::if_not_present(
			header::CACHE_CONTROL,
			HeaderValue::from_static("no-cache"),
		))
		.service(ServeFile::new(dist_path.join("index.html")));

	// the shell shown to a visitor with no pass carries a policy (see `shell_policy`)
	let policy = Arc::new(shell_csp(&dist_path.join("index.html")));

	Router::new()
		.route(INDEX, get(index_html))
		.route(INDEX_HTML, get(index_html))
		.route(FAVICON, get(favicon))
		.route(SW, get(serve_sw))
		.route(MANIFEST, get(serve_manifest))
		.nest_service(ASSETS, static_assets)
		.nest_service(DIST, dist_files)
		.fallback_service(spa_fallback)
		.layer(from_fn_with_state(policy, shell_policy))
}

/// The hash source (`'sha256-…'`) of every inline `<script>` of an HTML page: the few lines the
/// shell runs before the app loads (the theme and the font from `localStorage`). A script with a
/// `src` is not inline, and an empty one has nothing to run
fn inline_script_hashes(html: &str) -> Vec<String> {
	let mut hashes = Vec::new();
	let mut rest = html;
	while let Some(start) = rest.find("<script") {
		let tag_and_after = &rest[start..];
		let Some(tag_end) = tag_and_after.find('>') else {
			break;
		};
		let tag = &tag_and_after[..tag_end];
		let after_tag = &tag_and_after[tag_end + 1..];
		let Some(close) = after_tag.find("</script>") else {
			break;
		};
		let body = &after_tag[..close];
		if !tag.contains(" src=") && !body.trim().is_empty() {
			hashes.push(format!(
				"'sha256-{}'",
				STANDARD.encode(Sha256::digest(body.as_bytes()))
			));
		}
		rest = &after_tag[close + "</script>".len()..];
	}
	hashes
}

/// The Content-Security-Policy of the shell a visitor with **no pass** gets: nothing but this
/// site (and the page's own inline scripts, by hash), no frames, no plugins, forms only to
/// itself. Styles may be inline (the shell's own, React's style attributes). Computed from the
/// `index.html` the app serves, so the hashes can't fall out of step with the build. When the
/// file can't be read the list of hashes is empty: the inline bootstrap is then blocked (a
/// flash of the default theme), never the sign-in
fn shell_csp(index_html: &Path) -> String {
	let hashes = std::fs::read_to_string(index_html)
		.map(|html| inline_script_hashes(&html))
		.unwrap_or_default();
	let scripts = std::iter::once("'self'".to_string())
		.chain(hashes)
		.collect::<Vec<_>>()
		.join(" ");
	format!(
		"default-src 'none'; script-src {scripts}; style-src 'self' 'unsafe-inline'; \
		 img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self'; \
		 manifest-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; \
		 object-src 'none'"
	)
}

/// The shell (`/`, `/auth`, `/join`) shown through the public edge to somebody with **no valid
/// pass** carries the policy of [`shell_csp`] and is never cached, so a copy with the policy
/// never stays in the browser for a signed-in visit. A visitor with a pass gets the shell as
/// always, and a request that did not come through the edge, or from an edge that does not serve
/// the sign-in screen (no capability header), is not touched
async fn shell_policy(
	State(policy): State<Arc<String>>,
	req: Request<Body>,
	next: Next,
) -> Response {
	let pass_less_shell = matches!(*req.method(), Method::GET | Method::HEAD)
		&& matches!(req.uri().path(), "/" | "/auth" | "/join")
		&& req.extensions().get::<EdgeRequest>().is_some_and(|edge| {
			edge.via_public && edge.spa_login && edge.device.is_none()
		});
	let mut response = next.run(req).await;
	if pass_less_shell {
		if let Ok(value) = HeaderValue::from_str(&policy) {
			response
				.headers_mut()
				.insert(header::CONTENT_SECURITY_POLICY, value);
		}
		response
			.headers_mut()
			.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
		// the server-made pages this replaces said so too: a sign-in screen is not for search results
		response
			.headers_mut()
			.insert("x-robots-tag", HeaderValue::from_static("noindex"));
	}
	response
}

pub(crate) fn relative_favicon_path() -> String {
	format!("{ASSETS}{FAVICON}")
}

// https://github.com/tokio-rs/axum/discussions/608#discussioncomment-7772294
async fn favicon(
	State(ctx): State<AppState>,
	headers: HeaderMap,
) -> APIResult<impl IntoResponse> {
	let mut response = serve_dist_file(ctx, headers, "favicon.ico").await?;
	response.headers_mut().insert(
		header::CACHE_CONTROL,
		HeaderValue::from_static("public, max-age=86400"),
	);

	Ok(response)
}

async fn index_html(
	State(ctx): State<AppState>,
	headers: HeaderMap,
) -> APIResult<impl IntoResponse> {
	serve_with_no_cache(ctx, headers, "index.html").await
}

async fn serve_sw(
	State(ctx): State<AppState>,
	headers: HeaderMap,
) -> APIResult<impl IntoResponse> {
	serve_with_no_cache(ctx, headers, "sw.js").await
}

async fn serve_manifest(
	State(ctx): State<AppState>,
	headers: HeaderMap,
) -> APIResult<impl IntoResponse> {
	serve_with_no_store(ctx, headers, "assets/manifest.webmanifest").await
}

async fn serve_with_no_cache(
	ctx: AppState,
	headers: HeaderMap,
	path: &str,
) -> APIResult<Response> {
	let mut response = serve_dist_file(ctx, headers, path).await?;
	response.headers_mut().insert(
		header::CACHE_CONTROL,
		HeaderValue::from_static("no-cache, no-store, must-revalidate"),
	);

	Ok(response)
}

async fn serve_with_no_store(
	ctx: AppState,
	headers: HeaderMap,
	path: &str,
) -> APIResult<Response> {
	// Manifest and bootstrap must bypass caches.
	let mut response = serve_dist_file(ctx, headers, path).await?;
	response.headers_mut().insert(
		header::CACHE_CONTROL,
		HeaderValue::from_static("no-cache, no-store, must-revalidate"),
	);

	Ok(response)
}

async fn serve_dist_file(
	ctx: AppState,
	headers: HeaderMap,
	path: &str,
) -> APIResult<Response> {
	let mut req = Request::new(Body::empty());
	*req.headers_mut() = headers;

	match ServeFile::new(Path::new(&ctx.config.client_dir).join(path))
		.try_call(req)
		.await
	{
		Ok(res) => Ok(res.into_response()),
		Err(e) => {
			tracing::error!(error = ?e, path, "Error serving dist file");
			Err(APIError::InternalServerError(e.to_string()))
		},
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn only_inline_scripts_are_hashed() {
		let html = "<head><script>var a=1</script><script type=\"module\" crossorigin src=\"/assets/index.js\"></script><script>  </script><script>b()</script></head>";
		let hashes = inline_script_hashes(html);
		assert_eq!(hashes.len(), 2, "{hashes:?}");
		// the hash is of the exact bytes between the tags
		assert_eq!(
			hashes[0],
			format!("'sha256-{}'", STANDARD.encode(Sha256::digest("var a=1")))
		);
		assert!(hashes.iter().all(|hash| hash.starts_with("'sha256-")));
	}

	#[test]
	fn the_policy_allows_this_site_and_the_pages_own_scripts_only() {
		let dir = std::env::temp_dir().join(format!("stump-spa-{}", std::process::id()));
		std::fs::create_dir_all(&dir).unwrap();
		let index = dir.join("index.html");
		std::fs::write(
			&index,
			"<script>x()</script><script src=\"/a.js\"></script>",
		)
		.unwrap();
		let policy = shell_csp(&index);
		assert!(policy.starts_with("default-src 'none'; script-src 'self' 'sha256-"));
		assert_eq!(policy.matches("'sha256-").count(), 1, "{policy}");
		for wanted in [
			"frame-ancestors 'none'",
			"object-src 'none'",
			"base-uri 'none'",
			"form-action 'self'",
			"connect-src 'self'",
		] {
			assert!(policy.contains(wanted), "{wanted}");
		}
		assert!(!policy.contains("unsafe-eval"));
		assert!(!policy.contains("script-src 'self' 'unsafe-inline'"));
		// no file: no hashes, but still a policy
		let none = shell_csp(&dir.join("missing.html"));
		assert!(none.contains("script-src 'self';"), "{none}");
		let _ = std::fs::remove_dir_all(&dir);
	}
}
