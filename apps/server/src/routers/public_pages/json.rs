//! The web app's own sign-in screen on the public address: the password sign-in
//! (`POST /api/v2/auth/login` from a browser with **no pass**) and the signup
//! (`POST /api/v2/auth/join`), as JSON.
//!
//! These are thin doors on the same two cores the server-made pages use
//! ([`sign_in_from_internet`], [`join_from_internet`]): the guard, the one bcrypt, the
//! eligibility rule, the device limit, the audit and the warnings are not re-implemented, so the
//! doors can't drift and a failure through one counts towards the other's budget. They answer
//! only through an edge that serves this screen (the capability header) with the
//! `STUMP_EDGE_PUBLIC_SIGNIN` switch on; otherwise they answer 404 **before doing any work**, so a
//! pass-less sign-in can never reach the sign-in that counts failures towards locking an account.
//!
//! What they promise, beyond the server-made pages:
//! - **The same bytes for everything that is not a sign-in**: an unknown name, a wrong password,
//!   the owner, an account that is not enabled, a locked one, one with no password, a request
//!   another site made the browser send, a field too long or empty: all the same `401` body.
//! - **No cookie, no noise on a refusal**: the answers are written here, not by the API's error
//!   type. That one deletes the session cookie on every `401` (the owner's home session cookie is
//!   sent to this port too, so a refused attempt at the owner's name would sign the owner out at
//!   home) and logs every refusal as an error. A refusal here carries no `Set-Cookie` at all.
//! - **A pass is never replaced** and **only the first device of a signup is made here**: see the
//!   cores.

use std::net::IpAddr;

use axum::{
	extract::{rejection::JsonRejection, State},
	http::{header, HeaderMap, StatusCode},
	response::{IntoResponse, Response},
	Extension, Json,
};
use graphql::data::{EdgeRequest, ServiceContext};
use models::entity::user::LoginUser;
use sea_orm::prelude::*;
use serde::Deserialize;
use stump_core::access::pass::{self, InviteInput};
use tower_sessions::Session;

use super::{
	password_problem, post_is_same_origin, public_signin_on,
	signin::{
		sign_in_from_internet, PublicSignIn, MAX_PASSWORD_BYTES, MAX_USERNAME_CHARS,
	},
	signup::{join_from_internet, PublicJoin},
	username_problem, words, Words,
};
use crate::{
	config::state::AppState,
	middleware::host::{ClientIp, HostDetails, HostExtractor},
	routers::{auth_user_of, LoginResponse},
};

/// The answer to everything that is not a sign-in. The edge answers a pass-less request for the
/// viewer with these same bytes (`deploy/public-edge/nginx.conf`)
const UNAUTHORIZED: &str = r#"{"status":401,"message":"Unauthorized"}"#;

/// The longest link token, username and password the signup looks at: more is refused **without
/// hashing**
const MAX_TOKEN_CHARS: usize = 120;
const MAX_JOIN_USERNAME_CHARS: usize =
	4 * models::services::access_policy::USERNAME_MAX_CHARS;

/// A JSON answer: no store, no cookie unless `extra` has one
fn json_response(
	status: StatusCode,
	body: String,
	extra: &[(header::HeaderName, String)],
) -> Response {
	let mut response = (status, body).into_response();
	let headers = response.headers_mut();
	headers.insert(
		header::CONTENT_TYPE,
		header::HeaderValue::from_static("application/json"),
	);
	headers.insert(
		header::CACHE_CONTROL,
		header::HeaderValue::from_static("no-store"),
	);
	for (name, value) in extra {
		if let Ok(value) = header::HeaderValue::from_str(value) {
			headers.append(name.clone(), value);
		}
	}
	response
}

/// The API's error body (`{"status":…,"message":…}`, keys in this order)
fn error_response(
	status: StatusCode,
	message: &str,
	extra: &[(header::HeaderName, String)],
) -> Response {
	let message = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into());
	json_response(
		status,
		format!(r#"{{"status":{},"message":{message}}}"#, status.as_u16()),
		extra,
	)
}

fn unauthorized() -> Response {
	json_response(StatusCode::UNAUTHORIZED, UNAUTHORIZED.to_string(), &[])
}

fn invalid_link(words: &Words) -> Response {
	error_response(
		StatusCode::NOT_FOUND,
		&format!("{}. {}", words.invalid_title, words.invalid),
		&[],
	)
}

/// "You already use 3 devices. Remove one in Settings → My devices, or ask the owner."
fn limit_message(words: &Words, devices: i32) -> String {
	format!(
		"{}. {}",
		words.limit_title.replace("{n}", &devices.to_string()),
		words.limit_body
	)
}

/// The sign-in of a browser with no pass. The caller has already established that the request
/// came through the public edge with no device; this answers 404 (before any work) unless the
/// edge serves the web app's sign-in screen and the switch is on
pub(crate) async fn login(
	state: &AppState,
	session: &Session,
	ip: IpAddr,
	headers: &HeaderMap,
	details: &HostDetails,
	edge: &EdgeRequest,
	agent: &str,
	username: &str,
	password: String,
) -> Response {
	if !(edge.spa_login && public_signin_on(state, edge)) {
		return StatusCode::NOT_FOUND.into_response();
	}
	let Some(settings) = state.config.edge.as_ref() else {
		return StatusCode::NOT_FOUND.into_response();
	};
	let words = words(headers);

	// the same site, and a request of a size that is a sign-in (or the server takes no passwords
	// at all): the generic answer, without any work
	let local_auth_is_off = state
		.config
		.oidc
		.as_ref()
		.is_some_and(|oidc| oidc.disable_local_auth);
	if local_auth_is_off
		|| !post_is_same_origin(
			headers,
			settings.public_base_url().as_deref(),
			Some(&details.url()),
		) || username.chars().count() > MAX_USERNAME_CHARS
		|| password.len() > MAX_PASSWORD_BYTES
		|| username.trim().is_empty()
		|| password.is_empty()
	{
		return unauthorized();
	}

	match sign_in_from_internet(state, session, ip, agent, None, username, password).await
	{
		PublicSignIn::Refused => error_response(
			StatusCode::TOO_MANY_REQUESTS,
			&format!("{}. {}", words.too_many_title, words.too_many),
			&[(header::RETRY_AFTER, "300".to_string())],
		),
		PublicSignIn::Failed => unauthorized(),
		PublicSignIn::LimitReached { max } => {
			error_response(StatusCode::FORBIDDEN, &limit_message(words, max), &[])
		},
		PublicSignIn::SignedIn {
			account,
			pass_cookie,
		} => {
			signed_in_response(state, details, &account.id, pass_cookie, StatusCode::OK)
				.await
		},
	}
}

/// The answer to a sign-in or a signup that worked: the user as the web app's own sign-in
/// returns it, and the pass cookie of the device made here (the session cookie is added by the
/// session layer)
async fn signed_in_response(
	state: &AppState,
	details: &HostDetails,
	account_id: &str,
	pass_cookie: Option<String>,
	status: StatusCode,
) -> Response {
	let conn = state.conn.as_ref();
	let login = match LoginUser::find_by_id(account_id.to_string())
		.into_model::<LoginUser>()
		.one(conn)
		.await
	{
		Ok(Some(login)) => login,
		_ => return unauthorized(),
	};
	let service = ServiceContext::new(details.host.clone(), details.scheme.clone());
	let Ok(user) = auth_user_of(conn, login, service).await else {
		return unauthorized();
	};
	let body = match serde_json::to_string(&LoginResponse::User(user)) {
		Ok(body) => body,
		Err(_) => return unauthorized(),
	};
	let extra: Vec<_> = pass_cookie
		.into_iter()
		.map(|cookie| (header::SET_COOKIE, cookie))
		.collect();
	json_response(status, body, &extra)
}

#[derive(Deserialize)]
pub(crate) struct JoinBody {
	token: String,
	username: String,
	password: String,
}

/// `POST /api/v2/auth/join`: the signup of the web app's own join screen. The link's token
/// arrives in the body (the screen read it from the address's fragment and removed it), the
/// screen checks "repeat the password" itself
pub(crate) async fn join(
	State(state): State<AppState>,
	Extension(edge): Extension<EdgeRequest>,
	HostExtractor(details): HostExtractor,
	ClientIp(ip): ClientIp,
	session: Session,
	headers: HeaderMap,
	body: Result<Json<JoinBody>, JsonRejection>,
) -> Response {
	// the first check, before any work: through the edge, which serves this screen, with the
	// switch on
	if !(edge.via_public && edge.spa_login && public_signin_on(&state, &edge)) {
		return StatusCode::NOT_FOUND.into_response();
	}
	let Some(settings) = state.config.edge.as_ref() else {
		return StatusCode::NOT_FOUND.into_response();
	};
	let words = words(&headers);

	// the site, the shape and the size of what came: otherwise the generic answer, with no work
	let Ok(Json(body)) = body else {
		return invalid_link(words);
	};
	if !post_is_same_origin(
		&headers,
		settings.public_base_url().as_deref(),
		Some(&details.url()),
	) || body.token.len() > MAX_TOKEN_CHARS
		|| body.username.chars().count() > MAX_JOIN_USERNAME_CHARS
		|| body.password.len() > MAX_PASSWORD_BYTES
	{
		return invalid_link(words);
	}
	let Some(InviteInput::Token(token)) = pass::parse_invite_input(&body.token) else {
		return invalid_link(words);
	};

	let agent = headers
		.get(header::USER_AGENT)
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default()
		.to_string();
	match join_from_internet(
		&state,
		&session,
		ip,
		&agent,
		edge.device.is_some(),
		&token,
		&body.username,
		&body.password,
		None,
	)
	.await
	{
		PublicJoin::Invalid => invalid_link(words),
		PublicJoin::Username(problem) => error_response(
			StatusCode::UNPROCESSABLE_ENTITY,
			username_problem(words, problem),
			&[],
		),
		PublicJoin::PasswordsDiffer => error_response(
			StatusCode::UNPROCESSABLE_ENTITY,
			words.passwords_differ,
			&[],
		),
		PublicJoin::Password(problem) => error_response(
			StatusCode::UNPROCESSABLE_ENTITY,
			password_problem(words, problem),
			&[],
		),
		PublicJoin::NameTaken => {
			error_response(StatusCode::UNPROCESSABLE_ENTITY, words.name_taken, &[])
		},
		// the account exists, the session could not start: the screen sends them to sign in
		PublicJoin::SessionFailed => {
			error_response(StatusCode::CONFLICT, words.account_ready, &[])
		},
		PublicJoin::Joined {
			account_id,
			pass_cookie,
		} => {
			signed_in_response(&state, &details, &account_id, pass_cookie, StatusCode::OK)
				.await
		},
	}
}
