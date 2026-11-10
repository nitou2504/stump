//! How a request reached the app: through the public edge or not, with which pass, and whether
//! another site may have made the browser send it.
//!
//! The public edge (nginx, the only way in from the internet) adds `X-Edge-Public: 1` to
//! everything it forwards from the internet and `X-Edge-Pass: <cookie value>` when the browser
//! holds a valid, unrevoked pass; it overwrites both on every request, so a client can plant
//! neither. The app believes the marker because only the edge can reach it (the container
//! publishes no host port), and it believes the pass because it **verifies the MAC itself** with
//! the key: a bare id in a header means nothing.
//!
//! The marker is believed whatever the configuration (the lead's R2): `via_public` is `true`
//! exactly when the header is there, so the owner's password is refused through the edge even on
//! a server with no edge settings at all. Only checking a pass, and minting one, need a usable
//! key; without one there is no device, and a public request that must carry a pass
//! (`STUMP_EDGE_REQUIRE_PASS`) is refused (fails closed).
//!
//! Every request gets an [`EdgeRequest`] in its extensions; `graphql_handler` and the websocket
//! handler copy it into the GraphQL data, where the guards read it.

use std::{
	collections::HashMap,
	sync::{Mutex, OnceLock},
	time::{Duration, Instant},
};

use axum::{
	extract::{Request, State},
	http::{header, HeaderMap, StatusCode},
	middleware::Next,
	response::{IntoResponse, Response},
};
use chrono::Utc;
use graphql::data::{EdgeDevice, EdgeRequest};
use models::services::access as service;
use stump_core::access::pass;

use crate::{
	config::state::AppState,
	errors::APIError,
	middleware::host::{ClientIp, HostExtractor},
};

/// The marker the edge adds to what it forwards from the internet. Only its presence counts
pub const EDGE_PUBLIC_HEADER: &str = "x-edge-public";
/// The pass cookie's value, passed on by the edge when it checked out
pub const EDGE_PASS_HEADER: &str = "x-edge-pass";

/// A device's "last seen" is written at most once a minute
const LAST_SEEN_EVERY: Duration = Duration::from_secs(60);

/// The paths a browser without a pass may still fetch through the edge: the invite pages and the
/// two static files the browser asks for without cookies
pub fn is_open_public_path(path: &str) -> bool {
	path.starts_with("/api/_i/")
		|| path == "/assets/manifest.webmanifest"
		|| path
			.strip_prefix("/assets/favicon-")
			.and_then(|rest| rest.strip_suffix(".png"))
			.is_some_and(|size| {
				let mut parts = size.split('x');
				matches!(
					(parts.next(), parts.next(), parts.next()),
					(Some(a), Some(b), None)
						if !a.is_empty()
							&& !b.is_empty()
							&& a.bytes().all(|c| c.is_ascii_digit())
							&& b.bytes().all(|c| c.is_ascii_digit())
				)
			})
}

/// The origin (`scheme://host[:port]`) of an address, or of an `Origin` header's value
pub fn origin_of(url: &str) -> &str {
	let url = url.trim();
	match url.find("://") {
		Some(scheme_end) => {
			let after = scheme_end + 3;
			match url[after..].find('/') {
				Some(slash) => &url[..after + slash],
				None => url,
			}
		},
		None => url,
	}
}

/// Whether a browser may have been made to send the request by another site.
///
/// `Sec-Fetch-Site` is decisive when the browser sends it: `same-origin` (a page of this site)
/// and `none` (the person typed the address, or opened a bookmark) are fine, anything else is
/// not. A browser that sends none (older ones, and scripts) is judged by `Origin` when it has one:
/// the request's own origin or an allowed one is fine. A request with neither header is not from
/// a browser that is being steered by another site.
pub fn is_cross_site(
	headers: &HeaderMap,
	own_origin: Option<&str>,
	allowed_origins: &[String],
) -> bool {
	if let Some(site) = headers
		.get("sec-fetch-site")
		.and_then(|value| value.to_str().ok())
	{
		return !matches!(site.trim(), "same-origin" | "none");
	}
	match headers.get(header::ORIGIN) {
		None => false,
		Some(origin) => {
			let Ok(origin) = origin.to_str() else {
				return true;
			};
			let origin = origin.trim();
			!(own_origin.is_some_and(|own| own == origin)
				|| allowed_origins.iter().any(|allowed| allowed == origin))
		},
	}
}

fn should_note_seen(device_id: &str) -> bool {
	static SEEN: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
	let mut seen = SEEN
		.get_or_init(Default::default)
		.lock()
		.unwrap_or_else(|poisoned| poisoned.into_inner());
	let now = Instant::now();
	match seen.get(device_id) {
		Some(at) if now.duration_since(*at) < LAST_SEEN_EVERY => false,
		_ => {
			seen.insert(device_id.to_string(), now);
			true
		},
	}
}

const REVOKED_PAGE: &str = "<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width\"><title>Access turned off</title><p>Access from this device was turned off. Ask for a new invite link.</p>";

/// The middleware: makes the [`EdgeRequest`] of every request (see the module docs)
pub async fn edge_middleware(
	State(state): State<AppState>,
	host: Result<HostExtractor, APIError>,
	mut req: Request,
	next: Next,
) -> Response {
	let headers = req.headers().clone();
	let path = req.uri().path().to_string();
	let via_public = headers.contains_key(EDGE_PUBLIC_HEADER);

	let own_origin = host.ok().map(|HostExtractor(details)| details.url());
	let mut edge = EdgeRequest {
		via_public,
		device: None,
		cross_site: is_cross_site(
			&headers,
			own_origin.as_deref(),
			&state.config.allowed_origins,
		),
	};

	if via_public {
		// found here, before anything is awaited: a request's body is not `Sync`, so the request
		// itself can't be held across an await
		let ClientIp(ip) = ClientIp::find(
			&headers,
			req.extensions(),
			state.config.trust_proxy_headers,
		);
		let settings = state.config.edge.as_ref();
		let key = settings.and_then(|settings| settings.usable_key());
		let pass_text = headers
			.get(EDGE_PASS_HEADER)
			.and_then(|value| value.to_str().ok());

		if let (Some(key), Some(pass_text)) = (key, pass_text) {
			if let Some(verified) =
				pass::verify_pass(key, pass_text, Utc::now().timestamp())
			{
				match note_device(&state, &headers, ip, &verified.id).await {
					Ok(Seen::Active(device)) => {
						edge.device = Some(EdgeDevice {
							id: device.id,
							person_id: device.person_id,
						});
					},
					// a new invite replaces a revoked device, so the invite pages take it as
					// no device at all; everywhere else it is turned away at once
					Ok(Seen::Revoked) if path.starts_with("/api/_i/") => {},
					Ok(Seen::Revoked) => {
						return (
							StatusCode::FORBIDDEN,
							[(header::CONTENT_TYPE, "text/html; charset=utf-8")],
							REVOKED_PAGE,
						)
							.into_response();
					},
					// the database failed: no device, which is the safe side
					Err(error) => {
						tracing::error!(?error, "Public access: could not look up the pass");
					},
				}
			}
		}

		if settings.is_some_and(|settings| settings.require_pass)
			&& edge.device.is_none()
			&& !is_open_public_path(&path)
		{
			return StatusCode::NOT_FOUND.into_response();
		}
	}

	req.extensions_mut().insert(edge);
	next.run(req).await
}

enum Seen {
	Active(models::entity::access_device::Model),
	Revoked,
}

/// Looks the verified pass up: a revoked device is refused at once (the state file is how the
/// edge follows, the app asks its own database); a pass the app has never seen is registered as a
/// legacy device; a device that is active is noted as seen, at most once a minute
async fn note_device(
	state: &AppState,
	headers: &HeaderMap,
	ip: std::net::IpAddr,
	id: &str,
) -> Result<Seen, sea_orm::DbErr> {
	let conn = state.conn.as_ref();
	let now = Utc::now();
	let device = match service::find_device(conn, id).await? {
		Some(device) => device,
		None => service::register_legacy_device(conn, now, id).await?,
	};
	if device.revoked_at.is_some() {
		return Ok(Seen::Revoked);
	}

	if should_note_seen(id) {
		let agent = headers
			.get(header::USER_AGENT)
			.and_then(|value| value.to_str().ok())
			.unwrap_or_default()
			.to_string();
		let conn = state.conn.clone();
		let id = id.to_string();
		tokio::spawn(async move {
			if let Err(error) =
				service::touch_device(conn.as_ref(), Utc::now(), &id, &ip.to_string(), &agent)
					.await
			{
				tracing::warn!(?error, "Public access: could not note that a device was seen");
			}
		});
	}
	Ok(Seen::Active(device))
}

#[cfg(test)]
mod tests {
	use super::*;
	use axum::http::HeaderValue;

	fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
		let mut map = HeaderMap::new();
		for (name, value) in pairs {
			map.insert(
				axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
				HeaderValue::from_str(value).unwrap(),
			);
		}
		map
	}

	#[test]
	fn the_open_paths_are_the_invite_pages_and_two_static_files() {
		for path in [
			"/api/_i/",
			"/api/_i/abc",
			"/assets/manifest.webmanifest",
			"/assets/favicon-32x32.png",
			"/assets/favicon-192x192.png",
		] {
			assert!(is_open_public_path(path), "{path}");
		}
		for path in [
			"/",
			"/api/v2/ping",
			"/api/_i",
			"/api/graphql",
			"/assets/index.js",
			"/assets/favicon-.png",
			"/assets/favicon-32.png",
			"/assets/favicon-32x32x1.png",
			"/assets/favicon-ax32.png",
			"/assets/favicon-32x32.png/../../x",
			"/assets/manifest.webmanifest/x",
		] {
			assert!(!is_open_public_path(path), "{path}");
		}
	}

	#[test]
	fn an_origin_is_cut_from_an_address() {
		assert_eq!(origin_of("https://g3.ts.net:8443"), "https://g3.ts.net:8443");
		assert_eq!(origin_of("https://g3.ts.net:8443/x/y"), "https://g3.ts.net:8443");
		assert_eq!(origin_of(" http://a "), "http://a");
	}

	#[test]
	fn sec_fetch_site_decides_when_it_is_there() {
		let own = Some("https://home.test");
		for (value, cross) in [
			("same-origin", false),
			("none", false),
			("same-site", true),
			("cross-site", true),
			("", true),
			("nonsense", true),
		] {
			assert_eq!(
				is_cross_site(&headers(&[("sec-fetch-site", value)]), own, &[]),
				cross,
				"{value:?}"
			);
		}
		// it wins over Origin, which a `no-referrer` page makes `null`
		assert!(!is_cross_site(
			&headers(&[("sec-fetch-site", "same-origin"), ("origin", "null")]),
			own,
			&[]
		));
		assert!(is_cross_site(
			&headers(&[("sec-fetch-site", "cross-site"), ("origin", "https://home.test")]),
			own,
			&[]
		));
	}

	#[test]
	fn origin_decides_when_the_browser_sends_no_sec_fetch_site() {
		let own = Some("https://home.test");
		let allowed = vec!["https://other.test".to_string()];
		// no headers at all: a script with a token, not a browser steered by another site
		assert!(!is_cross_site(&headers(&[]), own, &allowed));
		assert!(!is_cross_site(&headers(&[("origin", "https://home.test")]), own, &allowed));
		assert!(!is_cross_site(&headers(&[("origin", "https://other.test")]), own, &allowed));
		assert!(is_cross_site(&headers(&[("origin", "https://evil.test")]), own, &allowed));
		assert!(is_cross_site(&headers(&[("origin", "null")]), own, &allowed));
		// no way to tell what "own" is: an Origin can't be vouched for
		assert!(is_cross_site(&headers(&[("origin", "https://home.test")]), None, &allowed));
	}

	#[test]
	fn a_device_is_noted_once_a_minute() {
		let id = format!("test-{}", uuid::Uuid::new_v4());
		assert!(should_note_seen(&id));
		assert!(!should_note_seen(&id));
		assert!(should_note_seen(&format!("{id}-other")));
	}
}
