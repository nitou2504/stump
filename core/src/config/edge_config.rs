use schematic::Config;
use serde::Serialize;

/// The smallest key the passes are signed with: the edge's own script refuses a shorter one too
pub const MIN_EDGE_KEY_LEN: usize = 64;

// see https://moonrepo.github.io/schematic/config/struct/env.html#nested-prefixes, tldr;
// child structs require their own env_prefix
/// The public edge: the proxy that is the only way in from the internet, and the passes it asks
/// for (see `stump_core::access`). Nothing here is read from `Stump.toml` or written to it: the
/// settings come from the environment (`STUMP_EDGE_*`), and the key is never printed
#[derive(Config, Serialize, Clone, PartialEq)]
#[config(env_prefix = "STUMP_EDGE_")]
pub struct EdgeConfig {
	/// The secret the passes and the legacy invite links are signed with: at least 64
	/// characters, the same one the edge has. Without it no pass verifies and no invite can be
	/// made
	#[serde(skip_serializing)]
	pub key: Option<String>,

	/// The name of the pass cookie
	#[setting(default = "__Host-g3-stump")]
	pub cookie: String,

	/// The address people reach Stump at from the internet (an invite link is made from it),
	/// like `https://g3.tail3e401.ts.net:8443`
	pub public_url: Option<String>,

	/// The folder the app writes the edge's state file in (the edge mounts it read-only)
	pub state_dir: Option<String>,

	/// Whether a request that came through the public edge must carry a valid pass: a belt and
	/// braces for an edge that is misconfigured. Turn it on once the edge sends the pass
	#[setting(default = false)]
	pub require_pass: bool,

	/// Whether a pass made from an invite may only sign in as the person the invite was for
	#[setting(default = true)]
	pub binds_person: bool,
}

impl EdgeConfig {
	/// The key, when it is long enough to be one
	pub fn usable_key(&self) -> Option<&str> {
		self.key
			.as_deref()
			.map(str::trim)
			.filter(|key| key.len() >= MIN_EDGE_KEY_LEN)
	}

	/// The public address without a trailing slash
	pub fn public_base_url(&self) -> Option<String> {
		self.public_url
			.as_deref()
			.map(|url| url.trim().trim_end_matches('/').to_string())
			.filter(|url| url.starts_with("https://") || url.starts_with("http://"))
	}

	pub fn state_directory(&self) -> Option<std::path::PathBuf> {
		self.state_dir
			.as_deref()
			.map(str::trim)
			.filter(|dir| !dir.is_empty())
			.map(std::path::PathBuf::from)
	}
}

impl std::fmt::Debug for EdgeConfig {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("EdgeConfig")
			.field("key", &self.key.as_ref().map(|_| "[REDACTED]"))
			.field("cookie", &self.cookie)
			.field("public_url", &self.public_url)
			.field("state_dir", &self.state_dir)
			.field("require_pass", &self.require_pass)
			.field("binds_person", &self.binds_person)
			.finish()
	}
}
