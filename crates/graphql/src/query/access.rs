use std::collections::HashMap;

use async_graphql::{Context, Object, Result};
use chrono::Utc;
use models::{
	entity::{library, user},
	services::{
		access as service,
		access_people::{people_of_links, person_summaries},
		access_warnings, signup,
	},
};
use sea_orm::{prelude::*, QuerySelect};
use stump_core::access::state as edge_state;

use crate::{
	data::CoreContext,
	guard::{NotViaPublicEdgeGuard, ServerOwnerGuard},
	object::{
		access::{
			page_of, AccessAuditPage, AccessBlockedUser, AccessDevicePage, AccessInvite,
			AccessInvitePage, AccessOverview, AccessPaginationValidator, AccessPerson,
			AccessPersonRow, AccessSignInPage,
		},
		access_signup::{
			AccessSigninStats, AccessSignupLink, AccessWarning, PresetLibrary,
			SignupPreset,
		},
	},
	pagination::{OffsetPagination, OffsetPaginationInfo},
};

/// The names of the libraries, by id
async fn library_names(
	conn: &sea_orm::DatabaseConnection,
) -> Result<HashMap<String, String>, DbErr> {
	Ok(library::Entity::find()
		.select_only()
		.column(library::Column::Id)
		.column(library::Column::Name)
		.into_tuple::<(String, String)>()
		.all(conn)
		.await?
		.into_iter()
		.collect())
}

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

		let stats = core_ctx.signin_guard.stats(core_ctx.signin_guard.now());
		Ok(AccessOverview {
			public_signin: edge.is_some_and(|edge| edge.public_signin),
			signin_stats: AccessSigninStats {
				failures_last_10_min: stats.failures_last_10_min as u32,
				names_waiting: stats.names_waiting as u32,
				global_budget_tripped: stats.global_tripped,
			},
			active_signup_links: signup::active_signup_links(conn, Utc::now())
				.await?
				.len() as u64,
			has_key,
			has_public_url,
			has_state_dir: state_dir.is_some(),
			configured: has_key && has_public_url && state_dir.is_some(),
			require_pass: edge.is_some_and(|edge| edge.require_pass),
			public_url: edge.and_then(|edge| edge.public_base_url()),
			state_written_at: state_dir.as_deref().and_then(edge_state::state_written_at),
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

		let people = service::people(conn).await?;
		let accounts: Vec<user::Model> =
			people.iter().map(|person| person.user.clone()).collect();
		let mut summaries = person_summaries(conn, Utc::now(), &accounts).await?;
		Ok(people
			.into_iter()
			.map(|person| {
				let summary = summaries.remove(&person.user.id);
				AccessPersonRow::new(person, summary)
			})
			.collect())
	}

	/// What a new signup link offers by default: the friend permissions, the library named
	/// "Biblioteca", and three devices. The owner can change all of it per link
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)")]
	async fn signup_preset_defaults(&self, ctx: &Context<'_>) -> Result<SignupPreset> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let names = library_names(conn).await?;
		let default = names
			.iter()
			.find(|(_, name)| {
				name.eq_ignore_ascii_case(
					models::services::access_policy::DEFAULT_PRESET_LIBRARY,
				)
			})
			.map(|(id, name)| PresetLibrary {
				id: id.clone(),
				name: name.clone(),
			});
		Ok(SignupPreset::defaults(default.into_iter().collect()))
	}

	/// The signup links that were made, the latest first, with the accounts each one made. The
	/// links themselves are not kept
	#[graphql(
		guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)",
		complexity = "crate::complexity::list_cost(pagination.page_size, child_complexity)"
	)]
	async fn access_signup_links(
		&self,
		ctx: &Context<'_>,
		#[graphql(default, validator(custom = "AccessPaginationValidator"))]
		pagination: OffsetPagination,
	) -> Result<crate::object::access_signup::AccessSignupLinkPage> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let (page, size) = page_of(&pagination);
		let now = Utc::now();

		let (rows, total) = signup::list_signup_links(conn, page, size).await?;
		let names = library_names(conn).await?;
		let ids: Vec<String> = rows.iter().map(|link| link.id.clone()).collect();
		let mut made = people_of_links(conn, &ids).await?;
		Ok(crate::object::access_signup::AccessSignupLinkPage {
			nodes: rows
				.into_iter()
				.map(|link| {
					let libraries = signup::preset_of(&link)
						.map(|preset| {
							preset
								.library_ids
								.iter()
								.filter_map(|id| {
									names.get(id).map(|name| PresetLibrary {
										id: id.clone(),
										name: name.clone(),
									})
								})
								.collect()
						})
						.unwrap_or_default();
					let people = made
						.remove(&link.id)
						.unwrap_or_default()
						.into_iter()
						.map(AccessPerson::from)
						.collect();
					AccessSignupLink::new(link, now, libraries, people)
				})
				.collect(),
			page_info: OffsetPaginationInfo::new(pagination, total),
		})
	}

	/// What the owner should look at: people at or over their device limit, four or more
	/// sessions, bursts of failed sign-ins, links near their budget. The alerts come first
	#[graphql(
		guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)",
		complexity = "crate::complexity::UNPAGINATED_LIST as usize * child_complexity"
	)]
	async fn access_warnings(&self, ctx: &Context<'_>) -> Result<Vec<AccessWarning>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let found = access_warnings::warnings(conn, Utc::now()).await?;
		let ids: Vec<String> = found
			.iter()
			.filter_map(|warning| warning.person_id.clone())
			.collect();
		let people: HashMap<String, AccessPerson> = if ids.is_empty() {
			HashMap::new()
		} else {
			user::Entity::find()
				.filter(user::Column::Id.is_in(ids))
				.all(conn)
				.await?
				.into_iter()
				.map(|account| (account.id.clone(), AccessPerson::from(account)))
				.collect()
		};
		Ok(found
			.into_iter()
			.map(|warning| {
				let person = warning
					.person_id
					.as_ref()
					.and_then(|id| people.get(id).cloned());
				AccessWarning::new(warning, person)
			})
			.collect())
	}

	/// How many warnings there are: what the badge in the sidebar shows. The owner at home only
	#[graphql(guard = "ServerOwnerGuard.and(NotViaPublicEdgeGuard)")]
	async fn access_warning_count(&self, ctx: &Context<'_>) -> Result<u32> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(access_warnings::warning_count(conn, Utc::now()).await? as u32)
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
