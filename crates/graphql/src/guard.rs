use async_graphql::{Context, Guard, Result};
use models::{
	entity::book_club_member,
	shared::{book_club::BookClubMemberRole, enums::UserPermission},
};

use crate::{
	data::{AuthContext, CoreContext, EdgeRequest},
	error_message,
};

/// Guard that checks if the user is the owner of the server.
pub struct ServerOwnerGuard;

impl Guard for ServerOwnerGuard {
	async fn check(&self, ctx: &Context<'_>) -> Result<()> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;

		if user.is_server_owner {
			Ok(())
		} else {
			Err(error_message::FORBIDDEN_ACTION.into())
		}
	}
}

/// Guard: the request must not have come through the public edge, whoever sent it. The owner's
/// session cookie is not tied to a port, so an owner who opens an invite link in their own browser
/// would otherwise reach their admin pages through the internet.
///
/// A request with no [`EdgeRequest`] (a handler that forgot to add it) is refused: not knowing
/// how a request arrived is not a reason to let it in.
pub struct NotViaPublicEdgeGuard;

impl Guard for NotViaPublicEdgeGuard {
	async fn check(&self, ctx: &Context<'_>) -> Result<()> {
		match ctx.data_opt::<EdgeRequest>() {
			Some(edge) if !edge.via_public => Ok(()),
			_ => Err(error_message::FORBIDDEN_ACTION.into()),
		}
	}
}

/// Guard for the mutations of the admin pages: a browser that another site made send the request
/// is refused. A form on another site can post a GraphQL multipart request (and, from the same
/// site, `SameSite=Lax` cookies still go along), so the check is on where the browser says the
/// request comes from. Clients that are not browsers (a script with a token) send neither header
/// and are not affected.
///
/// A request with no [`EdgeRequest`] is refused, like in [`NotViaPublicEdgeGuard`].
pub struct SameOriginGuard;

impl Guard for SameOriginGuard {
	async fn check(&self, ctx: &Context<'_>) -> Result<()> {
		match ctx.data_opt::<EdgeRequest>() {
			Some(edge) if !edge.cross_site => Ok(()),
			_ => Err(error_message::FORBIDDEN_ACTION.into()),
		}
	}
}

/// Guard that checks if the user is themselves. This is dependent on the user ID
/// provided and compared with the ID of the current user. So be sure to provide
/// the correct user ID.
pub struct SelfGuard {
	pub user_id: String,
}

impl SelfGuard {
	pub fn new(user_id: &str) -> Self {
		Self {
			user_id: user_id.to_string(),
		}
	}
}

impl Guard for SelfGuard {
	async fn check(&self, ctx: &Context<'_>) -> Result<()> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;

		if user.id == self.user_id {
			Ok(())
		} else {
			Err(error_message::FORBIDDEN_ACTION.into())
		}
	}
}

/// Guard that checks if the user has the required permissions to perform an action.
/// If the user does not have the required permissions, an error is returned.
///
/// [`PermissionGuard::new`] lets in a user with **any one** of the permissions, which suits
/// an action that more than one permission is meant to allow. When an action needs several
/// of them at once (changing who may see a library takes both the library and the users),
/// use [`PermissionGuard::all`]: listing them in `new` would let in anyone with only one.
pub struct PermissionGuard {
	permissions: Vec<UserPermission>,
	require_all: bool,
}

impl PermissionGuard {
	/// The user needs any one of the permissions
	pub fn new(permissions: &[UserPermission]) -> Self {
		Self {
			permissions: permissions.to_vec(),
			require_all: false,
		}
	}

	/// The user needs every one of the permissions (an empty list lets in nobody)
	pub fn all(permissions: &[UserPermission]) -> Self {
		Self {
			permissions: permissions.to_vec(),
			require_all: true,
		}
	}

	pub fn one(permission: UserPermission) -> Self {
		Self {
			permissions: vec![permission],
			require_all: false,
		}
	}
}

impl Guard for PermissionGuard {
	async fn check(&self, ctx: &Context<'_>) -> Result<()> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;

		if user.is_server_owner {
			return Ok(());
		}

		let authorized = if self.require_all {
			!self.permissions.is_empty()
				&& self
					.permissions
					.iter()
					.all(|p| user.permissions.contains(p))
		} else {
			self.permissions
				.iter()
				.any(|p| user.permissions.contains(p))
		};
		let permitted = authorized && !user.is_locked;

		if permitted {
			Ok(())
		} else {
			Err(error_message::FORBIDDEN_ACTION.into())
		}
	}
}

/// Optional features that can be enabled or disabled in the server configuration.
#[derive(Debug, Clone, Copy)]
pub enum OptionalFeature {
	Upload,
	KoReader,
}

/// Guard that checks if an optional feature is enabled in the server configuration.
/// If the feature is disabled, an error is returned.
pub struct OptionalFeatureGuard {
	feature: OptionalFeature,
}

impl OptionalFeatureGuard {
	pub fn new(feature: OptionalFeature) -> Self {
		Self { feature }
	}
}

impl Guard for OptionalFeatureGuard {
	async fn check(&self, ctx: &Context<'_>) -> Result<()> {
		let core = ctx.data::<CoreContext>()?;

		let permitted = match self.feature {
			OptionalFeature::Upload => core.config.enable_upload,
			OptionalFeature::KoReader => core.config.enable_koreader_sync,
		};

		if permitted {
			Ok(())
		} else {
			Err(error_message::DISABLED_FEATURE.into())
		}
	}
}

pub struct BookClubRoleGuard {
	club_id: String,
	role: BookClubMemberRole,
}

impl BookClubRoleGuard {
	pub fn new(club_id: &str, role: BookClubMemberRole) -> Self {
		Self {
			club_id: club_id.to_string(),
			role,
		}
	}
}

impl Guard for BookClubRoleGuard {
	async fn check(&self, ctx: &Context<'_>) -> Result<()> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;

		if user.is_server_owner {
			return Ok(());
		}

		let Some(membership) =
			book_club_member::Entity::find_by_club_for_user(user, &self.club_id)
				.one(core.conn.as_ref())
				.await?
		else {
			return Err(error_message::FORBIDDEN_ACTION.into());
		};

		let is_permitted = membership.role >= self.role && !user.is_locked;

		if is_permitted {
			Ok(())
		} else {
			Err(error_message::FORBIDDEN_ACTION.into())
		}
	}
}
