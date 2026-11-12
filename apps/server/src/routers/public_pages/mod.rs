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
	// the front door with public sign-in off
	pub private_title: &'static str,
	pub private_text: &'static str,
	// signing in
	pub signin_title: &'static str,
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
	ask: "This link opens Stump in this browser, just for you.",
	button: "Open Stump",
	invalid_title: "This link doesn't work",
	invalid: "It expired, was used up, or isn't valid. Ask for a new one.",
	paste: "Paste your link here",
	open: "Open",
	private_title: "Private site",
	private_text: "Paste your invite link:",
	signin_title: "Sign in",
	username: "Username",
	password: "Password",
	signin_button: "Sign in",
	signin_failed: "Wrong username or password.",
	have_link: "Have an invite link?",
	forgot: "Forgot your password? Ask the owner.",
	too_many_title: "Too many attempts",
	too_many: "Wait a few minutes and try again.",
	limit_title: "You already use {n} devices",
	limit_body: "Remove one in Settings → My devices, or ask the owner.",
	join_title: "Create your account",
	password_again: "Repeat the password",
	rules: "Username: 3 to 32 characters. Password: at least 10.",
	join_button: "Create my account",
	have_account: "Have an account? Sign in",
	passwords_differ: "The passwords don't match",
	name_taken: "That username is taken",
};

pub(crate) const SPANISH: Words = Words {
	lang: "es",
	title: "¿Abrir Stump en este dispositivo?",
	ask: "Este enlace abre Stump en este navegador, solo para ti.",
	button: "Abrir Stump",
	invalid_title: "Este enlace no funciona",
	invalid: "Caducó, se agotó o no es válido. Pide uno nuevo.",
	paste: "Pega tu enlace aquí",
	open: "Abrir",
	private_title: "Sitio privado",
	private_text: "Pega tu enlace de invitación:",
	signin_title: "Entrar",
	username: "Usuario",
	password: "Contraseña",
	signin_button: "Entrar",
	signin_failed: "Usuario o contraseña incorrectos.",
	have_link: "¿Tienes un enlace de invitación?",
	forgot: "¿Olvidaste tu contraseña? Pregunta al administrador.",
	too_many_title: "Demasiados intentos",
	too_many: "Espera unos minutos e inténtalo de nuevo.",
	limit_title: "Ya usas {n} dispositivos",
	limit_body: "Quita uno en Ajustes → Mis dispositivos, o pregunta al administrador.",
	join_title: "Crea tu cuenta",
	password_again: "Repite la contraseña",
	rules: "Usuario: de 3 a 32 caracteres. Contraseña: al menos 10.",
	join_button: "Crear mi cuenta",
	have_account: "¿Ya tienes cuenta? Entra",
	passwords_differ: "Las contraseñas no coinciden",
	name_taken: "Ese usuario ya existe",
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
			UsernameProblem::TooLong => {
				"El usuario puede tener 32 caracteres como máximo"
			},
			UsernameProblem::BadCharacter => {
				"Usa solo letras, números, puntos, guiones y guiones bajos en el usuario"
			},
			UsernameProblem::BadStart => {
				"El usuario debe empezar con una letra o un número"
			},
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

// The app's own look (the dark and light themes' colours, the brand gradient, the login screen's
// layout), inline: these pages are served without a pass, so they can't load the app's files
const STYLE: &str = ":root{color-scheme:dark;--bg:#161719;--fg:#f5f3ef;--muted:#898d94;--border:#292c30;--input:#1b1c1d;--brand:#c48259;--brand-dark:#a9663c;--danger:#e53935}@media (prefers-color-scheme:light){:root{color-scheme:light;--bg:#fafafa;--fg:#26272a;--muted:#737373;--border:#e5e5e5;--input:#fff;--danger:#dc2626}}*{box-sizing:border-box}body{margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;background:var(--bg);color:var(--fg);font-family:Inter,ui-sans-serif,system-ui,-apple-system,sans-serif;line-height:1.5}main{width:100%;max-width:22rem;padding:2rem 1.5rem}header{display:flex;align-items:center;justify-content:center;gap:1rem;margin-bottom:2rem}header img{width:80px;height:80px}.brand{font-size:1.875rem;font-weight:700;background:linear-gradient(to right,var(--brand-dark),var(--brand));-webkit-background-clip:text;background-clip:text;color:transparent}h1{font-size:1.125rem;font-weight:600;margin:0 0 .75rem;text-align:center}p{margin:.5rem 0}label{display:block;margin:.9rem 0 .35rem;font-size:.875rem;font-weight:500}input,button{font:inherit;font-size:.95rem;border-radius:.375rem;width:100%}input{height:2.5rem;padding:0 .75rem;border:1px solid var(--border);background:var(--input);color:var(--fg)}input:focus{outline:2px solid var(--brand);outline-offset:1px;border-color:transparent}button{height:2.5rem;cursor:pointer;margin-top:1.25rem;border:0;background:var(--brand);color:#fff;font-weight:500}button:hover{background:var(--brand-dark)}.error{color:var(--danger);font-size:.875rem;margin:.9rem 0 0}.hint{color:var(--muted);font-size:.8rem;margin:.4rem 0 0}.muted{color:var(--muted);font-size:.85rem;text-align:center;margin-top:1.5rem}details{margin-top:1.5rem;font-size:.85rem;color:var(--muted);text-align:center}summary{cursor:pointer}details form{text-align:left}a{color:var(--muted)}a:hover{color:var(--fg)}.sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}";

/// The app's logo as a `data:` address (the page can't fetch it: see [`STYLE`])
static LOGO: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
	use base64::{engine::general_purpose::STANDARD, Engine};
	format!(
		"data:image/webp;base64,{}",
		STANDARD.encode(include_bytes!("stump-logo.webp"))
	)
});

/// A page: the same headers for all of them (see the module docs). `extra_headers` are appended
/// (a `Set-Cookie`, a `Location`)
pub(crate) fn page(
	status: StatusCode,
	words: &Words,
	body: &str,
	extra_headers: &[(header::HeaderName, String)],
) -> Response {
	let html = format!(
		"<!doctype html><html lang=\"{}\"><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><meta name=robots content=noindex><title>Stump</title><style>{STYLE}</style><body><main><header><img src=\"{logo}\" alt=\"\" width=80 height=80><span class=brand>Stump</span></header>{body}</main></body></html>",
		words.lang,
		logo = *LOGO
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
			"default-src 'none'; style-src 'unsafe-inline'; img-src data:; form-action 'self'",
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

/// What the front door shows while public sign-in is off: the old "private site" page, a box to
/// paste an invite link into (the sign-in form and the signup form answer 404 until it is on)
pub(crate) fn private_page(words: &Words) -> Response {
	let body = format!(
		"<h1>{}</h1><p>{}</p><form method=\"get\" action=\"/api/_i/\"><input name=\"t\" autocomplete=\"off\" autocapitalize=\"off\" spellcheck=\"false\" aria-label=\"{}\"><button>{}</button></form>",
		words.private_title, words.private_text, words.paste, words.open
	);
	page(StatusCode::OK, words, &body, &[])
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
			(ENGLISH.private_title, SPANISH.private_title),
			(ENGLISH.private_text, SPANISH.private_text),
			(ENGLISH.signin_title, SPANISH.signin_title),
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
		assert!(
			ENGLISH.limit_title.contains("{n}") && SPANISH.limit_title.contains("{n}")
		);
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
