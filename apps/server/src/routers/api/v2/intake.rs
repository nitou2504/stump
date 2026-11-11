//! `POST /api/v2/intake/upload`: the way books arrive in the "Add books" inbox.
//!
//! The body is streamed to disk piece by piece, never held in memory, and everything that can be
//! decided without it is decided first (who is asking, which library, how big the request says it
//! is, the user's quota). While it streams, every byte is counted against the limits, a file that
//! stalls is given up, and whatever an upload that ends early had staged is removed. See
//! `stump_core::intake::upload`.
//!
//! ```text
//! POST /api/v2/intake/upload?libraryId=<id>&mode=auto|books|series
//! Content-Type: multipart/form-data   Content-Length: <required>
//! each file is a part with a file name (its path inside the folder, for a folder upload)
//! ```
//!
//! Answers `200 {"items": [{"id", "state", "name", "kind", "libraryId"}]}`; the items are being
//! analysed, poll `intakeItems`. Refusals are `{"status", "message"}` with the status that fits:
//! 400, 401, 403, 404, 408 (a file stalled), 411 (no `Content-Length`), 413 (too big, or no room
//! in the quota), 415 (not a book of a kind Stump reads), 429 (too many files or items), 503
//! (uploads that wait for approval aren't open, or the disk is full).

use axum::{
	extract::{DefaultBodyLimit, Multipart, Query, State},
	http::{header, HeaderMap, StatusCode},
	middleware,
	response::{IntoResponse, Response},
	routing::post,
	Extension, Json, Router,
};
use graphql::data::AuthContext;
use models::{
	entity::{intake_item, library},
	shared::{
		enums::UserPermission,
		intake::{IntakeKind, IntakeState, IntakeUploadMode},
	},
};
use sea_orm::prelude::*;
use serde::{Deserialize, Serialize};
use stump_core::{
	intake::upload::{IncomingUpload, UploadError},
	CoreError,
};

use crate::{config::state::AppState, middleware::auth::auth_middleware};

const MIB: u64 = 1024 * 1024;

pub(crate) fn mount(app_state: AppState) -> Router<AppState> {
	// a request is a body of files plus the multipart framing of each: a little more than the
	// files may hold
	let body_limit = app_state
		.config
		.intake_max_request_mb
		.max(1)
		.saturating_add(1)
		.saturating_mul(MIB);
	Router::new()
		.route(
			"/intake/upload",
			post(upload).layer(DefaultBodyLimit::max(
				usize::try_from(body_limit).unwrap_or(usize::MAX),
			)),
		)
		.layer(middleware::from_fn_with_state(app_state, auth_middleware))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadParams {
	library_id: String,
	mode: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadedItem {
	id: String,
	state: IntakeState,
	name: String,
	kind: IntakeKind,
	library_id: String,
}

#[derive(Debug, Serialize)]
struct UploadResponse {
	items: Vec<UploadedItem>,
}

/// An upload that was refused, with the status that tells the client why
#[derive(Debug)]
struct Refusal {
	status: StatusCode,
	message: String,
}

impl Refusal {
	fn new(status: StatusCode, message: impl Into<String>) -> Self {
		Self {
			status,
			message: message.into(),
		}
	}
}

impl From<UploadError> for Refusal {
	fn from(error: UploadError) -> Self {
		let status = match &error {
			UploadError::NotFound(_) => StatusCode::NOT_FOUND,
			UploadError::Forbidden(_) => StatusCode::FORBIDDEN,
			UploadError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
			UploadError::TooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
			UploadError::TooMany(_) => StatusCode::TOO_MANY_REQUESTS,
			UploadError::UnsupportedType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
			UploadError::BadRequest(_) => StatusCode::BAD_REQUEST,
			UploadError::TimedOut(_) => StatusCode::REQUEST_TIMEOUT,
			UploadError::Core(CoreError::BadRequest(_)) => StatusCode::BAD_REQUEST,
			UploadError::Core(_) | UploadError::Io(_) | UploadError::Db(_) => {
				StatusCode::INTERNAL_SERVER_ERROR
			},
		};
		let message = if status == StatusCode::INTERNAL_SERVER_ERROR {
			tracing::error!(?error, "An upload to the inbox failed");
			// what went wrong inside is for the log, not for the client
			"The upload failed on the server".to_string()
		} else {
			error.to_string()
		};
		Self { status, message }
	}
}

impl From<axum::extract::multipart::MultipartError> for Refusal {
	fn from(error: axum::extract::multipart::MultipartError) -> Self {
		let status = error.status();
		let message = if status == StatusCode::PAYLOAD_TOO_LARGE {
			"The upload is larger than a request can hold".to_string()
		} else {
			"The upload could not be read".to_string()
		};
		Self { status, message }
	}
}

impl From<sea_orm::DbErr> for Refusal {
	fn from(error: sea_orm::DbErr) -> Self {
		UploadError::from(error).into()
	}
}

impl IntoResponse for Refusal {
	fn into_response(self) -> Response {
		let body = serde_json::json!({
			"status": self.status.as_u16(),
			"message": self.message,
		});
		let mut response = (self.status, Json(body)).into_response();
		response.headers_mut().insert(
			header::CACHE_CONTROL,
			"no-store".parse().expect("a header value"),
		);
		// the body of a refused upload may still be on its way: no use in waiting for it
		response
			.headers_mut()
			.insert(header::CONNECTION, "close".parse().expect("a header value"));
		response
	}
}

fn mode_of(value: Option<&str>) -> Result<IntakeUploadMode, Refusal> {
	match value.map(str::to_lowercase).as_deref() {
		None | Some("") | Some("auto") => Ok(IntakeUploadMode::Auto),
		Some("books") => Ok(IntakeUploadMode::Books),
		Some("series") => Ok(IntakeUploadMode::Series),
		Some(_) => Err(Refusal::new(
			StatusCode::BAD_REQUEST,
			"mode must be auto, books or series",
		)),
	}
}

/// The size the request says its body has. A request that doesn't say (a chunked one) can't have
/// its room reserved: it is refused
fn declared_length(headers: &HeaderMap) -> Result<u64, Refusal> {
	headers
		.get(header::CONTENT_LENGTH)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.trim().parse::<u64>().ok())
		.ok_or_else(|| {
			Refusal::new(
				StatusCode::LENGTH_REQUIRED,
				"The upload must say how big it is (Content-Length)",
			)
		})
}

async fn upload(
	State(ctx): State<AppState>,
	Extension(auth): Extension<AuthContext>,
	Query(params): Query<UploadParams>,
	headers: HeaderMap,
	mut multipart: Multipart,
) -> Result<Json<UploadResponse>, Refusal> {
	let user = auth.user();
	if user.is_locked && !user.is_server_owner {
		return Err(Refusal::new(
			StatusCode::FORBIDDEN,
			"Your account is locked",
		));
	}
	if !user.has_permission(UserPermission::AddBooks) {
		return Err(Refusal::new(
			StatusCode::FORBIDDEN,
			"You can't add books to this server",
		));
	}
	if !ctx.config.enable_upload {
		return Err(Refusal::new(
			StatusCode::FORBIDDEN,
			"Uploads are turned off on this server",
		));
	}
	let mode = mode_of(params.mode.as_deref())?;
	// a library that doesn't exist and one that is hidden from the user are the same answer
	let library = library::Entity::find_for_user(&user)
		.filter(library::Column::Id.eq(params.library_id.clone()))
		.one(ctx.conn.as_ref())
		.await?
		.ok_or_else(|| Refusal::new(StatusCode::NOT_FOUND, "Library not found"))?;
	let declared = declared_length(&headers)?;

	// from here on room is held for the upload, and what it stages goes with it if it ends early
	let mut incoming =
		IncomingUpload::begin(ctx.clone(), &user, library, declared).await?;
	let idle = incoming.idle();
	let stalled = |what: &str| {
		UploadError::TimedOut(format!("The upload stopped arriving while {what}"))
	};

	loop {
		let field = tokio::time::timeout(idle, multipart.next_field())
			.await
			.map_err(|_| stalled("waiting for the next file"))??;
		let Some(mut field) = field else {
			break;
		};
		// only the parts that are files: a part without a file name carries nothing to stage
		let Some(name) = field.file_name().map(str::to_string) else {
			continue;
		};
		let mut file = incoming.begin_file(&name).await?;
		loop {
			let chunk = tokio::time::timeout(idle, field.chunk())
				.await
				.map_err(|_| stalled(&format!("{name} was being sent")))??;
			match chunk {
				Some(chunk) => file.write(&chunk).await?,
				None => break,
			}
		}
		let done = file.finish().await?;
		incoming.add(done);
	}

	let items = incoming.finish(mode).await?;
	Ok(Json(UploadResponse {
		items: items.into_iter().map(uploaded).collect(),
	}))
}

fn uploaded(item: intake_item::Model) -> UploadedItem {
	UploadedItem {
		id: item.id,
		state: item.state,
		name: item.name,
		kind: item.kind,
		library_id: item.library_id,
	}
}
