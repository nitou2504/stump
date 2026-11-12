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
		access::{SignInMark, MAX_AGENT_CHARS},
		access_policy::{check_password, check_username, USERNAME_MAX_CHARS},
		signup::{self, Signup, SignupError},
	},
};
use sea_orm::prelude::*;
use serde::Deserialize;
use stump_core::access::pass::{self, InviteInput};

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
		"<h1>{title}</h1><p>{intro}</p>{message}\
		<form method=\"post\" action=\"/api/_i/join\">\
		<input type=\"hidden\" name=\"t\" value=\"{token}\">\
		<label>{username}<input name=\"username\" value=\"{typed}\" autocomplete=\"username\" autocapitalize=\"none\" spellcheck=\"false\" minlength=\"3\" maxlength=\"{maxu}\" required autofocus></label>\
		<label>{password}<input name=\"password\" type=\"password\" autocomplete=\"new-password\" minlength=\"10\" maxlength=\"72\" required></label>\
		<label>{again}<input name=\"password2\" type=\"password\" autocomplete=\"new-password\" minlength=\"10\" maxlength=\"72\" required></label>\
		<p class=muted>{rules}</p>\
		<button>{button}</button></form>\
		<p class=muted><a href=\"/api/_i/signin\">{have_account}</a></p>",
		title = words.join_title,
		intro = words.join_intro,
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
	Some(if usable {
		join_page(StatusCode::OK, words, token, None, "")
	} else {
		invalid_page(words)
	})
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
	let Some(key) = settings.usable_key() else {
		return StatusCode::NOT_FOUND.into_response();
	};

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

	// 2. the link, before the name or the password is looked at
	let Some(InviteInput::Token(token)) = pass::parse_invite_input(&form.t) else {
		return invalid_page(words);
	};
	let token_hash = pass::token_hash(&token);
	let now = Utc::now();
	match signup::peek_signup_link(state.conn.as_ref(), now, &token_hash).await {
		Ok(Some(_)) => {},
		Ok(None) => return invalid_page(words),
		Err(error) => {
			tracing::error!(?error, "Signup: could not look the link up");
			return invalid_page(words);
		},
	}

	// 3. the name and the password: the visitor holds a valid link, so telling them what is wrong
	// reveals nothing to a stranger
	let again = |message: &str| {
		join_page(
			StatusCode::UNPROCESSABLE_ENTITY,
			words,
			&token,
			Some(message),
			&form.username,
		)
	};
	let username = match check_username(&form.username) {
		Ok(username) => username,
		Err(problem) => return again(username_problem(words, problem)),
	};
	if form.password != form.password2 {
		return again(words.passwords_differ);
	}
	if let Err(problem) = check_password(&form.password, &username) {
		return again(password_problem(words, problem));
	}

	// 4. the hash, outside the transaction
	let cost = state.config.password_hash_cost;
	let password = form.password.clone();
	let hashed_password =
		match tokio::task::spawn_blocking(move || bcrypt::hash(password, cost)).await {
			Ok(Ok(hash)) => hash,
			_ => {
				tracing::error!("Signup: could not hash the password");
				return invalid_page(words);
			},
		};

	// 5. the account
	let agent = headers
		.get(header::USER_AGENT)
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default()
		.to_string();
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
			make_device: edge.device.is_none(),
		},
	)
	.await
	{
		Ok(created) => created,
		Err(SignupError::Invalid) => return invalid_page(words),
		Err(SignupError::UsernameTaken) => return again(words.name_taken),
		Err(SignupError::Db(error)) => {
			tracing::error!(?error, "Signup: the account could not be made");
			return invalid_page(words);
		},
	};

	// 6. in: the session, and the pass
	if let Err(error) =
		super::signin::start_session(&state, &session, &created.user.id).await
	{
		tracing::error!(
			?error,
			"Signup: the account was made but the session was not"
		);
		// the account exists: send them to sign in with it
		return page(
			StatusCode::SEE_OTHER,
			words,
			"",
			&[(header::LOCATION, "/api/_i/signin".to_string())],
		);
	}
	if let Ok(Some(login)) = LoginUser::find_by_id(created.user.id.clone())
		.into_model::<LoginUser>()
		.one(state.conn.as_ref())
		.await
	{
		record_attempt(
			state.conn.as_ref(),
			&login,
			agent,
			ip,
			true,
			&SignInMark::counting(
				created.device.as_ref().map(|device| device.id.clone()),
				None,
			),
		)
		.await;
	}

	let mut extra = vec![(header::LOCATION, "/".to_string())];
	if let Some(device) = &created.device {
		let value = pass::mint_pass(key, &device.id, now.timestamp());
		extra.push((
			header::SET_COOKIE,
			pass::pass_cookie_header(&settings.cookie, &value),
		));
	}
	page(StatusCode::SEE_OTHER, words, "", &extra)
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
