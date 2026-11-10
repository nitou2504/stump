//! The "Add books" intake: per-user uploads are staged under the config directory,
//! analysed (embedded metadata, file name, Open Library / Google Books, matching against the
//! library), reviewed by the uploader, then filed into the library with their metadata
//! written into the files and a scan of just the new folders. The filed items (who added
//! what, when, where) are the record: `recentlyAddedBooks` reads them from the database.
//! Nothing runs between requests: analysis is a short task per upload, and the library index
//! it uses is dropped minutes later.

pub mod approval;
pub mod batch;
pub mod describe;
pub mod edit;
pub mod extract;
pub mod library_index;
pub mod limits;
pub mod lookup;
pub mod naming;
pub mod probe;
pub mod rules;
pub mod scope;
pub mod staging;
pub mod suggest;
pub mod title_page;
pub mod upload;
pub mod write;

use std::{
	collections::BTreeSet,
	path::{Component, Path, PathBuf},
};

use models::{
	entity::{
		intake_item, library, library_config,
		library_intake_settings::{self, typed_series_kind},
		media, user,
	},
	shared::{
		enums::{LibraryType, OrganizationMode},
		intake::{
			IntakeDecision, IntakeDestination, IntakeDestinationFile, IntakeDuplicate,
			IntakeFile, IntakeKind, IntakeMetadata, IntakeProposal, IntakeResult,
			IntakeReviewInput, IntakeSeriesKind, IntakeSource, IntakeState,
			IntakeUploaderView,
		},
	},
};
use sea_orm::{prelude::*, ActiveValue::Set, IntoActiveModel, QueryOrder};

use crate::{
	job::StumpJob,
	media::processor::{process_file, MediaProcessorOptions},
	scan::own_writes,
	CoreError, CoreResult, Ctx,
};

use self::{
	extract::{extract_file, metadata_of},
	library_index::library_index,
	naming::{
		book_file_name, fold_name, is_plain_component, series_folder_name,
		series_volume_file_name,
	},
	suggest::{build_proposal, Inputs, READY_CONFIDENCE},
	write::{write_cbz, write_epub, write_pdf, FileMetadata, KeptPdfInfo},
};

/// Book formats the intake accepts
pub const BOOK_EXTENSIONS: &[&str] = &["epub", "pdf", "cbz", "cbr"];

/// The root of all staged uploads
pub fn staging_root(ctx: &Ctx) -> PathBuf {
	ctx.config.config_directory().join("intake")
}

/// The staging directory of one item
pub fn staging_dir(ctx: &Ctx, user_id: &str, item_id: &str) -> PathBuf {
	staging_root(ctx).join(user_id).join(item_id)
}

/// The intake settings of a library (its stored row, or the defaults of its type)
pub async fn settings_for(
	conn: &DatabaseConnection,
	library_id: &str,
) -> CoreResult<library_intake_settings::Model> {
	let (library, config) = load_library(conn, library_id).await?;
	settings_of(conn, &library.id, library_type_in(config.as_ref())).await
}

/// The settings of a library whose type is known: its stored row, or the defaults of the type
async fn settings_of(
	conn: &DatabaseConnection,
	library_id: &str,
	library_type: LibraryType,
) -> CoreResult<library_intake_settings::Model> {
	Ok(
		library_intake_settings::Entity::find_by_id(library_id.to_string())
			.one(conn)
			.await?
			.unwrap_or_else(|| {
				library_intake_settings::Model::defaults_for(library_id, library_type)
			}),
	)
}

/// The type of a library; one without a configuration is a mixed library, as its default says
pub fn library_type_in(config: Option<&library_config::Model>) -> LibraryType {
	config.map(|c| c.library_type).unwrap_or_default()
}

/// The type of the library with this id
pub async fn library_type_of(
	conn: &DatabaseConnection,
	library_id: &str,
) -> CoreResult<LibraryType> {
	let (_, config) = load_library(conn, library_id).await?;
	Ok(library_type_in(config.as_ref()))
}

pub(crate) async fn load_library(
	conn: &DatabaseConnection,
	library_id: &str,
) -> CoreResult<(library::Model, Option<library_config::Model>)> {
	library::Entity::find_by_id(library_id.to_string())
		.find_also_related(library_config::Entity)
		.one(conn)
		.await?
		.ok_or_else(|| CoreError::NotFound(format!("Library {library_id} not found")))
}

pub fn extension_of(name: &str) -> String {
	Path::new(name)
		.extension()
		.map(|e| e.to_string_lossy().to_lowercase())
		.unwrap_or_default()
}

/// Analyses an item: reads its files, looks it up online (when enabled), matches it
/// against the library and stores the proposal, a default decision and its destination
pub async fn analyse(ctx: &Ctx, item_id: &str, online: Option<bool>) -> CoreResult<()> {
	let conn = ctx.conn.as_ref();
	let item = intake_item::Entity::find_by_id(item_id.to_string())
		.one(conn)
		.await?
		.ok_or_else(|| CoreError::NotFound(format!("Intake item {item_id} not found")))?;
	match analyse_item(ctx, &item, online).await {
		Ok(()) => Ok(()),
		Err(error) => {
			tracing::warn!(?error, item_id, "Intake analysis failed");
			// the failure is stored under the same rule as an outcome: an item moved to another
			// library meanwhile (it is analysed there), discarded or filed keeps its own state
			let failed = intake_item::ActiveModel {
				state: Set(IntakeState::Failed),
				error: Set(Some(format!("Analysis failed: {error}"))),
				..Default::default()
			};
			if let Err(write_error) = store_outcome(conn, &item, failed).await {
				tracing::warn!(
					?write_error,
					item_id,
					"Couldn't store the failure of an intake analysis"
				);
			}
			Err(error)
		},
	}
}

/// Whether a book is looked up online: it was asked for, the item is a book (not a series of
/// volumes) and not a comic, and the file isn't tagged well enough already (an author, and two
/// subjects that the library knows).
///
/// That a lookup was made, and what it found, is part of what whoever reads the outcome is shown
/// (`lookedUpOnline`, its sources), so what the library "knows" is what THEY may see of it:
/// `viewer_index` is the library as an uploader who may not file books themselves can read it.
/// Decided on the whole library, it would tell them whether any book holds a subject they guess,
/// those they can't read included.
pub(crate) fn lookup_wanted(
	wants_online: bool,
	kind: IntakeKind,
	is_comic: bool,
	meta: &IntakeMetadata,
	index: &library_index::LibraryIndex,
	viewer_index: Option<&library_index::LibraryIndex>,
) -> bool {
	let known = viewer_index.unwrap_or(index);
	let has_good_tags =
		meta.subjects
			.iter()
			.filter(|s| known.vocab_index(&library_index::norm(s)).is_some())
			.count() >= 2;
	wants_online
		&& kind == IntakeKind::Book
		&& !is_comic
		&& !(has_good_tags && !meta.authors.is_empty())
}

async fn analyse_item(
	ctx: &Ctx,
	item: &intake_item::Model,
	online: Option<bool>,
) -> CoreResult<()> {
	let conn = ctx.conn.as_ref();
	let (library, config) = load_library(conn, &item.library_id).await?;
	let library_type = library_type_in(config.as_ref());
	let settings = settings_of(conn, &library.id, library_type).await?;
	let root = PathBuf::from(&library.path);
	let index = library_index(
		conn,
		&library.id,
		&root,
		&settings.manga_dir,
		&settings.light_novel_dir,
	)
	.await?;

	// What somebody who may not file books themselves is shown is made from the books they can
	// read alone: the same analysis over a narrower library (see `scope`). That goes for what
	// is decided here too, if what is decided shows (that a book was looked up online does): so
	// their index is made first. The proposal for whoever approves is made from the whole of it.
	let uploader_can_file =
		approval::uploader_can_file_directly(conn, &item.user_id).await?;
	let viewer_index = if uploader_can_file {
		None
	} else {
		let viewer = approval::load_auth_user(conn, &item.user_id).await?;
		Some(
			library_index::viewer_index(
				conn,
				viewer.as_ref(),
				&item.user_id,
				&library.id,
				&root,
				&settings.manga_dir,
				&settings.light_novel_dir,
			)
			.await?,
		)
	};

	let staging = PathBuf::from(&item.staging_dir);
	let mut files = item.files.0.clone();
	let mut per_file: Vec<IntakeMetadata> = vec![];
	let mut series_hint = None;
	for file in files.iter_mut() {
		let extracted = extract_file(
			&staging.join(&file.staged_name),
			&file.original_name,
			&ctx.config,
		)
		.await;
		file.hash = extracted.hash.clone();
		let meta = metadata_of(&extracted);
		file.title = meta.title.clone();
		file.volume = meta
			.series_index
			.or(extracted.parsed_name.volume)
			.or(file.volume);
		if extracted
			.embedded
			.as_ref()
			.and_then(|e| e.format.as_deref())
			.is_some_and(|f| f.to_lowercase().contains("manga"))
		{
			series_hint = Some(IntakeSeriesKind::Manga);
		}
		per_file.push(meta);
	}

	// a series item takes the first volume's metadata, minus what is per volume
	let mut file_meta = per_file.first().cloned().unwrap_or_default();
	if item.kind == IntakeKind::Series {
		file_meta.series_index = None;
		if let Some(series) = per_file.iter().find_map(|m| m.series.clone()) {
			file_meta.series = Some(series);
		}
	}

	let formats: Vec<String> = files.iter().map(|f| f.format.clone()).collect();
	let wants_online = online.unwrap_or(settings.online_lookup);
	let is_comic = formats.iter().all(|f| f == "cbz" || f == "cbr");
	let lookup = if lookup_wanted(
		wants_online,
		item.kind,
		is_comic,
		&file_meta,
		&index,
		viewer_index.as_deref(),
	) {
		let epub = files
			.iter()
			.find(|f| f.format == "epub")
			.map(|f| staging.join(&f.staged_name));
		let original_title = epub.as_deref().and_then(lookup::original_title_of);
		let query = lookup::LookupQuery {
			isbn: file_meta.isbn.clone(),
			title: file_meta.title.clone(),
			authors: file_meta.authors.clone(),
			original_title,
			series: file_meta.series.clone(),
			by_series: false,
		};
		// junk metadata (`OUP Book`, a swapped title and author): searched by the title page
		let (query, _) = title_page::repair_for_lookup(
			query,
			epub.as_deref().unwrap_or_else(|| Path::new("")),
		)
		.await;
		lookup::lookup(conn, &query).await
	} else {
		lookup::LookupOutcome::default()
	};
	let online_records = lookup.records.clone();

	let description = file_meta
		.description
		.clone()
		.or_else(|| online_records.iter().find_map(|r| r.description.clone()))
		.unwrap_or_default();
	let similar = index.similar_by_description(conn, &description, 10).await;
	let group_name = (item.kind == IntakeKind::Series).then(|| item.name.clone());

	// The proposal below is the approver's, made from the whole library; the uploader's view is
	// made over their own index.
	let uploader_view = if let Some(viewer_index) = &viewer_index {
		let viewer_similar = viewer_index
			.similar_by_description(conn, &description, 10)
			.await;
		let restricted = build_proposal(Inputs {
			kind: item.kind,
			index: viewer_index,
			file_meta: file_meta.clone(),
			online: online_records.clone(),
			formats: formats.clone(),
			series_hint,
			group_name: group_name.clone(),
			similar: viewer_similar,
			library_type,
		});
		let mut warnings = restricted.warnings;
		warnings.extend(lookup.notes());
		Some(IntakeUploaderView {
			series_kind: restricted.series_kind,
			genre_suggestions: restricted.genre_suggestions,
			similar_books: restricted.similar_books,
			warnings,
			confidence: restricted.confidence,
		})
	} else {
		None
	};

	let mut proposal = build_proposal(Inputs {
		kind: item.kind,
		index: &index,
		file_meta,
		online: online_records,
		formats,
		series_hint,
		group_name,
		similar,
		library_type,
	});
	proposal.looked_up_online = lookup.found();
	proposal.warnings.extend(lookup.notes());
	proposal.online_lookups = lookup.sources;
	proposal.uploader_view = uploader_view.clone();
	if !uploader_can_file {
		// the description of a book is shown as Markdown, HTML included, on the screens of
		// whoever reads the library: what comes from somebody whose text isn't trusted (their
		// file, an online source) is plain text from here on
		proposal.metadata.description = proposal
			.metadata
			.description
			.take()
			.and_then(|d| rules::untrusted_description(&d));
	}

	// duplicates: the same file, or the same author and title
	for file in &files {
		if let Some(hash) = &file.hash {
			let same = media::Entity::find()
				.filter(media::Column::Hash.eq(hash.clone()))
				.filter(media::Column::DeletedAt.is_null())
				.one(conn)
				.await?;
			if let Some(m) = same {
				proposal.duplicates.push(IntakeDuplicate {
					media_id: m.id,
					path: m.path,
					title: m.name,
					reason: "SAME_FILE".to_string(),
					file_name: file.original_name.clone(),
				});
			}
		}
	}
	if item.kind == IntakeKind::Book {
		if let Some(title) = &proposal.metadata.title {
			for book in index.same_title(title, &proposal.metadata.authors) {
				let b = &index.books[book as usize];
				if proposal.duplicates.iter().any(|d| d.media_id == b.media_id) {
					continue;
				}
				proposal.duplicates.push(IntakeDuplicate {
					media_id: b.media_id.clone(),
					path: String::new(),
					title: b.title.clone(),
					reason: "SAME_TITLE".to_string(),
					file_name: files
						.first()
						.map(|f| f.original_name.clone())
						.unwrap_or_default(),
				});
			}
		}
	}
	if !proposal.duplicates.is_empty() {
		proposal
			.warnings
			.push("This book may already be in the library".to_string());
	}

	// the proposal the default decision and the state come from: an uploader who may not file
	// gets the choices (the kind of series, the tags) and the confidence of their own view
	let decision_source = match &uploader_view {
		Some(view) => IntakeProposal {
			series_kind: view.series_kind,
			genre_suggestions: view.genre_suggestions.clone(),
			confidence: view.confidence,
			..proposal.clone()
		},
		None => proposal.clone(),
	};
	let mut decision = default_decision(item.kind, &files, &decision_source);
	if uploader_view.is_some() {
		// the folder is worked out from the whole library, as for every change an uploader
		// makes: they never see it
		decision.folder = derived_folder(conn, &library, &settings, &decision).await?;
		decision.file_names = default_file_names(&files, &decision);
	}
	let destination = compute_destination(&root, &settings, &files, &decision);
	// an item in the approval queue stays there; for somebody who may not file, the state is
	// what they can see and fix, not what only the library knows (that the book may be in it)
	let state = if item.submitted_at.is_some() {
		IntakeState::PendingApproval
	} else if uploader_can_file {
		state_for(&proposal, &decision, &destination)
	} else {
		state_for_uploader(&decision_source, &decision, &destination)
	};

	// the item may have been moved to another library, discarded or filed while this ran, or
	// answered by another analysis: then this outcome is dropped, not stored over it
	store_outcome(
		conn,
		item,
		intake_item::ActiveModel {
			files: Set(intake_files(files)),
			proposal: Set(Some(proposal)),
			decision: Set(Some(decision)),
			destination: Set(Some(destination)),
			state: Set(state),
			error: Set(None),
			..Default::default()
		},
	)
	.await?;
	Ok(())
}

fn intake_files(files: Vec<IntakeFile>) -> models::shared::intake::IntakeFiles {
	models::shared::intake::IntakeFiles(files)
}

/// Stores what an analysis found in the item it started with, in one statement: the item must
/// still be in the library the analysis read and still be waiting for an analysis. Checking first
/// and writing after would leave a gap for the item to be moved, discarded or filed, and a failed
/// check would be read as a move. `Ok(false)`: the item moved, was discarded or filed, or another
/// analysis answered first, so the outcome is dropped. A database error is an error.
async fn store_outcome(
	conn: &DatabaseConnection,
	item: &intake_item::Model,
	mut outcome: intake_item::ActiveModel,
) -> CoreResult<bool> {
	// an update of many rows doesn't run the model's `before_save`
	outcome.updated_at = Set(chrono::Utc::now().into());
	let stored = intake_item::Entity::update_many()
		.set(outcome)
		.filter(intake_item::Column::Id.eq(item.id.clone()))
		.filter(intake_item::Column::LibraryId.eq(item.library_id.clone()))
		.filter(intake_item::Column::State.eq(IntakeState::Analysing))
		.exec(conn)
		.await?;
	Ok(stored.rows_affected > 0)
}

/// The proposal's best choices
pub fn default_decision(
	kind: IntakeKind,
	files: &[IntakeFile],
	proposal: &IntakeProposal,
) -> IntakeDecision {
	let mut metadata = proposal.metadata.clone();
	metadata.genres = proposal
		.genre_suggestions
		.iter()
		.filter(|s| s.selected)
		.map(|s| s.value.clone())
		.collect();
	let folder = match proposal.series_kind {
		Some(_) => proposal
			.series_folders
			.first()
			.map(|c| c.folder.clone())
			.unwrap_or_default(),
		None => proposal
			.author_folders
			.first()
			.map(|c| c.folder.clone())
			.unwrap_or_default(),
	};
	let volumes: Vec<Option<f64>> =
		if kind == IntakeKind::Series || proposal.series_kind.is_some() {
			files
				.iter()
				.map(|f| f.volume.or(metadata.series_index))
				.collect()
		} else {
			files.iter().map(|_| None).collect()
		};
	let mut decision = IntakeDecision {
		metadata,
		series_kind: proposal.series_kind,
		folder,
		file_names: vec![],
		volumes,
	};
	decision.file_names = default_file_names(files, &decision);
	decision
}

/// The library-style file names for a decision
pub fn default_file_names(
	files: &[IntakeFile],
	decision: &IntakeDecision,
) -> Vec<String> {
	files
		.iter()
		.enumerate()
		.map(|(i, file)| match decision.series_kind {
			Some(_) => series_volume_file_name(
				decision
					.metadata
					.series
					.as_deref()
					.unwrap_or(&decision.folder),
				decision.volumes.get(i).copied().flatten(),
				&file.format,
			),
			None => book_file_name(
				file.title
					.as_deref()
					.filter(|_| files.len() > 1)
					.or(decision.metadata.title.as_deref())
					.unwrap_or("Untitled"),
				&file.format,
			),
		})
		.collect()
}

/// A folder inside `dir`, relative to the library root. An empty `dir` (a manga or light novel
/// library keeps its series folders at its root) gives the folder itself: joining the two with a
/// `/` would make an absolute path, which leaves the library when it is joined to the root.
fn join_relative(dir: &str, name: &str) -> String {
	let dir = dir.trim().trim_matches('/');
	if dir.is_empty() {
		name.to_string()
	} else {
		format!("{dir}/{name}")
	}
}

/// Whether a path relative to the library root stays inside it: a plain, relative path made of
/// ordinary components (no `..`, no leading `/`, no drive)
pub fn stays_in_library(relative: &str) -> bool {
	let path = Path::new(relative);
	!relative.is_empty()
		&& path
			.components()
			.all(|component| matches!(component, Component::Normal(_)))
}

/// Where a decision puts each file, relative to the library root
pub fn compute_destination(
	root: &Path,
	settings: &library_intake_settings::Model,
	files: &[IntakeFile],
	decision: &IntakeDecision,
) -> IntakeDestination {
	let relative_dir = match decision.series_kind {
		Some(IntakeSeriesKind::Manga) => {
			join_relative(&settings.manga_dir, &decision.folder)
		},
		Some(IntakeSeriesKind::LightNovel) => {
			join_relative(&settings.light_novel_dir, &decision.folder)
		},
		None => decision.folder.clone(),
	};
	let dir = root.join(&relative_dir);
	// the folder and the file names come from the uploader: only plain ones are looked up on the
	// disk, so no name can ask whether some other path exists (the problems say what is wrong
	// with the others, and they block the filing)
	let inside = is_plain_component(&decision.folder) && stays_in_library(&relative_dir);
	IntakeDestination {
		dir_exists: inside && dir.is_dir(),
		files: files
			.iter()
			.zip(decision.file_names.iter())
			.map(|(file, name)| IntakeDestinationFile {
				original_name: file.original_name.clone(),
				relative_path: join_relative(&relative_dir, name),
				exists: inside && is_plain_component(name) && name_taken(&dir.join(name)),
			})
			.collect(),
		relative_dir,
	}
}

/// Ready when the confident proposal leaves nothing to decide; else needs review
pub fn state_for(
	proposal: &IntakeProposal,
	decision: &IntakeDecision,
	destination: &IntakeDestination,
) -> IntakeState {
	if decision_problems(decision, destination).is_empty()
		&& proposal.confidence >= READY_CONFIDENCE
		&& proposal.duplicates.is_empty()
	{
		IntakeState::Ready
	} else {
		IntakeState::NeedsReview
	}
}

/// The same for somebody who may not file books themselves: ready when what they can fix is
/// fixed. A possible duplicate or a name that exists is the approver's to see, and showing it
/// through the state would tell the uploader what the library holds
pub fn state_for_uploader(
	proposal: &IntakeProposal,
	decision: &IntakeDecision,
	destination: &IntakeDestination,
) -> IntakeState {
	if scope::problems_for_uploader(decision_problems(decision, destination)).is_empty()
		&& proposal.confidence >= READY_CONFIDENCE
	{
		IntakeState::Ready
	} else {
		IntakeState::NeedsReview
	}
}

/// A file name must end in the extension of the format of the file it names: a book that is an
/// EPUB is not filed as `Title.pdf`
pub fn extension_problems(
	decision: &IntakeDecision,
	files: &[IntakeFile],
) -> Vec<String> {
	decision
		.file_names
		.iter()
		.zip(files.iter())
		.filter(|(name, file)| extension_of(name) != file.format.to_lowercase())
		.map(|(name, file)| {
			format!(
				"The file name '{name}' must end in .{}",
				file.format.to_lowercase()
			)
		})
		.collect()
}

/// What keeps a decision from being filed
pub fn decision_problems(
	decision: &IntakeDecision,
	destination: &IntakeDestination,
) -> Vec<String> {
	let mut problems = vec![];
	if !is_plain_component(&decision.folder) {
		problems.push("Choose a folder".to_string());
	} else if !stays_in_library(&destination.relative_dir) {
		// the folder is fine, so the library's own folder setting is what leaves the library
		problems.push("The folder is outside the library".to_string());
	}
	if decision.series_kind.is_none() {
		if decision
			.metadata
			.title
			.as_deref()
			.is_none_or(|t| t.trim().is_empty())
		{
			problems.push("Add a title".to_string());
		}
		if decision.metadata.authors.is_empty() {
			problems.push("Add an author".to_string());
		}
	} else if decision
		.metadata
		.series
		.as_deref()
		.is_none_or(|s| s.trim().is_empty())
	{
		problems.push("Add the series name".to_string());
	}
	for name in &decision.file_names {
		if !is_plain_component(name) {
			problems.push(format!("Invalid file name '{name}'"));
		}
	}
	let unique: BTreeSet<&String> = decision.file_names.iter().collect();
	if unique.len() != decision.file_names.len() {
		problems.push("Two files would get the same name".to_string());
	}
	if destination.files.iter().any(|f| f.exists) {
		problems.push(scope::EXISTS_PROBLEM.to_string());
	}
	problems
}

fn set_text(
	meta: &mut IntakeMetadata,
	field: &str,
	value: Option<String>,
) -> Option<String> {
	let value = value
		.map(|v| v.trim().to_string())
		.filter(|v| !v.is_empty());
	meta.set_source(field, IntakeSource::User);
	value
}

/// Whether the uploader can still change an item (review it, move it to another library): it
/// is waiting for them. An allow-list on purpose: a state added later is refused here until
/// someone decides that it belongs.
pub fn is_open_for_changes(state: IntakeState) -> bool {
	matches!(
		state,
		IntakeState::NeedsReview
			| IntakeState::Ready
			| IntakeState::Failed
			| IntakeState::Rejected
	)
}

/// Who is changing an item, and what they may do to it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reviewer {
	/// An approver is trusted: no limits on what they write, and they choose the folder
	pub approver: bool,
	/// The item is theirs. Otherwise it is somebody else's in the approval queue, which is the
	/// only thing an approver can change that isn't theirs
	pub owns_item: bool,
}

impl Reviewer {
	/// Whether an item in this state is open to this person for changes
	pub fn may_change(&self, state: IntakeState) -> bool {
		if self.owns_item {
			is_open_for_changes(state)
		} else {
			self.approver && state == IntakeState::PendingApproval
		}
	}
}

/// Whether an item can be analysed again: the ones open for changes, and one still marked as
/// being analysed (an analysis that never answered, across a restart, would leave it there
/// for good)
pub fn can_reanalyse(state: IntakeState, reviewer: Reviewer) -> bool {
	reviewer.may_change(state) || (reviewer.owns_item && state == IntakeState::Analysing)
}

/// Why an item in this state can't be changed
pub fn closed_for_changes(state: IntakeState, what: &str) -> CoreError {
	CoreError::BadRequest(match state {
		IntakeState::Filed | IntakeState::Discarded => {
			"This item is no longer in the inbox".to_string()
		},
		IntakeState::PendingApproval => {
			format!("This item is waiting for approval: take it back to have it {what}")
		},
		IntakeState::Filing => "This item is being filed".to_string(),
		state => format!("An item that is {state} can't be {what}"),
	})
}

/// A line of text from somebody who may not file books themselves: no control characters, one
/// space between words
fn clean_line(text: &str) -> String {
	text.chars()
		.filter(|c| !c.is_control() || c.is_whitespace())
		.collect::<String>()
		.split_whitespace()
		.collect::<Vec<_>>()
		.join(" ")
}

/// The folder an item goes to, for somebody who doesn't choose it: the best candidate the library
/// has for the first author (or the series), or the new folder their names make
async fn derived_folder(
	conn: &DatabaseConnection,
	library: &library::Model,
	settings: &library_intake_settings::Model,
	decision: &IntakeDecision,
) -> CoreResult<String> {
	let index = library_index(
		conn,
		&library.id,
		Path::new(&library.path),
		&settings.manga_dir,
		&settings.light_novel_dir,
	)
	.await?;
	Ok(match decision.series_kind {
		None => suggest::author_folder_candidates(&index, &decision.metadata.authors)
			.into_iter()
			.next()
			.map(|c| c.folder),
		Some(kind) => decision.metadata.series.as_deref().and_then(|series| {
			suggest::series_folder_candidates(&index, series, kind)
				.into_iter()
				.next()
				.map(|c| c.folder)
		}),
	}
	.unwrap_or_default())
}

/// Applies a review to an item and recomputes its destination and state.
///
/// An approver's review is taken as it comes. Somebody else's is held to the rules of
/// [`rules`]: texts have a length and no markup, the age rating can't go down, and the folder and
/// the file names are not theirs to choose: they are worked out again from the authors or the
/// series. An item waiting for approval stays waiting; one that goes back to its uploader is
/// ready, or needs review, by what they can still fix.
pub async fn apply_review(
	ctx: &Ctx,
	item: intake_item::Model,
	input: IntakeReviewInput,
	reviewer: Reviewer,
) -> CoreResult<intake_item::Model> {
	let conn = ctx.conn.as_ref();
	if !reviewer.may_change(item.state) {
		return Err(CoreError::BadRequest(format!(
			"An item that is {} can't be changed",
			item.state
		)));
	}
	let mut input = input;
	if !reviewer.approver {
		rules::check_uploader_review(&input).map_err(CoreError::BadRequest)?;
		input.title = input.title.map(|t| clean_line(&t));
		input.series = input.series.map(|t| clean_line(&t));
		input.publisher = input.publisher.map(|t| clean_line(&t));
		input.authors = input
			.authors
			.map(|a| a.iter().map(|a| clean_line(a)).collect());
		input.genres = input
			.genres
			.map(|g| g.iter().map(|g| clean_line(g)).collect());
		// the descriptions of a book are shown, as Markdown, on the screens of people who trust
		// what is in them
		input.description = input.description.map(|d| rules::plain_text(&d));
	}
	let (library, config) = load_library(conn, &item.library_id).await?;
	let library_type = library_type_in(config.as_ref());
	let settings = settings_of(conn, &library.id, library_type).await?;
	let files = item.files.0.clone();
	let mut decision = item.decision.clone().unwrap_or_default();
	let old_names = default_file_names(&files, &decision);
	let names_were_default = decision.file_names == old_names;
	let meta = &mut decision.metadata;

	if input.title.is_some() {
		meta.title = set_text(meta, "title", input.title);
	}
	if let Some(authors) = input.authors {
		meta.authors = authors
			.into_iter()
			.map(|a| a.trim().to_string())
			.filter(|a| !a.is_empty())
			.collect();
		meta.set_source("authors", IntakeSource::User);
	}
	if input.series.is_some() {
		meta.series = set_text(meta, "series", input.series);
	}
	if let Some(index) = input.series_index {
		meta.series_index = Some(index);
		meta.set_source("seriesIndex", IntakeSource::User);
	}
	if let Some(language) = input.language {
		meta.language = naming::language_code(&language);
		meta.set_source("language", IntakeSource::User);
	}
	if let Some(year) = input.year {
		meta.year = Some(year);
		meta.set_source("year", IntakeSource::User);
	}
	if input.publisher.is_some() {
		meta.publisher = set_text(meta, "publisher", input.publisher);
	}
	if input.description.is_some() {
		meta.description = set_text(meta, "description", input.description);
	}
	if let Some(genres) = input.genres {
		let mut seen = BTreeSet::new();
		meta.genres = genres
			.into_iter()
			.map(|g| g.trim().to_string())
			.filter(|g| !g.is_empty() && seen.insert(g.clone()))
			.collect();
	}
	if let Some(age) = input.age_rating {
		if !reviewer.approver {
			rules::check_age_rating(meta.age_rating, age)
				.map_err(CoreError::BadRequest)?;
		}
		meta.age_rating = Some(age);
	}
	if input.clear_series_kind {
		decision.series_kind = None;
	} else if let Some(kind) = input.series_kind {
		decision.series_kind = Some(kind);
	}
	// a manga or light novel library only holds series of its own kind
	if let Some(kind) = typed_series_kind(library_type) {
		decision.series_kind = Some(kind);
	}
	if let Some(volumes) = input.volumes {
		decision.volumes = volumes;
	}
	decision.volumes.resize(files.len(), None);
	if reviewer.approver {
		match input.folder {
			Some(folder) => {
				let folder = folder.trim().to_string();
				decision.folder = if decision.series_kind.is_some() {
					series_folder_name(&folder)
				} else if is_plain_component(&folder) {
					folder
				} else {
					fold_name(&folder)
				};
			},
			None => {
				// switching between author and series filing picks the matching default folder
				let proposal = item.proposal.clone().unwrap_or_default();
				let current_is_series_folder = proposal
					.series_folders
					.iter()
					.any(|c| c.folder == decision.folder);
				if decision.series_kind.is_some() && !current_is_series_folder {
					decision.folder = decision
						.metadata
						.series
						.as_deref()
						.map(series_folder_name)
						.unwrap_or_default();
				} else if decision.series_kind.is_none() && current_is_series_folder {
					decision.folder = decision
						.metadata
						.authors
						.first()
						.map(|a| fold_name(a))
						.unwrap_or_default();
				}
			},
		}
		match input.file_names {
			Some(names) => {
				decision.file_names =
					names.into_iter().map(|n| n.trim().to_string()).collect()
			},
			None if names_were_default => {
				decision.file_names = default_file_names(&files, &decision)
			},
			None => {},
		}
		decision.file_names.resize(files.len(), String::new());
	} else {
		// not theirs to choose: worked out again from the authors or the series
		decision.folder = derived_folder(conn, &library, &settings, &decision).await?;
		decision.file_names = default_file_names(&files, &decision);
	}
	let destination =
		compute_destination(Path::new(&library.path), &settings, &files, &decision);
	let mut problems = decision_problems(&decision, &destination);
	problems.extend(extension_problems(&decision, &files));
	if !reviewer.approver {
		problems = scope::problems_for_uploader(problems);
	}
	let state = if item.state == IntakeState::PendingApproval {
		IntakeState::PendingApproval
	} else if problems.is_empty() {
		IntakeState::Ready
	} else {
		IntakeState::NeedsReview
	};
	let change = intake_item::ActiveModel {
		decision: Set(Some(decision)),
		destination: Set(Some(destination)),
		state: Set(state),
		..Default::default()
	};
	change_if_unchanged(conn, &item, change).await
}

/// Writes `change` into an item in one statement, and only while the item is still in the state it
/// was read in: a review, a move or an analysis that was read while the item waited must not
/// write its state over an approval that claimed it since (the item would be waiting again while
/// it is filed, and approved and filed a second time). Nothing is written otherwise.
pub async fn change_if_unchanged(
	conn: &DatabaseConnection,
	item: &intake_item::Model,
	mut change: intake_item::ActiveModel,
) -> CoreResult<intake_item::Model> {
	// an update of many rows doesn't run the model's `before_save`
	change.updated_at = Set(chrono::Utc::now().into());
	let done = intake_item::Entity::update_many()
		.set(change)
		.filter(intake_item::Column::Id.eq(item.id.clone()))
		.filter(intake_item::Column::State.eq(item.state))
		.exec(conn)
		.await?;
	if done.rows_affected == 0 {
		return Err(CoreError::BadRequest(
			"This item changed while it was being saved: look at it again".to_string(),
		));
	}
	intake_item::Entity::find_by_id(item.id.clone())
		.one(conn)
		.await?
		.ok_or_else(|| CoreError::NotFound("Inbox item not found".to_string()))
}

/// Writes down which files of an item are in the library, so that filing it again carries on
/// after them
async fn record_progress(
	conn: &DatabaseConnection,
	id: &str,
	paths: &[String],
	notes: &[String],
) -> CoreResult<()> {
	intake_item::ActiveModel {
		id: sea_orm::ActiveValue::Unchanged(id.to_string()),
		result: Set(Some(IntakeResult {
			paths: paths.to_vec(),
			notes: notes.to_vec(),
		})),
		..Default::default()
	}
	.update(conn)
	.await?;
	Ok(())
}

/// Whether two texts are the same but for their white space
fn same_text(a: &str, b: &str) -> bool {
	a.split_whitespace().eq(b.split_whitespace())
}

/// A book from somebody whose text isn't trusted is filed only when what the scan will read as
/// its description is a plain one: the description is shown as Markdown, HTML included, on the
/// screens of whoever reads the library, and a file can say it in more places than the one that
/// is written (a `summary` or a `meta` in an OPF, a `ComicInfo.xml` in a folder). So the staged
/// file is read the way the scan reads it, whatever the writing did; one that can't be read is
/// not filed either.
///
/// A format that is written (`rewritten`) carries the plain description it was given, or none:
/// anything else is the file's own that was not replaced. A format that can't be written (a RAR)
/// keeps its own, which then only has to be plain text already.
pub(crate) async fn only_the_plain_description(
	config: &crate::config::StumpConfig,
	staged: &Path,
	original_name: &str,
	plain: Option<&str>,
	rewritten: bool,
) -> CoreResult<()> {
	let options = MediaProcessorOptions {
		process_metadata: true,
		organization_mode: OrganizationMode::Metadata,
		..Default::default()
	};
	let found = match process_file(staged, options, config).await {
		Ok(processed) => processed
			.metadata
			.and_then(|metadata| metadata.summary)
			.filter(|summary| !summary.trim().is_empty()),
		Err(error) => {
			tracing::warn!(
				?error,
				original_name,
				"Couldn't read a staged file's description"
			);
			return Err(CoreError::BadRequest(format!(
				"'{original_name}': its description could not be checked"
			)));
		},
	};
	let Some(found) = found else {
		return Ok(());
	};
	let is_the_plain_one = plain.is_some_and(|plain| same_text(&found, plain));
	let is_plain_text =
		rules::untrusted_description(&found).is_some_and(|text| same_text(&text, &found));
	if is_the_plain_one || (!rewritten && is_plain_text) {
		return Ok(());
	}
	Err(CoreError::BadRequest(format!(
		"'{original_name}': it carries a description of its own that could not be replaced with plain text"
	)))
}

/// Whether `placed` is the copy that `staged` made when it was put in the library: a file there
/// with the length of the staged file
fn is_copy_of(placed: &Path, staged: &Path) -> bool {
	match (std::fs::metadata(placed), std::fs::metadata(staged)) {
		(Ok(placed), Ok(staged)) => placed.is_file() && placed.len() == staged.len(),
		_ => false,
	}
}

/// Copies `source` to `target` without ever replacing an existing file: the copy goes to
/// a temporary name in the target folder, then is hard-linked into place
fn place_file(source: &Path, target: &Path) -> std::io::Result<()> {
	let dir = target
		.parent()
		.ok_or_else(|| std::io::Error::other("no parent folder"))?;
	// a name of its own for every filing: two filings that file the same name (two editions of a
	// title, one file twice, two approvers) must not copy into the same temporary file, or the
	// book that is linked could hold the bytes of the other. Only the start of the book's name is
	// in it (to know what a left-over is): a name near the limit of the file system would not fit
	// with the rest.
	let start: String = target
		.file_name()
		.map(|n| n.to_string_lossy().chars().take(40).collect())
		.unwrap_or_default();
	let tmp = dir.join(format!(".{start}.{}.intake", Uuid::new_v4().simple()));
	if let Err(error) = std::fs::copy(source, &tmp) {
		let _ = std::fs::remove_file(&tmp);
		return Err(error);
	}
	if let Ok(meta) = std::fs::metadata(dir) {
		// the files of a folder get its permissions without the execute bits
		use std::os::unix::fs::PermissionsExt;
		let mode = meta.permissions().mode() & 0o666;
		let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode));
	}
	// the scan this filing enqueues reads the new file: the library watcher needn't
	own_writes::expect(target);
	let linked = std::fs::hard_link(&tmp, target);
	match &linked {
		Ok(()) => own_writes::wrote(target),
		Err(_) => own_writes::forget(target),
	}
	let _ = std::fs::remove_file(&tmp);
	linked
}

/// Creates the folder a filing goes to, telling the library watcher the new folders are
/// Stump's own (the filing's scan reads them)
fn create_folders(root: &Path, dir: &Path) -> std::io::Result<()> {
	let mut new: Vec<PathBuf> = vec![];
	let mut current = dir.to_path_buf();
	while current.starts_with(root) && current != root && !current.exists() {
		new.push(current.clone());
		match current.parent() {
			Some(parent) => current = parent.to_path_buf(),
			None => break,
		}
	}
	for folder in &new {
		own_writes::expect(folder);
	}
	let created = std::fs::create_dir_all(dir);
	for folder in &new {
		own_writes::wrote(folder);
	}
	created
}

/// Whether anything is at `path`: a file, a folder or a link, even one that points nowhere.
/// `Path::exists` follows links, so a link to nothing reads as free, and the link into place then
/// fails on it for ever. Any answer but "not found" counts as taken.
fn name_taken(path: &Path) -> bool {
	!matches!(
		std::fs::symlink_metadata(path),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound
	)
}

/// How many names (`_2`, `_3`, ...) a filing looks at before it gives up: a folder that holds
/// this many of one name is not one a book should go to
const MAX_NAMES_TRIED: usize = 1_000;
/// How many times a filing copies the book to a name that was taken in the moment between the
/// look and the link (another filing, a link that was not seen): each try copies the whole book
const MAX_PLACE_TRIES: usize = 10;

/// The error of a filing that found no name to put a book under
fn no_free_name(name: &str) -> std::io::Error {
	std::io::Error::new(
		std::io::ErrorKind::AlreadyExists,
		format!("no free name was found for {name} in the folder"),
	)
}

/// A name with `_2`, `_3`, ... before the extension, for a clash at filing time
fn suffixed(name: &str, n: usize) -> String {
	match name.rsplit_once('.') {
		Some((stem, ext)) => format!("{stem}_{n}.{ext}"),
		None => format!("{name}_{n}"),
	}
}

/// Moves an item to another library and sets it to be analysed there from scratch. Its staged
/// files stay where they are: staging doesn't depend on the library. The caller checks that the
/// user may use the library, then runs the analysis.
pub async fn change_library(
	ctx: &Ctx,
	item: intake_item::Model,
	library_id: &str,
	reviewer: Reviewer,
) -> CoreResult<intake_item::Model> {
	if !reviewer.may_change(item.state) {
		return Err(closed_for_changes(item.state, "moved to another library"));
	}
	let conn = ctx.conn.as_ref();
	let (library, _) = load_library(conn, library_id).await?;
	let change = intake_item::ActiveModel {
		library_id: Set(library.id),
		state: Set(IntakeState::Analysing),
		error: Set(None),
		..Default::default()
	};
	change_if_unchanged(conn, &item, change).await
}

/// Files an item that was claimed for it (its state is `FILING`, see [`approval`]). If filing
/// fails the item goes back to `back` with the reason, and what was already placed in the
/// library stays recorded, so trying again continues where it stopped and copies nothing twice.
pub async fn file_claimed(
	ctx: &Ctx,
	item: intake_item::Model,
	back: IntakeState,
) -> CoreResult<intake_item::Model> {
	match file_claimed_inner(ctx, &item).await {
		Ok(filed) => Ok(filed),
		Err(error) => {
			tracing::warn!(?error, item_id = item.id, "Filing an intake item failed");
			let restore = intake_item::ActiveModel {
				state: Set(back),
				error: Set(Some(error.to_string())),
				updated_at: Set(chrono::Utc::now().into()),
				..Default::default()
			};
			if let Err(write_error) = intake_item::Entity::update_many()
				.set(restore)
				.filter(intake_item::Column::Id.eq(item.id.clone()))
				.filter(intake_item::Column::State.eq(IntakeState::Filing))
				.exec(ctx.conn.as_ref())
				.await
			{
				tracing::error!(
					?write_error,
					item_id = item.id,
					"Couldn't put a book back after filing it failed"
				);
			}
			Err(error)
		},
	}
}

async fn file_claimed_inner(
	ctx: &Ctx,
	item: &intake_item::Model,
) -> CoreResult<intake_item::Model> {
	let conn = ctx.conn.as_ref();
	if item.state != IntakeState::Filing {
		return Err(CoreError::BadRequest(format!(
			"'{}' is not ready to be filed",
			item.name
		)));
	}
	let decision = item
		.decision
		.clone()
		.ok_or_else(|| CoreError::BadRequest("The item has no decision".into()))?;
	let (library, config) = load_library(conn, &item.library_id).await?;
	let settings =
		settings_of(conn, &library.id, library_type_in(config.as_ref())).await?;
	let root = PathBuf::from(&library.path);
	let files = item.files.0.clone();
	let destination = compute_destination(&root, &settings, &files, &decision);
	let mut problems = decision_problems(&decision, &destination);
	problems.extend(extension_problems(&decision, &files));
	// a name taken since the review gets a suffix instead of failing: it is no reason, and an
	// error that says the library holds a file is not for whoever reads it
	problems.retain(|p| p != scope::EXISTS_PROBLEM);
	if !problems.is_empty() {
		return Err(CoreError::BadRequest(problems.join("; ")));
	}
	// a copy of the file that is in the library since the book was looked at: told to whoever
	// approves, who decides (approving once more files it as well)
	let fresh = unseen_duplicates(conn, item).await?;
	if !fresh.is_empty() {
		let mut proposal = item.proposal.clone().unwrap_or_default();
		proposal.duplicates.extend(fresh);
		if !proposal
			.warnings
			.iter()
			.any(|w| w == scope::DUPLICATE_WARNING)
		{
			proposal.warnings.push(scope::DUPLICATE_WARNING.to_string());
		}
		let seen = intake_item::ActiveModel {
			proposal: Set(Some(proposal)),
			updated_at: Set(chrono::Utc::now().into()),
			..Default::default()
		};
		intake_item::Entity::update_many()
			.set(seen)
			.filter(intake_item::Column::Id.eq(item.id.clone()))
			.filter(intake_item::Column::State.eq(IntakeState::Filing))
			.exec(conn)
			.await?;
		return Err(CoreError::BadRequest(scope::ADDED_SINCE.to_string()));
	}

	// a book from somebody who needs approval is filed with a plain description of its own: the
	// file's (an EPUB's long one is kept otherwise) could carry markup that the library renders
	let untrusted = !approval::uploader_can_file_directly(conn, &item.user_id).await?;

	let staging = PathBuf::from(&item.staging_dir);
	let dir = root.join(&destination.relative_dir);
	{
		let (root, dir) = (root.clone(), dir.clone());
		tokio::task::spawn_blocking(move || create_folders(&root, &dir))
			.await
			.map_err(|e| CoreError::InternalError(e.to_string()))??;
	}

	// what an earlier try placed: those files are in the library, in the order of the item's files
	let mut paths: Vec<String> = item
		.result
		.as_ref()
		.map(|r| r.paths.clone())
		.unwrap_or_default();
	// A path counts as placed only while the file there is the copy its staged file made (same
	// length: the staged file is rewritten once before it is copied and not touched after). A
	// path recorded just before the server stopped names a file that was never put there, and a
	// file somebody else has put there since is not ours.
	let placed = paths
		.iter()
		.enumerate()
		.take_while(|(i, path)| {
			files.get(*i).is_some_and(|file| {
				is_copy_of(Path::new(path.as_str()), &staging.join(&file.staged_name))
			})
		})
		.count();
	paths.truncate(placed);
	let mut notes = item
		.result
		.as_ref()
		.map(|r| r.notes.clone())
		.unwrap_or_default();
	// What is written into the files that are not placed yet comes first, and for the book of
	// somebody who needs approval it is checked before any file of it is placed: a book is
	// filed whole or not at all, never a few volumes of it and then a refusal.
	let placed_before = paths.len();
	for (i, file) in files.iter().enumerate().skip(placed_before) {
		let mut meta = FileMetadata::from_metadata(
			&decision.metadata,
			decision.series_kind == Some(IntakeSeriesKind::Manga),
		);
		if untrusted {
			meta.description = decision
				.metadata
				.description
				.as_deref()
				.and_then(rules::untrusted_description);
			meta.replace_description = true;
		}
		if decision.series_kind.is_some() {
			meta.series_index = decision.volumes.get(i).copied().flatten();
			// volumes keep their own title when they have one
			meta.title = volume_title(
				file.title.as_deref(),
				decision.metadata.series.as_deref(),
				meta.series_index,
			)
			.or_else(|| decision.metadata.title.clone());
		}
		let staged = staging.join(&file.staged_name);
		let format = file.format.clone();
		let staged_clone = staged.clone();
		let meta_clone = meta.clone();
		let written = tokio::task::spawn_blocking(move || match format.as_str() {
			"epub" => write_epub(&staged_clone, &meta_clone).map(|_| None),
			"cbz" => write_cbz(&staged_clone, &meta_clone).map(|_| None),
			"pdf" => write_pdf(&staged_clone, &meta_clone, &KeptPdfInfo::default())
				.map(|_| None),
			other => Ok(Some(format!(
				"{other} files keep their metadata as uploaded"
			))),
		})
		.await
		.map_err(|e| CoreError::InternalError(e.to_string()))?;
		match written {
			Ok(Some(note)) => notes.push(note),
			Ok(None) => {},
			Err(error) => {
				// the book still goes in, with the metadata it came with (a book of somebody
				// who needs approval, only if what is read from it is a plain description:
				// below)
				notes.push(format!(
					"{}: metadata not written ({error})",
					file.original_name
				));
			},
		}
		if untrusted {
			only_the_plain_description(
				&ctx.config,
				&staged,
				&file.original_name,
				meta.description.as_deref(),
				matches!(file.format.as_str(), "epub" | "cbz" | "pdf"),
			)
			.await?;
		}
	}

	for (i, file) in files.iter().enumerate().skip(placed_before) {
		let staged = staging.join(&file.staged_name);
		let name = decision.file_names.get(i).cloned().unwrap_or_default();
		let mut target = dir.join(&name);
		let mut n = 2;
		let mut tries = 0;
		loop {
			while name_taken(&target) {
				if n > MAX_NAMES_TRIED {
					return Err(no_free_name(&name).into());
				}
				target = dir.join(suffixed(&name, n));
				n += 1;
			}
			// Written down BEFORE the file is put there. If the server stops between the two,
			// trying again finds a recorded path with no file and places it again under the same
			// name; written down afterwards, a file placed just before the stop would be unknown
			// to the next try, which would copy it a second time as `_2`.
			paths.push(target.to_string_lossy().into_owned());
			record_progress(conn, &item.id, &paths, &notes).await?;
			let (from, to) = (staged.clone(), target.clone());
			let put = tokio::task::spawn_blocking(move || place_file(&from, &to))
				.await
				.map_err(|e| CoreError::InternalError(e.to_string()))?;
			match put {
				Ok(()) => break,
				// another filing took the name since it was looked at (nothing was replaced), or
				// something is there that the look did not see: the next name, by number, so a
				// name that keeps failing is never tried again
				Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
					paths.pop();
					tries += 1;
					if tries >= MAX_PLACE_TRIES || n > MAX_NAMES_TRIED {
						let _ = record_progress(conn, &item.id, &paths, &notes).await;
						return Err(no_free_name(&name).into());
					}
					target = dir.join(suffixed(&name, n));
					n += 1;
				},
				Err(error) => {
					// not placed: the record names only files that are there
					paths.pop();
					let _ = record_progress(conn, &item.id, &paths, &notes).await;
					return Err(error.into());
				},
			}
		}
	}

	let mut active = item.clone().into_active_model();
	active.state = Set(IntakeState::Filed);
	active.destination = Set(Some(destination));
	active.result = Set(Some(IntakeResult { paths, notes }));
	active.filed_at = Set(Some(chrono::Utc::now().into()));
	active.submitted_at = Set(None);
	active.error = Set(None);
	let updated = active.update(conn).await?;

	let _ = tokio::fs::remove_dir_all(&staging).await;
	library_index::invalidate_index(&library.id);
	scope::forget_vocabularies();
	enqueue_scan(ctx, &library, config.as_ref(), &dir).await;
	Ok(updated)
}

/// The copies of this item's files that are in the library now and weren't when it was looked at.
/// What the approver was shown (the duplicates of the proposal) is theirs to decide; what came
/// after is not: another copy of the file was filed (by another approval, or by hand), or is being
/// filed this moment. Of two items being filed together only the one claimed first goes on.
async fn unseen_duplicates(
	conn: &DatabaseConnection,
	item: &intake_item::Model,
) -> CoreResult<Vec<IntakeDuplicate>> {
	let hashed: Vec<(&str, &str)> = item
		.files
		.0
		.iter()
		.filter_map(|f| Some((f.hash.as_deref()?, f.original_name.as_str())))
		.collect();
	if hashed.is_empty() {
		return Ok(vec![]);
	}
	let seen: std::collections::HashSet<&str> = item
		.proposal
		.as_ref()
		.map(|p| {
			p.duplicates
				.iter()
				.filter(|d| d.reason == "SAME_FILE")
				.map(|d| d.media_id.as_str())
				.collect()
		})
		.unwrap_or_default();
	let mut fresh: Vec<IntakeDuplicate> = vec![];
	let mut note = |id: String, title: String, file_name: &str| {
		if !seen.contains(id.as_str()) && !fresh.iter().any(|d| d.media_id == id) {
			fresh.push(IntakeDuplicate {
				media_id: id,
				path: String::new(),
				title,
				reason: "SAME_FILE".to_string(),
				file_name: file_name.to_string(),
			});
		}
	};
	// books of the library (a book that was filed here has the hash of the file as it was
	// written, so only a copy added some other way is found like this)
	for book in media::Entity::find()
		.filter(
			media::Column::Hash.is_in(hashed.iter().map(|(hash, _)| String::from(*hash))),
		)
		.filter(media::Column::DeletedAt.is_null())
		.all(conn)
		.await?
	{
		let file_name = hashed
			.iter()
			.find(|(hash, _)| Some(*hash) == book.hash.as_deref())
			.map(|(_, name)| *name)
			.unwrap_or_default();
		note(book.id, book.name, file_name);
	}
	// items filed from the same file: their book may not be in the library yet (the scan after
	// filing makes it), and what they filed has other bytes than what was uploaded
	for (hash, file_name) in &hashed {
		let others = intake_item::Entity::find()
			.filter(intake_item::Column::Id.ne(item.id.clone()))
			.filter(
				intake_item::Column::State
					.is_in([IntakeState::Filing, IntakeState::Filed]),
			)
			.filter(sea_orm::sea_query::Expr::cust_with_values(
				"CAST(files AS TEXT) LIKE ?",
				[format!("%{hash}%")],
			))
			.all(conn)
			.await?;
		for other in others {
			let still_there = match other.state {
				// the one claimed first files; the other waits (its approver approves again)
				IntakeState::Filing => {
					(other.reviewed_at, &other.id) < (item.reviewed_at, &item.id)
				},
				// a filed book that was taken out of the library since is no copy
				_ => other
					.result
					.as_ref()
					.is_some_and(|r| r.paths.iter().any(|p| Path::new(p).exists())),
			};
			if still_there {
				note(
					format!("intake:{}", other.id),
					other.name.clone(),
					file_name,
				);
			}
		}
	}
	Ok(fresh)
}

/// A volume's own title, unless it is just the series name: then `<Series> vNN`
pub fn volume_title(
	title: Option<&str>,
	series: Option<&str>,
	volume: Option<f64>,
) -> Option<String> {
	let own = title.filter(|t| {
		series.is_none_or(|s| library_index::norm(t) != library_index::norm(s))
	});
	match (own, series) {
		(Some(title), _) => Some(title.to_string()),
		(None, Some(series)) => Some(match volume {
			Some(v) => format!("{series} {}", series_volume_label(v)),
			None => series.to_string(),
		}),
		(None, None) => None,
	}
}

fn series_volume_label(v: f64) -> String {
	if v.fract() == 0.0 {
		format!("v{:02}", v as i64)
	} else {
		format!("v{v}")
	}
}

/// Scans just the folder that received books when the library is organised by metadata
/// (a scan rooted there only creates what is new in it); else the whole library
pub async fn enqueue_scan(
	ctx: &Ctx,
	library: &library::Model,
	config: Option<&library_config::Model>,
	dir: &Path,
) {
	let metadata_mode =
		config.is_some_and(|c| c.organization_mode == OrganizationMode::Metadata);
	let path = if metadata_mode {
		dir.to_string_lossy().into_owned()
	} else {
		library.path.clone()
	};
	if let Err(error) = ctx
		.enqueue(StumpJob::library_scan(library.id.clone(), path, None))
		.await
	{
		tracing::error!(?error, "Failed to enqueue the scan after filing");
	}
}

/// Throws an item away with its staged files
pub async fn discard(ctx: &Ctx, item: intake_item::Model) -> CoreResult<()> {
	if item.state == IntakeState::Discarded {
		return Ok(());
	}
	if item.state == IntakeState::Filed {
		return Err(CoreError::BadRequest(
			"A filed item can't be discarded".into(),
		));
	}
	if item.state == IntakeState::Filing {
		return Err(CoreError::BadRequest(
			"This item is being filed: it can't be discarded".into(),
		));
	}
	// one statement says whether it is thrown away: an approver may have claimed it since it was
	// read, and its files must not be removed from under the filing
	let change = intake_item::ActiveModel {
		state: Set(IntakeState::Discarded),
		submitted_at: Set(None),
		updated_at: Set(chrono::Utc::now().into()),
		..Default::default()
	};
	let discarded = intake_item::Entity::update_many()
		.set(change)
		.filter(intake_item::Column::Id.eq(item.id.clone()))
		.filter(intake_item::Column::State.is_not_in([
			IntakeState::Filed,
			IntakeState::Filing,
			IntakeState::Discarded,
		]))
		.exec(ctx.conn.as_ref())
		.await?;
	if discarded.rows_affected == 0 {
		return Err(CoreError::BadRequest(
			"This item changed while it was being discarded: look at it again".into(),
		));
	}
	let _ = tokio::fs::remove_dir_all(&item.staging_dir).await;
	Ok(())
}

/// Filed items, newest first, for the "Recently added" page
pub async fn recently_filed(
	conn: &DatabaseConnection,
	user_id: Option<&str>,
	limit: u64,
	offset: u64,
) -> CoreResult<Vec<(intake_item::Model, Option<user::Model>)>> {
	let mut query = intake_item::Entity::find()
		.filter(intake_item::Column::State.eq(IntakeState::Filed))
		.order_by_desc(intake_item::Column::FiledAt);
	if let Some(user_id) = user_id {
		query = query.filter(intake_item::Column::UserId.eq(user_id));
	}
	let items =
		sea_orm::QuerySelect::offset(sea_orm::QuerySelect::limit(query, limit), offset)
			.all(conn)
			.await?;
	let mut out = vec![];
	for item in items {
		let user = user::Entity::find_by_id(item.user_id.clone())
			.one(conn)
			.await?;
		out.push((item, user));
	}
	Ok(out)
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A zip of comic pages with the given entries (name, text), the way an uploader might build one
	fn cbz(path: &Path, entries: &[(&str, &str)]) {
		use std::io::Write;
		let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
		for (name, text) in entries {
			zip.start_file(*name, zip::write::SimpleFileOptions::default())
				.unwrap();
			zip.write_all(text.as_bytes()).unwrap();
		}
		zip.finish().unwrap();
	}

	fn comic_info(summary: &str) -> String {
		format!(
			r#"<?xml version="1.0"?><ComicInfo><Summary>{summary}</Summary></ComicInfo>"#
		)
	}

	/// A book of somebody who needs approval is filed only with what the scan will read as its
	/// description being the plain one or none: from any entry that holds it, and for a format
	/// that was not written at all; a file that can't be read is not filed
	#[tokio::test]
	async fn a_book_is_filed_only_when_its_description_reads_plain() {
		let config = crate::config::StumpConfig::debug();
		let dir = tempfile::tempdir().unwrap();
		let check = |path: &Path, plain: Option<&str>| {
			let path = path.to_path_buf();
			let config = config.clone();
			let plain = plain.map(str::to_string);
			async move {
				only_the_plain_description(
					&config,
					&path,
					"book.cbz",
					plain.as_deref(),
					true,
				)
				.await
				.map_err(|error| error.to_string())
			}
		};
		let evil = comic_info("&lt;iframe srcdoc=EVIL&gt;");

		// nothing to read, or the plain description (spaces apart): filed
		let none = dir.path().join("none.cbz");
		cbz(&none, &[("01.png", "png")]);
		assert_eq!(check(&none, None).await, Ok(()));
		assert_eq!(check(&none, Some("Plain.")).await, Ok(()));
		let plain = dir.path().join("plain.cbz");
		cbz(
			&plain,
			&[
				("01.png", "png"),
				("ComicInfo.xml", &comic_info("A  plain\n description.")),
			],
		);
		assert_eq!(check(&plain, Some("A plain description.")).await, Ok(()));

		// a description that is not the plain one, in the entry the writer replaced or in a
		// folder, whether or not there is a plain one to compare with
		for (name, entries) in [
			("root", vec![("ComicInfo.xml", evil.as_str())]),
			("folder", vec![("sub/ComicInfo.xml", evil.as_str())]),
			(
				"both",
				vec![
					("sub/ComicInfo.xml", evil.as_str()),
					("ComicInfo.xml", evil.as_str()),
				],
			),
		] {
			let path = dir.path().join(format!("{name}.cbz"));
			cbz(&path, &entries);
			for plain in [None, Some("Plain.")] {
				let error = check(&path, plain).await.expect_err(name);
				assert!(error.contains("description of its own"), "{name}: {error}");
				assert!(error.contains("'book.cbz'"), "{name}: {error}");
			}
		}

		// a format that can't be written keeps what it has: filed when that is plain text
		// already, whatever the description it was given, and not when it holds markup
		let unwritten = |path: &Path, plain: Option<&str>| {
			let (path, config) = (path.to_path_buf(), config.clone());
			let plain = plain.map(str::to_string);
			async move {
				only_the_plain_description(
					&config,
					&path,
					"book.cbr",
					plain.as_deref(),
					false,
				)
				.await
				.map_err(|error| error.to_string())
			}
		};
		assert_eq!(unwritten(&plain, None).await, Ok(()));
		assert_eq!(unwritten(&plain, Some("Another.")).await, Ok(()));
		let folder = dir.path().join("folder.cbz");
		cbz(&folder, &[("sub/ComicInfo.xml", &evil)]);
		let error = unwritten(&folder, Some("Plain."))
			.await
			.expect_err("markup");
		assert!(error.contains("description of its own"), "{error}");
		let tagged = dir.path().join("tagged.cbz");
		cbz(
			&tagged,
			&[("ComicInfo.xml", &comic_info("&lt;b&gt;bold&lt;/b&gt;"))],
		);
		assert!(unwritten(&tagged, None).await.is_err());
		// a format that is written holds the plain one or none: its own plain text is no
		// excuse (it was meant to be replaced)
		assert!(check(&plain, None).await.is_err());

		// a file that can't be read is not one that was shown to be plain
		let broken = dir.path().join("broken.cbz");
		std::fs::write(&broken, "this is not a zip").unwrap();
		let error = check(&broken, None).await.expect_err("an unreadable file");
		assert!(error.contains("could not be checked"), "{error}");
		// and what is said says nothing of the server
		assert!(
			!error.contains(&dir.path().to_string_lossy().to_string()),
			"{error}"
		);
	}

	/// A RAR has no writer: what it says is read the way the scan reads it, a `ComicInfo.xml` in
	/// a folder included
	#[tokio::test]
	async fn a_rar_is_read_as_the_scan_reads_it() {
		let config = crate::config::StumpConfig::debug();
		let rar = ::tests::fixtures::get_test_complex_rar_path();
		let options = MediaProcessorOptions {
			process_metadata: true,
			organization_mode: OrganizationMode::Metadata,
			..Default::default()
		};
		let scanned = process_file(&rar, options, &config)
			.await
			.expect("the RAR reads")
			.metadata
			.and_then(|metadata| metadata.summary)
			.filter(|summary| !summary.trim().is_empty())
			.expect("the fixture's ComicInfo.xml, in a folder, has a summary");
		let path = Path::new(&rar);
		// it has no writer: its own description, in a folder, is what it keeps, and it is plain
		for plain in [None, Some(scanned.as_str()), Some("Another one.")] {
			assert!(
				only_the_plain_description(&config, path, "x.cbr", plain, false)
					.await
					.is_ok(),
				"{plain:?}"
			);
		}
		// were it a format that is written, its own would be one that was not replaced
		for plain in [None, Some("Another one.")] {
			let error = only_the_plain_description(&config, path, "x.cbr", plain, true)
				.await
				.expect_err("a description that is not the plain one");
			assert!(
				error.to_string().contains("description of its own"),
				"{error}"
			);
		}
		assert!(
			only_the_plain_description(&config, path, "x.cbr", Some(&scanned), true)
				.await
				.is_ok()
		);
	}

	/// Whether a book is looked up is decided on the library its reader can read: subjects that
	/// only books they can't read carry don't make a lookup unnecessary for them, so they can't
	/// tell from the lookup (shown to them as `lookedUpOnline` and its sources) whether any book
	/// holds a subject they guess
	#[test]
	fn a_lookup_is_decided_on_the_books_its_reader_can_read() {
		use library_index::LibraryIndex;

		let meta = IntakeMetadata {
			authors: vec!["Somebody".into()],
			subjects: vec!["Dark".into(), "Mystery".into(), "Not in any library".into()],
			..Default::default()
		};
		// the whole library knows two of the file's subjects; the uploader can read the books
		// of only one of them
		let whole = LibraryIndex::with_vocabulary(&["Dark", "Mystery"]);
		let readable = LibraryIndex::with_vocabulary(&["Mystery"]);
		let wanted = |index, viewer| {
			lookup_wanted(true, IntakeKind::Book, false, &meta, index, viewer)
		};
		// whoever approves reads the whole library: two subjects known, nothing to look up
		assert!(!wanted(&whole, None));
		// the uploader's outcome is as if the books they can't read weren't there
		assert!(wanted(&whole, Some(&readable)));
		assert_eq!(wanted(&whole, Some(&readable)), wanted(&readable, None));
		// whatever the whole library holds, two subjects they can read skip the lookup
		let both = LibraryIndex::with_vocabulary(&["Dark", "Mystery"]);
		assert!(!wanted(&LibraryIndex::with_vocabulary(&[]), Some(&both)));
		// without an author the lookup is made all the same
		let anonymous = IntakeMetadata {
			authors: vec![],
			..meta.clone()
		};
		assert!(lookup_wanted(
			true,
			IntakeKind::Book,
			false,
			&anonymous,
			&whole,
			None
		));
		// not asked for, not a book, a comic: never
		assert!(!lookup_wanted(
			false,
			IntakeKind::Book,
			false,
			&anonymous,
			&whole,
			None
		));
		assert!(!lookup_wanted(
			true,
			IntakeKind::Series,
			false,
			&anonymous,
			&whole,
			None
		));
		assert!(!lookup_wanted(
			true,
			IntakeKind::Book,
			true,
			&anonymous,
			&whole,
			None
		));
	}

	/// Two filings of one name each copy into a temporary file of their own: a copy that is half
	/// made isn't written into by the other, the linked book is whole, and the one that finds the
	/// name taken replaces nothing and leaves nothing behind
	#[test]
	fn filings_of_one_name_do_not_share_a_temporary_file() {
		let dir = tempfile::tempdir().unwrap();
		let library = dir.path().join("library");
		std::fs::create_dir_all(&library).unwrap();
		let (first, second) = (
			dir.path().join("first.epub"),
			dir.path().join("second.epub"),
		);
		std::fs::write(&first, vec![1u8; 8192]).unwrap();
		std::fs::write(&second, vec![2u8; 8192]).unwrap();
		let target = library.join("Book.epub");
		// another filing's copy of this very name, half made, where the one fixed temporary name
		// of the old way would have been
		let theirs = library.join(".Book.epub.intake");
		std::fs::write(&theirs, b"half of somebody else's book").unwrap();

		place_file(&first, &target).unwrap();
		assert_eq!(std::fs::read(&target).unwrap(), vec![1u8; 8192]);
		assert_eq!(
			std::fs::read(&theirs).unwrap(),
			b"half of somebody else's book",
			"a copy being made by somebody else is not touched"
		);

		let error = place_file(&second, &target).unwrap_err();
		assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
		assert_eq!(
			std::fs::read(&target).unwrap(),
			vec![1u8; 8192],
			"the book that is there is not replaced"
		);
		let mut left: Vec<String> = std::fs::read_dir(&library)
			.unwrap()
			.filter_map(Result::ok)
			.map(|e| e.file_name().to_string_lossy().into_owned())
			.collect();
		left.sort();
		assert_eq!(left, [".Book.epub.intake", "Book.epub"]);
	}

	/// The temporary name has room for the longest name a folder takes: a book whose name is near
	/// the limit of the file system is filed like any other
	#[test]
	fn a_file_name_near_the_limit_still_has_a_temporary_name_that_fits() {
		let dir = tempfile::tempdir().unwrap();
		let source = dir.path().join("source.epub");
		std::fs::write(&source, b"book").unwrap();
		let target = dir.path().join(format!("{}.epub", "é".repeat(120)));
		assert!(target.file_name().unwrap().len() <= 255);
		place_file(&source, &target).unwrap();
		assert_eq!(std::fs::read(&target).unwrap(), b"book");
		let others = std::fs::read_dir(dir.path()).unwrap().count();
		assert_eq!(others, 2, "the source and the book, no temporary file left");
	}

	fn settings() -> library_intake_settings::Model {
		library_intake_settings::Model::defaults_for("lib", LibraryType::Book)
	}

	fn manga_settings() -> library_intake_settings::Model {
		library_intake_settings::Model::defaults_for("manga", LibraryType::Manga)
	}

	fn file(name: &str, format: &str, volume: Option<f64>) -> IntakeFile {
		IntakeFile {
			original_name: name.into(),
			staged_name: name.into(),
			format: format.into(),
			volume,
			..Default::default()
		}
	}

	#[test]
	fn places_books_under_their_author_and_volumes_under_their_series() {
		let dir = tempfile::tempdir().unwrap();
		std::fs::create_dir_all(dir.path().join("Umberto_Eco")).unwrap();
		std::fs::write(
			dir.path().join("Umberto_Eco/El_nombre_de_la_rosa.epub"),
			b"x",
		)
		.unwrap();

		let files = vec![file("rosa.epub", "epub", None)];
		let mut decision = IntakeDecision {
			metadata: IntakeMetadata {
				title: Some("El nombre de la rosa".into()),
				authors: vec!["Umberto Eco".into()],
				..Default::default()
			},
			folder: "Umberto_Eco".into(),
			volumes: vec![None],
			..Default::default()
		};
		decision.file_names = default_file_names(&files, &decision);
		let destination = compute_destination(dir.path(), &settings(), &files, &decision);
		assert_eq!(
			destination.files[0].relative_path,
			"Umberto_Eco/El_nombre_de_la_rosa.epub"
		);
		assert!(destination.dir_exists);
		assert!(destination.files[0].exists);
		assert!(decision_problems(&decision, &destination)
			.contains(&"A file with that name is already in the library".to_string()));

		let files = vec![
			file("y15.cbz", "cbz", Some(15.0)),
			file("y16.cbz", "cbz", Some(16.0)),
		];
		let mut decision = IntakeDecision {
			metadata: IntakeMetadata {
				series: Some("Yotsuba&!".into()),
				..Default::default()
			},
			series_kind: Some(IntakeSeriesKind::Manga),
			folder: "Yotsuba&!".into(),
			volumes: vec![Some(15.0), Some(16.0)],
			..Default::default()
		};
		decision.file_names = default_file_names(&files, &decision);
		let destination = compute_destination(dir.path(), &settings(), &files, &decision);
		assert_eq!(destination.relative_dir, "_Manga/Yotsuba&!");
		assert_eq!(
			destination.files[1].relative_path,
			"_Manga/Yotsuba&!/Yotsuba&! v16.cbz"
		);
		assert!(decision_problems(&decision, &destination).is_empty());
	}

	/// a manga library keeps its series folders in the root: an empty folder setting must never
	/// produce `/Series`, which is an absolute path and leaves the library once joined to the root
	#[test]
	fn a_manga_library_files_series_directly_in_its_root() {
		let root = Path::new("/manga");
		let files = vec![
			file("b1.cbz", "cbz", Some(1.0)),
			file("b2.cbz", "cbz", Some(2.0)),
		];
		let mut decision = IntakeDecision {
			metadata: IntakeMetadata {
				series: Some("Berserk".into()),
				..Default::default()
			},
			series_kind: Some(IntakeSeriesKind::Manga),
			folder: "Berserk".into(),
			volumes: vec![Some(1.0), Some(2.0)],
			..Default::default()
		};
		decision.file_names = default_file_names(&files, &decision);
		let destination = compute_destination(root, &manga_settings(), &files, &decision);
		assert_eq!(destination.relative_dir, "Berserk");
		assert_eq!(
			destination.files[0].relative_path,
			"Berserk/Berserk v01.cbz"
		);
		assert_eq!(
			destination.files[1].relative_path,
			"Berserk/Berserk v02.cbz"
		);
		for relative in std::iter::once(&destination.relative_dir)
			.chain(destination.files.iter().map(|f| &f.relative_path))
		{
			assert!(!relative.starts_with('/'), "{relative}");
			assert!(stays_in_library(relative), "{relative}");
			assert!(root.join(relative).starts_with(root), "{relative}");
		}
		assert!(decision_problems(&decision, &destination).is_empty());

		// a light novel aimed at the same library goes to the same place: both kinds sit at
		// its root, and no `_Manga` or `_Novela ligera` folder is part of its paths
		decision.series_kind = Some(IntakeSeriesKind::LightNovel);
		let destination = compute_destination(root, &manga_settings(), &files, &decision);
		assert_eq!(destination.relative_dir, "Berserk");
	}

	#[test]
	fn joins_folders_without_leaving_the_library() {
		assert_eq!(join_relative("", "Berserk"), "Berserk");
		assert_eq!(join_relative("  ", "Berserk"), "Berserk");
		assert_eq!(join_relative("/", "Berserk"), "Berserk");
		assert_eq!(join_relative("_Manga", "Berserk"), "_Manga/Berserk");
		assert_eq!(join_relative("/_Manga/", "Berserk"), "_Manga/Berserk");
		assert!(stays_in_library("Berserk"));
		assert!(stays_in_library("_Manga/Berserk/v01.cbz"));
		for outside in ["", "/Berserk", "../Berserk", "a/../../b", "./a", "/"] {
			assert!(!stays_in_library(outside), "{outside:?}");
		}
	}

	/// a folder setting that leaves the library (however it got into the database) blocks the filing
	#[test]
	fn a_folder_that_leaves_the_library_is_a_problem() {
		let files = vec![file("b1.cbz", "cbz", Some(1.0))];
		let mut decision = IntakeDecision {
			metadata: IntakeMetadata {
				series: Some("Berserk".into()),
				..Default::default()
			},
			series_kind: Some(IntakeSeriesKind::Manga),
			folder: "Berserk".into(),
			volumes: vec![Some(1.0)],
			..Default::default()
		};
		decision.file_names = default_file_names(&files, &decision);
		let mut escaping = settings();
		escaping.manga_dir = "../elsewhere".into();
		let destination =
			compute_destination(Path::new("/lib"), &escaping, &files, &decision);
		assert_eq!(destination.relative_dir, "../elsewhere/Berserk");
		assert!(decision_problems(&decision, &destination)
			.contains(&"The folder is outside the library".to_string()));
		escaping.manga_dir = "_Manga".into();
		let destination =
			compute_destination(Path::new("/lib"), &escaping, &files, &decision);
		assert!(decision_problems(&decision, &destination).is_empty());
	}

	#[test]
	fn volumes_named_like_their_series_get_a_volume_title() {
		assert_eq!(
			volume_title(Some("Yotsuba&!"), Some("Yotsuba&!"), Some(3.0)).as_deref(),
			Some("Yotsuba&! v03")
		);
		assert_eq!(
			volume_title(
				Some("Spice and Wolf, Vol. 3"),
				Some("Spice and Wolf"),
				Some(3.0)
			)
			.as_deref(),
			Some("Spice and Wolf, Vol. 3")
		);
		assert_eq!(
			volume_title(None, Some("Dr. Stone"), None).as_deref(),
			Some("Dr. Stone")
		);
	}

	#[test]
	fn never_replaces_an_existing_file() {
		let dir = tempfile::tempdir().unwrap();
		let source = dir.path().join("a.epub");
		let target = dir.path().join("b.epub");
		std::fs::write(&source, b"new").unwrap();
		std::fs::write(&target, b"old").unwrap();
		assert!(place_file(&source, &target).is_err());
		assert_eq!(std::fs::read(&target).unwrap(), b"old");
		assert_eq!(suffixed("b.epub", 2), "b_2.epub");
		let fresh = dir.path().join("c.epub");
		place_file(&source, &fresh).unwrap();
		assert_eq!(std::fs::read(&fresh).unwrap(), b"new");
		assert!(source.exists());
		// the filing's own scan reads it: not a reason for the library watcher to scan
		assert!(own_writes::is_own(dir.path(), &fresh));
		let folder = dir.path().join("New_Author/Sub");
		create_folders(dir.path(), &folder).unwrap();
		assert!(folder.is_dir());
		assert!(own_writes::is_own(dir.path(), &folder));
		assert!(own_writes::is_own(
			dir.path(),
			&dir.path().join("New_Author")
		));
	}

	#[test]
	fn a_file_name_keeps_the_extension_of_its_format() {
		let files = vec![file("a.epub", "epub", None), file("b.cbz", "cbz", None)];
		let mut decision = IntakeDecision {
			file_names: vec!["Book.epub".into(), "Comic.CBZ".into()],
			..Default::default()
		};
		assert!(extension_problems(&decision, &files).is_empty());
		decision.file_names = vec!["Book.pdf".into(), "Comic".into()];
		assert_eq!(
			extension_problems(&decision, &files),
			[
				"The file name 'Book.pdf' must end in .epub",
				"The file name 'Comic' must end in .cbz"
			]
		);
	}

	/// every state is decided here: a state added later doesn't compile until someone says
	/// whether the uploader, and an approver, can still change an item in it
	#[test]
	fn only_the_states_that_wait_for_the_uploader_are_open_for_changes() {
		use sea_orm::strum::IntoEnumIterator;

		fn expected(state: IntakeState) -> (bool, bool, bool) {
			// (open for the uploader's changes, can be analysed again by them, open to an
			// approver for somebody else's item)
			match state {
				IntakeState::NeedsReview
				| IntakeState::Ready
				| IntakeState::Failed
				| IntakeState::Rejected => (true, true, false),
				IntakeState::Analysing => (false, true, false),
				IntakeState::PendingApproval => (false, false, true),
				IntakeState::Filing | IntakeState::Filed | IntakeState::Discarded => {
					(false, false, false)
				},
			}
		}
		let uploader = Reviewer {
			approver: false,
			owns_item: true,
		};
		let approver_of_others = Reviewer {
			approver: true,
			owns_item: false,
		};
		for state in IntakeState::iter() {
			let (open, again, approvers) = expected(state);
			assert_eq!(is_open_for_changes(state), open, "{state}");
			assert_eq!(uploader.may_change(state), open, "{state}");
			assert_eq!(can_reanalyse(state, uploader), again, "{state}");
			assert_eq!(approver_of_others.may_change(state), approvers, "{state}");
			// somebody who isn't an approver never changes an item that isn't theirs
			assert!(
				!Reviewer {
					approver: false,
					owns_item: false
				}
				.may_change(state),
				"{state}"
			);
		}
		// what an uploader is told
		assert!(matches!(
			closed_for_changes(IntakeState::Filed, "moved to another library"),
			CoreError::BadRequest(m) if m == "This item is no longer in the inbox"
		));
		assert!(matches!(
			closed_for_changes(IntakeState::Analysing, "moved to another library"),
			CoreError::BadRequest(m)
				if m == "An item that is ANALYSING can't be moved to another library"
		));
		assert!(matches!(
			closed_for_changes(IntakeState::PendingApproval, "analysed again"),
			CoreError::BadRequest(m) if m.contains("waiting for approval")
		));
	}

	/// a file or folder name from the uploader is only looked up on the disk when it stays in the
	/// folder: otherwise the `exists` flag would say whether any path on the server exists
	#[test]
	fn a_name_that_leaves_its_folder_is_never_looked_up() {
		let dir = tempfile::tempdir().unwrap();
		let root = dir.path().join("library");
		std::fs::create_dir_all(root.join("Berserk")).unwrap();
		std::fs::write(dir.path().join("secret.txt"), b"x").unwrap();
		std::fs::write(root.join("Berserk/Berserk v01.cbz"), b"x").unwrap();

		let files = vec![file("b1.cbz", "cbz", Some(1.0))];
		let decision_with = |folder: &str, name: &str| IntakeDecision {
			metadata: IntakeMetadata {
				series: Some("Berserk".into()),
				..Default::default()
			},
			series_kind: Some(IntakeSeriesKind::Manga),
			folder: folder.into(),
			file_names: vec![name.into()],
			volumes: vec![Some(1.0)],
			..Default::default()
		};

		// the control: a plain folder and name are looked up
		let decision = decision_with("Berserk", "Berserk v01.cbz");
		let found = compute_destination(&root, &manga_settings(), &files, &decision);
		assert!(found.dir_exists);
		assert!(found.files[0].exists);

		// a name that climbs out of the folder is not, and is a problem of its own
		let decision = decision_with("Berserk", "../../secret.txt");
		let climbing = compute_destination(&root, &manga_settings(), &files, &decision);
		assert!(root.join("Berserk").join("../../secret.txt").exists());
		assert!(!climbing.files[0].exists);
		assert!(decision_problems(&decision, &climbing)
			.contains(&"Invalid file name '../../secret.txt'".to_string()));

		// nor is a folder that is not a plain name
		for folder in ["..", "../library", "Berserk/..", ""] {
			let decision = decision_with(folder, "Berserk v01.cbz");
			let escaping =
				compute_destination(&root, &manga_settings(), &files, &decision);
			assert!(!escaping.dir_exists, "{folder:?}");
			assert!(!escaping.files[0].exists, "{folder:?}");
			assert!(
				!decision_problems(&decision, &escaping).is_empty(),
				"{folder:?}"
			);
		}
	}

	async fn analysing_item(
		conn: &DatabaseConnection,
		user_id: &str,
		library_id: &str,
	) -> intake_item::Model {
		let item = intake_item::ActiveModel {
			id: Set(uuid::Uuid::new_v4().to_string()),
			user_id: Set(user_id.to_string()),
			library_id: Set(library_id.to_string()),
			state: Set(IntakeState::Analysing),
			kind: Set(IntakeKind::Book),
			name: Set("Book.epub".to_string()),
			staging_dir: Set("/tmp/intake-test".to_string()),
			files: Set(models::shared::intake::IntakeFiles(vec![])),
			..Default::default()
		};
		item.insert(conn).await.unwrap()
	}

	fn found(state: IntakeState) -> intake_item::ActiveModel {
		intake_item::ActiveModel {
			state: Set(state),
			decision: Set(Some(IntakeDecision::default())),
			error: Set(None),
			..Default::default()
		}
	}

	/// an analysis answers into the item it started with, once, and only while the item waits for
	/// it: never into an item that moved, was discarded or filed, or already has an answer
	#[tokio::test]
	async fn an_outcome_is_stored_only_in_the_item_that_waits_for_it() {
		use ::tests::{db::test_database, fake_data};

		let conn = test_database().await;
		let user = fake_data::User::new("uploader").insert(&conn).await;
		let here = fake_data::Library::default().insert(&conn).await;
		let there = fake_data::Library::default().insert(&conn).await;
		let row = |id: String| {
			let conn = conn.clone();
			async move {
				intake_item::Entity::find_by_id(id)
					.one(&conn)
					.await
					.unwrap()
					.unwrap()
			}
		};

		// the plain case: stored, with the state and a fresh `updated_at`
		let item = analysing_item(&conn, &user.id, &here.id).await;
		assert!(store_outcome(&conn, &item, found(IntakeState::Ready))
			.await
			.unwrap());
		let stored = row(item.id.clone()).await;
		assert_eq!(stored.state, IntakeState::Ready);
		assert!(stored.decision.is_some());
		assert!(stored.updated_at >= item.updated_at);

		// it has its answer: another analysis that finishes later is dropped, as is a failure
		assert!(
			!store_outcome(&conn, &item, found(IntakeState::NeedsReview))
				.await
				.unwrap()
		);
		let failed = intake_item::ActiveModel {
			state: Set(IntakeState::Failed),
			error: Set(Some("Analysis failed: late".into())),
			..Default::default()
		};
		assert!(!store_outcome(&conn, &item, failed).await.unwrap());
		let stored = row(item.id.clone()).await;
		assert_eq!(stored.state, IntakeState::Ready);
		assert!(stored.error.is_none());

		// moved to another library meanwhile: the analysis there owns the item
		let item = analysing_item(&conn, &user.id, &here.id).await;
		let mut moved = item.clone().into_active_model();
		moved.library_id = Set(there.id.clone());
		moved.update(&conn).await.unwrap();
		assert!(!store_outcome(&conn, &item, found(IntakeState::Ready))
			.await
			.unwrap());
		let stored = row(item.id.clone()).await;
		assert_eq!(
			(stored.library_id.as_str(), stored.state),
			(there.id.as_str(), IntakeState::Analysing)
		);
		assert!(stored.decision.is_none());

		// discarded or filed meanwhile: an analysis that ends later doesn't bring it back
		for closed in [IntakeState::Discarded, IntakeState::Filed] {
			let item = analysing_item(&conn, &user.id, &here.id).await;
			let mut gone = item.clone().into_active_model();
			gone.state = Set(closed);
			gone.update(&conn).await.unwrap();
			assert!(!store_outcome(&conn, &item, found(IntakeState::Ready))
				.await
				.unwrap());
			assert_eq!(row(item.id.clone()).await.state, closed);
		}

		// a database that fails is an error to report, not an item that moved
		let item = analysing_item(&conn, &user.id, &here.id).await;
		let closed = conn.clone();
		conn.close().await.unwrap();
		assert!(store_outcome(&closed, &item, found(IntakeState::Ready))
			.await
			.is_err());
	}
}
