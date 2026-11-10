use async_graphql::{Context, Object, Result};
use chrono::Utc;
use models::services::access as service;
use stump_core::access::state as edge_state;

use crate::{
	data::CoreContext,
	guard::{NotViaPublicEdgeGuard, ServerOwnerGuard},
	object::access::{
		page_of, AccessAuditPage, AccessBlockedUser, AccessDevicePage, AccessInvite,
		AccessInvitePage, AccessOverview, AccessPaginationValidator, AccessPersonRow,
		AccessSignInPage,
	},
	pagination::{OffsetPagination, OffsetPaginationInfo},
};

/// The queries of the *Public access* page. Every one is for the server owner only, and is
/// refused on a request that came through the public edge: it shows device addresses, browsers
/// and who was locked out
#[derive(Default)]
pub struct AccessQuery;

#[Object]
impl AccessQuery {
	/// What is set up for public access, and what is missing
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)")]
	async fn access_overview(&self, ctx: &Context<'_>) -> Result<AccessOverview> {
		let core_ctx = ctx.data::<CoreContext>()?;
		let conn = core_ctx.conn.as_ref();
		let edge = core_ctx.config.edge.as_ref();

		let has_key = edge.is_some_and(|edge| edge.usable_key().is_some());
		let has_public_url = edge.is_some_and(|edge| edge.public_base_url().is_some());
		let state_dir = edge_state::state_dir(core_ctx.config.as_ref());

		Ok(AccessOverview {
			has_key,
			has_public_url,
			has_state_dir: state_dir.is_some(),
			configured: has_key && has_public_url && state_dir.is_some(),
			require_pass: edge.is_some_and(|edge| edge.require_pass),
			public_url: edge.and_then(|edge| edge.public_base_url()),
			state_written_at: state_dir
				.as_deref()
				.and_then(edge_state::state_written_at),
			active_devices: service::count_active_devices(conn).await?,
			open_invites: service::count_open_invites(conn, Utc::now()).await?,
		})
	}

	/// The invite links that were made, the latest first. The links themselves are not kept
	#[graphql(
		guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)",
		complexity = "crate::complexity::list_cost(pagination.page_size, child_complexity)"
	)]
	async fn access_invites(
		&self,
		ctx: &Context<'_>,
		#[graphql(default, validator(custom = "AccessPaginationValidator"))]
		pagination: OffsetPagination,
	) -> Result<AccessInvitePage> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let (page, size) = page_of(&pagination);

		let now = Utc::now();
		let (rows, total) = service::list_invites(conn, page, size).await?;
		Ok(AccessInvitePage {
			nodes: rows
				.into_iter()
				.map(|invite| AccessInvite::new(invite, now))
				.collect(),
			page_info: OffsetPaginationInfo::new(pagination, total),
		})
	}

	/// The devices that hold a pass, the latest made first
	#[graphql(
		guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)",
		complexity = "crate::complexity::list_cost(pagination.page_size, child_complexity)"
	)]
	async fn access_devices(
		&self,
		ctx: &Context<'_>,
		#[graphql(default, validator(custom = "AccessPaginationValidator"))]
		pagination: OffsetPagination,
	) -> Result<AccessDevicePage> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let (page, size) = page_of(&pagination);

		let (rows, total) = service::list_devices(conn, page, size).await?;
		Ok(AccessDevicePage {
			nodes: rows.into_iter().map(Into::into).collect(),
			page_info: OffsetPaginationInfo::new(pagination, total),
		})
	}

	/// The sign-in attempts, the latest first: the account's login activity with the pass each
	/// came with and why a failure failed. A password is never stored
	#[graphql(
		guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)",
		complexity = "crate::complexity::list_cost(pagination.page_size, child_complexity)"
	)]
	async fn access_sign_ins(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] failures_only: bool,
		#[graphql(default, validator(custom = "AccessPaginationValidator"))]
		pagination: OffsetPagination,
	) -> Result<AccessSignInPage> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let (page, size) = page_of(&pagination);

		let (rows, total) =
			service::list_sign_ins(conn, failures_only, page, size).await?;
		Ok(AccessSignInPage {
			nodes: rows.into_iter().map(Into::into).collect(),
			page_info: OffsetPaginationInfo::new(pagination, total),
		})
	}

	/// The accounts that are locked right now, with who locked them
	#[graphql(
		guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)",
		complexity = "crate::complexity::RELATED_LIST * child_complexity"
	)]
	async fn access_blocked_users(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<AccessBlockedUser>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		Ok(service::blocked_users(conn, Utc::now())
			.await?
			.into_iter()
			.map(Into::into)
			.collect())
	}

	/// The accounts of the server and how each stands, for setting a password or unblocking
	#[graphql(
		guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)",
		complexity = "crate::complexity::UNPAGINATED_LIST as usize * child_complexity"
	)]
	async fn access_people(&self, ctx: &Context<'_>) -> Result<Vec<AccessPersonRow>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		Ok(service::people(conn)
			.await?
			.into_iter()
			.map(Into::into)
			.collect())
	}

	/// What the owner did on this page, the latest first
	#[graphql(
		guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)",
		complexity = "crate::complexity::list_cost(pagination.page_size, child_complexity)"
	)]
	async fn access_audit(
		&self,
		ctx: &Context<'_>,
		#[graphql(default, validator(custom = "AccessPaginationValidator"))]
		pagination: OffsetPagination,
	) -> Result<crate::object::access::AccessAuditPage> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let (page, size) = page_of(&pagination);

		let (rows, total) = service::list_audit(conn, page, size).await?;
		Ok(AccessAuditPage {
			nodes: rows.into_iter().map(Into::into).collect(),
			page_info: OffsetPaginationInfo::new(pagination, total),
		})
	}
}
