//! The rules for people who come in from the internet: what an account may hold to be one of
//! them (the *friend set*), the names and passwords of accounts made through a signup link, how
//! many devices a person has, and how a browser is named on the device list.
//!
//! Pure functions and constants, no database. `stump_core::access::policy` re-exports this
//! module, so the server and the GraphQL crate find it where the design says.

use std::str::FromStr;

use crate::shared::{enums::UserPermission, permission_set::PermissionSet};

/// What an account may hold and still sign in from the internet, and what a signup link may
/// grant. Everything else (Scan library, Upload files, anything that manages, edits, creates or
/// deletes) is for the owner's own devices at home: **default-deny**, so a permission added to
/// Stump later is out of the set until somebody puts it here.
///
/// *Upload files* is not in the set on purpose: every library upload path needs *Manage
/// library*, so it does nothing useful for a friend, and the owner asked for a preset that never
/// includes it. Friends send books through *Add books*, which waits for the owner's approval.
pub const FRIEND_PERMISSIONS: [UserPermission; 6] = [
	UserPermission::ChangePassword,
	UserPermission::ChangeUsername,
	UserPermission::ChangeAvatar,
	UserPermission::DownloadFile,
	UserPermission::AddBooks,
	UserPermission::AccessSmartList,
];

/// What a signup link grants unless the owner changes it
pub const DEFAULT_PRESET_PERMISSIONS: [UserPermission; 4] = [
	UserPermission::ChangePassword,
	UserPermission::ChangeAvatar,
	UserPermission::DownloadFile,
	UserPermission::AddBooks,
];

/// The library a signup link offers by default, found by name (case-insensitive)
pub const DEFAULT_PRESET_LIBRARY: &str = "Biblioteca";

/// Devices a person may have, unless the owner says otherwise
pub const DEFAULT_MAX_DEVICES: i32 = 3;
pub const MIN_MAX_DEVICES: i32 = 1;
pub const MAX_MAX_DEVICES: i32 = 10;
/// A device (or a reading-app key) unseen for this long no longer counts against the limit
pub const DORMANT_AFTER_DAYS: i64 = 60;

pub const USERNAME_MIN_CHARS: usize = 3;
pub const USERNAME_MAX_CHARS: usize = 32;
pub const PASSWORD_MIN_CHARS: usize = 10;
/// bcrypt silently uses only the first 72 bytes of a password, so a longer one is refused
/// instead of being quietly cut
pub const PASSWORD_MAX_BYTES: usize = 72;
/// A password needs at least this many different characters
pub const PASSWORD_MIN_DISTINCT: usize = 5;

/// Whether `permission` is in the friend set
pub fn is_friend_permission(permission: UserPermission) -> bool {
	FRIEND_PERMISSIONS.contains(&permission)
}

/// The permissions in `permissions` that are outside the friend set, each once, in order
pub fn outside_friend_set(permissions: &[UserPermission]) -> Vec<UserPermission> {
	let mut outside = Vec::new();
	for permission in permissions {
		if !is_friend_permission(*permission) && !outside.contains(permission) {
			outside.push(*permission);
		}
	}
	outside
}

/// The permissions an account holds, from its stored text (`A,B,C`), with the ones they imply:
/// what the rest of Stump checks. Unknown names are dropped, as everywhere else
pub fn permissions_of(stored: Option<&str>) -> Vec<UserPermission> {
	PermissionSet::from(stored.unwrap_or_default().to_string()).resolve_into_vec()
}

/// The permissions an account holds that are outside the friend set, from its stored text
pub fn outside_friend_set_of(stored: Option<&str>) -> Vec<UserPermission> {
	outside_friend_set(&permissions_of(stored))
}

/// Whether a stored permission text holds only friend-set permissions (none at all is fine)
pub fn is_friend_class(stored: Option<&str>) -> bool {
	outside_friend_set_of(stored).is_empty()
}

/// Parses the permission names of an input (`CHANGE_PASSWORD`, ...). `None` for an unknown one
pub fn parse_permissions(names: &[String]) -> Option<Vec<UserPermission>> {
	names
		.iter()
		.map(|name| UserPermission::from_str(name).ok())
		.collect()
}

// -------------------------------------------------------------------------------------------
// usernames
// -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsernameProblem {
	TooShort,
	TooLong,
	/// Something other than letters, digits, `.`, `_` and `-`
	BadCharacter,
	/// Does not start with a letter or a digit
	BadStart,
	/// A name that would be mistaken for the server's own ("admin")
	Reserved,
}

impl UsernameProblem {
	pub fn message(self) -> &'static str {
		match self {
			Self::TooShort => "The username needs at least 3 characters",
			Self::TooLong => "The username can have at most 32 characters",
			Self::BadCharacter => {
				"Use only letters, numbers, dots, hyphens and underscores in the username"
			},
			Self::BadStart => "The username must start with a letter or a number",
			Self::Reserved => "That username is not available",
		}
	}
}

const RESERVED_USERNAMES: [&str; 22] = [
	"admin",
	"administrator",
	"root",
	"owner",
	"stump",
	"system",
	"support",
	"server",
	"staff",
	"moderator",
	"guest",
	"anonymous",
	"public",
	"null",
	"undefined",
	"book-curator",
	"curator",
	"opds",
	"api",
	"me",
	"user",
	"users",
];

/// Checks a username chosen at a signup link and returns it as it is stored: lowercase.
/// 3 to 32 characters of `a-z 0-9 . _ -`, starting with a letter or a digit, not reserved. A
/// capital is lowercased (a phone capitalises the first letter); every other character outside
/// the set, a look-alike from another alphabet included, is refused
pub fn check_username(input: &str) -> Result<String, UsernameProblem> {
	let name = input.trim().to_ascii_lowercase();
	if name.chars().count() < USERNAME_MIN_CHARS {
		return Err(UsernameProblem::TooShort);
	}
	if name.chars().count() > USERNAME_MAX_CHARS {
		return Err(UsernameProblem::TooLong);
	}
	if !name.bytes().all(|b| {
		b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
	}) {
		return Err(UsernameProblem::BadCharacter);
	}
	if !name
		.bytes()
		.next()
		.is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
	{
		return Err(UsernameProblem::BadStart);
	}
	if RESERVED_USERNAMES.contains(&name.as_str()) {
		return Err(UsernameProblem::Reserved);
	}
	Ok(name)
}

// -------------------------------------------------------------------------------------------
// passwords
// -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordProblem {
	TooShort,
	/// More than 72 bytes: bcrypt would ignore the rest
	TooLong,
	/// Fewer than 5 different characters
	TooSimple,
	/// One of the passwords everybody tries first
	TooCommon,
	/// Contains the username
	HasUsername,
}

impl PasswordProblem {
	pub fn message(self) -> &'static str {
		match self {
			Self::TooShort => "The password needs at least 10 characters",
			Self::TooLong => "The password is too long (72 bytes at most)",
			Self::TooSimple => {
				"The password repeats the same few characters: use more different ones"
			},
			Self::TooCommon => "That password is too common: choose another one",
			Self::HasUsername => "The password can't contain the username",
		}
	}
}

/// The passwords people try first, 10 characters or longer (shorter ones are refused by length),
/// lowercase, English and Spanish. A password is compared to this list after lowercasing
const COMMON_PASSWORDS: &[&str] = &[
	"1234567890",
	"0123456789",
	"12345678910",
	"123456789012",
	"1234567890123",
	"0987654321",
	"1q2w3e4r5t",
	"1qaz2wsx3edc",
	"qwertyuiop",
	"qwertyuiop1",
	"qwerty12345",
	"qwerty123456",
	"qwertyuiopasdfghjkl",
	"asdfghjkl1",
	"asdfghjklñ",
	"asdfghjkl123",
	"zxcvbnm123",
	"zxcvbnm1234",
	"abcdefghij",
	"abcdefg123",
	"abc1234567",
	"abcd123456",
	"password12",
	"password123",
	"password1234",
	"password12345",
	"password123456",
	"password1!",
	"password!1",
	"passw0rd123",
	"p@ssw0rd123",
	"p@ssword123",
	"p@ssw0rd!23",
	"letmein123",
	"welcome123",
	"welcome1234",
	"changeme123",
	"iloveyou12",
	"iloveyou123",
	"iloveyou1234",
	"iloveyou123456",
	"monkey1234",
	"dragon1234",
	"football123",
	"baseball123",
	"superman123",
	"trustno1234",
	"starwars123",
	"internet123",
	"administrator",
	"administrador",
	"administrador1",
	"admin12345",
	"admin123456",
	"adminadmin",
	"contrasena",
	"contrasena1",
	"contrasena12",
	"contrasena123",
	"contrasena1234",
	"contraseña",
	"contraseña1",
	"contraseña12",
	"contraseña123",
	"contraseña1234",
	"micontrasena",
	"micontraseña",
	"micontrasena123",
	"mipassword",
	"mipassword1",
	"mipassword123",
	"clave12345",
	"clave123456",
	"claveclave",
	"te quiero 1",
	"tequiero123",
	"tequieromucho",
	"teamo12345",
	"teamo123456",
	"123456789a",
	"123456789abc",
	"a123456789",
	"a1234567890",
	"abc123456789",
	"qwerty1234",
	"1111111111",
	"0000000000",
	"1234512345",
	"1212121212",
	"1122334455",
	"5555555555",
	"stump12345",
	"stump123456",
	"biblioteca1",
	"biblioteca123",
	"libros12345",
	"librolibro",
	"lectura123",
];

/// Checks a password chosen at a signup link (and when a person of the internet changes their
/// own): 10 characters at least, 72 bytes at most (bcrypt's own limit), at least 5 different
/// characters, not one everybody tries first, not containing the username
pub fn check_password(password: &str, username: &str) -> Result<(), PasswordProblem> {
	if password.chars().count() < PASSWORD_MIN_CHARS {
		return Err(PasswordProblem::TooShort);
	}
	if password.len() > PASSWORD_MAX_BYTES {
		return Err(PasswordProblem::TooLong);
	}
	let mut distinct: Vec<char> = password.chars().collect();
	distinct.sort_unstable();
	distinct.dedup();
	if distinct.len() < PASSWORD_MIN_DISTINCT {
		return Err(PasswordProblem::TooSimple);
	}
	let lower = password.trim().to_lowercase();
	if COMMON_PASSWORDS.contains(&lower.as_str()) {
		return Err(PasswordProblem::TooCommon);
	}
	let name = username.trim().to_lowercase();
	if name.chars().count() >= USERNAME_MIN_CHARS && lower.contains(&name) {
		return Err(PasswordProblem::HasUsername);
	}
	Ok(())
}

// -------------------------------------------------------------------------------------------
// the name of a device
// -------------------------------------------------------------------------------------------

/// A short name for the device list, from the browser's `User-Agent`: "Chrome on Android",
/// "Safari on iOS", "Firefox on Windows". "Browser" when nothing is recognised. Plain text, at
/// most 40 characters, and never the user agent itself
pub fn device_label(user_agent: &str) -> String {
	let agent = user_agent;
	let browser =
		if agent.contains("Edg/") || agent.contains("EdgA/") || agent.contains("EdgiOS/")
		{
			"Edge"
		} else if agent.contains("OPR/") || agent.contains("Opera") {
			"Opera"
		} else if agent.contains("SamsungBrowser") {
			"Samsung Internet"
		} else if agent.contains("Firefox/") || agent.contains("FxiOS/") {
			"Firefox"
		} else if agent.contains("Chrome/")
			|| agent.contains("CriOS/")
			|| agent.contains("Chromium/")
		{
			"Chrome"
		} else if agent.contains("Safari/") && agent.contains("Version/") {
			"Safari"
		} else {
			""
		};
	let system = if agent.contains("Android") {
		"Android"
	} else if agent.contains("iPhone") || agent.contains("iPad") || agent.contains("iPod")
	{
		"iOS"
	} else if agent.contains("Windows") {
		"Windows"
	} else if agent.contains("CrOS") {
		"ChromeOS"
	} else if agent.contains("Macintosh") || agent.contains("Mac OS X") {
		"macOS"
	} else if agent.contains("Linux") || agent.contains("X11") {
		"Linux"
	} else {
		""
	};
	match (browser, system) {
		("", "") => "Browser".to_string(),
		("", system) => format!("Browser on {system}"),
		(browser, "") => browser.to_string(),
		(browser, system) => format!("{browser} on {system}"),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_friend_set_is_the_six_permissions_and_nothing_that_manages() {
		assert_eq!(FRIEND_PERMISSIONS.len(), 6);
		for allowed in [
			UserPermission::ChangePassword,
			UserPermission::ChangeUsername,
			UserPermission::ChangeAvatar,
			UserPermission::DownloadFile,
			UserPermission::AddBooks,
			UserPermission::AccessSmartList,
		] {
			assert!(is_friend_permission(allowed), "{allowed}");
		}
		// B2: Upload files is out, and so is everything that scans, manages, edits or deletes
		for refused in [
			UserPermission::UploadFile,
			UserPermission::ScanLibrary,
			UserPermission::ManageLibrary,
			UserPermission::CreateLibrary,
			UserPermission::EditLibrary,
			UserPermission::DeleteLibrary,
			UserPermission::EditMetadata,
			UserPermission::WriteBackMetadata,
			UserPermission::RemoveBook,
			UserPermission::ManageUsers,
			UserPermission::ReadUsers,
			UserPermission::ManageServer,
			UserPermission::ManageJobs,
			UserPermission::FileExplorer,
			UserPermission::AccessApiKeys,
			UserPermission::AccessKoreaderSync,
			UserPermission::AccessKoboSync,
			UserPermission::EmailArbitrarySend,
			UserPermission::ReadSystemLogs,
		] {
			assert!(!is_friend_permission(refused), "{refused}");
		}
	}

	#[test]
	fn the_default_preset_is_inside_the_friend_set_and_has_no_upload_or_scan() {
		for permission in DEFAULT_PRESET_PERMISSIONS {
			assert!(is_friend_permission(permission), "{permission}");
		}
		assert!(!DEFAULT_PRESET_PERMISSIONS.contains(&UserPermission::UploadFile));
		assert!(!DEFAULT_PRESET_PERMISSIONS.contains(&UserPermission::ScanLibrary));
	}

	#[test]
	fn stored_permissions_are_judged_with_what_they_imply() {
		assert!(is_friend_class(None));
		assert!(is_friend_class(Some("")));
		assert!(is_friend_class(Some("CHANGE_PASSWORD,ADD_BOOKS")));
		// Dani today: Upload files and Scan library keep her out
		let dani = "CHANGE_PASSWORD,CHANGE_USERNAME,CHANGE_AVATAR,DOWNLOAD_FILE,UPLOAD_FILE,SCAN_LIBRARY,ADD_BOOKS";
		assert_eq!(
			outside_friend_set_of(Some(dani)),
			vec![UserPermission::UploadFile, UserPermission::ScanLibrary]
		);
		// a permission that implies others is outside through what it implies, too
		let outside = outside_friend_set_of(Some("MANAGE_LIBRARY"));
		assert!(outside.contains(&UserPermission::ManageLibrary));
		assert!(outside.contains(&UserPermission::ScanLibrary));
		// an unknown name is dropped (it grants nothing), as everywhere else
		assert!(is_friend_class(Some("NOT_A_PERMISSION,DOWNLOAD_FILE")));
	}

	#[test]
	fn permission_names_parse_or_the_whole_list_is_refused() {
		assert_eq!(
			parse_permissions(&["DOWNLOAD_FILE".to_string(), "ADD_BOOKS".to_string()]),
			Some(vec![UserPermission::DownloadFile, UserPermission::AddBooks])
		);
		assert_eq!(
			parse_permissions(&["DOWNLOAD_FILE".to_string(), "NOPE".to_string()]),
			None
		);
	}

	#[test]
	fn usernames_follow_the_rules() {
		assert_eq!(check_username("ana"), Ok("ana".to_string()));
		assert_eq!(
			check_username("  Ana.Lopez-2_x "),
			Ok("ana.lopez-2_x".to_string())
		);
		assert_eq!(check_username("a1b"), Ok("a1b".to_string()));
		assert_eq!(check_username("9lives"), Ok("9lives".to_string()));
		assert_eq!(check_username("ab"), Err(UsernameProblem::TooShort));
		assert_eq!(check_username(""), Err(UsernameProblem::TooShort));
		assert_eq!(check_username(&"a".repeat(32)), Ok("a".repeat(32)));
		assert_eq!(
			check_username(&"a".repeat(33)),
			Err(UsernameProblem::TooLong)
		);
		assert_eq!(
			check_username("ana lopez"),
			Err(UsernameProblem::BadCharacter)
		);
		assert_eq!(
			check_username("ana@mail.com"),
			Err(UsernameProblem::BadCharacter)
		);
		assert_eq!(
			check_username("ana/../x"),
			Err(UsernameProblem::BadCharacter)
		);
		assert_eq!(
			check_username("<b>ana</b>"),
			Err(UsernameProblem::BadCharacter)
		);
		assert_eq!(check_username("-ana"), Err(UsernameProblem::BadStart));
		assert_eq!(check_username(".ana"), Err(UsernameProblem::BadStart));
		assert_eq!(check_username("_ana"), Err(UsernameProblem::BadStart));
		assert_eq!(check_username("Admin"), Err(UsernameProblem::Reserved));
		assert_eq!(check_username("root"), Err(UsernameProblem::Reserved));
		assert_eq!(
			check_username("book-curator"),
			Err(UsernameProblem::Reserved)
		);
	}

	#[test]
	fn a_lookalike_from_another_alphabet_is_not_a_username() {
		// Cyrillic "а" (U+0430) and a Turkish dotted capital i, for "ana" and "ilsa"
		assert_eq!(
			check_username("\u{0430}na"),
			Err(UsernameProblem::BadCharacter)
		);
		assert_eq!(
			check_username("\u{0130}lsa"),
			Err(UsernameProblem::BadCharacter)
		);
		assert_eq!(
			check_username("ana\u{200b}"),
			Err(UsernameProblem::BadCharacter)
		);
		assert_eq!(check_username("ñandú"), Err(UsernameProblem::BadCharacter));
		// a name that differs only in case is the same name once stored
		assert_eq!(check_username("ANA"), check_username("ana"));
	}

	#[test]
	fn passwords_follow_the_rules() {
		assert_eq!(check_password("tulip-Harbor-47", "ana"), Ok(()));
		// 9 and 10 characters
		assert_eq!(
			check_password("abcdefg12", "ana"),
			Err(PasswordProblem::TooShort)
		);
		assert_eq!(check_password("abcdefg12x", "ana"), Ok(()));
		// 72 and 73 bytes
		let p72 = format!("{}xy", "abcde".repeat(14));
		assert_eq!(p72.len(), 72);
		assert_eq!(check_password(&p72, "ana"), Ok(()));
		assert_eq!(
			check_password(&format!("{p72}z"), "ana"),
			Err(PasswordProblem::TooLong)
		);
		// bytes, not characters: 40 two-byte characters are 80 bytes
		assert_eq!(
			check_password(&"é".repeat(40), "ana"),
			Err(PasswordProblem::TooLong)
		);
		// 4 and 5 different characters
		assert_eq!(
			check_password("abababababab", "ana"),
			Err(PasswordProblem::TooSimple)
		);
		assert_eq!(
			check_password("abcdabcdabcd", "ana"),
			Err(PasswordProblem::TooSimple)
		);
		assert_eq!(check_password("abcdeabcdeabcde", "ana"), Ok(()));
		assert_eq!(
			check_password("aaaaaaaaaaaa", "ana"),
			Err(PasswordProblem::TooSimple)
		);
	}

	#[test]
	fn the_common_passwords_are_refused_in_english_and_spanish_whatever_the_case() {
		for password in [
			"1234567890",
			"qwertyuiop",
			"Password123",
			"PASSWORD123",
			"iloveyou123",
			"contrasena123",
			"Contraseña123",
			"tequiero123",
		] {
			assert_eq!(
				check_password(password, "ana"),
				Err(PasswordProblem::TooCommon),
				"{password}"
			);
		}
	}

	#[test]
	fn a_password_with_the_username_in_it_is_refused() {
		assert_eq!(
			check_password("xxana-lopez-77", "ana-lopez"),
			Err(PasswordProblem::HasUsername)
		);
		assert_eq!(
			check_password("My-ANA-lopez-2026", "Ana-Lopez"),
			Err(PasswordProblem::HasUsername)
		);
		// too short a name to mean anything (and the rule is for real usernames)
		assert_eq!(check_password("xxx-purple-zebra", "xx"), Ok(()));
	}

	#[test]
	fn a_device_is_named_after_its_browser_and_system() {
		for (agent, label) in [
			(
				"Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36",
				"Chrome on Android",
			),
			(
				"Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
				"Safari on iOS",
			),
			(
				"Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:127.0) Gecko/20100101 Firefox/127.0",
				"Firefox on Windows",
			),
			(
				"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0",
				"Edge on Windows",
			),
			(
				"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
				"Safari on macOS",
			),
			(
				"Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/126.0.6478.153 Mobile/15E148 Safari/604.1",
				"Chrome on iOS",
			),
			(
				"Mozilla/5.0 (Linux; Android 13; SAMSUNG SM-S918B) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/24.0 Chrome/117.0.0.0 Mobile Safari/537.36",
				"Samsung Internet on Android",
			),
			(
				"Mozilla/5.0 (X11; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0",
				"Firefox on Linux",
			),
			("curl/8.5.0", "Browser"),
			("", "Browser"),
			("Mozilla/5.0 (Windows NT 10.0)", "Browser on Windows"),
		] {
			assert_eq!(device_label(agent), label, "{agent}");
		}
		// never the user agent itself, never long
		assert!(device_label(&"x".repeat(5000)).chars().count() <= 40);
	}
}
