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
//! - A token that belongs to a **signup link** (a link that makes an account) shows the signup
//!   form instead (`public_pages::signup`), when public sign-in is on.

use axum::{
	extract::{Path, Query, State},
	http::{header, HeaderMap, StatusCode},
	response::{IntoResponse, Response},
	routing::get,
	Extension, Form, Router,
};
use chrono::Utc;
use graphql::data::EdgeRequest;
use models::{entity::access_device, services::access as service};
use serde::Deserialize;
use stump_core::access::pass::{self, InviteInput};

use super::public_pages::{
	available, escape_html, invalid_page, page, post_is_same_origin, public_signin_on,
	signup, words, Words,
};
use crate::{
	config::state::AppState,
	middleware::host::{ClientIp, HostExtractor},
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

async fn show_from_path(
	State(state): State<AppState>,
	Extension(edge): Extension<EdgeRequest>,
	headers: HeaderMap,
	Path(token): Path<String>,
) -> Response {
	show(&state, &edge, &headers, Some(&token)).await
}

async fn show_from_query(
	State(state): State<AppState>,
	Extension(edge): Extension<EdgeRequest>,
	headers: HeaderMap,
	Query(link): Query<PastedLink>,
) -> Response {
	show(&state, &edge, &headers, link.t.as_deref()).await
}

/// A GET: shows the signup form for a token that belongs to a signup link that can be used (the
/// generic page for one that can't), or the confirmation page for a link that is shaped like a
/// device link (it checks nothing against the database for those, and spends nothing), or the
/// page every failure shows
async fn show(
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
		Some(input) => {
			if let (InviteInput::Token(token), true) =
				(&input, public_signin_on(state, edge))
			{
				if let Some(response) =
					signup::page_for_token(state, words, token, edge.spa_login).await
				{
					return response;
				}
			}
			confirm_page(words, &input)
		},
		None => invalid_page(words),
	}
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
				.map_err(|error| {
					tracing::error!(?error, "Public access: legacy invite failed")
				})
				.ok()?;
			let _ = service::audit(
				conn,
				now,
				"public",
				"invite.redeemed",
				&device.id,
				"legacy link",
			)
			.await;
			device
		},
		Err(error) => {
			tracing::error!(?error, "Public access: legacy invite failed");
			return None;
		},
	};
	device.revoked_at.is_none().then_some(device)
}
