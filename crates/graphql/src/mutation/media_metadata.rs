use crate::{
	data::{AuthContext, CoreContext},
	guard::PermissionGuard,
	input::media::{MediaMetadataInput, MediaMetadataSearchInput},
	object::{
		media::Media,
		metadata_edit::{MetadataEditFile, MetadataEditResult},
		metadata_fetch_record::MetadataFetchRecord,
	},
};
use async_graphql::{Context, MaybeUndefined, Object, Result, ID};
use metadata_integrations::{
	MatchCandidate, MergeStrategy, MetadataField, MetadataFieldOverride, SearchQuery,
};
use models::{
	entity::{media, media_metadata, metadata_fetch_record, user::AuthUser},
	shared::enums::{MetadataFetchStatus, UserPermission},
};
use sea_orm::{prelude::*, ActiveValue::Set, IntoActiveModel, TransactionTrait};
use std::collections::HashMap;
use stump_core::{
	intake::edit::{
		clean_genres, clean_writers, revert_save, save_edit, Editor, SavedEdit,
	},
	metadata::{
		membership::{remove_series_thumbnails, sync_media_membership},
		provider::ProviderClientCache,
	},
};

fn clean_list(
	value: MaybeUndefined<Vec<String>>,
	clean: fn(&[String]) -> Vec<String>,
) -> MaybeUndefined<Vec<String>> {
	match value {
		MaybeUndefined::Value(values) => MaybeUndefined::Value(clean(&values)),
		other => other,
	}
}

#[derive(Default)]
pub struct MediaMetadataMutation;

#[Object]
impl MediaMetadataMutation {
	#[graphql(guard = "PermissionGuard::one(UserPermission::EditMetadata)")]
	async fn update_media_metadata(
		&self,
		ctx: &Context<'_>,
		id: ID,
		input: MediaMetadataInput,
	) -> Result<Media> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let model = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::Id.eq(id.to_string()))
			.into_model::<media::ModelWithMetadata>()
			.one(conn)
			.await?
			.ok_or("Media not found")?;

		let txn = conn.begin().await?;
		let updated_metadata = if let Some(existing) = model.metadata {
			let mut active_model = input.into_active_model();
			active_model.id = Set(existing.id);
			active_model.media_id = Set(Some(model.media.id.clone()));
			active_model.update(&txn).await?
		} else {
			let mut active_model = input.into_active_model();
			active_model.media_id = Set(Some(model.media.id.clone()));
			active_model.insert(&txn).await?
		};
		let (book, thumbnails) =
			sync_media_membership(&txn, model.media, updated_metadata.series.clone())
				.await?;
		txn.commit().await?;
		remove_series_thumbnails(thumbnails).await;

		let model = media::ModelWithMetadata {
			media: book,
			metadata: Some(updated_metadata),
		};

		Ok(model.into())
	}

	/// Save a book's metadata and write the changes into the book file (EPUB, PDF and CBZ
	/// files carry title, authors, series, genres, summary and more): the edited copy is
	/// read back before it replaces the book, so a failed write leaves the file untouched
	/// and the changes in Stump only. Every edited field is locked so rescans keep it, and
	/// the save is recorded in the book's edit history (see `revertMediaMetadataEdit`) and
	/// in `stump_edits.csv`.
	#[graphql(guard = "PermissionGuard::one(UserPermission::EditMetadata)")]
	async fn edit_media_metadata(
		&self,
		ctx: &Context<'_>,
		id: ID,
		mut input: MediaMetadataInput,
	) -> Result<MetadataEditResult> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;

		input.genres = clean_list(input.genres, clean_genres);
		input.writers = clean_list(input.writers, clean_writers);

		let model = find_book(core, user, &id).await?;
		let saved = save_edit(
			core,
			model.media,
			model.metadata,
			input.into_active_model(),
			Editor {
				id: &user.id,
				username: &user.username,
			},
			None,
		)
		.await?;
		edit_result(core, user, saved).await
	}

	/// Revert a save of the book's edit history: the fields it changed get their old values
	/// back (in the book file and in Stump), except the ones changed again since, which
	/// are left as they are (`skippedFields`). The revert is recorded as a new save.
	#[graphql(guard = "PermissionGuard::one(UserPermission::EditMetadata)")]
	async fn revert_media_metadata_edit(
		&self,
		ctx: &Context<'_>,
		media_id: ID,
		save_id: String,
	) -> Result<MetadataEditResult> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let core = ctx.data::<CoreContext>()?;

		let model = find_book(core, user, &media_id).await?;
		let saved = revert_save(
			core,
			model.media,
			model.metadata,
			&save_id,
			Editor {
				id: &user.id,
				username: &user.username,
			},
		)
		.await?;
		edit_result(core, user, saved).await
	}

	/// Search external metadata providers for a media item and return the resulting fetch
	/// record with its match candidates. When `search` is provided, the caller's fields
	/// take precedence over the media's stored metadata, the search can be restricted to a
	/// single provider, and auto-apply is skipped so the caller can review candidates before
	/// anything is written.
	#[graphql(guard = "PermissionGuard::one(UserPermission::MetadataFetchRecordManage)")]
	async fn fetch_media_metadata(
		&self,
		ctx: &Context<'_>,
		id: ID,
		search: Option<MediaMetadataSearchInput>,
	) -> Result<MetadataFetchRecord> {
		let AuthContext { .. } = ctx.data::<AuthContext>()?;
		let core_ctx = ctx.data::<CoreContext>()?;
		let conn = core_ctx.conn.as_ref();

		let model = media::ModelWithMetadata::find()
			.filter(media::Column::Id.eq(id.to_string()))
			.into_model::<media::ModelWithMetadata>()
			.one(conn)
			.await?
			.ok_or("Media not found")?;

		let encryption_key = core_ctx.get_encryption_key().await?;
		let provider_cache = ProviderClientCache::new(encryption_key);

		let title = search
			.as_ref()
			.and_then(|s| s.title.clone())
			.or_else(|| model.metadata.as_ref().and_then(|m| m.title.clone()))
			.unwrap_or_else(|| model.media.name.clone());

		let author = search.as_ref().and_then(|s| s.author.clone()).or_else(|| {
			model
				.metadata
				.as_ref()
				.and_then(|m| m.writers.clone())
				.and_then(|authors_str| {
					authors_str.split(',').map(|s| s.trim().to_string()).next()
				})
		});

		let isbn = search.as_ref().and_then(|s| s.isbn.clone()).or_else(|| {
			model
				.metadata
				.as_ref()
				.and_then(|m| m.identifier_isbn.clone())
		});

		let year = search.as_ref().and_then(|s| s.year);
		let number = search.as_ref().and_then(|s| s.number.map(|n| n as f32));

		let mut provider_hints = HashMap::new();
		if let Some(ref s) = search {
			if let Some(ref volume_id) = s.comic_vine_volume_id {
				provider_hints
					.insert("comic_vine_volume_id".to_string(), volume_id.clone());
			}
		}

		let limit = search
			.as_ref()
			.and_then(|s| s.limit)
			.map(|l| l.max(0) as u32);
		let provider_filter = search.as_ref().and_then(|s| s.provider);
		let skip_auto_apply = search.is_some();

		stump_core::metadata::provider::fetch_media_metadata(
			conn,
			&model.media.id,
			SearchQuery {
				title,
				author,
				isbn,
				year,
				number,
				limit: limit.or(Some(10)),
				provider_hints,
			},
			&provider_cache,
			provider_filter,
			skip_auto_apply,
		)
		.await?;

		let updated = metadata_fetch_record::Entity::find()
			.filter(metadata_fetch_record::Column::MediaId.eq(model.media.id))
			.one(conn)
			.await?
			.ok_or("Failed to load fetch record after search")?;

		Ok(MetadataFetchRecord::from(updated))
	}

	/// Accept a match candidate and apply it to media metadata
	#[graphql(guard = "PermissionGuard::one(UserPermission::MetadataFetchRecordManage)")]
	async fn accept_media_match(
		&self,
		ctx: &Context<'_>,
		media_id: ID,
		candidate_index: u32,
		strategy: Option<MergeStrategy>,
		exclude_fields: Option<Vec<MetadataField>>,
		overrides: Option<Vec<MetadataFieldOverride>>,
	) -> Result<MetadataFetchRecord> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let strategy = strategy.unwrap_or(MergeStrategy::FillGaps);
		let exclude_fields = exclude_fields.unwrap_or_default();
		let overrides = overrides.unwrap_or_default();

		let status = metadata_fetch_record::Entity::find()
			.filter(metadata_fetch_record::Column::MediaId.eq(media_id.to_string()))
			.one(conn)
			.await?
			.ok_or("No fetch status found for this media")?;

		if status.status != MetadataFetchStatus::AwaitingReview {
			return Err(async_graphql::Error::new(format!(
				"Fetch status is {:?}, expected AwaitingReview",
				status.status
			)));
		}

		let candidates: Vec<MatchCandidate> = status
			.match_candidates
			.as_ref()
			.and_then(|v| serde_json::from_value(v.clone()).ok())
			.unwrap_or_default();

		let candidate = candidates
			.get(candidate_index as usize)
			.ok_or("Candidate index out of bounds")?;

		stump_core::metadata::provider::apply_media_match(
			conn,
			media_id.as_ref(),
			candidate,
			strategy,
			exclude_fields,
			overrides,
		)
		.await?;

		let updated = metadata_fetch_record::Entity::find()
			.filter(metadata_fetch_record::Column::MediaId.eq(media_id.to_string()))
			.one(conn)
			.await?
			.ok_or("Failed to re-fetch status")?;

		Ok(MetadataFetchRecord::from(updated))
	}

	/// Reject the current match candidates for a media item
	#[graphql(guard = "PermissionGuard::one(UserPermission::MetadataFetchRecordManage)")]
	async fn reject_media_match(
		&self,
		ctx: &Context<'_>,
		media_id: ID,
		candidate_index: u32,
	) -> Result<MetadataFetchRecord> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let status = metadata_fetch_record::Entity::find()
			.filter(metadata_fetch_record::Column::MediaId.eq(media_id.to_string()))
			.one(conn)
			.await?
			.ok_or("No fetch status found for this media")?;

		let existing_candidates: Vec<MatchCandidate> = status
			.match_candidates
			.as_ref()
			.and_then(|v| serde_json::from_value(v.clone()).ok())
			.unwrap_or_default();

		if (candidate_index as usize) >= existing_candidates.len() {
			return Err(async_graphql::Error::new("Candidate index out of bounds"));
		}

		let adjusted_candidates = existing_candidates
			.into_iter()
			.enumerate()
			.filter(|(i, _)| *i != candidate_index as usize)
			.map(|(_, c)| c)
			.collect::<Vec<_>>();

		let mut active = status.into_active_model();
		if adjusted_candidates.is_empty() {
			active.status = Set(MetadataFetchStatus::NoMatch);
		}
		active.match_candidates = Set(Some(serde_json::to_value(adjusted_candidates)?));

		let updated = metadata_fetch_record::Entity::update(active)
			.exec(conn)
			.await?;

		Ok(MetadataFetchRecord::from(updated))
	}

	/// Set the locked metadata fields for a media item
	#[graphql(guard = "PermissionGuard::one(UserPermission::EditMetadata)")]
	async fn set_media_locked_fields(
		&self,
		ctx: &Context<'_>,
		media_id: ID,
		locked_fields: Vec<MetadataField>,
	) -> Result<Media> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let model = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::Id.eq(media_id.to_string()))
			.into_model::<media::ModelWithMetadata>()
			.one(conn)
			.await?
			.ok_or("Media not found")?;

		let locked_json = serde_json::to_value(&locked_fields)?;

		let updated_metadata = if let Some(existing) = model.metadata {
			let mut active = existing.into_active_model();
			active.locked_fields = Set(Some(locked_json));
			active.update(conn).await?
		} else {
			let active = media_metadata::ActiveModel {
				media_id: Set(Some(model.media.id.clone())),
				locked_fields: Set(Some(locked_json)),
				..Default::default()
			};
			active.insert(conn).await?
		};

		let model = media::ModelWithMetadata {
			media: model.media,
			metadata: Some(updated_metadata),
		};

		Ok(model.into())
	}
}

/// A book the user can see, with its metadata
async fn find_book(
	core: &stump_core::Ctx,
	user: &AuthUser,
	id: &ID,
) -> Result<media::ModelWithMetadata> {
	Ok(media::ModelWithMetadata::find_for_user(user)
		.filter(media::Column::Id.eq(id.to_string()))
		.into_model::<media::ModelWithMetadata>()
		.one(core.conn.as_ref())
		.await?
		.ok_or("Media not found")?)
}

async fn edit_result(
	core: &stump_core::Ctx,
	user: &AuthUser,
	saved: SavedEdit,
) -> Result<MetadataEditResult> {
	let media = find_book(core, user, &ID::from(saved.book.id.clone())).await?;
	Ok(MetadataEditResult {
		media: media.into(),
		file: MetadataEditFile::from(saved.outcome),
		save_id: saved.save_id,
		skipped_fields: saved.skipped,
	})
}
