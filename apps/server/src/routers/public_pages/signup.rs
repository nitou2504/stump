//! The signup form: somebody opens a link the owner made and **makes their own account** with the
//! username and password they choose. There is no account to find, no password to be told and
//! nobody to wait for.
//!
//! - `GET /api/_i/<token>` (in `invite.rs`) shows the form for a link that can be used; every
//!   other state of a signup link (expired, used up, revoked, no libraries left) shows the one
//!   generic "this link doesn't work" page. A `GET` spends nothing and sets nothing, so a chat
//!   app that fetches the link for a preview cannot use it up.
//! - `POST /api/_i/join` does it: the link is looked up **before** the name or password is looked
//!   at (so a stranger can't use the form to find out which names exist), the rules for names and
//!   passwords are applied, the password is hashed **outside** the transaction (the writer lock
//!   is never held for a hash), and one transaction spends a use, makes the account with the
//!   link's preset and its first device (`models::services::signup`).
//! - The pass is made like the sign-in page makes one, and for the same reasons a pass that is
//!   already in the browser is never replaced.
//! - The token is never logged: it travels in the form's body, not an address.

use axum::{
	extract::{rejection::FormRejection, State},
	http::{header, HeaderMap, StatusCode},
	response::{IntoResponse, Response},
	Extension, Form,
};
use chrono::Utc;
use graphql::data::EdgeRequest;
use models::{
	entity::user::LoginUser,
	services::{
		access::{self as service, SignInMark, MAX_AGENT_CHARS},
		access_policy::{
			check_password, check_username, PasswordProblem, UsernameProblem,
			USERNAME_MAX_CHARS,
		},
		signup::{self, Signup, SignupError},
	},
};
use sea_orm::prelude::*;
use serde::Deserialize;
use std::net::IpAddr;
use stump_core::access::pass::{self, InviteInput};
use tower_sessions::Session;

use super::{
	escape_html, invalid_page, page, password_problem, post_is_same_origin,
	public_signin_on, username_problem, words, Words,
};
use crate::{
	config::state::AppState,
	middleware::host::{ClientIp, HostExtractor},
	routers::record_attempt,
};

/// Field caps, checked before anything is hashed
const MAX_TOKEN_CHARS: usize = 120;
const MAX_PASSWORD_BYTES: usize = 128;

#[derive(Deserialize)]
pub(crate) struct JoinForm {
	t: String,
	username: String,
	password: String,
	password2: String,
}

/// The signup form for the link with this token. The token is echoed as a hidden field, and only
/// when it is shaped like one
pub(crate) fn join_page(
	status: StatusCode,
	words: &Words,
	token: &str,
	message: Option<&str>,
	typed: &str,
) -> Response {
	let message = message
		.map(|text| format!("<p class=error role=alert>{}</p>", escape_html(text)))
		.unwrap_or_default();
	let body = format!(
		"<h1>{title}</h1>\
		<form method=\"post\" action=\"/api/_i/join\">\
		<input type=\"hidden\" name=\"t\" value=\"{token}\">\
		<label>{username}<input name=\"username\" value=\"{typed}\" autocomplete=\"username\" autocapitalize=\"none\" spellcheck=\"false\" minlength=\"3\" maxlength=\"{maxu}\" required autofocus></label>\
		<label>{password}<input name=\"password\" type=\"password\" autocomplete=\"new-password\" minlength=\"10\" maxlength=\"72\" required></label>\
		<label>{again}<input name=\"password2\" type=\"password\" autocomplete=\"new-password\" minlength=\"10\" maxlength=\"72\" required></label>\
		<p class=hint>{rules}</p>{message}\
		<button>{button}</button></form>\
		<p class=muted><a href=\"/api/_i/signin\">{have_account}</a></p>",
		title = words.join_title,
		username = words.username,
		password = words.password,
		again = words.password_again,
		rules = words.rules,
		button = words.join_button,
		have_account = words.have_account,
		token = escape_html(token),
		typed = escape_html(typed),
		maxu = USERNAME_MAX_CHARS,
	);
	page(status, words, &body, &[])
}

/// What a `GET` of a token shows when the token belongs to a signup link: the form if the link
/// can be used, the generic page if it can't. `None` when the token is no signup link at all (it
/// may be a device link)
pub(crate) async fn page_for_token(
	state: &AppState,
	words: &Words,
	token: &str,
	spa: bool,
) -> Option<Response> {
	let hash = pass::token_hash(token);
	let now = Utc::now();
	let link = match models::entity::access_signup_link::Entity::find()
		.filter(models::entity::access_signup_link::Column::TokenHash.eq(hash))
		.one(state.conn.as_ref())
		.await
	{
		Ok(Some(link)) => link,
		Ok(None) => return None,
		Err(error) => {
			tracing::error!(?error, "Signup: could not look the link up");
			return Some(invalid_page(words));
		},
	};
	let usable = signup::signup_status(&link, now) == signup::SignupStatus::Active
		&& signup::preset_of(&link).is_some();
	Some(if usable && spa {
		// the web app's own join screen: the token travels in the address's fragment, which no
		// server ever receives and no Referer carries, and the screen removes it at once
		page(
			StatusCode::SEE_OTHER,
			words,
			"",
			&[(header::LOCATION, format!("/join#{token}"))],
		)
	} else if usable {
		join_page(StatusCode::OK, words, token, None, "")
	} else {
		invalid_page(words)
	})
}

/// What a signup from the internet came to: what the form and the web app's own signup both
/// answer from
pub(crate) enum PublicJoin {
	/// Not a link that can be used (unknown, used up, expired, revoked, or not a link): the same
	/// answer for all of them
	Invalid,
	/// The visitor holds a valid link, so telling them what is wrong reveals nothing to a stranger
	Username(UsernameProblem),
	PasswordsDiffer,
	Password(PasswordProblem),
	NameTaken,
	/// The account was made, but its session could not be started: they sign in with it
	SessionFailed,
	/// In: the session is started. `pass_cookie` is the whole `Set-Cookie` value of the pass of the
	/// first device, none when the browser already held a pass (never replaced)
	Joined {
		account_id: String,
		pass_cookie: Option<String>,
	},
}

/// The signup itself, behind both doors, from the link on: the link is looked up **before** the
/// name or the password is looked at, then the rules, the hash (outside the transaction), one
/// transaction that spends the use and makes the account, its preset and its first device, the
/// session, the recorded sign-in and the pass. `token` is the link's token (already taken out of
/// whatever the visitor pasted), `password_again` is what a form's second box held
pub(crate) async fn join_from_internet(
	state: &AppState,
	session: &Session,
	ip: IpAddr,
	agent: &str,
	has_device: bool,
	token: &str,
	typed_username: &str,
	password: &str,
	password_again: Option<&str>,
) -> PublicJoin {
	let Some(settings) = state.config.edge.as_ref() else {
		return PublicJoin::Invalid;
	};
	let Some(key) = settings.usable_key() else {
		return PublicJoin::Invalid;
	};

	// 1. the link, before the name or the password is looked at
	let token_hash = pass::token_hash(token);
	let now = Utc::now();
	match signup::peek_signup_link(state.conn.as_ref(), now, &token_hash).await {
		Ok(Some(_)) => {},
		Ok(None) => return PublicJoin::Invalid,
		Err(error) => {
			tracing::error!(?error, "Signup: could not look the link up");
			return PublicJoin::Invalid;
		},
	}

	// 2. the name and the password
	let username = match check_username(typed_username) {
		Ok(username) => username,
		Err(problem) => return PublicJoin::Username(problem),
	};
	if password_again.is_some_and(|again| again != password) {
		return PublicJoin::PasswordsDiffer;
	}
	if let Err(problem) = check_password(password, &username) {
		return PublicJoin::Password(problem);
	}

	// 3. the hash, outside the transaction
	let cost = state.config.password_hash_cost;
	let password = password.to_string();
	let hashed_password =
		match tokio::task::spawn_blocking(move || bcrypt::hash(password, cost)).await {
			Ok(Ok(hash)) => hash,
			_ => {
				tracing::error!("Signup: could not hash the password");
				return PublicJoin::Invalid;
			},
		};

	// 4. the account
	let ip_text = ip.to_string();
	let created = match signup::redeem_signup_link(
		state.conn.as_ref(),
		now,
		Signup {
			token_hash: &token_hash,
			username: &username,
			hashed_password,
			ip: &ip_text,
			agent: &agent.chars().take(MAX_AGENT_CHARS).collect::<String>(),
			// a pass that is already in the browser is never replaced
			make_device: !has_device,
		},
	)
	.await
	{
		Ok(created) => created,
		Err(SignupError::Invalid) => return PublicJoin::Invalid,
		Err(SignupError::UsernameTaken) => return PublicJoin::NameTaken,
		Err(SignupError::Db(error)) => {
			tracing::error!(?error, "Signup: the account could not be made");
			return PublicJoin::Invalid;
		},
	};

	// 5. in: the session, and the pass
	if let Err(error) =
		super::signin::start_session(state, session, &created.user.id).await
	{
		tracing::error!(
			?error,
			"Signup: the account was made but the session was not"
		);
		// no pass is sent for the first device, so it would use up one of the person's three for
		// nothing: it is revoked, as the sign-in page does
		if let Some(device) = &created.device {
			let _ = service::revoke_device(
				state.conn.as_ref(),
				Utc::now(),
				&device.id,
				"system",
			)
			.await;
		}
		return PublicJoin::SessionFailed;
	}
	if let Ok(Some(login)) = LoginUser::find_by_id(created.user.id.clone())
		.into_model::<LoginUser>()
		.one(state.conn.as_ref())
		.await
	{
		record_attempt(
			state.conn.as_ref(),
			&login,
			agent.to_string(),
			ip,
			true,
			&SignInMark::counting(
				created.device.as_ref().map(|device| device.id.clone()),
				None,
			),
		)
		.await;
	}

	let pass_cookie = created.device.as_ref().map(|device| {
		let value = pass::mint_pass(key, &device.id, now.timestamp());
		pass::pass_cookie_header(&settings.cookie, &value)
	});
	PublicJoin::Joined {
		account_id: created.user.id.clone(),
		pass_cookie,
	}
}

pub(crate) async fn submit(
	State(state): State<AppState>,
	Extension(edge): Extension<EdgeRequest>,
	host: Result<HostExtractor, crate::errors::APIError>,
	ClientIp(ip): ClientIp,
	session: tower_sessions::Session,
	headers: HeaderMap,
	form: Result<Form<JoinForm>, FormRejection>,
) -> Response {
	if !public_signin_on(&state, &edge) {
		return StatusCode::NOT_FOUND.into_response();
	}
	let words = words(&headers);
	let Some(settings) = state.config.edge.as_ref() else {
		return StatusCode::NOT_FOUND.into_response();
	};
	if settings.usable_key().is_none() {
		return StatusCode::NOT_FOUND.into_response();
	}

	// 1. the site, the shape and the size of what came: otherwise the generic page
	let own_origin = host.ok().map(|HostExtractor(details)| details.url());
	let public_url = settings.public_base_url();
	let Ok(Form(form)) = form else {
		return invalid_page(words);
	};
	if !post_is_same_origin(&headers, public_url.as_deref(), own_origin.as_deref())
		|| form.t.len() > MAX_TOKEN_CHARS
		|| form.username.chars().count() > 4 * USERNAME_MAX_CHARS
		|| form.password.len() > MAX_PASSWORD_BYTES
		|| form.password2.len() > MAX_PASSWORD_BYTES
	{
		return invalid_page(words);
	}

	// 2. what was pasted must be a link's token
	let Some(InviteInput::Token(token)) = pass::parse_invite_input(&form.t) else {
		return invalid_page(words);
	};

	let again = |message: &str| {
		join_page(
			StatusCode::UNPROCESSABLE_ENTITY,
			words,
			&token,
			Some(message),
			&form.username,
		)
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
		&form.username,
		&form.password,
		Some(&form.password2),
	)
	.await
	{
		PublicJoin::Invalid => invalid_page(words),
		PublicJoin::Username(problem) => again(username_problem(words, problem)),
		PublicJoin::PasswordsDiffer => again(words.passwords_differ),
		PublicJoin::Password(problem) => again(password_problem(words, problem)),
		PublicJoin::NameTaken => again(words.name_taken),
		// the account exists: send them to sign in with it
		PublicJoin::SessionFailed => page(
			StatusCode::SEE_OTHER,
			words,
			"",
			&[(header::LOCATION, "/api/_i/signin".to_string())],
		),
		PublicJoin::Joined { pass_cookie, .. } => {
			let mut extra = vec![(header::LOCATION, "/".to_string())];
			if let Some(cookie) = pass_cookie {
				extra.push((header::SET_COOKIE, cookie));
			}
			page(StatusCode::SEE_OTHER, words, "", &extra)
		},
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn the_form_escapes_the_token_and_the_name() {
		let response = join_page(
			StatusCode::OK,
			&super::super::ENGLISH,
			"\"><script>x</script>",
			Some("<b>no</b>"),
			"<i>ana</i>",
		);
		let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
			.await
			.unwrap();
		let text = String::from_utf8_lossy(&body);
		assert!(!text.contains("<script>"), "{text}");
		assert!(!text.contains("<b>no</b>"), "{text}");
		assert!(!text.contains("<i>ana</i>"), "{text}");
		assert!(text.contains("autocomplete=\"new-password\""));
	}
}
