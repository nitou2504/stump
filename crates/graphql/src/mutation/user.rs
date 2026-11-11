use crate::{
	data::{AuthContext, CoreContext},
	error_message::FORBIDDEN_ACTION,
	guard::{OptionalFeature, OptionalFeatureGuard, PermissionGuard, ServerOwnerGuard},
	input::user::{
		AgeRestrictionInput, CreateUserInput, HomeArrangementInput,
		NavigationArrangementInput, UpdateUserInput, UpdateUserPreferencesInput,
	},
	object::{user::User, user_preferences::UserPreferences},
};

use super::upload::read_uploaded_image;
use async_graphql::{Context, Object, Result, Upload, ID};
use chrono::Utc;
use models::{
	entity::{
		age_restriction, session,
		user::{self, AuthUser},
		user_discover_rules, user_login_activity, user_preferences, user_reader_settings,
	},
	services::{
		access as access_service,
		user::{
			create_user_with_preferences, revoke_user_api_keys, revoke_user_sessions,
			NewAgeRestriction,
		},
	},
	shared::{
		arrangement::{Arrangement, HomeArrangement},
		discover::DiscoverRules,
		enums::UserPermission,
		permission_set::{user_has_all_permissions, PermissionSet},
	},
};
use sea_orm::{
	prelude::*, ActiveValue::NotSet, ColumnTrait, DatabaseTransaction, IntoActiveModel,
	Set, TransactionTrait, TryIntoModel,
};

use stump_core::{
	config::StumpConfig, image::thumbnail::generate_image_metadata_from_bytes,
};
use tower_sessions::Session;

#[derive(Default)]
pub struct UserMutation;

#[Object]
impl UserMutation {
	#[graphql(guard = "ServerOwnerGuard")]
	async fn delete_login_activity(&self, ctx: &Context<'_>) -> Result<u64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let deleted_rows = user_login_activity::Entity::delete_many()
			.exec(conn)
			.await?;
		tracing::debug!("Deleted login activity entries");

		access_service::audit(
			conn,
			Utc::now(),
			&user.username,
			"login_activity.cleared",
			"",
			&format!("{} entries", deleted_rows.rows_affected),
		)
		.await?;

		Ok(deleted_rows.rows_affected)
	}

	/// Upload an avatar image for either the authenticated viewer or for any user if
	/// called by a server owner
	#[graphql(
		guard = "OptionalFeatureGuard::new(OptionalFeature::Upload).and(PermissionGuard::new(&[UserPermission::UploadFile, UserPermission::ChangeAvatar]))"
	)]
	async fn upload_user_avatar(
		&self,
		ctx: &Context<'_>,
		id: Option<ID>,
		upload: Upload,
	) -> Result<User> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();

		let target_id = match &id {
			Some(id) => {
				if id.as_str() != user.id && !user.is_server_owner {
					return Err(FORBIDDEN_ACTION.into());
				}
				id.to_string()
			},
			None => user.id.clone(),
		};

		let value = upload.value(ctx)?;

		let (image_bytes, extension) =
			read_uploaded_image(value, core.config.max_image_upload_size)?;

		let avatars_dir = core.config.avatars_directory();
		if let Ok(mut entries) = tokio::fs::read_dir(&avatars_dir).await {
			let prefix = format!("{}.", target_id);
			while let Ok(Some(entry)) = entries.next_entry().await {
				let name = entry.file_name();
				if name.to_string_lossy().starts_with(&prefix) {
					let _ = tokio::fs::remove_file(entry.path()).await;
				}
			}
		}

		let avatar_meta =
			match generate_image_metadata_from_bytes(image_bytes.clone()).await {
				Ok(meta) => Some(meta),
				Err(e) => {
					tracing::error!(error = ?e, "Failed to generate image metadata");
					None
				},
			};

		let avatar_path = avatars_dir.join(format!("{}.{}", target_id, extension));
		tokio::fs::write(&avatar_path, &image_bytes)
			.await
			.map_err(|e| format!("Failed to write avatar to disk: {e}"))?;

		let avatar_path_str = avatar_path.to_string_lossy().to_string();

		let updated_user = user::Entity::find()
			.filter(user::Column::Id.eq(&target_id))
			.one(conn)
			.await?
			.ok_or("User not found")?
			.into_active_model();

		let mut active = updated_user;
		active.avatar_path = Set(Some(avatar_path_str));
		active.avatar_meta = Set(avatar_meta);
		active.avatar_updated_at = Set(Some(Utc::now().into()));
		let result = active.update(conn).await?;

		Ok(User::from(result))
	}

	/// Delete the avatar for the authenticated viewer, or for any user if
	/// called by a server owner (by passing `id`).
	async fn delete_user_avatar(
		&self,
		ctx: &Context<'_>,
		id: Option<ID>,
	) -> Result<User> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;
		let conn = core.conn.as_ref();

		let target_id = match &id {
			Some(id) => {
				if id.as_str() != user.id && !user.is_server_owner {
					return Err(FORBIDDEN_ACTION.into());
				}
				id.to_string()
			},
			None => user.id.clone(),
		};

		let existing = user::Entity::find()
			.filter(user::Column::Id.eq(&target_id))
			.one(conn)
			.await?
			.ok_or("User not found")?;

		if let Some(ref path_str) = existing.avatar_path {
			let _ = tokio::fs::remove_file(path_str).await;
		}

		let mut active = existing.into_active_model();
		active.avatar_path = Set(None);
		active.avatar_meta = Set(None);
		active.avatar_updated_at = Set(Some(Utc::now().into()));
		let result = active.update(conn).await?;

		Ok(User::from(result))
	}

	#[graphql(guard = "PermissionGuard::one(UserPermission::ManageUsers)")]
	#[tracing::instrument(skip(self, ctx, input), fields(username = ?input.username))]
	async fn create_user(
		&self,
		ctx: &Context<'_>,
		input: CreateUserInput,
	) -> Result<User> {
		let AuthContext { user: creator, .. } = ctx.data::<AuthContext>()?;
		ensure_can_grant(creator, &input.permissions)?;

		let core_ctx = ctx.data::<CoreContext>()?;
		let hashed_password =
			bcrypt::hash(input.password, core_ctx.config.password_hash_cost)?;

		let conn = core_ctx.conn.as_ref();

		let permissions = PermissionSet::new(input.permissions);

		let user = user::ActiveModel {
			is_server_owner: Set(false),
			username: Set(input.username),
			hashed_password: Set(hashed_password),
			permissions: Set(permissions.resolve_into_string()),
			max_sessions_allowed: Set(input.max_sessions_allowed),
			..Default::default()
		};

		let age_restriction = input.age_restriction.map(|ar| NewAgeRestriction {
			age: ar.age,
			restrict_on_unset: ar.restrict_on_unset,
		});

		let user_model = create_user_with_preferences(conn, user, age_restriction)
			.await
			.map_err(|error| {
				tracing::error!(?error, "Failed to create user");
				"Failed to create user"
			})?;

		Ok(User::from(user_model))
	}

	async fn update_viewer(
		&self,
		ctx: &Context<'_>,
		input: UpdateUserInput,
	) -> Result<User> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core_ctx = ctx.data::<CoreContext>()?;
		let config = core_ctx.config.as_ref();
		let conn = core_ctx.conn.as_ref();

		// the password the owner set must be replaced by a different one, or "choosing your own"
		// would mean nothing
		let must_change = access_service::must_change_password(conn, &user.id).await?;
		if let (true, Some(password)) = (must_change, input.password.as_deref()) {
			let current = user::Entity::find_by_id(user.id.clone())
				.one(conn)
				.await?
				.ok_or("User not found")?;
			if bcrypt::verify(password, &current.hashed_password).unwrap_or(false) {
				return Err(
					"Choose a password that is different from the one you were given"
						.into(),
				);
			}
		}

		let updated_user =
			update_user(user, user.id.clone(), conn, config, &input).await?;

		if input.password.is_some() {
			revoke_other_sessions(ctx, &user.id, conn).await?;
			// they chose their own: the requirement is over
			if access_service::clear_password_change(conn, &user.id).await? {
				access_service::audit(
					conn,
					Utc::now(),
					&user.username,
					"user.password_changed",
					&user.id,
					"chose their own password",
				)
				.await?;
			}
		}

		Ok(updated_user)
	}

	async fn update_viewer_preferences(
		&self,
		ctx: &Context<'_>,
		input: UpdateUserPreferencesInput,
	) -> Result<UserPreferences> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core_ctx = ctx.data::<CoreContext>()?;
		let conn = core_ctx.conn.as_ref();

		let user_preferences = user_preferences::Entity::find()
			.filter(user_preferences::Column::UserId.eq(user.id.clone()))
			.one(conn)
			.await?;

		if let Some(user_preferences_model) = user_preferences {
			tracing::trace!(user_id = ?user.id, ?user_preferences_model, updates = ?input, "Updating viewer's preferences");

			let updated_user_preferences = update_user_preferences_by_id(
				user_preferences_model.id,
				user.id.clone(),
				input,
				conn,
			)
			.await?;

			// A session holds only its user's id (every request loads the user, with these
			// preferences, from the database), so there is nothing to refresh in it. A request
			// that comes with an access token or an API key has no session at all: writing the
			// user into one made the server answer 500 after the preferences were saved
			Ok(updated_user_preferences)
		} else {
			Err("User preferences not found".into())
		}
	}

	async fn update_user(
		&self,
		ctx: &Context<'_>,
		id: ID,
		input: UpdateUserInput,
	) -> Result<User> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core_ctx = ctx.data::<CoreContext>()?;
		let config = core_ctx.config.as_ref();
		let conn = core_ctx.conn.as_ref();

		let is_self = user.id == id.to_string();
		let can_manage_users =
			user.is_server_owner || user.has_permission(UserPermission::ManageUsers);

		if !is_self && !can_manage_users {
			return Err(FORBIDDEN_ACTION.into());
		}

		// TODO(permissions): server owner goes away
		// nobody can update the server owner
		if !is_self && !user.is_server_owner {
			let target = user::Entity::find_by_id(id.to_string())
				.one(conn)
				.await?
				.ok_or("User not found")?;
			if target.is_server_owner {
				return Err(FORBIDDEN_ACTION.into());
			}

			// Whoever manages users can't change, or reset the password of, an account
			// that holds permissions they don't hold themselves: signing in to it would
			// hand them those permissions
			let target_permissions =
				PermissionSet::from(target.permissions.unwrap_or_default())
					.resolve_into_vec();
			if !user_has_all_permissions(user, &target_permissions) {
				return Err(FORBIDDEN_ACTION.into());
			}
		}

		let updated_user =
			update_user(user, id.to_string(), conn, config, &input).await?;
		tracing::debug!(?updated_user, "Updated user");

		if !is_self {
			remove_all_session_for_user(id.to_string(), conn).await?;
			if input.password.is_some() {
				access_service::audit(
					conn,
					Utc::now(),
					&user.username,
					"user.password_set",
					&id,
					&updated_user.model.username,
				)
				.await?;
			}
		} else if input.password.is_some() {
			revoke_other_sessions(ctx, &user.id, conn).await?;
		}

		Ok(updated_user)
	}

	#[graphql(guard = "ServerOwnerGuard")]
	async fn delete_user(
		&self,
		ctx: &Context<'_>,
		id: ID,
		hard_delete: Option<bool>,
	) -> Result<User> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core_ctx = ctx.data::<CoreContext>()?;
		let conn = core_ctx.conn.as_ref();

		if id.to_string() == user.id {
			return Err("You cannot delete your own account".into());
		}

		let hard_delete = hard_delete.unwrap_or(false);

		let existing_user = user::Entity::find()
			.filter(user::Column::Id.eq(id.to_string()))
			.one(conn)
			.await?
			.ok_or("User not found")?;

		if existing_user.is_server_owner {
			return Err("You cannot delete the server owner".into());
		}

		let deleted_user = if hard_delete {
			user::Entity::delete_by_id(id.to_string())
				.exec_with_returning(conn)
				.await?
				.ok_or("Failed to delete user".to_string())?
		} else {
			let mut active_model = existing_user.into_active_model();
			active_model.deleted_at = Set(Some(chrono::Utc::now().into()));
			let deleted_user = active_model.update(conn).await?.try_into_model()?;

			// a deleted user is refused everywhere, but nothing they hold should be left behind:
			// no cookie, no refresh token and no API key (a hard delete removes them with the user)
			remove_all_session_for_user(id.to_string(), conn).await?;
			revoke_user_api_keys(conn, id.as_str()).await?;

			deleted_user
		};

		tracing::debug!(?deleted_user, "Deleted user");
		Ok(User::from(deleted_user))
	}

	#[graphql(guard = "ServerOwnerGuard")]
	async fn delete_user_sessions(&self, ctx: &Context<'_>, id: ID) -> Result<u64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core_ctx = ctx.data::<CoreContext>()?;
		let conn = core_ctx.conn.as_ref();

		let removed_sessions = remove_all_session_for_user(id.to_string(), conn).await?;
		access_service::audit(
			conn,
			Utc::now(),
			&user.username,
			"user.sessions_deleted",
			&id,
			&format!("{} sessions", removed_sessions.len()),
		)
		.await?;
		Ok(removed_sessions.len().try_into()?)
	}

	#[graphql(guard = "ServerOwnerGuard")]
	async fn update_user_lock_status(
		&self,
		ctx: &Context<'_>,
		id: ID,
		lock: bool,
	) -> Result<User> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core_ctx = ctx.data::<CoreContext>()?;
		let conn = core_ctx.conn.as_ref();

		if id.to_string() == user.id {
			return Err("You cannot lock your own account".into());
		}

		let model = user::Entity::find()
			.filter(user::Column::Id.eq(id.to_string()))
			.one(conn)
			.await?
			.ok_or("User not found")?;

		if !lock {
			// Unlocking forgives the failed sign-ins of the last 24 hours: the lock rule counts
			// every one of them, so an unlock that only cleared the flag would lock the account
			// again on its next typo
			access_service::unblock_user(conn, Utc::now(), &model.id, &user.username)
				.await?;
			let unlocked = user::Entity::find_by_id(model.id)
				.one(conn)
				.await?
				.ok_or("User not found")?;
			return Ok(User::from(unlocked));
		}

		let mut active_model = model.into_active_model();
		active_model.is_locked = Set(lock);

		let updated_user = active_model.update(conn).await?;

		// Delete all sessions and refresh tokens for this user if they are being locked
		remove_all_session_for_user(id.to_string(), conn).await?;
		access_service::audit(
			conn,
			Utc::now(),
			&user.username,
			"user.lock",
			&id,
			&updated_user.username,
		)
		.await?;

		Ok(User::from(updated_user))
	}

	/// Saves the signed-in user's reading settings (a JSON object as text, at most 1 MB),
	/// replacing what was saved before
	async fn update_my_reader_settings(
		&self,
		ctx: &Context<'_>,
		settings: String,
	) -> Result<bool> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		validate_reader_settings(&settings)?;

		let existing = user_reader_settings::Entity::find_by_id(user.id.clone())
			.one(conn)
			.await?;
		let row = user_reader_settings::ActiveModel {
			user_id: Set(user.id.clone()),
			settings: Set(settings),
			updated_at: Set(Utc::now()),
		};
		if existing.is_some() {
			row.update(conn).await?;
		} else {
			row.insert(conn).await?;
		}
		Ok(true)
	}

	/// Saves the signed-in user's rules for the home's picks and Surprise me (cleaned:
	/// trimmed lists without repeats, a rating within 0-5), returning them as saved
	async fn update_my_discover_rules(
		&self,
		ctx: &Context<'_>,
		input: DiscoverRules,
	) -> Result<DiscoverRules> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let rules = input.normalized();

		let existing = user_discover_rules::Entity::find_by_id(user.id.clone())
			.one(conn)
			.await?;
		let row = user_discover_rules::ActiveModel {
			user_id: Set(user.id.clone()),
			rules: Set(serde_json::to_string(&rules)?),
			updated_at: Set(Utc::now()),
		};
		if existing.is_some() {
			row.update(conn).await?;
		} else {
			row.insert(conn).await?;
		}
		Ok(rules)
	}

	/// Replace the authenticated user's home sections
	async fn update_home_arrangement(
		&self,
		ctx: &Context<'_>,
		input: HomeArrangementInput,
	) -> Result<HomeArrangement> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let arrangement = HomeArrangement::new(input.sections);

		let preferences = user_preferences::Entity::find()
			.filter(user_preferences::Column::UserId.eq(&user.id))
			.one(conn)
			.await?
			.ok_or("User preferences not found")?;

		let mut active_model = preferences.into_active_model();
		active_model.home_arrangement = Set(Some(arrangement.clone().into()));
		active_model.update(conn).await?;

		Ok(arrangement)
	}

	async fn update_navigation_arrangement_lock(
		&self,
		ctx: &Context<'_>,
		locked: bool,
	) -> Result<Arrangement> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;

		let preferences = user_preferences::Entity::find()
			.filter(user_preferences::Column::UserId.eq(&user.id))
			.one(conn)
			.await?
			.ok_or("User preferences not found")?;

		let updated_arrangement = Arrangement {
			locked,
			..preferences
				.navigation_arrangement
				.clone()
				.unwrap_or_else(Arrangement::default_navigation)
		};

		let mut active_model = preferences.into_active_model();
		active_model.navigation_arrangement = Set(Some(updated_arrangement.clone()));
		active_model.update(conn).await?;

		Ok(updated_arrangement)
	}

	async fn update_navigation_arrangement(
		&self,
		ctx: &Context<'_>,
		input: NavigationArrangementInput,
	) -> Result<Arrangement> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;

		let preferences = user_preferences::Entity::find()
			.filter(user_preferences::Column::UserId.eq(&user.id))
			.one(conn)
			.await?
			.ok_or("User preferences not found")?;

		let arrangement = preferences
			.navigation_arrangement
			.clone()
			.unwrap_or_else(Arrangement::default_navigation);

		if arrangement.locked {
			return Err("Navigation arrangement is locked".into());
		}

		let updated_arrangement = Arrangement {
			locked: arrangement.locked,
			sections: input.sections,
		};

		let mut active_model = preferences.into_active_model();
		active_model.navigation_arrangement = Set(Some(updated_arrangement.clone()));

		active_model.update(conn).await?;

		Ok(updated_arrangement)
	}
}

/// Signs a user out everywhere: removes their sessions and their refresh tokens
async fn remove_all_session_for_user(
	id: String,
	conn: &DatabaseConnection,
) -> Result<Vec<session::Model>> {
	let removed_sessions = revoke_user_sessions(conn, &id, None).await?;

	tracing::debug!(?removed_sessions, "Removed sessions for user");
	Ok(removed_sessions)
}

/// Signs a user out everywhere but the session this request came with (if it came with one),
/// for a user who just changed their own password: other browsers and apps must sign in again
async fn revoke_other_sessions(
	ctx: &Context<'_>,
	user_id: &str,
	conn: &DatabaseConnection,
) -> Result<()> {
	let current_session_id = ctx
		.data_opt::<Session>()
		.and_then(|session| session.id())
		.map(|id| id.to_string());

	let removed_sessions =
		revoke_user_sessions(conn, user_id, current_session_id.as_deref()).await?;

	tracing::debug!(?removed_sessions, "Removed the other sessions of user");
	Ok(())
}

async fn update_user_preferences_by_id(
	id: i32,
	user_id: String,
	user_preferences: UpdateUserPreferencesInput,
	conn: &DatabaseConnection,
) -> Result<UserPreferences> {
	// FIXME(graphql): I think this will overwrite the NotSet values if they are
	// currently set
	let updated_user_preferences = user_preferences::ActiveModel {
		id: Set(id),
		user_id: Set(Some(user_id)),
		locale: Set(user_preferences.locale),
		preferred_layout_mode: Set(user_preferences.preferred_layout_mode),
		app_theme: Set(user_preferences.app_theme),
		enable_gradients: Set(user_preferences.enable_gradients),
		app_font: Set(user_preferences.app_font),
		show_query_indicator: Set(user_preferences.show_query_indicator),
		enable_live_refetch: Set(user_preferences.enable_live_refetch),
		enable_discord_presence: Set(user_preferences.enable_discord_presence),
		enable_hide_scrollbar: Set(user_preferences.enable_hide_scrollbar),
		enable_job_overlay: Set(user_preferences.enable_job_overlay),
		enable_fancy_animations: Set(user_preferences.enable_fancy_animations),
		prefer_accent_color: Set(user_preferences.prefer_accent_color),
		thumbnail_ratio: Set(user_preferences.thumbnail_ratio),
		thumbnail_placeholder_style: Set(user_preferences.thumbnail_placeholder_style),
		enable_alphabet_select: Set(user_preferences.enable_alphabet_select),
		enable_reading_journal: Set(user_preferences.enable_reading_journal),
		day_reset_hour_offset: Set(user_preferences.day_reset_hour_offset),
		reading_session_grace_period_secs: Set(
			user_preferences.reading_session_grace_period_secs
		),
		interface_roundness: Set(user_preferences.interface_roundness),
		thumbnail_roundness: Set(user_preferences.thumbnail_roundness),
		home_arrangement: NotSet,
		navigation_arrangement: NotSet,
	};

	let updated_user_prefs = updated_user_preferences.update(conn).await?;

	Ok(UserPreferences::from(updated_user_prefs))
}

async fn update_user(
	by_user: &AuthUser,
	for_user_id: String,
	conn: &DatabaseConnection,
	config: &StumpConfig,
	input: &UpdateUserInput,
) -> Result<User> {
	// NOTE: there are other mechanisms in place to effectively disable logging in,
	// so I am making this a bad request. In the future, perhaps this can change.
	match input.max_sessions_allowed {
		Some(max_sessions_allowed) if max_sessions_allowed <= 0 => {
			return Err("max_sessions_allowed must be greater than 0 when set".into());
		},
		Some(max_sessions_allowed) => {
			tracing::trace!(?max_sessions_allowed, "The max sessions allowed is set");
		},
		_ => {},
	}

	let is_self_update = by_user.id == for_user_id;

	let is_different_username = input.username != by_user.username;
	if is_self_update
		&& is_different_username
		&& !by_user.has_permission(UserPermission::ChangeUsername)
	{
		return Err("You do not have permission to change the username".into());
	}

	// TODO(permissions): server owner goes away
	// only a server owner or a user with ManageUsers may set another user's
	// permissions, age restriction, and session cap.
	let can_manage_privileged_fields = (by_user.is_server_owner
		|| by_user.has_permission(UserPermission::ManageUsers))
		&& !is_self_update;
	if can_manage_privileged_fields {
		ensure_can_grant(by_user, &input.permissions)?;
	}

	let mut update_user = user::ActiveModel {
		id: Set(for_user_id.clone()),
		username: Set(input.username.clone()),
		..Default::default()
	};

	if let Some(password) = input.password.clone() {
		if is_self_update && !by_user.has_permission(UserPermission::ChangePassword) {
			return Err("You do not have permission to change the password".into());
		}
		let hashed_password = bcrypt::hash(password, config.password_hash_cost)?;
		update_user.hashed_password = Set(hashed_password);
	}

	let txn = conn.begin().await?;

	if can_manage_privileged_fields {
		update_user.max_sessions_allowed = Set(input.max_sessions_allowed);
		update_user_age_restriction(&for_user_id, &input.age_restriction, &txn).await?;
		let permissions = PermissionSet::new(input.permissions.clone());
		update_user.permissions = Set(permissions.resolve_into_string());
	}

	let updated_user_entity = update_user.update(&txn).await?;

	txn.commit().await?;

	Ok(User::from(updated_user_entity))
}

/// Only the permissions the person granting them holds can be granted, to a new account or an
/// existing one. Otherwise `ManageUsers` would be a way to every other permission (a user
/// created with all of them). The server owner holds them all.
fn ensure_can_grant(granter: &AuthUser, permissions: &[UserPermission]) -> Result<()> {
	if user_has_all_permissions(granter, permissions) {
		Ok(())
	} else {
		Err("You can only grant the permissions you hold yourself".into())
	}
}

async fn update_user_age_restriction(
	user_id: &str,
	age_restriction: &Option<AgeRestrictionInput>,
	txn: &DatabaseTransaction,
) -> Result<()> {
	let existing_age_restriction = age_restriction::Entity::find()
		.filter(age_restriction::Column::UserId.eq(user_id))
		.one(txn)
		.await?;

	if let Some(age_restriction) = age_restriction {
		let set_age_restriction_id = if let Some(restriction) = existing_age_restriction {
			Set(restriction.id)
		} else {
			NotSet
		};

		let _ = age_restriction::ActiveModel {
			id: set_age_restriction_id,
			user_id: Set(user_id.to_string()),
			age: Set(age_restriction.age),
			restrict_on_unset: Set(age_restriction.restrict_on_unset),
		}
		.save(txn)
		.await
		.map_err(|e| {
			tracing::error!("Failed to save age restriction: {:?}", e);
			"Failed to save age restriction"
		})?;

		Ok(())
	} else if let Some(existing_restriction) = existing_age_restriction {
		// delete age restriction
		let result = age_restriction::Entity::delete_by_id(existing_restriction.id)
			.exec(txn)
			.await
			.map_err(|e| {
				tracing::error!("Failed to delete age restriction: {:?}", e);
				"Failed to delete age restriction"
			})?;

		if result.rows_affected != 1 {
			return Err("Failed to delete age restriction".into());
		}

		Ok(())
	} else {
		Ok(())
	}
}

// TODO(tests): Add meaningful tests after permissions refactor flows through

/// The most a user's saved reading settings may weigh
const MAX_READER_SETTINGS_BYTES: usize = 1024 * 1024;

/// Reading settings must be a JSON object, and not a huge one
fn validate_reader_settings(settings: &str) -> Result<()> {
	if settings.len() > MAX_READER_SETTINGS_BYTES {
		return Err("Reading settings are too large".into());
	}
	match serde_json::from_str::<serde_json::Value>(settings) {
		Ok(serde_json::Value::Object(_)) => Ok(()),
		_ => Err("Reading settings must be a JSON object".into()),
	}
}

#[cfg(test)]
mod reader_settings_tests {
	use super::validate_reader_settings;

	#[test]
	fn accepts_only_json_objects_of_a_sane_size() {
		assert!(validate_reader_settings(r#"{"v":1,"reader":{}}"#).is_ok());
		assert!(validate_reader_settings("[1,2]").is_err());
		assert!(validate_reader_settings("not json").is_err());
		let huge = format!(r#"{{"x":"{}"}}"#, "a".repeat(1024 * 1024));
		assert!(validate_reader_settings(&huge).is_err());
	}
}
