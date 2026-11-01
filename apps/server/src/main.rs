use cli::{handle_command, Cli, Parser};
use errors::{EntryError, ServerError};
use stump_core::{
	config::bootstrap_config_dir, config::logging::init_tracing, StumpCore,
};

mod config;
mod errors;
mod http_server;
mod middleware;
mod routers;
mod utils;

/// jemalloc instead of glibc's malloc: glibc gives every thread that allocates its own arena
/// (up to 8 per core) and keeps what those threads freed, so after a burst of work (thumbnails,
/// scans, OPDS feeds) the server holds several times the memory it actually uses. jemalloc
/// shares a few arenas and hands unused pages back to the OS. It replaces `malloc` for the whole
/// process, so the C libraries (SQLite, libwebp, PDFium) use it too.
#[cfg(target_os = "linux")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// A pointer to jemalloc's configuration string, laid out like the `const char *` it expects
#[cfg(target_os = "linux")]
#[repr(transparent)]
pub struct MallocConf(*const std::ffi::c_char);

// SAFETY: the pointer refers to an immutable 'static C string
#[cfg(target_os = "linux")]
unsafe impl Sync for MallocConf {}

/// jemalloc's settings, read once when it starts. Background threads return pages that have
/// been free for a second, so an idle server shrinks back without waiting for new allocations,
/// and two arenas (instead of four per core) keep the freed memory of all threads in one place.
/// Override at runtime with the `MALLOC_CONF` environment variable (same syntax).
#[cfg(target_os = "linux")]
#[export_name = "malloc_conf"]
pub static MALLOC_CONF: MallocConf = MallocConf(
	c"background_thread:true,dirty_decay_ms:1000,muzzy_decay_ms:0,narenas:2".as_ptr(),
);

/// The environment variable that caps tokio's blocking thread pool
const MAX_BLOCKING_THREADS_KEY: &str = "STUMP_MAX_BLOCKING_THREADS";

/// Tokio's blocking pool runs file IO, archive and EPUB parsing, image work and scans. Its
/// default of 512 threads lets a burst of requests (an OPDS feed opens every book it lists) start
/// dozens of threads that each hold their own buffers, so it is capped to a few per core;
/// extra work waits in a queue instead.
fn max_blocking_threads() -> usize {
	std::env::var(MAX_BLOCKING_THREADS_KEY)
		.ok()
		.and_then(|value| value.trim().parse::<usize>().ok())
		.filter(|value| *value > 0)
		.unwrap_or_else(|| {
			let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
			(cores * 4).clamp(16, 64)
		})
}

#[cfg(debug_assertions)]
fn debug_setup() {
	std::env::set_var(
		"STUMP_CLIENT_DIR",
		env!("CARGO_MANIFEST_DIR").to_string() + "/../web/dist",
	);
	std::env::set_var("STUMP_PROFILE", "debug");
	std::env::set_var("STUMP_COLORFUL_LOGS", "true");
}

fn main() -> Result<(), EntryError> {
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.max_blocking_threads(max_blocking_threads())
		.build()
		.map_err(|e| {
			EntryError::ServerError(ServerError::ServerStartError(format!(
				"Failed to start the async runtime: {e}"
			)))
		})?;
	runtime.block_on(run())
}

async fn run() -> Result<(), EntryError> {
	#[cfg(debug_assertions)]
	debug_setup();

	let config_dir = bootstrap_config_dir();

	let config = StumpCore::init_config(config_dir)
		.map_err(|e| EntryError::InvalidConfig(e.to_string()))?;

	let cli = Cli::parse();

	if let Some(command) = cli.command {
		Ok(handle_command(command, &cli.config.merge_stump_config(config)).await?)
	} else {
		let resolved_config = cli.config.merge_stump_config(config);
		// Note: init_tracing after loading the environment so the correct verbosity
		// level is used for logging.
		init_tracing(&resolved_config);

		if let Some(ref oidc) = resolved_config.oidc {
			tracing::info!(enabled = oidc.enabled, "OIDC configuration loaded");
		}

		if resolved_config.verbosity >= 3 {
			tracing::trace!(?resolved_config, "App config");
		}

		Ok(http_server::run_http_server(resolved_config).await?)
	}
}
