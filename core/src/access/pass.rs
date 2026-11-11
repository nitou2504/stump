//! The pass: the signed cookie the public edge asks for, the invite tokens that make one, and the
//! old invite links the terminal script made.
//!
//! Formats (they are shared with the edge's script, `stump-gate.js`, and pinned by
//! `deploy/public-edge/vectors.json`, which both read):
//!
//! - a **pass** is `<id>.<issued>.<mac>`: `id` is `[a-z0-9-]{1,24}`, `issued` is ten digits of
//!   epoch seconds, `mac` is the unpadded base64url of `HMAC-SHA256(key, "dev|<id>|<issued>")`
//!   (43 characters). The key is the hex string used **as is**: its text bytes are the HMAC key,
//!   it is not hex-decoded;
//! - a **legacy invite** is `<name>.<expiry>.<mac>` with the MAC over `"inv|<name>|<expiry>"`;
//! - an **invite token** is 22 characters of base64url from 16 random bytes. Only its SHA-256 is
//!   stored.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::{
	digest,
	hmac::{self, HMAC_SHA256},
	rand::{SecureRandom, SystemRandom},
};

use super::is_pass_name;

/// A pass is valid for 180 days from the moment it was issued (the edge renews it daily)
pub const PASS_MAX_AGE_SECS: i64 = 180 * 86_400;
/// A pass issued further ahead than this is refused (the clocks of the edge and the app agree
/// to within a day, or something is wrong)
pub const PASS_FUTURE_SLACK_SECS: i64 = 86_400;
/// The length of an invite token as text
pub const INVITE_TOKEN_LEN: usize = 22;

/// The base64url (no padding) of `HMAC-SHA256(key, message)`
fn mac(key: &str, message: &str) -> String {
	let key = hmac::Key::new(HMAC_SHA256, key.as_bytes());
	URL_SAFE_NO_PAD.encode(hmac::sign(&key, message.as_bytes()).as_ref())
}

/// Whether `tag` (base64url text) is the MAC of `message`. The comparison is constant time
fn mac_matches(key: &str, message: &str, tag: &str) -> bool {
	let Ok(tag) = URL_SAFE_NO_PAD.decode(tag) else {
		return false;
	};
	let key = hmac::Key::new(HMAC_SHA256, key.as_bytes());
	hmac::verify(&key, message.as_bytes(), &tag).is_ok()
}

/// A pass for device `id`, issued at `issued` (epoch seconds)
pub fn mint_pass(key: &str, id: &str, issued: i64) -> String {
	let issued = format!("{issued:010}");
	format!("{id}.{issued}.{}", mac(key, &format!("dev|{id}|{issued}")))
}

/// A pass that checked out
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPass {
	pub id: String,
	pub issued: i64,
}

/// Splits `<a>.<b>.<c>` into its three parts, each checked by `shape`
fn split3(text: &str) -> Option<(&str, &str, &str)> {
	let mut parts = text.split('.');
	let (a, b, c) = (parts.next()?, parts.next()?, parts.next()?);
	parts.next().is_none().then_some((a, b, c))
}

fn is_mac_text(text: &str) -> bool {
	text.len() == 43
		&& text
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn is_epoch_text(text: &str) -> bool {
	text.len() == 10 && text.bytes().all(|b| b.is_ascii_digit())
}

/// Checks a pass: its shape, its MAC, and its age at `now` (epoch seconds)
pub fn verify_pass(key: &str, value: &str, now: i64) -> Option<VerifiedPass> {
	let (id, issued_text, tag) = split3(value)?;
	if !is_pass_name(id) || !is_epoch_text(issued_text) || !is_mac_text(tag) {
		return None;
	}
	if !mac_matches(key, &format!("dev|{id}|{issued_text}"), tag) {
		return None;
	}
	let issued: i64 = issued_text.parse().ok()?;
	if now - issued > PASS_MAX_AGE_SECS || issued - now > PASS_FUTURE_SLACK_SECS {
		return None;
	}
	Some(VerifiedPass {
		id: id.to_string(),
		issued,
	})
}

/// The `Set-Cookie` value that hands a pass to a browser: for the length of the pass, to the
/// whole site, never to scripts, only over HTTPS. The `__Host-` name prefix makes a browser
/// refuse it unless all of that holds
pub fn pass_cookie_header(cookie_name: &str, pass: &str) -> String {
	format!(
		"{cookie_name}={pass}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={PASS_MAX_AGE_SECS}"
	)
}

/// A legacy invite (the terminal script's link) that checked out
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyInvite {
	pub name: String,
	pub expiry: i64,
}

/// Checks a legacy invite `<name>.<expiry>.<mac>` at `now`: valid until its expiry
pub fn verify_legacy_invite(key: &str, value: &str, now: i64) -> Option<LegacyInvite> {
	let (name, expiry_text, tag) = split3(value)?;
	if !is_pass_name(name) || !is_epoch_text(expiry_text) || !is_mac_text(tag) {
		return None;
	}
	if !mac_matches(key, &format!("inv|{name}|{expiry_text}"), tag) {
		return None;
	}
	let expiry: i64 = expiry_text.parse().ok()?;
	(now <= expiry).then(|| LegacyInvite {
		name: name.to_string(),
		expiry,
	})
}

/// Makes the legacy link text for `name`, valid until `expiry`. The app never hands these out
/// (the terminal script does); it is here for the tests
pub fn mint_legacy_invite(key: &str, name: &str, expiry: i64) -> String {
	let expiry = format!("{expiry:010}");
	format!(
		"{name}.{expiry}.{}",
		mac(key, &format!("inv|{name}|{expiry}"))
	)
}

/// The SHA-256 of an invite token, as lowercase hex: what is stored
pub fn token_hash(token: &str) -> String {
	data_encoding::HEXLOWER
		.encode(digest::digest(&digest::SHA256, token.as_bytes()).as_ref())
}

/// A new invite token and its hash. Fails only if the system has no random source
pub fn new_invite_token() -> Result<(String, String), String> {
	let mut bytes = [0u8; 16];
	SystemRandom::new()
		.fill(&mut bytes)
		.map_err(|_| "no random source".to_string())?;
	let token = URL_SAFE_NO_PAD.encode(bytes);
	let hash = token_hash(&token);
	Ok((token, hash))
}

/// Whether `text` is shaped like an invite token
pub fn is_invite_token(text: &str) -> bool {
	text.len() == INVITE_TOKEN_LEN
		&& text
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// What somebody typed or pasted into the invite box
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InviteInput {
	Token(String),
	/// A legacy link, as text (checked with [`verify_legacy_invite`])
	Legacy(String),
}

/// Finds the invite in what was pasted: the whole link (`https://…/api/_i/<token>`), or just the
/// token. Anything else is nothing
pub fn parse_invite_input(text: &str) -> Option<InviteInput> {
	let text = text.trim();
	let candidate = match text.rfind("/api/_i/") {
		Some(at) => &text[at + "/api/_i/".len()..],
		None => text,
	};
	let end = candidate
		.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
		.unwrap_or(candidate.len());
	let candidate = &candidate[..end];

	if is_invite_token(candidate) {
		return Some(InviteInput::Token(candidate.to_string()));
	}
	match split3(candidate) {
		Some((name, expiry, tag))
			if is_pass_name(name) && is_epoch_text(expiry) && is_mac_text(tag) =>
		{
			Some(InviteInput::Legacy(candidate.to_string()))
		},
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::Deserialize;

	/// The vectors the edge's script is tested with too: the same key, the same texts
	#[derive(Deserialize)]
	struct Vectors {
		key: String,
		pass: PassVector,
		legacy_invite: LegacyVector,
		forged: Vec<String>,
	}
	#[derive(Deserialize)]
	struct PassVector {
		id: String,
		issued: i64,
		value: String,
	}
	#[derive(Deserialize)]
	struct LegacyVector {
		name: String,
		expiry: i64,
		value: String,
	}

	fn vectors() -> Vectors {
		serde_json::from_str(include_str!("../../../deploy/public-edge/vectors.json"))
			.expect("vectors.json should parse")
	}

	#[test]
	fn the_edges_vectors_verify_here() {
		let v = vectors();
		assert_eq!(mint_pass(&v.key, &v.pass.id, v.pass.issued), v.pass.value);
		assert_eq!(
			verify_pass(&v.key, &v.pass.value, v.pass.issued + 10),
			Some(VerifiedPass {
				id: v.pass.id.clone(),
				issued: v.pass.issued
			})
		);
		assert_eq!(
			mint_legacy_invite(&v.key, &v.legacy_invite.name, v.legacy_invite.expiry),
			v.legacy_invite.value
		);
		assert_eq!(
			verify_legacy_invite(&v.key, &v.legacy_invite.value, v.legacy_invite.expiry),
			Some(LegacyInvite {
				name: v.legacy_invite.name.clone(),
				expiry: v.legacy_invite.expiry
			})
		);
		// none of the forgeries is a pass (the last one is a genuine legacy invite made to look
		// like one: a different kind of MAC)
		for forged in &v.forged {
			assert_eq!(
				verify_pass(&v.key, forged, v.pass.issued + 10),
				None,
				"{forged}"
			);
		}
	}

	#[test]
	fn a_pass_is_bound_to_its_key_name_and_time() {
		let v = vectors();
		let now = v.pass.issued + 10;
		// another key
		assert_eq!(verify_pass(&"x".repeat(64), &v.pass.value, now), None);
		// another name, another time, another tag
		let tampered = v.pass.value.replacen(&v.pass.id, "someone", 1);
		assert_eq!(verify_pass(&v.key, &tampered, now), None);
		let tampered =
			v.pass
				.value
				.replacen(&format!("{}", v.pass.issued), "1700000000", 1);
		assert_eq!(verify_pass(&v.key, &tampered, now), None);
		let mut chars: Vec<char> = v.pass.value.chars().collect();
		let last = chars.len() - 1;
		chars[last] = if chars[last] == 'A' { 'B' } else { 'A' };
		let tampered: String = chars.into_iter().collect();
		assert_eq!(verify_pass(&v.key, &tampered, now), None);
		// a legacy invite is not a pass, and a pass is not an invite
		assert_eq!(verify_pass(&v.key, &v.legacy_invite.value, now), None);
		assert_eq!(verify_legacy_invite(&v.key, &v.pass.value, 0), None);
	}

	#[test]
	fn a_pass_runs_out_after_180_days_and_is_not_from_the_future() {
		let key = "k".repeat(64);
		let issued = 1_794_400_000;
		let pass = mint_pass(&key, "ana-phone", issued);
		assert!(verify_pass(&key, &pass, issued).is_some());
		assert!(verify_pass(&key, &pass, issued + PASS_MAX_AGE_SECS).is_some());
		assert!(verify_pass(&key, &pass, issued + PASS_MAX_AGE_SECS + 1).is_none());
		// a day ahead is allowed (clocks), a day and a second is not
		assert!(verify_pass(&key, &pass, issued - PASS_FUTURE_SLACK_SECS).is_some());
		assert!(verify_pass(&key, &pass, issued - PASS_FUTURE_SLACK_SECS - 1).is_none());
	}

	#[test]
	fn a_legacy_invite_is_valid_until_its_expiry() {
		let key = "k".repeat(64);
		let link = mint_legacy_invite(&key, "sister-phone", 1_794_400_000);
		assert!(verify_legacy_invite(&key, &link, 1_794_400_000).is_some());
		assert!(verify_legacy_invite(&key, &link, 1_794_400_001).is_none());
	}

	#[test]
	fn text_that_is_not_a_pass_is_refused() {
		let key = "k".repeat(64);
		for text in [
			"",
			".",
			"..",
			"a.b.c",
			"ana.0000000001.short",
			"Ana.1794400000.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
			"ana.17944000.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
			"ana.1794400000.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.extra",
		] {
			assert_eq!(verify_pass(&key, text, 1_794_400_000), None, "{text:?}");
		}
	}

	#[test]
	fn the_set_cookie_header_has_every_attribute_a_host_cookie_needs() {
		let header = pass_cookie_header("__Host-g3-stump", "ana.1794400000.abc");
		for part in [
			"__Host-g3-stump=ana.1794400000.abc",
			"Path=/",
			"Secure",
			"HttpOnly",
			"SameSite=Lax",
			"Max-Age=15552000",
		] {
			assert!(header.contains(part), "{part} is missing from {header}");
		}
		assert!(!header.contains("Domain"));
	}

	#[test]
	fn an_invite_token_is_random_and_only_its_hash_is_kept() {
		let (token, hash) = new_invite_token().unwrap();
		let (other, _) = new_invite_token().unwrap();
		assert_ne!(token, other);
		assert!(is_invite_token(&token));
		assert_eq!(hash, token_hash(&token));
		assert_eq!(hash.len(), 64);
		assert!(!hash.contains(&token));
		// a known SHA-256, so the hash is the real one
		assert_eq!(
			token_hash("abc"),
			"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
		);
	}

	#[test]
	fn a_pasted_link_is_understood() {
		let (token, _) = new_invite_token().unwrap();
		let legacy = "ana-phone.1794400000.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
		for text in [
			token.clone(),
			format!("  {token}\n"),
			format!("https://g3.tail3e401.ts.net:8443/api/_i/{token}"),
			format!("https://g3.tail3e401.ts.net:8443/api/_i/{token}?x=1#frag"),
			format!("here you go: https://h/api/_i/{token} thanks"),
		] {
			assert_eq!(
				parse_invite_input(&text),
				Some(InviteInput::Token(token.clone())),
				"{text:?}"
			);
		}
		assert_eq!(
			parse_invite_input(&format!("https://h/api/_i/{legacy}")),
			Some(InviteInput::Legacy(legacy.to_string()))
		);
		for text in [
			"",
			"hello",
			"https://h/api/_i/",
			"https://h/api/_i/short",
			&"a".repeat(300),
		] {
			assert_eq!(parse_invite_input(text), None, "{text:?}");
		}
	}
}
