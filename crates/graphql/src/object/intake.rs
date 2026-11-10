use async_graphql::{ComplexObject, Context, Result, SimpleObject, ID};
use models::{
	entity::{intake_item, library, library_config, media, user},
	shared::{
		enums::LibraryType,
		intake::{
			IntakeDecision, IntakeDestination, IntakeFile, IntakeKind, IntakeProposal,
			IntakeResult, IntakeSeriesKind, IntakeState,
		},
	},
};
use sea_orm::{prelude::*, QueryOrder, QuerySelect};
use stump_core::intake::{
	approval::can_file_directly, decision_problems, library_type_of, scope,
};

use crate::{
	data::{AuthContext, CoreContext},
	object::reader_profile::ReaderProfile,
};

/// An item of the uploader's "Add books" inbox
///
/// What it shows depends on who looks. An approver (the server owner, or anyone who manages
/// libraries) sees it as the analysis made it. Whoever uploaded it, and may not file books
/// themselves, sees it through their own limits: no folders, no destination, and similar books,
/// duplicates and suggested tags only from books they can read (see `stump_core::intake::scope`).
#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct IntakeItem {
	pub id: ID,
	pub library_id: String,
	pub state: IntakeState,
	pub kind: IntakeKind,
	/// The uploaded file or folder name
	pub name: String,
	pub files: Vec<IntakeFile>,
	#[graphql(skip)]
	pub proposal: Option<IntakeProposal>,
	#[graphql(skip)]
	pub decision: Option<IntakeDecision>,
	#[graphql(skip)]
	pub destination: Option<IntakeDestination>,
	#[graphql(skip)]
	pub result: Option<IntakeResult>,
	pub error: Option<String>,
	pub created_at: DateTimeWithTimeZone,
	pub updated_at: DateTimeWithTimeZone,
	pub filed_at: Option<DateTimeWithTimeZone>,
	/// When it was sent for approval, while it waits there
	pub submitted_at: Option<DateTimeWithTimeZone>,
	/// When an approver last decided on it
	pub reviewed_at: Option<DateTimeWithTimeZone>,
	/// What the approver wrote when sending it back
	pub review_note: Option<String>,
	#[graphql(skip)]
	pub user_id: String,
}

impl From<intake_item::Model> for IntakeItem {
	fn from(model: intake_item::Model) -> Self {
		Self {
			id: model.id.into(),
			library_id: model.library_id,
			state: model.state,
			kind: model.kind,
			name: model.name,
			files: model.files.0,
			proposal: model.proposal,
			decision: model.decision,
			destination: model.destination,
			result: model.result,
			error: model.error,
			created_at: model.created_at,
			updated_at: model.updated_at,
			filed_at: model.filed_at,
			submitted_at: model.submitted_at,
			reviewed_at: model.reviewed_at,
			review_note: model.review_note,
			user_id: model.user_id,
		}
	}
}

/// Whether the viewer sees items as the analysis made them (they file books themselves)
fn sees_everything(ctx: &Context<'_>) -> Result<bool> {
	let AuthContext { user, .. } = ctx.data()?;
	Ok(can_file_directly(user))
}

#[ComplexObject]
impl IntakeItem {
	/// What the server proposes (empty while analysing)
	async fn proposal(&self, ctx: &Context<'_>) -> Result<Option<IntakeProposal>> {
		let Some(proposal) = self.proposal.clone() else {
			return Ok(None);
		};
		if sees_everything(ctx)? {
			return Ok(Some(proposal));
		}
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(Some(
			scope::proposal_for_uploader(conn, user, &self.library_id, proposal).await?,
		))
	}

	/// What will be filed: the proposal's defaults plus the uploader's edits. For somebody who
	/// can't choose where a book goes, without the folder and the file names
	async fn decision(&self, ctx: &Context<'_>) -> Result<Option<IntakeDecision>> {
		let Some(decision) = self.decision.clone() else {
			return Ok(None);
		};
		if sees_everything(ctx)? {
			return Ok(Some(decision));
		}
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let subjects = self
			.proposal
			.as_ref()
			.map(|p| p.metadata.subjects.clone())
			.unwrap_or_default();
		Ok(Some(
			scope::decision_for_uploader(
				conn,
				user,
				&self.library_id,
				decision,
				&subjects,
			)
			.await?,
		))
	}

	/// Where each file goes, relative to the library root. Only for approvers: it is the
	/// library's own layout, and which names exist is an answer about the library
	async fn destination(&self, ctx: &Context<'_>) -> Result<Option<IntakeDestination>> {
		match self.destination.clone() {
			Some(destination) if sees_everything(ctx)? => Ok(Some(destination)),
			Some(destination) => Ok(scope::destination_for_uploader(destination)),
			None => Ok(None),
		}
	}

	/// What filing did. For somebody who files nothing themselves, the paths are inside the
	/// library, not on the server's disk
	async fn result(&self, ctx: &Context<'_>) -> Result<Option<IntakeResult>> {
		let Some(mut result) = self.result.clone() else {
			return Ok(None);
		};
		if sees_everything(ctx)? {
			return Ok(Some(result));
		}
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let root = library::Entity::find_by_id(self.library_id.clone())
			.select_only()
			.column(library::Column::Path)
			.into_tuple::<String>()
			.one(conn)
			.await?
			.unwrap_or_default();
		for path in &mut result.paths {
			*path = std::path::Path::new(path.as_str())
				.strip_prefix(&root)
				.map(|relative| relative.to_string_lossy().into_owned())
				.unwrap_or_default();
		}
		Ok(Some(result))
	}

	/// What still keeps the decision from being filed (empty when it can be accepted). For
	/// somebody who may not file books themselves, what they can fix
	async fn problems(&self, ctx: &Context<'_>) -> Result<Vec<String>> {
		let problems = match (&self.decision, &self.destination) {
			(Some(decision), Some(destination)) => {
				decision_problems(decision, destination)
			},
			_ => vec![],
		};
		Ok(if sees_everything(ctx)? {
			problems
		} else {
			scope::problems_for_uploader(problems)
		})
	}

	/// Who uploaded it
	async fn uploader(&self, ctx: &Context<'_>) -> Result<Option<ReaderProfile>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(user::Entity::find_by_id(self.user_id.clone())
			.filter(user::Column::DeletedAt.is_null())
			.one(conn)
			.await?
			.map(ReaderProfile::from))
	}

	async fn library_name(&self, ctx: &Context<'_>) -> Result<String> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(library::Entity::find_by_id(self.library_id.clone())
			.select_only()
			.column(library::Column::Name)
			.into_tuple::<String>()
			.one(conn)
			.await?
			.unwrap_or_default())
	}

	/// The type of the item's library
	async fn library_type(&self, ctx: &Context<'_>) -> Result<LibraryType> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(library_type_of(conn, &self.library_id).await?)
	}

	/// A library of its own for what the item looks like, when it sits in a book or mixed
	/// library: the first manga or light novel library (by name) the viewer may use. Only
	/// libraries the viewer may use are ever named.
	async fn suggested_library(
		&self,
		ctx: &Context<'_>,
	) -> Result<Option<IntakeSuggestedLibrary>> {
		let Some(kind) = self.proposal.as_ref().and_then(|p| p.series_kind) else {
			return Ok(None);
		};
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		if !matches!(
			library_type_of(conn, &self.library_id).await?,
			LibraryType::Book | LibraryType::Mixed
		) {
			return Ok(None);
		}
		let wanted = match kind {
			IntakeSeriesKind::Manga => LibraryType::Manga,
			IntakeSeriesKind::LightNovel => LibraryType::LightNovel,
		};
		Ok(library::Entity::find_for_user(user)
			.find_also_related(library_config::Entity)
			.filter(library_config::Column::LibraryType.eq(wanted))
			.order_by_asc(library::Column::Name)
			.one(conn)
			.await?
			.map(|(library, _)| IntakeSuggestedLibrary {
				id: library.id.into(),
				name: library.name,
			}))
	}

	/// The books created by the scan after filing that the viewer can read (empty until the scan
	/// has run)
	async fn media_ids(&self, ctx: &Context<'_>) -> Result<Vec<String>> {
		let Some(result) = &self.result else {
			return Ok(vec![]);
		};
		if result.paths.is_empty() {
			return Ok(vec![]);
		}
		let AuthContext { user, .. } = ctx.data()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(media::Entity::find_for_user(user)
			.select_only()
			.column(media::Column::Id)
			.filter(media::Column::Path.is_in(result.paths.clone()))
			.into_tuple::<String>()
			.all(conn)
			.await?)
	}
}

/// A library an item could be filed into instead, for what it looks like
#[derive(Debug, Clone, SimpleObject)]
pub struct IntakeSuggestedLibrary {
	pub id: ID,
	pub name: String,
}

/// A tag the library uses, for the tag picker
#[derive(Debug, Clone, SimpleObject)]
pub struct IntakeVocabularyEntry {
	pub value: String,
	/// How many books carry it
	pub count: i32,
	/// `GENRE` or `FACET`
	pub kind: String,
}

/// An existing author or series folder, for the folder picker
#[derive(Debug, Clone, SimpleObject)]
pub struct IntakeFolder {
	pub name: String,
	pub books: i32,
}

/// A book filed through the intake, for the admin "Recently added" page
#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct RecentlyAddedBook {
	pub item_id: ID,
	pub user_id: String,
	pub username: Option<String>,
	pub title: String,
	pub authors: Vec<String>,
	pub series: Option<String>,
	/// Relative to the library root
	pub paths: Vec<String>,
	pub library_id: String,
	pub library_name: String,
	pub filed_at: Option<DateTimeWithTimeZone>,
	#[graphql(skip)]
	pub absolute_paths: Vec<String>,
}

#[ComplexObject]
impl RecentlyAddedBook {
	/// The library books created from it (empty until the scan has run)
	async fn media_ids(&self, ctx: &Context<'_>) -> Result<Vec<String>> {
		if self.absolute_paths.is_empty() {
			return Ok(vec![]);
		}
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(media::Entity::find()
			.select_only()
			.column(media::Column::Id)
			.filter(media::Column::Path.is_in(self.absolute_paths.clone()))
			.filter(media::Column::DeletedAt.is_null())
			.into_tuple::<String>()
			.all(conn)
			.await?)
	}
}

/// A page of the items waiting for approval
#[derive(Debug, Clone, SimpleObject)]
pub struct IntakeItemPage {
	pub nodes: Vec<IntakeItem>,
	/// How many items wait in all, not only on this page
	pub total_count: i64,
}

/// What the viewer may upload now, and how much of it they have used
#[derive(Debug, Clone, SimpleObject)]
pub struct IntakeUploadLimits {
	/// The most bytes in one upload request
	pub max_request_bytes: i64,
	/// The biggest single file
	pub max_file_bytes: i64,
	pub max_files_per_request: i64,
	/// The most bytes of the viewer's books waiting in their inbox
	pub quota_bytes: i64,
	/// The bytes waiting there now
	pub used_bytes: i64,
	/// The most open items (not filed, discarded or sent back)
	pub max_items: i64,
	pub open_items: i64,
	/// Whether the quota and the item limit hold for the viewer (not for somebody who files
	/// books themselves)
	pub quota_applies: bool,
	/// Whether the viewer files books themselves: there is nobody to send them to
	pub can_file_directly: bool,
	/// Whether uploads from the viewer are accepted now
	pub uploads_open: bool,
	/// Why not, when they are not
	pub closed_reason: Option<String>,
}

/// The limits that apply to somebody, and the ones an admin gave them
#[derive(Debug, Clone, SimpleObject)]
pub struct UserIntakeLimits {
	pub user_id: ID,
	/// The user's own limit, in MB; empty follows the server's
	pub max_file_mb: Option<i64>,
	pub max_files_per_request: Option<i64>,
	pub quota_mb: Option<i64>,
	pub max_items: Option<i64>,
	/// What applies to them now, from the server's limits and their own
	pub effective: IntakeLimitValues,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct IntakeLimitValues {
	pub max_request_bytes: i64,
	pub max_file_bytes: i64,
	pub max_files_per_request: i64,
	pub quota_bytes: i64,
	pub max_items: i64,
}

impl From<stump_core::intake::limits::IntakeLimits> for IntakeLimitValues {
	fn from(limits: stump_core::intake::limits::IntakeLimits) -> Self {
		let clamp = |value: u64| i64::try_from(value).unwrap_or(i64::MAX);
		Self {
			max_request_bytes: clamp(limits.max_request_bytes),
			max_file_bytes: clamp(limits.max_file_bytes),
			max_files_per_request: clamp(limits.max_files_per_request),
			quota_bytes: clamp(limits.quota_bytes),
			max_items: clamp(limits.max_items),
		}
	}
}
