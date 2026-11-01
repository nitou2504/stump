//! The "Add books" intake: per-user uploads are staged under the config directory,
//! analysed (embedded metadata, file name, Open Library / Google Books, matching against the
//! library), reviewed by the uploader, then filed into the library with their metadata
//! written into the files, a row appended to the intake CSV and a scan of just the new
//! folders. Nothing runs between requests: analysis is a short task per upload, and the
//! library index it uses is dropped minutes later.

pub mod extract;
pub mod library_index;
pub mod lookup;
pub mod naming;
pub mod suggest;
pub mod write;

use std::{
	collections::BTreeSet,
	io::Write as _,
	path::{Path, PathBuf},
	sync::LazyLock,
};

use models::{
	entity::{
		intake_item, library, library_config,
		library_intake_settings::{self, INTAKE_CSV_NAME},
		media, user,
	},
	shared::{
		enums::OrganizationMode,
		intake::{
			IntakeDecision, IntakeDestination, IntakeDestinationFile, IntakeDuplicate,
			IntakeFile, IntakeKind, IntakeMetadata, IntakeProposal, IntakeResult,
			IntakeReviewInput, IntakeSeriesKind, IntakeSource, IntakeState,
		},
	},
};
use sea_orm::{prelude::*, ActiveValue::Set, IntoActiveModel, QueryOrder};
use tokio::sync::Mutex;

use crate::{job::StumpJob, CoreError, CoreResult, Ctx};

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

/// Serialises appends to the intake CSVs
static CSV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// The root of all staged uploads
pub fn staging_root(ctx: &Ctx) -> PathBuf {
	ctx.config.config_directory().join("intake")
}

/// The staging directory of one item
pub fn staging_dir(ctx: &Ctx, user_id: &str, item_id: &str) -> PathBuf {
	staging_root(ctx).join(user_id).join(item_id)
}

/// The intake settings of a library (its stored row, or the defaults)
pub async fn settings_for(
	conn: &DatabaseConnection,
	library_id: &str,
) -> CoreResult<library_intake_settings::Model> {
	Ok(library_intake_settings::Entity::find_by_id(library_id.to_string())
		.one(conn)
		.await?
		.unwrap_or_else(|| library_intake_settings::Model::defaults_for(library_id)))
}

async fn load_library(
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
			let mut active = item.into_active_model();
			active.state = Set(IntakeState::Failed);
			active.error = Set(Some(format!("Analysis failed: {error}")));
			active.update(conn).await?;
			Err(error)
		},
	}
}

async fn analyse_item(
	ctx: &Ctx,
	item: &intake_item::Model,
	online: Option<bool>,
) -> CoreResult<()> {
	let conn = ctx.conn.as_ref();
	let (library, _) = load_library(conn, &item.library_id).await?;
	let settings = settings_for(conn, &item.library_id).await?;
	let root = PathBuf::from(&library.path);
	let index = library_index(
		conn,
		&library.id,
		&root,
		&settings.manga_dir,
		&settings.light_novel_dir,
	)
	.await?;

	let staging = PathBuf::from(&item.staging_dir);
	let mut files = item.files.0.clone();
	let mut per_file: Vec<IntakeMetadata> = vec![];
	let mut series_hint = None;
	for file in files.iter_mut() {
		let extracted =
			extract_file(&staging.join(&file.staged_name), &file.original_name, &ctx.config).await;
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
	let has_good_tags = file_meta
		.subjects
		.iter()
		.filter(|s| index.vocab_index(&library_index::norm(s)).is_some())
		.count() >= 2;
	let is_comic = formats.iter().all(|f| f == "cbz" || f == "cbr");
	let online_records = if wants_online
		&& item.kind == IntakeKind::Book
		&& !is_comic
		&& !(has_good_tags && !file_meta.authors.is_empty())
	{
		lookup::lookup(
			file_meta.isbn.as_deref(),
			file_meta.title.as_deref(),
			&file_meta.authors,
		)
		.await
	} else {
		vec![]
	};

	let description = file_meta
		.description
		.clone()
		.or_else(|| online_records.iter().find_map(|r| r.description.clone()))
		.unwrap_or_default();
	let similar = index.similar_by_description(conn, &description, 10).await;
	let group_name = (item.kind == IntakeKind::Series).then(|| item.name.clone());

	let mut proposal = build_proposal(Inputs {
		kind: item.kind,
		index: &index,
		file_meta,
		online: online_records,
		formats,
		series_hint,
		group_name,
		similar,
	});

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

	let decision = default_decision(item.kind, &files, &proposal);
	let destination = compute_destination(&root, &settings, &files, &decision);
	let state = state_for(&proposal, &decision, &destination);

	let mut active = item.clone().into_active_model();
	active.files = Set(intake_files(files));
	active.proposal = Set(Some(proposal));
	active.decision = Set(Some(decision));
	active.destination = Set(Some(destination));
	active.state = Set(state);
	active.error = Set(None);
	active.update(conn).await?;
	Ok(())
}

fn intake_files(files: Vec<IntakeFile>) -> models::shared::intake::IntakeFiles {
	models::shared::intake::IntakeFiles(files)
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
	let volumes: Vec<Option<f64>> = if kind == IntakeKind::Series || proposal.series_kind.is_some() {
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
pub fn default_file_names(files: &[IntakeFile], decision: &IntakeDecision) -> Vec<String> {
	files
		.iter()
		.enumerate()
		.map(|(i, file)| match decision.series_kind {
			Some(_) => series_volume_file_name(
				decision.metadata.series.as_deref().unwrap_or(&decision.folder),
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

/// Where a decision puts each file, relative to the library root
pub fn compute_destination(
	root: &Path,
	settings: &library_intake_settings::Model,
	files: &[IntakeFile],
	decision: &IntakeDecision,
) -> IntakeDestination {
	let relative_dir = match decision.series_kind {
		Some(IntakeSeriesKind::Manga) => format!("{}/{}", settings.manga_dir, decision.folder),
		Some(IntakeSeriesKind::LightNovel) => {
			format!("{}/{}", settings.light_novel_dir, decision.folder)
		},
		None => decision.folder.clone(),
	};
	let dir = root.join(&relative_dir);
	IntakeDestination {
		dir_exists: !decision.folder.is_empty() && dir.is_dir(),
		files: files
			.iter()
			.zip(decision.file_names.iter())
			.map(|(file, name)| IntakeDestinationFile {
				original_name: file.original_name.clone(),
				relative_path: format!("{relative_dir}/{name}"),
				exists: dir.join(name).exists(),
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

/// What keeps a decision from being filed
pub fn decision_problems(
	decision: &IntakeDecision,
	destination: &IntakeDestination,
) -> Vec<String> {
	let mut problems = vec![];
	if !is_plain_component(&decision.folder) {
		problems.push("Choose a folder".to_string());
	}
	if decision.series_kind.is_none() {
		if decision.metadata.title.as_deref().is_none_or(|t| t.trim().is_empty()) {
			problems.push("Add a title".to_string());
		}
		if decision.metadata.authors.is_empty() {
			problems.push("Add an author".to_string());
		}
	} else if decision.metadata.series.as_deref().is_none_or(|s| s.trim().is_empty()) {
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
		problems.push("A file with that name is already in the library".to_string());
	}
	problems
}

fn set_text(meta: &mut IntakeMetadata, field: &str, value: Option<String>) -> Option<String> {
	let value = value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
	meta.set_source(field, IntakeSource::User);
	value
}

/// Applies the uploader's review to an item and recomputes its destination and state
pub async fn apply_review(
	ctx: &Ctx,
	item: intake_item::Model,
	input: IntakeReviewInput,
) -> CoreResult<intake_item::Model> {
	let conn = ctx.conn.as_ref();
	if matches!(item.state, IntakeState::Filed | IntakeState::Discarded | IntakeState::Analysing) {
		return Err(CoreError::BadRequest(format!(
			"An item that is {} can't be changed",
			item.state
		)));
	}
	let (library, _) = load_library(conn, &item.library_id).await?;
	let settings = settings_for(conn, &item.library_id).await?;
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
		meta.age_rating = Some(age);
	}
	if input.clear_series_kind {
		decision.series_kind = None;
	} else if let Some(kind) = input.series_kind {
		decision.series_kind = Some(kind);
	}
	if let Some(volumes) = input.volumes {
		decision.volumes = volumes;
	}
	decision.volumes.resize(files.len(), None);
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
		Some(names) => decision.file_names = names.into_iter().map(|n| n.trim().to_string()).collect(),
		None if names_were_default => decision.file_names = default_file_names(&files, &decision),
		None => {},
	}
	decision.file_names.resize(files.len(), String::new());

	let destination = compute_destination(Path::new(&library.path), &settings, &files, &decision);
	let state = if decision_problems(&decision, &destination).is_empty() {
		IntakeState::Ready
	} else {
		IntakeState::NeedsReview
	};
	let mut active = item.into_active_model();
	active.decision = Set(Some(decision));
	active.destination = Set(Some(destination));
	active.state = Set(state);
	Ok(active.update(conn).await?)
}

/// The intake CSV of a library: its setting, else next to `catalogo.csv`, else the root
pub fn csv_path_for(root: &Path, settings: &library_intake_settings::Model) -> PathBuf {
	if let Some(path) = settings.csv_path.as_deref().filter(|p| !p.trim().is_empty()) {
		return PathBuf::from(path);
	}
	match root.parent() {
		Some(parent) if parent.join("catalogo.csv").is_file() => parent.join(INTAKE_CSV_NAME),
		_ => root.join(INTAKE_CSV_NAME),
	}
}

const CSV_HEADER: &[&str] = &[
	"path", "title", "authors", "series", "series_index", "language", "format", "owned",
	"estante", "estantes", "tags", "libid", "author_wikidata", "goodreads_work", "rating",
	"rating_votes", "age", "shelf", "big", "small", "facets", "added_by", "added_at",
];

fn csv_row(
	csv_dir: &Path,
	file_path: &Path,
	format: &str,
	meta: &FileMetadata,
	added_by: &str,
) -> Vec<String> {
	let path = file_path
		.strip_prefix(csv_dir)
		.unwrap_or(file_path)
		.to_string_lossy()
		.into_owned();
	let big: Vec<&str> = meta
		.tags
		.iter()
		.filter(|t| !library_index::is_facet(t) && !t.contains(':'))
		.map(String::as_str)
		.collect();
	let small: Vec<&str> = meta
		.tags
		.iter()
		.filter(|t| !library_index::is_facet(t) && t.contains(':'))
		.map(String::as_str)
		.collect();
	let facets: Vec<&str> = meta
		.tags
		.iter()
		.filter(|t| library_index::is_facet(t))
		.map(String::as_str)
		.collect();
	vec![
		path,
		meta.title.clone().unwrap_or_default(),
		meta.authors.join(" & "),
		meta.series.clone().unwrap_or_default(),
		meta.series_index.map(|i| i.to_string()).unwrap_or_default(),
		meta.language.clone().unwrap_or_default(),
		format.to_string(),
		"False".to_string(),
		String::new(),
		String::new(),
		String::new(),
		String::new(),
		String::new(),
		String::new(),
		meta.rating.map(|r| format!("{r:.2}")).unwrap_or_default(),
		String::new(),
		meta.age_rating.map(|a| a.to_string()).unwrap_or_default(),
		meta.shelf().unwrap_or_default().to_string(),
		big.join(" | "),
		small.join(" | "),
		facets.join(" | "),
		added_by.to_string(),
		chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
	]
}

/// Appends rows to an intake CSV in one write (header first when the file is new)
pub async fn append_csv(path: &Path, rows: Vec<Vec<String>>) -> CoreResult<()> {
	let _guard = CSV_LOCK.lock().await;
	let path = path.to_path_buf();
	tokio::task::spawn_blocking(move || -> std::io::Result<()> {
		let is_new = std::fs::metadata(&path).map(|m| m.len() == 0).unwrap_or(true);
		let mut writer = csv::WriterBuilder::new().from_writer(vec![]);
		if is_new {
			writer.write_record(CSV_HEADER)?;
		}
		for row in rows {
			writer.write_record(row)?;
		}
		let bytes = writer
			.into_inner()
			.map_err(|e| std::io::Error::other(e.to_string()))?;
		let mut file = std::fs::OpenOptions::new()
			.create(true)
			.append(true)
			.open(&path)?;
		file.write_all(&bytes)?;
		file.sync_data()
	})
	.await
	.map_err(|e| CoreError::InternalError(e.to_string()))??;
	Ok(())
}

/// Copies `source` to `target` without ever replacing an existing file: the copy goes to
/// a temporary name in the target folder, then is hard-linked into place
fn place_file(source: &Path, target: &Path) -> std::io::Result<()> {
	let dir = target.parent().ok_or_else(|| std::io::Error::other("no parent folder"))?;
	let tmp = dir.join(format!(
		".{}.intake",
		target.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
	));
	std::fs::copy(source, &tmp)?;
	if let Ok(meta) = std::fs::metadata(dir) {
		// the files of a folder get its permissions without the execute bits
		use std::os::unix::fs::PermissionsExt;
		let mode = meta.permissions().mode() & 0o666;
		let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode));
	}
	let linked = std::fs::hard_link(&tmp, target);
	let _ = std::fs::remove_file(&tmp);
	linked
}

/// A name with `_2`, `_3`, ... before the extension, for a clash at filing time
fn suffixed(name: &str, n: usize) -> String {
	match name.rsplit_once('.') {
		Some((stem, ext)) => format!("{stem}_{n}.{ext}"),
		None => format!("{name}_{n}"),
	}
}

/// Files a reviewed item into its library
pub async fn file_item(
	ctx: &Ctx,
	item: intake_item::Model,
	username: &str,
) -> CoreResult<intake_item::Model> {
	let conn = ctx.conn.as_ref();
	if item.state != IntakeState::Ready {
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
	let settings = settings_for(conn, &item.library_id).await?;
	let root = PathBuf::from(&library.path);
	let files = item.files.0.clone();
	let destination = compute_destination(&root, &settings, &files, &decision);
	let problems = decision_problems(&decision, &destination);
	// a name taken since the review gets a suffix instead of failing
	let clash_only = problems
		.iter()
		.all(|p| p == "A file with that name is already in the library");
	if !problems.is_empty() && !clash_only {
		return Err(CoreError::BadRequest(problems.join("; ")));
	}

	let staging = PathBuf::from(&item.staging_dir);
	let dir = root.join(&destination.relative_dir);
	tokio::fs::create_dir_all(&dir).await?;
	let csv_path = csv_path_for(&root, &settings);
	let csv_dir = csv_path.parent().map(Path::to_path_buf).unwrap_or_else(|| root.clone());

	let mut paths = vec![];
	let mut notes = vec![];
	let mut rows = vec![];
	for (i, file) in files.iter().enumerate() {
		let mut meta = FileMetadata::from_metadata(
			&decision.metadata,
			decision.series_kind == Some(IntakeSeriesKind::Manga),
		);
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
			"pdf" => write_pdf(&staged_clone, &meta_clone, &KeptPdfInfo::default()).map(|_| None),
			other => Ok(Some(format!("{other} files keep their metadata as uploaded"))),
		})
		.await
		.map_err(|e| CoreError::InternalError(e.to_string()))?;
		match written {
			Ok(Some(note)) => notes.push(note),
			Ok(None) => {},
			Err(error) => {
				// the book still goes in, with the metadata it came with
				notes.push(format!("{}: metadata not written ({error})", file.original_name));
			},
		}

		let name = decision.file_names.get(i).cloned().unwrap_or_default();
		let mut target = dir.join(&name);
		let mut n = 2;
		while target.exists() {
			target = dir.join(suffixed(&name, n));
			n += 1;
		}
		let (from, to) = (staged.clone(), target.clone());
		tokio::task::spawn_blocking(move || place_file(&from, &to))
			.await
			.map_err(|e| CoreError::InternalError(e.to_string()))??;
		rows.push(csv_row(&csv_dir, &target, &file.format, &meta, username));
		paths.push(target.to_string_lossy().into_owned());
	}

	let mut csv_written = None;
	match append_csv(&csv_path, rows).await {
		Ok(()) => csv_written = Some(csv_path.to_string_lossy().into_owned()),
		Err(error) => notes.push(format!("The intake CSV could not be written: {error}")),
	}

	let mut active = item.clone().into_active_model();
	active.state = Set(IntakeState::Filed);
	active.destination = Set(Some(destination));
	active.result = Set(Some(IntakeResult {
		paths,
		csv_path: csv_written,
		notes,
	}));
	active.filed_at = Set(Some(chrono::Utc::now().into()));
	active.error = Set(None);
	let updated = active.update(conn).await?;

	let _ = tokio::fs::remove_dir_all(&staging).await;
	library_index::invalidate_index(&library.id);
	enqueue_scan(ctx, &library, config.as_ref(), &dir).await;
	Ok(updated)
}

/// A volume's own title, unless it is just the series name: then `<Series> vNN`
pub fn volume_title(title: Option<&str>, series: Option<&str>, volume: Option<f64>) -> Option<String> {
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
	if item.state == IntakeState::Filed {
		return Err(CoreError::BadRequest("A filed item can't be discarded".into()));
	}
	let _ = tokio::fs::remove_dir_all(&item.staging_dir).await;
	let mut active = item.into_active_model();
	active.state = Set(IntakeState::Discarded);
	active.update(ctx.conn.as_ref()).await?;
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
	let items = sea_orm::QuerySelect::offset(sea_orm::QuerySelect::limit(query, limit), offset)
		.all(conn)
		.await?;
	let mut out = vec![];
	for item in items {
		let user = user::Entity::find_by_id(item.user_id.clone()).one(conn).await?;
		out.push((item, user));
	}
	Ok(out)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn settings() -> library_intake_settings::Model {
		library_intake_settings::Model::defaults_for("lib")
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
		std::fs::write(dir.path().join("Umberto_Eco/El_nombre_de_la_rosa.epub"), b"x").unwrap();

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

		let files = vec![file("y15.cbz", "cbz", Some(15.0)), file("y16.cbz", "cbz", Some(16.0))];
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
		assert_eq!(destination.files[1].relative_path, "_Manga/Yotsuba&!/Yotsuba&! v16.cbz");
		assert!(decision_problems(&decision, &destination).is_empty());
	}

	#[test]
	fn volumes_named_like_their_series_get_a_volume_title() {
		assert_eq!(
			volume_title(Some("Yotsuba&!"), Some("Yotsuba&!"), Some(3.0)).as_deref(),
			Some("Yotsuba&! v03")
		);
		assert_eq!(
			volume_title(Some("Spice and Wolf, Vol. 3"), Some("Spice and Wolf"), Some(3.0)).as_deref(),
			Some("Spice and Wolf, Vol. 3")
		);
		assert_eq!(volume_title(None, Some("Dr. Stone"), None).as_deref(), Some("Dr. Stone"));
	}

	#[test]
	fn the_csv_goes_next_to_the_catalogue() {
		let dir = tempfile::tempdir().unwrap();
		let root = dir.path().join("biblioteca");
		std::fs::create_dir_all(&root).unwrap();
		assert_eq!(csv_path_for(&root, &settings()), root.join(INTAKE_CSV_NAME));
		std::fs::write(dir.path().join("catalogo.csv"), b"path\n").unwrap();
		assert_eq!(csv_path_for(&root, &settings()), dir.path().join(INTAKE_CSV_NAME));
	}

	#[tokio::test]
	async fn appends_csv_rows_with_one_header() {
		let dir = tempfile::tempdir().unwrap();
		let csv = dir.path().join("stump_intake.csv");
		let meta = FileMetadata {
			title: Some("El nombre de la rosa".into()),
			authors: vec!["Umberto Eco".into()],
			tags: vec!["Historical Fiction".into(), "Origin: Italian".into()],
			..Default::default()
		};
		let target = dir.path().join("biblioteca/Umberto_Eco/El_nombre_de_la_rosa.epub");
		for _ in 0..2 {
			append_csv(&csv, vec![csv_row(dir.path(), &target, "epub", &meta, "alice")])
				.await
				.unwrap();
		}
		let text = std::fs::read_to_string(&csv).unwrap();
		let lines: Vec<&str> = text.lines().collect();
		assert_eq!(lines.len(), 3);
		assert!(lines[0].starts_with("path,title,authors,series"));
		assert!(lines[0].ends_with("added_by,added_at"));
		assert!(lines[1].starts_with(
			"biblioteca/Umberto_Eco/El_nombre_de_la_rosa.epub,El nombre de la rosa,Umberto Eco,"
		));
		assert!(lines[1].contains(",Historical Fiction,Historical Fiction,,Origin: Italian,alice,"));
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
	}
}
