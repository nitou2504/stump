use axum::{
	extract::{Query, Request, State},
	http::{HeaderMap, Response, StatusCode},
	middleware,
	response::IntoResponse,
	routing::{get, post},
	Extension, Json, Router,
};
use axum_extra::{headers::UserAgent, TypedHeader};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use graphql::data::{AuthContext, EdgeRequest, ServiceContext};
use models::{
	entity::{
		session,
		user::{self, AuthUser, LoginUser},
		user_login_activity,
	},
	services::{
		access::{self as access_service, reason, SignInMark},
		user::{create_user_with_preferences, revoke_user_sessions},
	},
	shared::image::ImageRef,
};
use reqwest::header;
use sea_orm::{prelude::*, ExprTrait};
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use tracing::error;

use crate::{
	config::{
		jwt::{
			create_jwt_auth, exchange_refresh_token, extract_jti_from_refresh_token,
			JwtTokenPair,
		},
		session::{delete_cookie_header, SESSION_USER_KEY},
		state::AppState,
	},
	errors::{APIError, APIResult},
	middleware::{
		auth::auth_middleware,
		edge::EDGE_PUBLIC_HEADER,
		ClientIp, HostExtractor,
	},
	utils::{
		default_true, fetch_session_user, hash_password, verify_dummy_password,
		verify_password,
	},
};

/// Failed logins in the last 24 hours after which the next failed one locks the account
const FAILED_LOGINS_BEFORE_LOCK: u64 = 9;

/// A success, or a refusal of a locked account, is tracked once an hour for a client that signs in
/// with every request (OPDS Basic auth)
const BASIC_AUTH_TRACKING_INTERVAL_HOURS: i64 = 1;

pub(crate) fn mount(app_state: AppState) -> Router<AppState> {
	Router::new().nest(
		"/auth",
		Router::new()
			// TODO(chore): I really need to just deprecate this route, it gives me way too much headache
			// to have to have a two different public-facing users and mixing (de)serialization
			// btw gql and rest
			.route(
				"/me",
				get(viewer).layer(middleware::from_fn_with_state(
					app_state.clone(),
					auth_middleware,
				)),
			)
			.route(
				"/viewer",
				get(viewer)
					.layer(middleware::from_fn_with_state(app_state, auth_middleware)),
			)
			.route("/login", post(login))
			.route("/refresh-token", post(refresh_token))
			.route("/logout", post(logout))
			.route("/register", post(register)),
	)
}

async fn viewer(Extension(req): Extension<AuthContext>) -> APIResult<Json<AuthUser>> {
	Ok(Json(req.user()))
}

pub async fn enforce_max_sessions(
	for_user: &user::LoginUser,
	conn: &DatabaseConnection,
) -> APIResult<()> {
	let existing_sessions = session::Entity::find()
		.filter(session::Column::UserId.eq(for_user.id.clone()))
		.all(conn)
		.await?;
	let existing_login_sessions_count = existing_sessions.len() as i32;
	tracing::trace!(?existing_login_sessions_count, "Existing sessions count");

	match (for_user.max_sessions_allowed, existing_login_sessions_count) {
		(Some(max_login_sessions), count) if count >= max_login_sessions => {
			let oldest_session_id = existing_sessions
				.iter()
				.min_by_key(|session| session.expiry_time)
				.map(|session| session.id);

			handle_remove_earliest_session(conn, for_user.id.clone(), oldest_session_id)
				.await?;
		},
		_ => (),
	}

	Ok(())
}

async fn lock_account(conn: &DatabaseConnection, user_id: String) -> APIResult<()> {
	let affected_rows = user::Entity::update_many()
		.filter(user::Column::Id.eq(user_id.clone()))
		.col_expr(user::Column::IsLocked, Expr::value(true))
		.exec(conn)
		.await?
		.rows_affected;
	tracing::debug!(?affected_rows, "Locked user account");

	let deleted_sessions = revoke_user_sessions(conn, &user_id, None).await?.len();
	tracing::debug!(
		?deleted_sessions,
		"Removed all sessions and refresh tokens for locked user"
	);

	// who locked it is what tells the owner, in Public access, whether to look at the failures
	if let Err(error) = access_service::audit(
		conn,
		Utc::now(),
		"system",
		"user.locked_by_failures",
		&user_id,
		"too many failed sign-ins",
	)
	.await
	{
		error!(?error, "Failed to audit the lock");
	}

	Ok(())
}

#[derive(Deserialize)]
pub struct PasswordUserInput {
	pub username: String,
	pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthenticationOptions {
	#[serde(default)]
	generate_token: bool,
	#[serde(default = "default_true")]
	create_session: bool,
}

async fn handle_login_attempt(
	conn: &DatabaseConnection,
	for_user: &user::LoginUser,
	user_agent: String,
	client_ip: std::net::IpAddr,
	success: bool,
) -> APIResult<user_login_activity::Model> {
	let active_model = user_login_activity::ActiveModel {
		user_id: Set(for_user.id.clone()),
		ip_address: Set(client_ip.to_string()),
		user_agent: Set(user_agent),
		timestamp: Set(Utc::now().into()),
		authentication_successful: Set(success),
		..Default::default()
	};
	let login_activity = active_model.insert(conn).await?;
	tracing::trace!(?login_activity, "Tracked login activity");
	Ok(login_activity)
}

/// Tracks a password login attempt (the web login or OPDS HTTP Basic auth) in the user's login
/// activity and locks the account when it is a failure after [`FAILED_LOGINS_BEFORE_LOCK`] others
/// in the last 24 hours.
///
/// The server owner is never locked by failures: anyone who knows the username could lock the
/// owner out by guessing, and the owner is who unlocks everyone else. Their attempts are still
/// tracked. A locked account is not locked again, but a failure on it is tracked too.
async fn track_login_attempt(
	conn: &DatabaseConnection,
	for_user: &user::LoginUser,
	user_agent: String,
	client_ip: std::net::IpAddr,
	success: bool,
	mark: SignInMark,
) -> APIResult<()> {
	let today: DateTime<FixedOffset> = Utc::now().into();
	// TODO: make this configurable via environment variable so knowledgeable attackers can't bypass this
	let twenty_four_hours_ago = today - Duration::hours(24);

	// every failure of the window counts, but the ones that were not guesses at this account's
	// password (and the ones the owner forgave when they unblocked it)
	let should_lock_account = !success
		&& !for_user.is_server_owner
		&& !for_user.is_locked
		&& access_service::count_lock_failures(
			conn,
			&for_user.id,
			twenty_four_hours_ago,
			today,
		)
		.await? >= FAILED_LOGINS_BEFORE_LOCK;

	record_attempt(conn, for_user, user_agent, client_ip, success, &mark).await;

	if should_lock_account {
		lock_account(conn, for_user.id.clone()).await?;
	}

	Ok(())
}

/// Records a sign-in attempt in the user's login activity, with what is known about it (the pass
/// it came with, why it failed, whether the lock rule counts it). It never fails the sign-in: an
/// error is logged
async fn record_attempt(
	conn: &DatabaseConnection,
	for_user: &user::LoginUser,
	user_agent: String,
	client_ip: std::net::IpAddr,
	success: bool,
	mark: &SignInMark,
) {
	match handle_login_attempt(conn, for_user, user_agent, client_ip, success).await {
		Ok(activity) => {
			if let Err(error) = access_service::mark_sign_in(conn, activity.id, mark).await
			{
				error!(?error, "Failed to mark the login attempt!");
			}
		},
		Err(error) => error!(?error, "Failed to track login attempt!"),
	}
}

/// Tracks an OPDS HTTP Basic auth attempt like [`track_login_attempt`]. A reader sends the
/// credentials with every request, so a success, or a refusal of a locked account, is tracked
/// only when the user hasn't had the same from that address with that client in the last hour (a
/// row for every request would swell the login activity). Failures that count towards the lock
/// are always tracked.
pub(crate) async fn track_basic_auth_attempt(
	conn: &DatabaseConnection,
	for_user: &user::LoginUser,
	user_agent: String,
	client_ip: std::net::IpAddr,
	success: bool,
) -> APIResult<()> {
	if success || for_user.is_locked {
		let since: DateTime<FixedOffset> =
			(Utc::now() - Duration::hours(BASIC_AUTH_TRACKING_INTERVAL_HOURS)).into();
		let tracked_recently = user_login_activity::Entity::find()
			.filter(
				user_login_activity::Column::UserId
					.eq(for_user.id.clone())
					.and(user_login_activity::Column::IpAddress.eq(client_ip.to_string()))
					.and(user_login_activity::Column::UserAgent.eq(user_agent.clone()))
					.and(
						user_login_activity::Column::AuthenticationSuccessful.eq(success),
					)
					.and(user_login_activity::Column::Timestamp.gte(since)),
			)
			.one(conn)
			.await?
			.is_some();

		if tracked_recently {
			return Ok(());
		}
	}

	let reason = match (success, for_user.is_locked) {
		(true, _) => None,
		(false, true) => Some(reason::LOCKED),
		(false, false) => Some(reason::WRONG_PASSWORD),
	};
	track_login_attempt(
		conn,
		for_user,
		user_agent,
		client_ip,
		success,
		SignInMark::counting(None, reason),
	)
	.await
}

async fn handle_remove_earliest_session(
	conn: &DatabaseConnection,
	for_user_id: String,
	session_id: Option<i32>,
) -> APIResult<u64> {
	if let Some(oldest_session_id) = session_id {
		let affected_rows = session::Entity::delete_by_id(oldest_session_id)
			.filter(session::Column::UserId.eq(for_user_id))
			.exec(conn)
			.await?
			.rows_affected;
		tracing::trace!(?affected_rows, "Removed oldest session for user");
		Ok(affected_rows)
	} else {
		tracing::warn!("Deleting all sessions for user");
		let deleted_sessions = session::Entity::delete_many()
			.filter(session::Column::UserId.eq(for_user_id))
			.exec(conn)
			.await?
			.rows_affected;
		tracing::trace!(?deleted_sessions, "Removed all sessions for user");
		Ok(deleted_sessions)
	}
}

fn inject_avatar_url(mut user: AuthUser, service: ServiceContext) -> AuthUser {
	user.avatar = ImageRef {
		url: service.cache_friendly_url(
			format!("/api/v2/users/{}/avatar", user.id),
			&user.avatar.last_modified,
		),
		..user.avatar
	};
	user
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedToken {
	#[serde(flatten)]
	token: JwtTokenPair,
	for_user: AuthUser,
}

#[derive(Debug, Serialize)]
#[serde(untagged, rename_all = "camelCase")]
pub enum LoginResponse {
	User(AuthUser),
	AccessToken(GeneratedToken),
}

/// Authenticates the user and returns the user object. If the user is already logged in, returns the
/// user object from the session.
async fn login(
	TypedHeader(user_agent): TypedHeader<UserAgent>,
	ClientIp(client_ip): ClientIp,
	session: Session,
	State(state): State<AppState>,
	HostExtractor(details): HostExtractor,
	headers: HeaderMap,
	Extension(edge): Extension<EdgeRequest>,
	Query(AuthenticationOptions {
		generate_token,
		create_session,
	}): Query<AuthenticationOptions>,
	Json(PasswordUserInput { username, password }): Json<PasswordUserInput>,
) -> APIResult<Json<LoginResponse>> {
	let config = state.config.clone();
	let service = ServiceContext::new(details.host, details.scheme);

	let is_oidc_only_auth = config
		.oidc
		.as_ref()
		.is_some_and(|oidc_config| oidc_config.disable_local_auth);

	if is_oidc_only_auth {
		return Err(APIError::BadRequest(
			"Local authentication is disabled. Please log in with your identity provider."
				.to_string(),
		));
	}

	let Some(user) = LoginUser::find()
		.filter(
			user::Column::Username
				.eq(username)
				.and(user::Column::DeletedAt.is_null()),
		)
		.into_model::<LoginUser>()
		.one(state.conn.as_ref())
		.await?
	else {
		// an unknown username must take as long to refuse as a wrong password
		verify_dummy_password(password, &config).await;
		return Err(APIError::Unauthorized);
	};

	// Whether the request came through the public edge is what the edge's marker says, always
	// (the header is also looked at directly: the answer must not depend on the request having
	// passed through one more layer)
	let via_public = edge.via_public || headers.contains_key(EDGE_PUBLIC_HEADER);
	let device_id = edge.device.as_ref().map(|device| device.id.clone());

	// The owner signs in from home only. Their username is easy to guess, and the public edge's
	// limits can only slow down guessing at their password, so a sign-in as the owner that comes
	// through the edge is refused whatever the password is, before it is even checked. It is
	// refused like a wrong password, after the same work (so it takes as long, and doesn't say
	// that this is the owner), and recorded, without ever counting towards a lock: the owner is
	// never locked by failed sign-ins, and this one wasn't a guess
	if user.is_server_owner && via_public {
		verify_dummy_password(password, &config).await;
		record_attempt(
			state.conn.as_ref(),
			&user,
			user_agent.to_string(),
			client_ip,
			false,
			&SignInMark::ignored_by_lock(device_id, reason::OWNER_VIA_EDGE),
		)
		.await;
		return Err(APIError::Unauthorized);
	}

	// A pass made from an invite is for one person: through it, only that person can sign in. Any
	// other name gets the answer of a wrong password after the same work, and is recorded
	// without counting towards the lock of the account it was typed for: a leaked link or a lost
	// phone must not be a way to lock somebody else out. It is checked before the account's own
	// state, so a locked account doesn't answer differently for it either
	let binds_person = config.edge.as_ref().is_none_or(|edge| edge.binds_person);
	if via_public && binds_person {
		let bound_to = edge
			.device
			.as_ref()
			.and_then(|device| device.person_id.as_deref());
		if bound_to.is_some_and(|person_id| person_id != user.id) {
			verify_dummy_password(password, &config).await;
			record_attempt(
				state.conn.as_ref(),
				&user,
				user_agent.to_string(),
				client_ip,
				false,
				&SignInMark::ignored_by_lock(device_id, reason::WRONG_PERSON),
			)
			.await;
			return Err(APIError::Unauthorized);
		}
	}

	match session.get::<String>(SESSION_USER_KEY).await? {
		Some(user_id) if user_id == user.id && !user.is_locked => {
			// TODO: should this be tracked?
			// TODO: should this be permission gated?
			if generate_token {
				let token = create_jwt_auth(&user.id, &state.conn, &state.config).await?;
				return Ok(Json(LoginResponse::AccessToken(GeneratedToken {
					for_user: user.into(),
					token,
				})));
			}

			// The user already has a session, so we just return them immediately
			let auth_user = with_password_flag(state.conn.as_ref(), user.into()).await?;
			return Ok(Json(LoginResponse::User(inject_avatar_url(
				auth_user, service,
			))));
		},
		_ => {},
	}

	// Check if this is an OIDC-only user (no password set)
	if user.hashed_password.is_empty() && user.oidc_issuer_id.is_some() {
		return Err(APIError::BadRequest(
			"This account uses OIDC authentication. Please log in with your identity provider."
				.to_string(),
		));
	} else if user.hashed_password.is_empty() {
		tracing::error!(
			"Broken account! User {} has no password set and is not an OIDC user",
			user.username
		);
		verify_dummy_password(password, &config).await;
		return Err(APIError::Unauthorized);
	}

	if user.is_locked {
		// the same answer whatever the password is, so a locked account can't be used to check
		// guesses; the attempt is recorded
		track_login_attempt(
			state.conn.as_ref(),
			&user,
			user_agent.to_string(),
			client_ip,
			false,
			SignInMark::counting(device_id, Some(reason::LOCKED)),
		)
		.await?;
		return Err(APIError::AccountLocked);
	}

	let provided_valid_credentials =
		verify_password(user.hashed_password.clone(), password).await?;

	track_login_attempt(
		state.conn.as_ref(),
		&user,
		user_agent.to_string(),
		client_ip,
		provided_valid_credentials,
		SignInMark::counting(
			device_id,
			(!provided_valid_credentials).then_some(reason::WRONG_PASSWORD),
		),
	)
	.await?;

	if !provided_valid_credentials {
		return Err(APIError::Unauthorized);
	}

	if create_session {
		enforce_max_sessions(&user, state.conn.as_ref()).await?;
	}

	let auth_user = with_password_flag(state.conn.as_ref(), AuthUser::from(user)).await?;
	let auth_user = inject_avatar_url(auth_user, service);

	if create_session {
		session
			.insert(SESSION_USER_KEY, auth_user.id.clone())
			.await?;
	}

	// TODO: should this be permission gated?
	if generate_token {
		let token = create_jwt_auth(&auth_user.id, &state.conn, &state.config).await?;
		Ok(Json(LoginResponse::AccessToken(GeneratedToken {
			for_user: auth_user,
			token,
		})))
	} else {
		Ok(Json(LoginResponse::User(auth_user)))
	}
}

/// The user, with whether the owner set their password and they have to choose their own: the web
/// app learns it from the sign-in
async fn with_password_flag(
	conn: &DatabaseConnection,
	mut user: AuthUser,
) -> APIResult<AuthUser> {
	user.must_change_password =
		access_service::must_change_password(conn, &user.id).await?;
	Ok(user)
}

/// Destroys the session and logs the user out.
async fn logout(session: Session) -> APIResult<impl IntoResponse> {
	session.delete().await?;

	let body = serde_json::json!({
		"status": 200,
		"message": "OK",
	});

	let base_response = Json(body).into_response();

	let (name, value) = delete_cookie_header();
	let builder = Response::builder()
		.status(200)
		.header("Content-Type", "application/json")
		.header(name, value);

	Ok(builder
		.body(base_response.into_body())
		.unwrap_or_else(|error| {
			tracing::error!(?error, "Failed to build response");
			(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
		}))
}

/// Attempts to register a new user. If no users exist in the database, the user is registered as a server owner.
/// Otherwise, the registration is rejected by all users except the server owner.
pub async fn register(
	session: Session,
	State(ctx): State<AppState>,
	HostExtractor(details): HostExtractor,
	Extension(edge): Extension<EdgeRequest>,
	headers: HeaderMap,
	Json(input): Json<PasswordUserInput>,
) -> APIResult<Json<AuthUser>> {
	// The internet never creates an account, the very first one (the server owner) included: that
	// is done from home. This route reads the session itself, so it can't rely on the
	// authentication middleware's refusal of the owner
	if edge.via_public || headers.contains_key(EDGE_PUBLIC_HEADER) {
		return Err(APIError::Forbidden(
			"Accounts are created from home".to_string(),
		));
	}

	let config = ctx.config.clone();
	let service = ServiceContext::new(details.host, details.scheme);

	let is_oidc_only_auth = config
		.oidc
		.as_ref()
		.is_some_and(|oidc_config| oidc_config.disable_local_auth);

	if is_oidc_only_auth {
		return Err(APIError::BadRequest(
			"Local authentication is disabled. Please register with your identity provider."
				.to_string()
		));
	}

	let conn = ctx.conn.as_ref();
	let has_users = user::Entity::find()
		.filter(user::Column::DeletedAt.is_null())
		.count(conn)
		.await?
		> 0;

	let mut is_server_owner = false;

	let session_user = fetch_session_user(&session, ctx.conn.as_ref()).await?;

	if let Some(user) = session_user {
		if !user.is_server_owner {
			return Err(APIError::Forbidden(String::from(
				"You do not have permission to access this resource.",
			)));
		}
	} else if session_user.is_none() && has_users {
		// if users exist, a valid session is required to register a new user
		return Err(APIError::Unauthorized);
	} else if !has_users {
		// if no users present, the user is automatically a server owner
		is_server_owner = true;
	}

	let hashed_password = hash_password(&input.password, &ctx.config)?;

	let active_model = user::ActiveModel {
		username: Set(input.username.clone()),
		hashed_password: Set(hashed_password),
		is_server_owner: Set(is_server_owner),
		..Default::default()
	};
	let updated_user = create_user_with_preferences(conn, active_model, None).await?;

	let auth_user = user::LoginUser::find()
		.filter(user::Column::Id.eq(updated_user.id.clone()))
		.into_model::<user::LoginUser>()
		.one(conn)
		.await?
		.ok_or(APIError::InternalServerError(
			"Failed to fetch user after registration.".to_string(),
		))?;

	let auth_user = inject_avatar_url(AuthUser::from(auth_user), service);

	Ok(Json(auth_user))
}

/// Exchange a refresh token for a new access token and refresh token pair. This endpoint
/// expects a valid refresh token in the Authorization header as a Bearer token. The
/// access token should not be used for this endpoint.
async fn refresh_token(
	State(state): State<AppState>,
	req: Request,
) -> Result<Json<JwtTokenPair>, APIError> {
	let req_headers = req.headers().clone();
	let auth_header = req_headers
		.get(header::AUTHORIZATION)
		.and_then(|header| header.to_str().ok())
		.ok_or(APIError::Unauthorized)?;

	if auth_header.starts_with("Bearer ") {
		let token = auth_header.trim_start_matches("Bearer ").to_string();
		let jti = extract_jti_from_refresh_token(&token, state.conn.as_ref()).await?;
		let jwt_pair = exchange_refresh_token(&jti, state).await?;
		return Ok(Json(jwt_pair));
	}

	Err(APIError::Unauthorized)
}
