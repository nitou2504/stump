use async_graphql::{ComplexObject, Context, Enum, Result, SimpleObject, ID};
use models::{
	entity::{intake_item, library, media},
	shared::intake::{
		IntakeDecision, IntakeDestination, IntakeFile, IntakeKind, IntakeProposal,
		IntakeResult, IntakeState,
	},
};
use sea_orm::{prelude::*, QuerySelect};
use stump_core::intake::decision_problems;

use crate::data::CoreContext;

/// How an upload of several files is grouped into inbox items
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Enum)]
pub enum IntakeUploadMode {
	/// Files in a folder (or books inside a .zip) make one series item; loose files are
	/// one book each
	#[default]
	Auto,
	/// Every file is its own book
	Books,
	/// All the files are the volumes of one series
	Series,
}

/// An item of the uploader's "Add books" inbox
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
	/// What the server proposes (empty while analysing)
	pub proposal: Option<IntakeProposal>,
	/// What will be filed: the proposal's defaults plus the uploader's edits
	pub decision: Option<IntakeDecision>,
	/// Where each file goes, relative to the library root
	pub destination: Option<IntakeDestination>,
	/// What filing did
	pub result: Option<IntakeResult>,
	pub error: Option<String>,
	pub created_at: DateTimeWithTimeZone,
	pub updated_at: DateTimeWithTimeZone,
	pub filed_at: Option<DateTimeWithTimeZone>,
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
			user_id: model.user_id,
		}
	}
}

#[ComplexObject]
impl IntakeItem {
	/// What still keeps the decision from being filed (empty when it can be accepted)
	async fn problems(&self) -> Vec<String> {
		match (&self.decision, &self.destination) {
			(Some(decision), Some(destination)) => decision_problems(decision, destination),
			_ => vec![],
		}
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

	/// The books created by the scan after filing (empty until the scan has run)
	async fn media_ids(&self, ctx: &Context<'_>) -> Result<Vec<String>> {
		let Some(result) = &self.result else {
			return Ok(vec![]);
		};
		if result.paths.is_empty() {
			return Ok(vec![]);
		}
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(media::Entity::find()
			.select_only()
			.column(media::Column::Id)
			.filter(media::Column::Path.is_in(result.paths.clone()))
			.filter(media::Column::DeletedAt.is_null())
			.into_tuple::<String>()
			.all(conn)
			.await?)
	}
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
