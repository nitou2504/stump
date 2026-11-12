//! The public sign-in page: how a friend gets into Stump from any phone or computer, and the one
//! function ([`authenticate_public`]) every password check from the internet that holds no pass of
//! its own goes through.
//!
//! `GET /api/_i/signin` shows the form (the edge also serves it at `/` to a browser with no pass).
//! `POST /api/_i/signin` checks the password; a person who is **allowed in from the internet** (see
//! `access_people::eligibility`) and has fewer devices than their limit gets a pass for this
//! device, a session, and is sent to `/`.
//!
//! What it promises:
//! - **The same answer for everything that is not a sign-in**: an unknown name, a wrong password,
//!   the owner, an account that is not enabled, a locked one and one with no password all show the
//!   same page after the same work (one bcrypt, never more or less), so the page can't be used to
//!   find out which names exist. Only a correct password of an enabled account ever sees anything
//!   else: the device limit.
//! - **Never a lock**: a failure here never counts towards `users.is_locked`. It feeds the guard
//!   ([`stump_core::access::signin`]), which only ever *delays*.
//! - **Nothing slow for strangers**: the cheap gates come before the password, the fields are
//!   capped before any hashing, and what is written to the database for a failure is written
//!   after the answer is on its way, so how long an answer takes does not depend on whether the
//!   name exists.
//! - **A pass that is already there is never replaced.** A request that carries a valid pass (of
//!   anybody's) signs the person in without making a device.

use std::net::IpAddr;

use axum::{
	extract::{rejection::FormRejection, State},
	http::{header, HeaderMap, StatusCode},
	response::{IntoResponse, Response},
	routing::get,
	Extension, Form, Router,
};
use chrono::Utc;
use graphql::data::EdgeRequest;
use models::{
	entity::{
		access_device, access_person,
		user::{self, LoginUser},
	},
	services::{
		access::{self as service, reason, SignInMark},
		access_people::{self, NotAllowed},
		access_policy::device_label,
	},
};
use sea_orm::{prelude::*, sea_query::Expr};
use serde::Deserialize;
use stump_core::access::{
	pass,
	signin::{ClientKey, NameKey, Outcome, Verdict},
};
use tower_sessions::Session;

use super::{
	available, escape_html, page, post_is_same_origin, private_page, public_signin_on,
	too_many_page, words, Words,
};
use crate::{
	config::{session::SESSION_USER_KEY, state::AppState},
	middleware::host::{ClientIp, HostExtractor},
	routers::{enforce_max_sessions, record_attempt},
	utils::{verify_dummy_password, verify_password},
};

/// The longest username and password the page looks at: more is refused **without hashing**, so a
/// megabyte "password" costs nothing
const MAX_USERNAME_CHARS: usize = 64;
const MAX_PASSWORD_BYTES: usize = 128;

pub(crate) fn mount(_state: AppState) -> Router<AppState> {
	Router::new().route("/api/_i/signin", get(show).post(submit))
}

#[derive(Deserialize)]
struct SigninForm {
	username: String,
	password: String,
}

/// The sign-in form, optionally with a message above it and the name that was typed
pub(crate) fn signin_page(
	status: StatusCode,
	words: &Words,
	message: Option<&str>,
	typed: &str,
) -> Response {
	let message = message
		.map(|text| format!("<p class=error role=alert>{}</p>", escape_html(text)))
		.unwrap_or_default();
	let body = format!(
		"<h1>{title}</h1><p>{intro}</p>{message}\
		<form method=\"post\" action=\"/api/_i/signin\">\
		<label>{username}<input name=\"username\" value=\"{typed}\" autocomplete=\"username\" autocapitalize=\"none\" spellcheck=\"false\" maxlength=\"{maxu}\" required autofocus></label>\
		<label>{password}<input name=\"password\" type=\"password\" autocomplete=\"current-password\" maxlength=\"{maxp}\" required></label>\
		<button>{button}</button></form>\
		<details><summary>{have_link}</summary>\
		<form method=\"get\" action=\"/api/_i/\"><label><input name=\"t\" autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\" aria-label=\"{have_link}\"></label><button>{open}</button></form></details>\
		<p class=muted>{forgot}</p>",
		title = words.signin_title,
		intro = words.signin_intro,
		username = words.username,
		password = words.password,
		button = words.signin_button,
		have_link = words.have_link,
		open = words.open,
		forgot = words.forgot,
		typed = escape_html(typed),
		maxu = MAX_USERNAME_CHARS,
		maxp = MAX_PASSWORD_BYTES,
	);
	page(status, words, &body, &[])
}

fn failed_page(words: &Words, typed: &str) -> Response {
	signin_page(
		StatusCode::UNAUTHORIZED,
		words,
		Some(words.signin_failed),
		typed,
	)
}

fn limit_page(words: &Words, devices: i32) -> Response {
	let body = format!(
		"<h1>{}</h1><p>{}</p>",
		words.limit_title.replace("{n}", &devices.to_string()),
		words.limit_body
	);
	page(StatusCode::FORBIDDEN, words, &body, &[])
}

async fn show(
	State(state): State<AppState>,
	Extension(edge): Extension<EdgeRequest>,
	headers: HeaderMap,
) -> Response {
	if !public_signin_on(&state, &edge) {
		// public sign-in is off: the front door shows the old "private site, paste your invite link" page
		// (on a server that can make passes at all and only through the edge), and nothing else here answers
		return if available(&state, &edge) {
			private_page(words(&headers))
		} else {
			StatusCode::NOT_FOUND.into_response()
		};
	}
	signin_page(StatusCode::OK, words(&headers), None, "")
}

/// What a password check from the internet came to
pub(crate) enum PublicVerdict {
	/// Refused before any password work: a client, a name or the whole server has failed too
	/// often lately. The same page whichever it was
	Refused,
	/// Not a sign-in: the same answer whatever the reason
	Failed,
	/// A right password of an account that may come in from the internet
	Allowed {
		account: user::Model,
		person: access_person::Model,
	},
}

/// The account a typed name stands for: the exact name, else the only account that differs from
/// it in case. Several accounts that differ only in case (made before names were unique that
/// way) match nothing
async fn find_account(
	conn: &DatabaseConnection,
	typed: &str,
) -> Result<Option<user::Model>, DbErr> {
	let typed = typed.trim();
	if let Some(exact) = user::Entity::find()
		.filter(user::Column::Username.eq(typed))
		.filter(user::Column::DeletedAt.is_null())
		.one(conn)
		.await?
	{
		return Ok(Some(exact));
	}
	let mut matches = user::Entity::find()
		.filter(user::Column::DeletedAt.is_null())
		.filter(Expr::cust_with_values(
			"LOWER(username) = ?",
			[typed.to_lowercase()],
		))
		.all(conn)
		.await?;
	Ok(if matches.len() == 1 {
		matches.pop()
	} else {
		None
	})
}

/// The password check behind the sign-in page, and behind the web app's own sign-in for a request
/// whose pass is not this person's (see `login()`): the gates, one bcrypt, the eligibility rule,
/// the guard's bookkeeping, and the record of a failure (after the answer, for an account that
/// exists; nothing at all for a name that doesn't, but one aggregated audit row)
pub(crate) async fn authenticate_public(
	state: &AppState,
	typed_username: &str,
	password: String,
	ip: IpAddr,
	agent: &str,
	device_id: Option<String>,
) -> PublicVerdict {
	let guard = state.signin_guard.clone();
	let now = guard.now();
	let client = ClientKey::from_ip(ip);
	let name = NameKey::of(typed_username);

	if let Verdict::Refused(why) = guard.check(client, name, now) {
		tracing::debug!(?why, "Public sign-in refused before the password");
		return PublicVerdict::Refused;
	}

	let conn = state.conn.as_ref();
	let account = match find_account(conn, typed_username).await {
		Ok(account) => account,
		Err(error) => {
			tracing::error!(?error, "Public sign-in: could not look the account up");
			return PublicVerdict::Failed;
		},
	};

	// one bcrypt, always. The owner's password is never checked from the internet, and an
	// account with no password (an identity provider's) has nothing to check
	let (password_is_right, is_owner) = match &account {
		Some(account)
			if !account.is_server_owner && !account.hashed_password.is_empty() =>
		{
			(
				verify_password(account.hashed_password.clone(), password)
					.await
					.unwrap_or(false),
				false,
			)
		},
		other => {
			verify_dummy_password(password, &state.config).await;
			(
				false,
				other
					.as_ref()
					.is_some_and(|account| account.is_server_owner),
			)
		},
	};

	// eligibility is only looked at for an account that exists
	let (eligible, person) = match &account {
		Some(account) => match access_people::person_row(conn, &account.id).await {
			Ok(person) => (access_people::eligibility(account, person.as_ref()), person),
			Err(error) => {
				tracing::error!(?error, "Public sign-in: could not read the person");
				return PublicVerdict::Failed;
			},
		},
		None => (Err(NotAllowed::Gone), None),
	};

	if let (true, Some(account), Ok(()), Some(person)) =
		(password_is_right, &account, &eligible, person)
	{
		guard.success(client, name);
		return PublicVerdict::Allowed {
			account: account.clone(),
			person,
		};
	}

	// not a sign-in: count it, and record it after the answer
	let outcome = guard.fail(client, name, now);
	let failure_reason = if is_owner {
		reason::OWNER_VIA_EDGE
	} else if account.as_ref().is_some_and(|account| account.is_locked) {
		reason::LOCKED
	} else if password_is_right {
		reason::NOT_ALLOWED_PUBLICLY
	} else {
		reason::WRONG_PASSWORD
	};
	let unknown_flush = account
		.is_none()
		.then(|| guard.note_unknown_name(now))
		.flatten();
	let state = state.clone();
	let agent = agent.to_string();
	tokio::spawn(async move {
		record_failure(
			&state,
			account,
			name,
			outcome,
			unknown_flush,
			failure_reason,
			ip,
			agent,
			device_id,
		)
		.await;
	});
	PublicVerdict::Failed
}

/// Writes what a failed attempt leaves behind: the sign-in log row of an account that exists, and
/// the audit rows for what the failure started (a wait for the name, the global budget, the
/// aggregated failures on unknown names). Never the typed text
#[allow(clippy::too_many_arguments)]
async fn record_failure(
	state: &AppState,
	account: Option<user::Model>,
	name: NameKey,
	outcome: Outcome,
	unknown_flush: Option<u32>,
	failure_reason: &'static str,
	ip: IpAddr,
	agent: String,
	device_id: Option<String>,
) {
	let conn = state.conn.as_ref();
	let now = Utc::now();

	if let Some(account) = &account {
		let login = LoginUser::find_by_id(account.id.clone())
			.into_model::<LoginUser>()
			.one(conn)
			.await;
		match login {
			Ok(Some(login)) => {
				record_attempt(
					conn,
					&login,
					agent,
					ip,
					false,
					&SignInMark::ignored_by_lock(device_id, failure_reason),
				)
				.await;
			},
			Ok(None) => {},
			Err(error) => {
				tracing::error!(?error, "Public sign-in: could not record a failure")
			},
		}
	}

	let target = account.as_ref().map_or_else(
		|| format!("unknown:{}", name.short()),
		|account| account.id.clone(),
	);
	let mut rows: Vec<(&str, String, String)> = Vec::new();
	if outcome.pair_wait_minutes == Some(1) || outcome.name_ceiling_reached {
		rows.push((
			"signin.backoff",
			target.clone(),
			"a name was slowed down after repeated failures".to_string(),
		));
	}
	if outcome.global_tripped {
		rows.push((
			"signin.global_budget",
			"all".to_string(),
			"public sign-in is refusing everybody for a few minutes".to_string(),
		));
	}
	if let Some(count) = unknown_flush {
		rows.push((
			"signin.unknown_names",
			"unknown".to_string(),
			format!("{count} failures on names that do not exist"),
		));
	}
	for (action, target, detail) in rows {
		if let Err(error) =
			service::audit(conn, now, "public", action, &target, &detail).await
		{
			tracing::error!(?error, "Public sign-in: could not write an audit row");
		}
	}
}

async fn submit(
	State(state): State<AppState>,
	Extension(edge): Extension<EdgeRequest>,
	host: Result<HostExtractor, crate::errors::APIError>,
	ClientIp(ip): ClientIp,
	session: Session,
	headers: HeaderMap,
	form: Result<Form<SigninForm>, FormRejection>,
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

	// the same site, and a form of a size that is a sign-in: otherwise the generic answer,
	// without any work
	let own_origin = host.ok().map(|HostExtractor(details)| details.url());
	let public_url = settings.public_base_url();
	let Ok(Form(form)) = form else {
		return failed_page(words, "");
	};
	if !post_is_same_origin(&headers, public_url.as_deref(), own_origin.as_deref())
		|| form.username.chars().count() > MAX_USERNAME_CHARS
		|| form.password.len() > MAX_PASSWORD_BYTES
		|| form.username.trim().is_empty()
		|| form.password.is_empty()
	{
		return failed_page(words, "");
	}

	let agent = headers
		.get(header::USER_AGENT)
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default()
		.to_string();
	let passed_device = edge.device.as_ref().map(|device| device.id.clone());

	let (account, person) = match authenticate_public(
		&state,
		&form.username,
		form.password,
		ip,
		&agent,
		passed_device.clone(),
	)
	.await
	{
		PublicVerdict::Refused => return too_many_page(words),
		PublicVerdict::Failed => return failed_page(words, &form.username),
		PublicVerdict::Allowed { account, person } => (account, person),
	};

	let conn = state.conn.as_ref();
	let now = Utc::now();

	// a pass that is already in the browser stays: it is never replaced. Otherwise this device
	// gets one, within the person's limit
	let device: Option<access_device::Model> = if passed_device.is_some() {
		None
	} else {
		let label = device_label(&agent);
		let made = async {
			let id = service::unused_device_id(conn, &label).await?;
			service::insert_device_within_limit(
				conn,
				now,
				service::NewDevice {
					id,
					invite_id: None,
					person_id: &account.id,
					label: &label,
					kind: service::device_kind::BROWSER,
					ip: &ip.to_string(),
					agent: &agent,
				},
				person.max_devices,
			)
			.await
		}
		.await;
		match made {
			Ok(Some(device)) => Some(device),
			Ok(None) => {
				device_limit_reached(&state, &account, &person, ip, &agent).await;
				return limit_page(words, person.max_devices);
			},
			Err(error) => {
				tracing::error!(?error, "Public sign-in: could not make a device");
				return failed_page(words, &form.username);
			},
		}
	};

	if let Err(error) = start_session(&state, &session, &account.id).await {
		tracing::error!(?error, "Public sign-in: could not start the session");
		if let Some(device) = &device {
			let _ = service::revoke_device(conn, now, &device.id, "system").await;
		}
		return failed_page(words, &form.username);
	}

	// the sign-in is recorded, and a library that appeared since is hidden before the first page
	if let Ok(Some(login)) = LoginUser::find_by_id(account.id.clone())
		.into_model::<LoginUser>()
		.one(conn)
		.await
	{
		let device_for_mark = device
			.as_ref()
			.map(|device| device.id.clone())
			.or(passed_device);
		record_attempt(
			conn,
			&login,
			agent,
			ip,
			true,
			&SignInMark::counting(device_for_mark, None),
		)
		.await;
	}
	if person.new_libraries_hidden {
		if let Err(error) =
			access_people::hide_undecided_libraries(conn, &account.id).await
		{
			tracing::error!(?error, "Public sign-in: could not hide a new library");
		}
	}

	let mut extra = vec![(header::LOCATION, "/".to_string())];
	if let Some(device) = &device {
		let value = pass::mint_pass(key, &device.id, now.timestamp());
		extra.push((
			header::SET_COOKIE,
			pass::pass_cookie_header(&settings.cookie, &value),
		));
	}
	page(StatusCode::SEE_OTHER, words, "", &extra)
}

/// Starts the session of a person who just signed in: room is made under their session limit,
/// the session id is renewed (so one set before sign-in is useless) and the person is put in it
pub(crate) async fn start_session(
	state: &AppState,
	session: &Session,
	user_id: &str,
) -> Result<(), String> {
	let login = LoginUser::find_by_id(user_id.to_string())
		.into_model::<LoginUser>()
		.one(state.conn.as_ref())
		.await
		.map_err(|error| error.to_string())?
		.ok_or_else(|| "the account is gone".to_string())?;
	enforce_max_sessions(&login, state.conn.as_ref())
		.await
		.map_err(|error| format!("{error:?}"))?;
	session
		.cycle_id()
		.await
		.map_err(|error| error.to_string())?;
	session
		.insert(SESSION_USER_KEY, user_id.to_string())
		.await
		.map_err(|error| error.to_string())
}

/// A right password at the device limit: told to the owner (an audit row the warnings read) and
/// recorded in the sign-in log, never as something that counts towards a lock
async fn device_limit_reached(
	state: &AppState,
	account: &user::Model,
	person: &access_person::Model,
	ip: IpAddr,
	agent: &str,
) {
	let conn = state.conn.as_ref();
	let now = Utc::now();
	let active = service::count_person_devices(conn, now, &account.id)
		.await
		.unwrap_or_default();
	if let Err(error) = service::audit(
		conn,
		now,
		"public",
		"signin.device_limit",
		&account.id,
		&format!(
			"{} uses {active} of {} devices",
			account.username, person.max_devices
		),
	)
	.await
	{
		tracing::error!(?error, "Public sign-in: could not audit the device limit");
	}
	if let Ok(Some(login)) = LoginUser::find_by_id(account.id.clone())
		.into_model::<LoginUser>()
		.one(conn)
		.await
	{
		record_attempt(
			conn,
			&login,
			agent.to_string(),
			ip,
			false,
			&SignInMark::ignored_by_lock(None, reason::DEVICE_LIMIT),
		)
		.await;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_sign_in_form_escapes_what_was_typed_and_has_no_script() {
		let response = signin_page(
			StatusCode::UNAUTHORIZED,
			&super::super::ENGLISH,
			Some("bad <b>"),
			"\"><script>alert(1)</script>",
		);
		assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
		let csp = response
			.headers()
			.get(header::CONTENT_SECURITY_POLICY)
			.and_then(|value| value.to_str().ok())
			.unwrap_or_default()
			.to_string();
		assert!(csp.contains("default-src 'none'"));
		assert!(!csp.contains("script-src"));
	}

	#[tokio::test]
	async fn the_limit_page_names_the_number_of_devices() {
		let response = limit_page(&super::super::SPANISH, 3);
		assert_eq!(response.status(), StatusCode::FORBIDDEN);
		let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
			.await
			.unwrap();
		let text = String::from_utf8_lossy(&body);
		assert!(text.contains("Ya usas 3 dispositivos"), "{text}");
	}
}
