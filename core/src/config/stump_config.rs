//! Contains the [`StumpConfig`] struct and related functions for loading and saving configuration
//! values for a Stump application.
//!
//! Note: [`StumpConfig`] is constructed _before_ tracing is initializing. This is because the
//! configuration is used to determine the log file path and verbosity level. This means that any
//! logging that occurs during the construction of the [`StumpConfig`] should be done using the
//! standard `println!` or `eprintln!` macros.

use std::path::{Path, PathBuf};

use async_graphql::SimpleObject;
use schematic::{Config, ConfigLoader};
use serde::Serialize;

use crate::{
	config::{get_default_config_dir, OidcConfig, PartialOidcConfig},
	CoreError, CoreResult,
};

pub const CONFIG_DIR_KEY: &str = "STUMP_CONFIG_DIR";
pub const IN_DOCKER_KEY: &str = "STUMP_IN_DOCKER";

const DEFAULT_SESSION_TTL: i64 = 3600 * 24 * 3; // 3 days
const DEFAULT_ACCESS_TOKEN_TTL: i64 = 3600 * 24; // 1 day
const DEFAULT_REFRESH_TOKEN_TTL: i64 = 3600 * 24 * 30; // 30 days
const DEFAULT_UPLOAD_SIZE: usize = 20 * 1024 * 1024; // 20 MB

// TODO(env): i think DatabaseConfig enum with e.g. SQLite and Postgres variants would be nice
// TODO(postgres): the vars are not toml-supported atm, not sure if this really matters. i kept it
// like that bc idk what to do about the password, and having all of the config except password in toml
// felt funny? idk ill wait until someone complains maybe >:)

#[derive(Config, Serialize, Debug, Clone, PartialEq, SimpleObject)]
#[config(env_prefix = "STUMP_")]
pub struct StumpConfig {
	/// The IP address on which to listen on (default: "0.0.0.0")
	#[setting(default = "0.0.0.0")]
	pub ip: String,

	/// The port from which to serve the application (default: 10801)
	#[setting(default = 10801)]
	pub port: u16,

	/// The verbosity with which system logs are visible (default: 1)
	/// 0 = none at all, 1 = info, 2 = debug, 3 = trace
	#[setting(default = 1)]
	pub verbosity: u64,

	/// Whether or not to pretty print logs
	#[setting(default = true)]
	pub pretty_logs: bool,

	/// Whether or not to include ANSI color codes in log files
	#[setting(default = false)]
	pub colorful_logs: bool,

	/// The directory where the applicaiton logs will be stored. If unspecified,
	/// logs will be stored in the config_dir
	pub log_dir: Option<String>,

	/// An optional custom path for the database. If set, this assumes SQLite.
	pub db_path: Option<String>,

	/// The timeout in seconds for database connections
	#[setting(default = 30)]
	pub db_timeout_secs: u64,

	/// The directory where the web app bundle lives, which the server will serve as
	/// static files
	#[setting(default = "./client")]
	pub client_dir: String,

	// due to the way schematic works, this field will be loaded first and used
	// to kick off the rest of the config loading process
	/// The configuration root for the Stump application
	#[setting(default = "super::get_default_config_dir()")]
	pub config_dir: String,

	/// A comma-separated list of origins for CORS
	#[setting(
		default = vec![],
		parse_env = schematic::env::split_comma
	)]
	pub allowed_origins: Vec<String>,

	/// Path to the PDFium binary for enabling PDF support
	pub pdfium_path: Option<String>,

	/// Indicates if the GraphQL playground should be enabled. If true, the server
	/// will allow GET requests to the GraphQL endpoint and serve the playground UI
	#[setting(default = false)]
	pub enable_playground: bool,

	/// Indicates if the KoReader sync feature should be enabled
	#[setting(default = false)]
	pub enable_koreader_sync: bool,

	/// Indicates if the Kobo sync feature should be enabled
	#[setting(default = false)]
	pub enable_kobo_sync: bool,

	/// Indicates if OPDS page access should automatically track reading progression.
	/// When disabled, clients loading/preloading pages won't trigger progress updates.
	#[setting(default = false)]
	pub enable_opds_progression: bool,

	/// Password hash cost
	#[setting(default = 12)]
	pub password_hash_cost: u32,

	/// The time in seconds that a login session will be valid for
	#[setting(default = DEFAULT_SESSION_TTL)]
	pub session_ttl: i64,

	/// The time in seconds that an access token will be valid for
	#[setting(default = DEFAULT_ACCESS_TOKEN_TTL)]
	pub access_token_ttl: i64,

	/// The time in seconds that a refresh token will be valid for
	#[setting(default = DEFAULT_REFRESH_TOKEN_TTL)]
	pub refresh_token_ttl: i64,

	/// The interval in seconds at which expired sessions will be cleaned up
	#[setting(default = 86400)]
	pub expired_session_cleanup_interval: u64,

	/// A multiplier applied to the number of logical CPUs to derive the default scanner concurrency
	/// limit. Increasing can speed things up but will increase resource usage
	#[setting(default = 2)]
	pub parallelism_multiplier: usize,

	/// The maximum size in bytes of an image upload
	#[setting(default = DEFAULT_UPLOAD_SIZE)]
	pub max_image_upload_size: usize,

	/// Whether or not the server will allow users with the appropriate permissions
	/// to upload books, series, or other valid uploadable content
	#[setting(default = false)]
	pub enable_upload: bool,

	/// The maximum size in bytes of a file upload
	#[setting(default = DEFAULT_UPLOAD_SIZE)]
	pub max_file_upload_size: usize,

	/// The DPI (dots per inch) to use when rendering PDF pages as images
	#[setting(default = 150)]
	pub pdf_render_dpi: u32,

	/// The maximum width or height dimension for rendered PDF pages
	#[setting(default = 1200)]
	pub pdf_max_dimension: u32,

	/// The image format to use for rendered PDF pages (webp, png, jpeg)
	#[setting(default = "webp")]
	pub pdf_render_format: String,

	/// Whether to enable disk caching for rendered PDF pages
	#[setting(default = true)]
	pub pdf_cache_pages: bool,

	/// Number of pages to pre-render before and after the current page
	#[setting(default = 5)]
	pub pdf_prerender_range: u32,

	/// Whether to enable high-quality rendering with smoothing (slower but better quality)
	#[setting(default = true)]
	pub pdf_high_quality: bool,

	/// The size cap of the rendered PDF page cache in MB (0 = no cap). The oldest pages are
	/// removed when it is reached
	#[setting(default = 1024)]
	pub pdf_cache_max_mb: u64,

	/// The widest page picture (px) the PDF reader may request: 720, 1000, 1400, 2000 or 2800.
	/// Wider pictures are sharper on big HiDPI screens but take more memory to render
	#[setting(default = 2000)]
	pub pdf_max_tier: u32,

	/// The most megabytes read from one entry of an archive (a zip, a rar) that is read whole:
	/// a page, a cover, a book inside an uploaded pack. A bigger entry is refused, however little
	/// the archive says it holds (a few KiB of deflate can inflate to gigabytes)
	#[setting(default = 256)]
	#[graphql(skip)]
	pub archive_max_entry_mb: u64,

	/// The most megabytes read from one metadata text entry of an archive (`ComicInfo.xml`, an
	/// EPUB's package file)
	#[setting(default = 16)]
	#[graphql(skip)]
	pub archive_max_xml_mb: u64,

	/// The most megabytes unpacked from one archive that is unpacked as a whole (an uploaded
	/// `.zip` pack of books)
	#[setting(default = 4096)]
	#[graphql(skip)]
	pub archive_max_unpacked_mb: u64,

	/// The most books taken out of one uploaded `.zip` pack. Each is an open file until it is
	/// staged, and a container is usually allowed 1,024
	#[setting(default = 200)]
	#[graphql(skip)]
	pub archive_max_pack_books: u64,

	/// The most megabytes in one "Add books" upload request (a request holds many files). The
	/// server answers a bigger one before it reads the body. A reverse proxy in front of Stump
	/// needs a limit of at least this
	#[setting(default = 300)]
	#[graphql(skip)]
	pub intake_max_request_mb: u64,

	/// The biggest single file, in MB, someone who needs approval may upload (an approver's own
	/// uploads follow `max_file_upload_size`). A user can have a limit of their own
	#[setting(default = 300)]
	#[graphql(skip)]
	pub intake_max_file_mb: u64,

	/// The most files in one "Add books" upload request of someone who needs approval. Those who
	/// file books themselves (the server owner, anyone who manages libraries) may send as many as
	/// any request can hold (500) unless an admin gave them a number of their own
	#[setting(default = 50)]
	#[graphql(skip)]
	pub intake_max_files_per_request: u64,

	/// The most MB of a user's uploads that wait in their inbox at once (staged, not yet filed
	/// or discarded). A user can have a limit of their own
	#[setting(default = 2048)]
	#[graphql(skip)]
	pub intake_user_quota_mb: u64,

	/// The most inbox items a user can have open at once (not filed, discarded or sent back)
	#[setting(default = 100)]
	#[graphql(skip)]
	pub intake_user_max_items: u64,

	/// Uploads are refused when the disk holding the config folder has less than this many GB
	/// free, so a full inbox can't fill the disk the database lives on
	#[setting(default = 20)]
	#[graphql(skip)]
	pub intake_min_free_gb: u64,

	/// The most GB of uploads that wait in all the inboxes together
	#[setting(default = 50)]
	#[graphql(skip)]
	pub intake_total_max_gb: u64,

	/// An upload that sends nothing for this many seconds is given up (and what it staged is
	/// removed), so a stalled connection can't hold a slot of the quota
	#[setting(default = 60)]
	#[graphql(skip)]
	pub intake_upload_idle_secs: u64,

	/// How the files of people who need approval are checked. `off`: they may not upload at all.
	/// `inprocess`: they are read inside the server like any other book, which is only for a
	/// server whose uploaders are trusted: until the isolated checker exists, grant "Add books"
	/// to friends only with this set deliberately
	#[setting(default = "off")]
	#[graphql(skip)]
	pub intake_probe_mode: String,

	/// OIDC authentication configuration
	#[setting(nested)]
	#[graphql(skip)]
	pub oidc: Option<OidcConfig>,

	/// Whether to trust proxy headers for determining client IP and scheme (e.g., X-Forwarded-For)
	#[setting(default = false)]
	pub trust_proxy_headers: bool,

	/// No longer used (kept so an existing `Stump.toml` or `STUMP_QUARANTINE_DIR` still loads):
	/// the originals post-processing replaces are kept in `<config_dir>/trash` and listed in
	/// *Deleted books*, and books deleted permanently are deleted. Files already in that folder
	/// are left alone
	pub quarantine_dir: Option<String>,
}

impl StumpConfig {
	/// Load the config with the following precedence:
	/// env vars > Stump.toml > defaults
	pub fn load(path: impl AsRef<Path>) -> CoreResult<Self> {
		let config_dir = path.as_ref().to_path_buf();
		let toml_path = config_dir.join("Stump.toml");

		let config_dir_snippet = format!("config_dir = {:?}", config_dir);

		let mut loader = ConfigLoader::<StumpConfig>::new();
		loader.code(config_dir_snippet, "base.toml")?;

		if toml_path.exists() {
			loader.file(toml_path)?;
		}

		let result = loader.load()?;

		let config = result.config;

		Ok(config)
	}

	pub fn debug() -> Self {
		Self {
			client_dir: env!("CARGO_MANIFEST_DIR").to_string() + "/../web/dist",
			config_dir: get_default_config_dir(),
			enable_koreader_sync: true,
			enable_kobo_sync: true,
			enable_upload: true,
			// a development machine has no isolated checker and no disk floor to keep
			intake_probe_mode: "inprocess".to_string(),
			intake_min_free_gb: 0,
			..Default::default()
		}
	}

	pub fn config_directory(&self) -> PathBuf {
		PathBuf::from(&self.config_dir)
	}

	pub fn log_directory(&self) -> PathBuf {
		match &self.log_dir {
			Some(v) => PathBuf::from(v),
			None => self.config_directory(),
		}
	}

	pub fn cache_directory(&self) -> PathBuf {
		self.config_directory().join("cache")
	}

	pub fn thumbnails_directory(&self) -> PathBuf {
		self.config_directory().join("thumbnails")
	}

	pub fn avatars_directory(&self) -> PathBuf {
		self.config_directory().join("avatars")
	}

	pub fn emojis_directory(&self) -> PathBuf {
		self.config_directory().join("emojis")
	}

	pub fn pdf_cache_directory(&self) -> PathBuf {
		self.cache_directory().join("pdf_pages")
	}

	/// Work files of post-processing (a converted CBZ before it replaces the original)
	pub fn post_processing_work_directory(&self) -> PathBuf {
		self.cache_directory().join("post_processing")
	}

	/// The PDF queue shared with the PDF worker container (`docker/Dockerfile.pdf-worker`):
	/// `queue/`, `running/`, `done/` and `archive/`
	pub fn pdf_worker_directory(&self) -> PathBuf {
		self.config_directory().join("workers").join("pdf")
	}

	/// Where Stump keeps the files it takes out of a library for the *Deleted books* retention
	/// (originals replaced by post-processing), so they can be put back
	pub fn trash_directory(&self) -> PathBuf {
		self.config_directory().join("trash")
	}

	/// The old quarantine folder setting, if it is still set (only to say it is unused)
	pub fn legacy_quarantine_directory(&self) -> Option<PathBuf> {
		self.quarantine_dir
			.as_deref()
			.map(str::trim)
			.filter(|dir| !dir.is_empty())
			.map(PathBuf::from)
	}

	pub fn log_file(&self) -> PathBuf {
		self.config_directory().join("Stump.log")
	}

	pub fn cpu_concurrency_limit(&self) -> usize {
		let multiplier = self.parallelism_multiplier.max(1);
		std::thread::available_parallelism()
			.map(|n| n.get() * multiplier)
			.unwrap_or(multiplier)
	}

	/// How many thumbnails are generated at once. Each one holds a decoded cover (tens of MiB
	/// for a large image) while it is resized, so this is half the logical CPUs rather than the
	/// scanner's limit: generating thumbnails for a few hundred books peaked near 2 GiB at 16 at
	/// once on an 8-thread machine
	pub fn thumbnail_concurrency_limit(&self) -> usize {
		std::thread::available_parallelism()
			.map(|n| n.get() / 2)
			.unwrap_or(1)
			.max(2)
	}

	pub fn get_pdf_render_format(
		&self,
	) -> models::shared::image_processor_options::SupportedImageFormat {
		use models::shared::image_processor_options::SupportedImageFormat;

		match self.pdf_render_format.to_lowercase().as_str() {
			"webp" => SupportedImageFormat::Webp,
			"jpeg" | "jpg" => SupportedImageFormat::Jpeg,
			"png" => SupportedImageFormat::Png,
			_ => {
				tracing::warn!(
					format = self.pdf_render_format,
					"Invalid PDF render format, falling back to WebP"
				);
				SupportedImageFormat::Webp
			},
		}
	}

	/// Write the current config to Stump.toml and ensure all subdirectories exist.
	pub fn write_config(&self) -> CoreResult<()> {
		let config_dir = self.config_directory();

		if config_dir.is_file() {
			return Err(CoreError::InitializationError(format!(
				"config_dir {config_dir:?} is a file, not a directory"
			)));
		}

		if !config_dir.exists() {
			std::fs::create_dir_all(&config_dir).map_err(|e| {
				CoreError::InitializationError(format!(
					"failed to create config directory at {config_dir:?}: {e}"
				))
			})?;
		}

		for dir in [
			self.cache_directory(),
			self.thumbnails_directory(),
			self.avatars_directory(),
			self.emojis_directory(),
			self.pdf_cache_directory(),
		] {
			if !dir.exists() {
				std::fs::create_dir_all(&dir).map_err(|e| {
					CoreError::InitializationError(format!(
						"failed to create directory at {dir:?}: {e}"
					))
				})?;
			}
		}

		let content = toml::to_string(self).map_err(|e| {
			CoreError::InitializationError(format!("failed to serialize config: {e}"))
		})?;
		std::fs::write(config_dir.join("Stump.toml"), content)?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile;

	#[test]
	fn test_defaults_and_basic_env_override() {
		temp_env::with_vars(
			[
				("STUMP_PORT", Some("1337")),
				("STUMP_VERBOSITY", Some("2")),
				("STUMP_ENABLE_PLAYGROUND", Some("true")),
				("STUMP_PASSWORD_HASH_COST", Some("1")),
			],
			|| {
				let dir = tempfile::tempdir().expect("should create temp dir");
				let config_dir = dir.path().to_path_buf();

				let config =
					StumpConfig::load(config_dir.clone()).expect("should load config");

				assert_eq!(config.config_dir, config_dir.to_string_lossy().to_string());
				assert_eq!(config.port, 1337);
				assert_eq!(config.verbosity, 2);
				assert!(config.enable_playground);
				assert_eq!(config.password_hash_cost, 1);
				assert_eq!(config.ip, "0.0.0.0");
				assert_eq!(config.session_ttl, DEFAULT_SESSION_TTL);
				assert_eq!(config.pdf_render_format, "webp");
			},
		);
	}

	#[test]
	fn test_archive_limits_defaults_and_env_override() {
		let dir = tempfile::tempdir().expect("should create temp dir");
		let config =
			StumpConfig::load(dir.path().to_path_buf()).expect("should load config");
		assert_eq!(config.archive_max_entry_mb, 256);
		assert_eq!(config.archive_max_xml_mb, 16);
		assert_eq!(config.archive_max_unpacked_mb, 4096);
		assert_eq!(config.archive_max_pack_books, 200);

		temp_env::with_vars(
			[
				("STUMP_ARCHIVE_MAX_ENTRY_MB", Some("64")),
				("STUMP_ARCHIVE_MAX_PACK_BOOKS", Some("20")),
			],
			|| {
				let dir = tempfile::tempdir().expect("should create temp dir");
				let config = StumpConfig::load(dir.path().to_path_buf())
					.expect("should load config");
				assert_eq!(config.archive_max_entry_mb, 64);
				assert_eq!(config.archive_max_xml_mb, 16);
				assert_eq!(config.archive_max_pack_books, 20);
			},
		);
	}

	#[test]
	fn test_intake_limits_defaults_and_env_override() {
		let dir = tempfile::tempdir().expect("should create temp dir");
		let config =
			StumpConfig::load(dir.path().to_path_buf()).expect("should load config");
		assert_eq!(config.intake_max_request_mb, 300);
		assert_eq!(config.intake_max_file_mb, 300);
		assert_eq!(config.intake_max_files_per_request, 50);
		assert_eq!(config.intake_user_quota_mb, 2048);
		assert_eq!(config.intake_user_max_items, 100);
		assert_eq!(config.intake_min_free_gb, 20);
		assert_eq!(config.intake_total_max_gb, 50);
		assert_eq!(config.intake_upload_idle_secs, 60);
		assert_eq!(config.intake_probe_mode, "off");

		temp_env::with_vars(
			[
				("STUMP_INTAKE_USER_QUOTA_MB", Some("512")),
				("STUMP_INTAKE_PROBE_MODE", Some("inprocess")),
			],
			|| {
				let dir = tempfile::tempdir().expect("should create temp dir");
				let config = StumpConfig::load(dir.path().to_path_buf())
					.expect("should load config");
				assert_eq!(config.intake_user_quota_mb, 512);
				assert_eq!(config.intake_probe_mode, "inprocess");
				assert_eq!(config.intake_user_max_items, 100);
			},
		);
	}

	#[test]
	fn test_toml_loaded_and_env_overrides() {
		temp_env::with_vars(
			[
				("STUMP_PORT", None::<&str>),
				("STUMP_VERBOSITY", None::<&str>),
				("STUMP_ENABLE_PLAYGROUND", None::<&str>),
				("STUMP_ALLOWED_ORIGINS", None::<&str>),
				("STUMP_PASSWORD_HASH_COST", None::<&str>),
			],
			|| {
				let dir = tempfile::tempdir().expect("should create temp dir");
				let config_dir = dir.path().to_path_buf();

				std::fs::write(
					config_dir.join("Stump.toml"),
					r#"
port = 9000
verbosity = 3
enable_playground = true
allowed_origins = ["http://localhost:3000"]
"#,
				)
				.expect("should write toml");

				let config =
					StumpConfig::load(config_dir.clone()).expect("should load config");
				assert_eq!(config.port, 9000);
				assert_eq!(config.verbosity, 3);
				assert!(config.enable_playground);
				assert_eq!(config.allowed_origins, vec!["http://localhost:3000"]);

				temp_env::with_vars([("STUMP_PORT", Some("4242"))], || {
					let config = StumpConfig::load(config_dir.clone())
						.expect("should load config");
					assert_eq!(config.port, 4242);
					assert_eq!(config.verbosity, 3); // toml value
				});
			},
		);
	}

	#[test]
	fn test_invalid_toml_reports_field_level_error() {
		temp_env::with_vars(
			[
				("STUMP_PORT", None::<&str>),
				("STUMP_VERBOSITY", None::<&str>),
				("STUMP_ENABLE_PLAYGROUND", None::<&str>),
				("STUMP_ALLOWED_ORIGINS", None::<&str>),
				("STUMP_PASSWORD_HASH_COST", None::<&str>),
			],
			|| {
				let dir = tempfile::tempdir().expect("should create temp dir");
				let config_dir = dir.path().to_path_buf();

				std::fs::write(
					config_dir.join("Stump.toml"),
					"port = \"not a number\"\n",
				)
				.expect("should write toml");

				let error = StumpConfig::load(config_dir)
					.expect_err("config with invalid port should fail to load");

				let message = error.to_string();
				// there is a lot of ansi in the message so not asserting full msgs
				assert!(
					message.contains("port"),
					"should have complained about port, got: {message}"
				);
				assert!(
					message
						.contains("invalid type: string \"not a number\", expected u16"),
					"should have complained about port being a string, got: {message}"
				);
			},
		);
	}
}
