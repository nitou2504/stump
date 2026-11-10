//! The invite pages, served to people who hold no pass yet: `GET /api/_i/<token>` shows a page
//! with one button, `POST /api/_i/` redeems the invite (a use is spent, a device is made, the pass
//! is set as a cookie).
//!
//! Only through the public edge: from the LAN or the tailnet these routes answer 404, as they do
//! on a server with no edge key.
//!
//! - **A GET never spends anything**, so a chat app that fetches a link to show a preview can't
//!   use up an invite. Only the confirmation page's `POST` does.
//! - **Every failure looks the same** (unknown, expired, used up, revoked, its person gone or
//!   locked, a pass that is no good): nothing can be learnt from the page.
//! - The `POST` must come from this site (R4a): `Sec-Fetch-Site` decides when the browser sends it
//!   (the page sets `Referrer-Policy: no-referrer`, which makes `Origin` `null`, so it can't be
//!   relied on then); a browser without it is judged by `Origin`. Otherwise another site could
//!   post somebody else's invite and replace a visitor's pass with the attacker's.
//! - The token is never logged: not here, not in the trace spans (`http_server.rs` redacts the
//!   address), not by the edge. It is echoed in the page only when it is shaped like a token.

use axum::{
	extract::{Path, Query, State},
	http::{header, HeaderMap, HeaderValue, StatusCode},
	response::{IntoResponse, Response},
	routing::get,
	Extension, Form, Router,
};
use chrono::Utc;
use graphql::data::EdgeRequest;
use models::{entity::access_device, services::access as service};
use serde::Deserialize;
use stump_core::access::pass::{self, InviteInput};

use crate::{
	config::state::AppState,
	middleware::{
		edge::{is_cross_site, origin_of},
		host::{ClientIp, HostExtractor},
	},
};

pub(crate) fn mount(_app_state: AppState) -> Router<AppState> {
	Router::new()
		.route("/api/_i/", get(show_from_query).post(redeem))
		.route("/api/_i/{token}", get(show_from_path))
}

#[derive(Deserialize)]
struct PastedLink {
	t: Option<String>,
}

/// What the page says, in the visitor's language (English, or Spanish when the browser asks for it)
struct Words {
	lang: &'static str,
	title: &'static str,
	ask: &'static str,
	button: &'static str,
	invalid_title: &'static str,
	invalid: &'static str,
	paste: &'static str,
	open: &'static str,
}

const ENGLISH: Words = Words {
	lang: "en",
	title: "Open Stump on this device?",
	ask: "This link lets this browser open Stump. It works for you and your devices only.",
	button: "Open Stump",
	invalid_title: "This link doesn't work",
	invalid: "It has expired, was used up, or isn't valid. Ask for a new invite link.",
	paste: "Paste your link here",
	open: "Open",
};

const SPANISH: Words = Words {
	lang: "es",
	title: "¿Abrir Stump en este dispositivo?",
	ask: "Este enlace permite que este navegador abra Stump. Es solo para ti y tus dispositivos.",
	button: "Abrir Stump",
	invalid_title: "Este enlace no funciona",
	invalid: "Caducó, se agotó o no es válido. Pide un enlace de invitación nuevo.",
	paste: "Pega tu enlace aquí",
	open: "Abrir",
};

fn words(headers: &HeaderMap) -> &'static Words {
	let wants_spanish = headers
		.get(header::ACCEPT_LANGUAGE)
		.and_then(|value| value.to_str().ok())
		.is_some_and(|value| {
			value
				.split(',')
				.next()
				.is_some_and(|first| first.trim().to_ascii_lowercase().starts_with("es"))
		});
	if wants_spanish {
		&SPANISH
	} else {
		&ENGLISH
	}
}

fn escape_html(text: &str) -> String {
	let mut escaped = String::with_capacity(text.len());
	for c in text.chars() {
		match c {
			'&' => escaped.push_str("&amp;"),
			'<' => escaped.push_str("&lt;"),
			'>' => escaped.push_str("&gt;"),
			'"' => escaped.push_str("&quot;"),
			'\'' => escaped.push_str("&#39;"),
			other => escaped.push(other),
		}
	}
	escaped
}

const STYLE: &str = "body{font-family:system-ui,sans-serif;max-width:28rem;margin:12vh auto;padding:0 1rem;line-height:1.5}button,input{font:inherit;padding:.6rem 1rem}button{cursor:pointer}input{width:100%;box-sizing:border-box;margin:.5rem 0}";

fn page(status: StatusCode, words: &Words, body: &str, extra_headers: &[(header::HeaderName, String)]) -> Response {
	let html = format!(
		"<!doctype html><html lang=\"{}\"><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><meta name=robots content=noindex><title>Stump</title><style>{STYLE}</style><body>{body}</body></html>",
		words.lang
	);
	let mut response = (status, html).into_response();
	let headers = response.headers_mut();
	headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
	headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
	headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
	headers.insert("x-robots-tag", HeaderValue::from_static("noindex"));
	headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
	headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
	headers.insert(
		header::CONTENT_SECURITY_POLICY,
		HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; form-action 'self'"),
	);
	for (name, value) in extra_headers {
		if let Ok(value) = HeaderValue::from_str(value) {
			headers.append(name.clone(), value);
		}
	}
	response
}

/// The page every failure shows, with a box to paste another link
fn invalid_page(words: &Words) -> Response {
	let body = format!(
		"<h1>{}</h1><p>{}</p><form method=\"post\" action=\"/api/_i/\"><label>{}<input name=\"t\" autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\"></label><button>{}</button></form>",
		words.invalid_title, words.invalid, words.paste, words.open
	);
	page(StatusCode::NOT_FOUND, words, &body, &[])
}

/// The confirmation page: one button. The token is echoed in a hidden field, only when it is
/// shaped like one
fn confirm_page(words: &Words, input: &InviteInput) -> Response {
	let value = match input {
		InviteInput::Token(token) | InviteInput::Legacy(token) => escape_html(token),
	};
	let body = format!(
		"<h1>{}</h1><p>{}</p><form method=\"post\" action=\"/api/_i/\"><input type=\"hidden\" name=\"t\" value=\"{value}\"><button>{}</button></form>",
		words.title, words.ask, words.button
	);
	page(StatusCode::OK, words, &body, &[])
}

/// Whether the invite pages answer at all: only to a request that came through the public edge, on
/// a server that has a key to sign passes with
fn available(state: &AppState, edge: &EdgeRequest) -> bool {
	edge.via_public
		&& state
			.config
			.edge
			.as_ref()
			.is_some_and(|settings| settings.usable_key().is_some())
}

async fn show_from_path(
	State(state): State<AppState>,
	Extension(edge): Extension<EdgeRequest>,
	headers: HeaderMap,
	Path(token): Path<String>,
) -> Response {
	show(&state, &edge, &headers, Some(&token))
}

async fn show_from_query(
	State(state): State<AppState>,
	Extension(edge): Extension<EdgeRequest>,
	headers: HeaderMap,
	Query(link): Query<PastedLink>,
) -> Response {
	show(&state, &edge, &headers, link.t.as_deref())
}

/// A GET: shows the confirmation page for a link that is shaped like one (it checks nothing
/// against the database, and spends nothing), or the page every failure shows
fn show(
	state: &AppState,
	edge: &EdgeRequest,
	headers: &HeaderMap,
	text: Option<&str>,
) -> Response {
	if !available(state, edge) {
		return StatusCode::NOT_FOUND.into_response();
	}
	let words = words(headers);
	match text.and_then(pass::parse_invite_input) {
		Some(input) => confirm_page(words, &input),
		None => invalid_page(words),
	}
}

/// Whether a `POST` comes from this site (see the module docs). The origin to compare is the
/// public address when there is one, or else the request's own
fn post_is_same_origin(
	headers: &HeaderMap,
	public_url: Option<&str>,
	own_origin: Option<&str>,
) -> bool {
	let expected = public_url.map(origin_of).or(own_origin);
	!is_cross_site(headers, expected, &[])
}

async fn redeem(
	State(state): State<AppState>,
	Extension(edge): Extension<EdgeRequest>,
	host: Result<HostExtractor, crate::errors::APIError>,
	ClientIp(ip): ClientIp,
	headers: HeaderMap,
	Form(link): Form<PastedLink>,
) -> Response {
	if !available(&state, &edge) {
		return StatusCode::NOT_FOUND.into_response();
	}
	let words = words(&headers);
	let Some(settings) = state.config.edge.as_ref() else {
		return StatusCode::NOT_FOUND.into_response();
	};
	let Some(key) = settings.usable_key() else {
		return StatusCode::NOT_FOUND.into_response();
	};

	let own_origin = host.ok().map(|HostExtractor(details)| details.url());
	let public_url = settings.public_base_url();
	if !post_is_same_origin(&headers, public_url.as_deref(), own_origin.as_deref()) {
		return invalid_page(words);
	}

	let Some(input) = link.t.as_deref().and_then(pass::parse_invite_input) else {
		return invalid_page(words);
	};

	let now = Utc::now();
	let conn = state.conn.as_ref();
	let agent = headers
		.get(header::USER_AGENT)
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default()
		.to_string();

	let device = match input {
		InviteInput::Token(token) => {
			match service::redeem_invite(
				conn,
				now,
				&pass::token_hash(&token),
				edge.device.as_ref().map(|device| device.id.as_str()),
				&ip.to_string(),
				&agent,
			)
			.await
			{
				Ok(redeemed) => redeemed.device,
				Err(service::RedeemError::Invalid) => return invalid_page(words),
				Err(service::RedeemError::Db(error)) => {
					tracing::error!(?error, "Public access: redeeming an invite failed");
					return invalid_page(words);
				},
			}
		},
		InviteInput::Legacy(text) => {
			match redeem_legacy(&state, key, &text, now.timestamp(), now).await {
				Some(device) => device,
				None => return invalid_page(words),
			}
		},
	};

	let value = pass::mint_pass(key, &device.id, now.timestamp());
	let cookie = pass::pass_cookie_header(&settings.cookie, &value);
	page(
		StatusCode::SEE_OTHER,
		words,
		"",
		&[
			(header::LOCATION, "/".to_string()),
			(header::SET_COOKIE, cookie),
		],
	)
}

/// Redeems a link the terminal script made: it makes (or finds) the device named in it, which has
/// no person and no use limit. A device that was revoked is never brought back by its old link
async fn redeem_legacy(
	state: &AppState,
	key: &str,
	text: &str,
	now_epoch: i64,
	now: chrono::DateTime<Utc>,
) -> Option<access_device::Model> {
	let legacy = pass::verify_legacy_invite(key, text, now_epoch)?;
	let conn = state.conn.as_ref();

	let device = match service::find_device(conn, &legacy.name).await {
		Ok(Some(device)) => device,
		Ok(None) => {
			let device = service::register_legacy_device(conn, now, &legacy.name)
				.await
				.map_err(|error| tracing::error!(?error, "Public access: legacy invite failed"))
				.ok()?;
			let _ = service::audit(conn, now, "public", "invite.redeemed", &device.id, "legacy link").await;
			device
		},
		Err(error) => {
			tracing::error!(?error, "Public access: legacy invite failed");
			return None;
		},
	};
	device.revoked_at.is_none().then_some(device)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn text_is_escaped_for_html() {
		assert_eq!(
			escape_html("<script>\"a\" & 'b'</script>"),
			"&lt;script&gt;&quot;a&quot; &amp; &#39;b&#39;&lt;/script&gt;"
		);
	}

	#[test]
	fn the_page_follows_the_browsers_language() {
		let mut headers = HeaderMap::new();
		assert_eq!(words(&headers).lang, "en");
		headers.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_static("es-EC,es;q=0.9,en;q=0.8"));
		assert_eq!(words(&headers).lang, "es");
		headers.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,es;q=0.9"));
		assert_eq!(words(&headers).lang, "en");
	}

	#[test]
	fn a_post_from_another_site_is_refused() {
		let own = Some("https://g3.test:8443");
		let public = Some("https://g3.test:8443/");
		let mut headers = HeaderMap::new();
		// a form on this site, with the `null` origin of a `no-referrer` page
		headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
		headers.insert(header::ORIGIN, HeaderValue::from_static("null"));
		assert!(post_is_same_origin(&headers, public, own));
		// a form on another site
		headers.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
		assert!(!post_is_same_origin(&headers, public, own));
		// an older browser: judged by Origin
		let mut old = HeaderMap::new();
		old.insert(header::ORIGIN, HeaderValue::from_static("https://g3.test:8443"));
		assert!(post_is_same_origin(&old, public, own));
		old.insert(header::ORIGIN, HeaderValue::from_static("https://evil.test"));
		assert!(!post_is_same_origin(&old, public, own));
		old.insert(header::ORIGIN, HeaderValue::from_static("null"));
		assert!(!post_is_same_origin(&old, public, own));
	}
}
