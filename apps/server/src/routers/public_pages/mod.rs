//! The pages people see **before** they hold a pass or a session, and what they have in common:
//! the device-link pages (`invite`), the sign-in page (`signin`) and the signup form (`signup`).
//!
//! They are small, server-rendered HTML with no JavaScript at all (the web app needs a pass to
//! load, and loading it is the thing these pages are for), in English or Spanish by the
//! browser's `Accept-Language`, one column, inputs of 16 px so a phone does not zoom, light and
//! dark. Every page is `no-store`, `no-referrer`, `noindex`, cannot be framed, and has a CSP
//! that allows nothing but its own inline style and posting to this site. Every piece of text
//! that came from a visitor is HTML-escaped.
//!
//! They answer only through the public edge, and the sign-in page and the signup form only when
//! `STUMP_EDGE_PUBLIC_SIGNIN` is on; otherwise they answer 404 as if they did not exist.

use axum::{
	http::{header, HeaderMap, HeaderValue, StatusCode},
	response::{IntoResponse, Response},
};
use graphql::data::EdgeRequest;
use models::services::access_policy::{PasswordProblem, UsernameProblem};

use crate::{
	config::state::AppState,
	middleware::edge::{is_cross_site, origin_of},
};

pub(crate) mod signin;
pub(crate) mod signup;

/// The sign-in page and the signup form (the device-link pages are in `invite`)
pub(crate) fn mount(state: AppState) -> axum::Router<AppState> {
	signin::mount(state).route("/api/_i/join", axum::routing::post(signup::submit))
}

/// What a page says, in the visitor's language
pub(crate) struct Words {
	pub lang: &'static str,
	// device links
	pub title: &'static str,
	pub ask: &'static str,
	pub button: &'static str,
	pub invalid_title: &'static str,
	pub invalid: &'static str,
	pub paste: &'static str,
	pub open: &'static str,
	// signing in
	pub signin_title: &'static str,
	pub signin_intro: &'static str,
	pub username: &'static str,
	pub password: &'static str,
	pub signin_button: &'static str,
	pub signin_failed: &'static str,
	pub have_link: &'static str,
	pub forgot: &'static str,
	pub too_many_title: &'static str,
	pub too_many: &'static str,
	/// `{n}` is the number of devices
	pub limit_title: &'static str,
	pub limit_body: &'static str,
	// signing up
	pub join_title: &'static str,
	pub join_intro: &'static str,
	pub password_again: &'static str,
	pub rules: &'static str,
	pub join_button: &'static str,
	pub have_account: &'static str,
	pub passwords_differ: &'static str,
	pub name_taken: &'static str,
}

pub(crate) const ENGLISH: Words = Words {
	lang: "en",
	title: "Open Stump on this device?",
	ask:
		"This link lets this browser open Stump. It works for you and your devices only.",
	button: "Open Stump",
	invalid_title: "This link doesn't work",
	invalid: "It has expired, was used up, or isn't valid. Ask for a new invite link.",
	paste: "Paste your link here",
	open: "Open",
	signin_title: "Sign in to Stump",
	signin_intro: "Use the username and password you chose.",
	username: "Username",
	password: "Password",
	signin_button: "Sign in",
	signin_failed:
		"That username or password isn't right. Still stuck? Ask the person who runs this server.",
	have_link: "Have an invite link? Paste it here",
	forgot: "Forgot your password? Ask the person who runs this server.",
	too_many_title: "Too many attempts",
	too_many: "Please wait a few minutes and try again.",
	limit_title: "You already use {n} devices",
	limit_body:
		"To sign in here, remove one of your devices in Settings → My devices on a device where you are signed in, or ask the person who runs this server.",
	join_title: "Create your account",
	join_intro:
		"Choose a username and a password. You will sign in with them on every device.",
	password_again: "Repeat the password",
	rules:
		"Username: 3 to 32 letters, numbers, dots, hyphens or underscores. Password: at least 10 characters, and not a common one.",
	join_button: "Create my account",
	have_account: "Already have an account? Sign in",
	passwords_differ: "The two passwords are different",
	name_taken: "That username is taken. Choose another one",
};

pub(crate) const SPANISH: Words = Words {
	lang: "es",
	title: "¿Abrir Stump en este dispositivo?",
	ask: "Este enlace permite que este navegador abra Stump. Es solo para ti y tus dispositivos.",
	button: "Abrir Stump",
	invalid_title: "Este enlace no funciona",
	invalid: "Caducó, se agotó o no es válido. Pide un enlace de invitación nuevo.",
	paste: "Pega tu enlace aquí",
	open: "Abrir",
	signin_title: "Entra a Stump",
	signin_intro: "Usa el usuario y la contraseña que elegiste.",
	username: "Usuario",
	password: "Contraseña",
	signin_button: "Entrar",
	signin_failed:
		"Ese usuario o contraseña no es correcto. ¿Sigues sin poder entrar? Pregunta a quien administra este servidor.",
	have_link: "¿Tienes un enlace de invitación? Pégalo aquí",
	forgot: "¿Olvidaste tu contraseña? Pregunta a quien administra este servidor.",
	too_many_title: "Demasiados intentos",
	too_many: "Espera unos minutos e inténtalo de nuevo.",
	limit_title: "Ya usas {n} dispositivos",
	limit_body:
		"Para entrar aquí, quita uno de tus dispositivos en Ajustes → Mis dispositivos desde un dispositivo donde ya hayas entrado, o pregunta a quien administra este servidor.",
	join_title: "Crea tu cuenta",
	join_intro:
		"Elige un usuario y una contraseña. Los usarás para entrar en cada dispositivo.",
	password_again: "Repite la contraseña",
	rules:
		"Usuario: de 3 a 32 letras, números, puntos, guiones o guiones bajos. Contraseña: al menos 10 caracteres, y que no sea una común.",
	join_button: "Crear mi cuenta",
	have_account: "¿Ya tienes cuenta? Entra",
	passwords_differ: "Las dos contraseñas son distintas",
	name_taken: "Ese usuario ya existe. Elige otro",
};

pub(crate) fn words(headers: &HeaderMap) -> &'static Words {
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

/// What a username that was refused is told, in the visitor's language
pub(crate) fn username_problem(words: &Words, problem: UsernameProblem) -> &'static str {
	if words.lang == "es" {
		match problem {
			UsernameProblem::TooShort => "El usuario necesita al menos 3 caracteres",
			UsernameProblem::TooLong => "El usuario puede tener 32 caracteres como máximo",
			UsernameProblem::BadCharacter => {
				"Usa solo letras, números, puntos, guiones y guiones bajos en el usuario"
			},
			UsernameProblem::BadStart => "El usuario debe empezar con una letra o un número",
			UsernameProblem::Reserved => "Ese usuario no está disponible",
		}
	} else {
		problem.message()
	}
}

/// What a password that was refused is told, in the visitor's language
pub(crate) fn password_problem(words: &Words, problem: PasswordProblem) -> &'static str {
	if words.lang == "es" {
		match problem {
			PasswordProblem::TooShort => "La contraseña necesita al menos 10 caracteres",
			PasswordProblem::TooLong => "La contraseña es demasiado larga (72 bytes como máximo)",
			PasswordProblem::TooSimple => {
				"La contraseña repite los mismos pocos caracteres: usa más caracteres distintos"
			},
			PasswordProblem::TooCommon => "Esa contraseña es demasiado común: elige otra",
			PasswordProblem::HasUsername => "La contraseña no puede contener el usuario",
		}
	} else {
		problem.message()
	}
}

pub(crate) fn escape_html(text: &str) -> String {
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

const STYLE: &str = ":root{color-scheme:light dark}body{font-family:system-ui,sans-serif;max-width:26rem;margin:8vh auto;padding:0 1rem;line-height:1.5}h1{font-size:1.4rem;margin:.2rem 0 .6rem}label{display:block;margin:.8rem 0 .2rem;font-size:.95rem}input,button{font:inherit;font-size:1rem;padding:.7rem .8rem;border-radius:.5rem;box-sizing:border-box;width:100%}input{border:1px solid #8886;background:transparent;color:inherit;margin:.15rem 0}button{cursor:pointer;margin-top:1rem;border:0;background:#2563eb;color:#fff;font-weight:600}.muted{opacity:.75;font-size:.9rem}.error{background:#dc262622;border:1px solid #dc262666;padding:.6rem .8rem;border-radius:.5rem;margin:.8rem 0}details{margin-top:1.2rem}summary{cursor:pointer}a{color:inherit}";

/// A page: the same headers for all of them (see the module docs). `extra_headers` are appended
/// (a `Set-Cookie`, a `Location`)
pub(crate) fn page(
	status: StatusCode,
	words: &Words,
	body: &str,
	extra_headers: &[(header::HeaderName, String)],
) -> Response {
	let html = format!(
		"<!doctype html><html lang=\"{}\"><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><meta name=robots content=noindex><title>Stump</title><style>{STYLE}</style><body>{body}</body></html>",
		words.lang
	);
	let mut response = (status, html).into_response();
	let headers = response.headers_mut();
	headers.insert(
		header::CONTENT_TYPE,
		HeaderValue::from_static("text/html; charset=utf-8"),
	);
	headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
	headers.insert(
		header::REFERRER_POLICY,
		HeaderValue::from_static("no-referrer"),
	);
	headers.insert("x-robots-tag", HeaderValue::from_static("noindex"));
	headers.insert(
		header::X_CONTENT_TYPE_OPTIONS,
		HeaderValue::from_static("nosniff"),
	);
	headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
	headers.insert(
		header::CONTENT_SECURITY_POLICY,
		HeaderValue::from_static(
			"default-src 'none'; style-src 'unsafe-inline'; form-action 'self'",
		),
	);
	for (name, value) in extra_headers {
		if let Ok(value) = HeaderValue::from_str(value) {
			headers.append(name.clone(), value);
		}
	}
	response
}

/// Whether the device-link pages answer at all: only to a request that came through the public
/// edge, on a server that has a key to sign passes with
pub(crate) fn available(state: &AppState, edge: &EdgeRequest) -> bool {
	edge.via_public
		&& state
			.config
			.edge
			.as_ref()
			.is_some_and(|settings| settings.usable_key().is_some())
}

/// Whether the sign-in page and the signup form answer: [`available`] and the switch is on
pub(crate) fn public_signin_on(state: &AppState, edge: &EdgeRequest) -> bool {
	available(state, edge)
		&& state
			.config
			.edge
			.as_ref()
			.is_some_and(|settings| settings.public_signin)
}

/// Whether a `POST` comes from this site: `Sec-Fetch-Site` decides when the browser sends it
/// (the pages set `Referrer-Policy: no-referrer`, which makes `Origin` `null`, so it can't be
/// relied on then); a browser without it is judged by `Origin`. The origin to compare is the
/// public address when there is one, or else the request's own
pub(crate) fn post_is_same_origin(
	headers: &HeaderMap,
	public_url: Option<&str>,
	own_origin: Option<&str>,
) -> bool {
	let expected = public_url.map(origin_of).or(own_origin);
	!is_cross_site(headers, expected, &[])
}

/// The page every failure of a device or signup link shows, with a box to paste another link
pub(crate) fn invalid_page(words: &Words) -> Response {
	let body = format!(
		"<h1>{}</h1><p>{}</p><form method=\"get\" action=\"/api/_i/\"><label>{}<input name=\"t\" autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\"></label><button>{}</button></form><p class=muted><a href=\"/api/_i/signin\">{}</a></p>",
		words.invalid_title, words.invalid, words.paste, words.open, words.signin_title
	);
	page(StatusCode::NOT_FOUND, words, &body, &[])
}

/// "Too many attempts": the same page whatever the reason (a client, a name, the whole server)
pub(crate) fn too_many_page(words: &Words) -> Response {
	let body = format!("<h1>{}</h1><p>{}</p>", words.too_many_title, words.too_many);
	page(
		StatusCode::TOO_MANY_REQUESTS,
		words,
		&body,
		&[(header::RETRY_AFTER, "300".to_string())],
	)
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
		headers.insert(
			header::ACCEPT_LANGUAGE,
			HeaderValue::from_static("es-EC,es;q=0.9,en;q=0.8"),
		);
		assert_eq!(words(&headers).lang, "es");
		headers.insert(
			header::ACCEPT_LANGUAGE,
			HeaderValue::from_static("en-US,es;q=0.9"),
		);
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
		old.insert(
			header::ORIGIN,
			HeaderValue::from_static("https://g3.test:8443"),
		);
		assert!(post_is_same_origin(&old, public, own));
		old.insert(
			header::ORIGIN,
			HeaderValue::from_static("https://evil.test"),
		);
		assert!(!post_is_same_origin(&old, public, own));
		old.insert(header::ORIGIN, HeaderValue::from_static("null"));
		assert!(!post_is_same_origin(&old, public, own));
	}

	/// Every page has the same number of strings in both languages, none empty, and the Spanish
	/// ones are not left in English
	#[test]
	fn both_languages_have_every_string() {
		let pairs = [
			(ENGLISH.title, SPANISH.title),
			(ENGLISH.ask, SPANISH.ask),
			(ENGLISH.button, SPANISH.button),
			(ENGLISH.invalid_title, SPANISH.invalid_title),
			(ENGLISH.invalid, SPANISH.invalid),
			(ENGLISH.paste, SPANISH.paste),
			(ENGLISH.open, SPANISH.open),
			(ENGLISH.signin_title, SPANISH.signin_title),
			(ENGLISH.signin_intro, SPANISH.signin_intro),
			(ENGLISH.username, SPANISH.username),
			(ENGLISH.password, SPANISH.password),
			(ENGLISH.signin_button, SPANISH.signin_button),
			(ENGLISH.signin_failed, SPANISH.signin_failed),
			(ENGLISH.have_link, SPANISH.have_link),
			(ENGLISH.forgot, SPANISH.forgot),
			(ENGLISH.too_many_title, SPANISH.too_many_title),
			(ENGLISH.too_many, SPANISH.too_many),
			(ENGLISH.limit_title, SPANISH.limit_title),
			(ENGLISH.limit_body, SPANISH.limit_body),
			(ENGLISH.join_title, SPANISH.join_title),
			(ENGLISH.join_intro, SPANISH.join_intro),
			(ENGLISH.password_again, SPANISH.password_again),
			(ENGLISH.rules, SPANISH.rules),
			(ENGLISH.join_button, SPANISH.join_button),
			(ENGLISH.have_account, SPANISH.have_account),
			(ENGLISH.passwords_differ, SPANISH.passwords_differ),
			(ENGLISH.name_taken, SPANISH.name_taken),
		];
		for (english, spanish) in pairs {
			assert!(!english.is_empty() && !spanish.is_empty());
			assert_ne!(english, spanish, "{english}");
		}
		assert!(ENGLISH.limit_title.contains("{n}") && SPANISH.limit_title.contains("{n}"));
	}

	#[test]
	fn problems_are_told_in_both_languages() {
		for problem in [
			UsernameProblem::TooShort,
			UsernameProblem::TooLong,
			UsernameProblem::BadCharacter,
			UsernameProblem::BadStart,
			UsernameProblem::Reserved,
		] {
			assert_ne!(
				username_problem(&ENGLISH, problem),
				username_problem(&SPANISH, problem)
			);
		}
		for problem in [
			PasswordProblem::TooShort,
			PasswordProblem::TooLong,
			PasswordProblem::TooSimple,
			PasswordProblem::TooCommon,
			PasswordProblem::HasUsername,
		] {
			assert_ne!(
				password_problem(&ENGLISH, problem),
				password_problem(&SPANISH, problem)
			);
		}
	}
}
