use axum::{middleware, Router};

use crate::{config::state::AppState, middleware::edge::edge_middleware};

mod api;
mod invite;
mod kobo;
mod koreader;
mod opds;
mod spa;

pub(crate) use api::v2::auth::{enforce_max_sessions, track_basic_auth_attempt};
pub(crate) use spa::relative_favicon_path;

pub async fn mount(app_state: AppState) -> Router<AppState> {
	let mut app_router = Router::new();

	if app_state.config.enable_koreader_sync {
		app_router = app_router.merge(koreader::mount(app_state.clone()));
	}

	if app_state.config.enable_kobo_sync {
		app_router = app_router.merge(kobo::mount(app_state.clone()));
	}

	// the invite pages are for people with no pass yet, so they are not behind the sign-in
	// (they answer only through the public edge). The edge middleware is the last layer: it runs
	// for every request, whatever route it reaches, before any handler or authentication
	app_router
		.merge(spa::mount(app_state.clone()))
		.merge(invite::mount(app_state.clone()))
		.merge(api::mount(app_state.clone()).await)
		.merge(opds::mount(app_state.clone()))
		.layer(middleware::from_fn_with_state(app_state, edge_middleware))
}
