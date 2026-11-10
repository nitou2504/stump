use std::{net::SocketAddr, sync::Arc, time::Duration};

use apalis::prelude::*;
use axum::{
	extract::connect_info::Connected, http::Uri, serve::IncomingStream, Extension, Router,
};
use stump_core::{
	config::{bootstrap_config_dir, logging::init_tracing},
	job::dispatch_job,
	StumpCore,
};
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tower_http::{
	compression::{
		predicate::{DefaultPredicate, NotForContentType, Predicate},
		CompressionLayer,
	},
	trace::TraceLayer,
};

use crate::{
	config::{cors, oidc::OidcProvider, session::get_session_layer},
	errors::{EntryError, ServerError, ServerResult},
	routers,
	utils::shutdown_signal_with_cleanup,
};
use stump_core::config::StumpConfig;

pub async fn run_http_server(config: StumpConfig) -> ServerResult<()> {
	let core = StumpCore::new(config.clone()).await;

	// TODO: These need reorganizing, the core-specific initializations should just be
	// in some initialization function. The server-specific things, e.g. watcher, scheduler,
	// should be fully managed by the server and removed from the core...

	// Cancel any islanded jobs from a previous run
	core.get_context()
		.apalis_state
		.cancel_islanded_jobs()
		.await
		.map_err(|e| ServerError::ServerStartError(e.to_string()))?;

	// Initialize the server configuration. If it already exists, nothing will happen.
	core.init_server_config()
		.await
		.map_err(|e| ServerError::ServerStartError(e.to_string()))?;

	// Initialize the encryption key, if it doesn't exist
	core.init_encryption()
		.await
		.map_err(|e| ServerError::ServerStartError(e.to_string()))?;

	core.init_jwt_secrets()
		.await
		.map_err(|e| ServerError::ServerStartError(e.to_string()))?;

	core.init_journal_mode()
		.await
		.map_err(|e| ServerError::ServerStartError(e.to_string()))?;

	let _scheduler = core
		.init_scheduler()
		.await
		.map_err(|e| ServerError::ServerStartError(e.to_string()))?;

	core.init_library_watcher()
		.await
		.map_err(|e| ServerError::ServerStartError(e.to_string()))?;

	let _post_processing_collector = core.init_post_processing();

	let oidc_provider: Option<Arc<OidcProvider>> = {
		if let Some(oidc_config) = config.oidc.as_ref().filter(|c| c.is_valid()) {
			let state = OidcProvider::new(oidc_config).await.map_err(|e| {
				tracing::error!(?e, "OIDC client initialization failed");
				ServerError::ServerStartError(format!("OIDC client init failed: {e:?}"))
			})?;
			tracing::info!("OIDC client initialized successfully");
			Some(Arc::new(state))
		} else {
			None
		}
	};

	let server_ctx = core.get_context();
	let app_state = server_ctx.arced();

	// Public access: say whether it is set up (never the key), write the file the edge follows
	// before the first request is served, and keep pruning and refreshing it in the background
	match config.edge.as_ref() {
		Some(edge) => tracing::info!(
			has_key = edge.usable_key().is_some(),
			has_public_url = edge.public_base_url().is_some(),
			has_state_dir = edge.state_directory().is_some(),
			require_pass = edge.require_pass,
			binds_person = edge.binds_person,
			"Public access is configured"
		),
		None => tracing::info!("Public access is not configured (no STUMP_EDGE_* settings)"),
	}
	match stump_core::access::state::publish_if_configured(
		server_ctx.conn.as_ref(),
		&config,
	)
	.await
	{
		Ok(true) => tracing::info!("Public access: the edge's state file was written"),
		Ok(false) => {},
		Err(error) => {
			tracing::error!(?error, "Public access: the edge's state file was not written");
		},
	}
	stump_core::access::state::spawn_housekeeping(
		server_ctx.conn.clone(),
		server_ctx.config.clone(),
	);

	// Keeps the books of every smart list worked out in the background, so a reader finds them
	// ready: at start-up, and a few seconds after the catalog stops changing (see
	// `graphql::query::smart_list_cache`)
	tokio::spawn(graphql::query::smart_list_cache::run_warmer(
		server_ctx.conn.clone(),
		graphql::query::smart_list_cache::WarmerConfig::default(),
	));
	let cors_layer = cors::get_cors_layer(config.clone());

	println!("{}", core.get_shadow_text());

	let app_router = routers::mount(app_state.clone()).await;

	// we have to exclude downloadable content types from compression, 1 bc a lot are already compressed
	// but also because compression seems to strip the content-length header which breaks download progress
	// tracking on the mobile app
	let compression_predicate = DefaultPredicate::new()
		.and(NotForContentType::const_new("application/epub"))
		.and(NotForContentType::const_new("application/zip"))
		.and(NotForContentType::const_new("application/pdf"))
		.and(NotForContentType::const_new("application/octet-stream"))
		.and(NotForContentType::const_new("application/x-rar"))
		.and(NotForContentType::const_new("application/vnd.rar"))
		.and(NotForContentType::const_new(
			"application/vnd.comicbook+zip",
		))
		.and(NotForContentType::const_new("application/x-cbr"))
		.and(NotForContentType::const_new("application/x-cbz"));

	let app = Router::new()
		.merge(app_router)
		.with_state(app_state.clone())
		.layer(get_session_layer(app_state.clone()))
		.layer(cors_layer)
		.layer(CompressionLayer::new().compress_when(compression_predicate))
		.layer(
			TraceLayer::new_for_http()
				.make_span_with(|request: &axum::http::Request<_>| {
					tracing::debug_span!(
						"request",
						method = %request.method(),
						uri = %loggable_uri(request.uri()),
						version = ?request.version(),
					)
				}),
		)
		.layer(Extension(oidc_provider));

	let shutdown_notify = Arc::new(Notify::new());

	// TODO: Refactor to use https://docs.rs/async-shutdown/latest/async_shutdown/
	let cleanup = {
		let shutdown_notify = shutdown_notify.clone();
		|| async move {
			println!("Initializing graceful shutdown...");
			let _ = core.get_context().library_watcher.stop().await;
			shutdown_notify.notify_waiters();
		}
	};

	let ip: std::net::IpAddr =
		config.ip.parse().map_err(|e: std::net::AddrParseError| {
			ServerError::ServerStartError(e.to_string())
		})?;
	let addr = SocketAddr::from((ip, config.port));
	let listener = tokio::net::TcpListener::bind(&addr)
		.await
		.map_err(|e| ServerError::ServerStartError(e.to_string()))?;

	tracing::info!("⚡️ Stump HTTP server starting on http://{}", addr);

	// TODO: Experiment with higher concurrency, YEARS ago at this point (before enforcing WAL even)
	// I experienced multi-writer issues but perhaps with SeaORM + WAL we can have parallel scans.
	let monitor = Monitor::new()
		.register(
			WorkerBuilder::new("stump-worker")
				.enable_tracing()
				.data(server_ctx.apalis_state.clone())
				.concurrency(1)
				.backend(server_ctx.job_storage.clone())
				.build_fn(dispatch_job),
		)
		// post-processing (16 greys, PDF queue) on its own worker so it never holds up scans
		.register(
			WorkerBuilder::new("stump-post-processing")
				.enable_tracing()
				.data(server_ctx.apalis_state.clone())
				.concurrency(1)
				.backend(server_ctx.apalis_state.post_processing_storage.clone())
				.build_fn(dispatch_job),
		)
		.with_terminator(tokio::time::sleep(Duration::from_secs(30)))
		.run_with_signal({
			let shutdown_notify = shutdown_notify.clone();
			async move {
				shutdown_notify.notified().await;
				Ok(())
			}
		});

	let http = axum::serve(
		listener,
		app.into_make_service_with_connect_info::<StumpRequestInfo>(),
	)
	.with_graceful_shutdown(shutdown_signal_with_cleanup(Some(cleanup)));

	let _ = tokio::join!(monitor, http);

	Ok(())
}

/// The address of a request as the trace spans record it. An invite link carries its token in the
/// address (`/api/_i/<token>`, or `?t=` when it was pasted), so nothing after `/api/_i/` is kept:
/// a token must not end up in a log
pub(crate) fn loggable_uri(uri: &Uri) -> String {
	if uri.path().starts_with("/api/_i/") {
		"/api/_i/-hidden-".to_string()
	} else {
		uri.to_string()
	}
}

#[allow(dead_code)]
pub async fn bootstrap_http_server_config() -> Result<StumpConfig, EntryError> {
	// Get STUMP_CONFIG_DIR to bootstrap startup
	let config_dir = bootstrap_config_dir();

	let config = StumpCore::init_config(config_dir)
		.map_err(|e| EntryError::InvalidConfig(e.to_string()))?;

	// Note: init_tracing after loading the environment so the correct verbosity
	// level is used for logging.
	init_tracing(&config);

	if config.verbosity >= 3 {
		tracing::trace!(?config, "App config");
	}

	Ok(config)
}

#[derive(Clone, Debug)]
pub struct StumpRequestInfo {
	pub ip_addr: std::net::IpAddr,
}

impl Connected<IncomingStream<'_, TcpListener>> for StumpRequestInfo {
	fn connect_info(target: IncomingStream<'_, TcpListener>) -> Self {
		StumpRequestInfo {
			ip_addr: target.remote_addr().ip(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn an_invite_token_is_never_in_a_logged_address() {
		for uri in [
			"/api/_i/abcdefghijklmnopqrstuv",
			"/api/_i/",
			"/api/_i/?t=abcdefghijklmnopqrstuv",
			"https://g3.test/api/_i/abcdefghijklmnopqrstuv?x=1",
		] {
			let uri: Uri = uri.parse().unwrap();
			let logged = loggable_uri(&uri);
			assert_eq!(logged, "/api/_i/-hidden-");
			assert!(!logged.contains("abcdefghij"));
		}
		let other: Uri = "/api/v2/ping?x=1".parse().unwrap();
		assert_eq!(loggable_uri(&other), "/api/v2/ping?x=1");
	}
}
