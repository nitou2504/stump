use std::path::{Path, PathBuf};

use chrono::Utc;
use futures::{stream::FuturesUnordered, StreamExt};
use models::{
	entity::{library, media, series},
	shared::{
		image::ImageMetadata,
		image_processor_options::{ImageProcessorOptions, SupportedImageFormat},
	},
};
use sea_orm::{
	prelude::*, sea_query::Query, ConnectionTrait, EntityTrait, Order, QueryFilter,
	QueryOrder, QuerySelect,
};
use tokio::{fs, task::spawn_blocking};

use crate::{
	config::StumpConfig,
	image::{
		error::ImageProcessorError,
		is_generated_thumbnail,
		process::ImageProcessor,
		processor::{GenericImageProcessor, WebpProcessor},
		thumbnail::{
			no_cover::{is_known_coverless, remember_coverless},
			placeholder::{generate_image_metadata, generate_image_metadata_from_bytes},
		},
		PlaceholderGenerationJob, PlaceholderGenerationOutput, ThumbnailGenerationJob,
		ThumbnailGenerationOutput,
	},
	job::{JobContext, JobExecuteLog, JobTaskOutput},
	media::processor::{error::MediaProcessorError, get_page},
};

/// An error enum for thumbnail generation errors
#[derive(Debug, thiserror::Error)]
pub enum ThumbnailGenerateError {
	#[error("Could not write to disk: {0}")]
	WriteFailed(#[from] std::io::Error),
	#[error("{0}")]
	ProcessorError(#[from] ImageProcessorError),
	#[error("A candidate source for thumbnail generation could not be found")]
	NothingToGenerate,
	#[error("Source thumbnail is missing an extension")]
	SourceMissingExtension,
	#[error("Database error: {0}")]
	DbError(#[from] sea_orm::DbErr),
	#[error("Failed to pull image from file: {0}")]
	ImagePullFailed(String),
	#[error("The book has no cover image")]
	NoCover,
	#[error("Something unexpected went wrong: {0}")]
	Unknown(String),
}

impl From<MediaProcessorError> for ThumbnailGenerateError {
	fn from(error: MediaProcessorError) -> Self {
		match error {
			// A cover is optional in EPUB: such a book keeps the default cover
			MediaProcessorError::CoverNotFound => Self::NoCover,
			error => Self::ImagePullFailed(error.to_string()),
		}
	}
}

/// The options for generating a thumbnail
#[derive(Debug, Clone)]
pub struct GenerateThumbnailOptions {
	pub image_options: ImageProcessorOptions,
	pub core_config: StumpConfig,
	pub force_regen: bool,
}

pub async fn bump_media_thumbnail_fallbacks<C>(
	conn: &C,
	library_id: &str,
	series_id: Option<&str>,
) -> Result<(), DbErr>
where
	C: ConnectionTrait,
{
	let updated_at = Some(DateTimeWithTimeZone::from(Utc::now()));

	if let Some(series_id) = series_id {
		series::Entity::update_many()
			.filter(series::Column::Id.eq(series_id))
			.col_expr(series::Column::UpdatedAt, Expr::value(updated_at))
			.exec(conn)
			.await?;
	}

	library::Entity::update_many()
		.filter(library::Column::Id.eq(library_id))
		.col_expr(library::Column::UpdatedAt, Expr::value(updated_at))
		.exec(conn)
		.await?;

	Ok(())
}

pub async fn bump_series_thumbnail_fallbacks<C>(
	conn: &C,
	series_ids: &[String],
) -> Result<(), DbErr>
where
	C: ConnectionTrait,
{
	if series_ids.is_empty() {
		return Ok(());
	}

	library::Entity::update_many()
		.filter(
			library::Column::Id.in_subquery(
				Query::select()
					.column(series::Column::LibraryId)
					.from(series::Entity)
					.and_where(series::Column::Id.is_in(series_ids.iter().cloned()))
					.to_owned(),
			),
		)
		.col_expr(
			library::Column::UpdatedAt,
			Expr::value(Some(DateTimeWithTimeZone::from(Utc::now()))),
		)
		.exec(conn)
		.await?;

	Ok(())
}

/// A type alias for whether a thumbnail was generated or not during the generation process. This is
/// not indicative of success or failure, but rather whether the thumbnail was newly generated or
/// already existed.
pub type DidGenerate = bool;
/// The output of a thumbnail generation operation
pub type GenerateOutput = (Vec<u8>, PathBuf, DidGenerate);

/// The main function for generating a thumbnail for a book. This should be called from within the
/// scope of a blocking task in the [`generate_thumbnail_from_book`] function.
fn do_generate_book_thumbnail(
	page_data: Vec<u8>,
	file_name: &str,
	config: &StumpConfig,
	options: ImageProcessorOptions,
) -> Result<GenerateOutput, ImageProcessorError> {
	let ext = options.format.extension();

	let thumbnail_path = config
		.thumbnails_directory()
		.join(format!("{}.{ext}", file_name));

	let thumbnail_buffer = match options.format {
		SupportedImageFormat::Webp => WebpProcessor::generate(&page_data, options),
		_ => GenericImageProcessor::generate(&page_data, options),
	}?;

	// Explicitly drop the page data to free memory immediately
	drop(page_data);

	Ok((thumbnail_buffer, thumbnail_path, true))
}

/// Generate a thumbnail for a book, returning the thumbnail data, the path to the thumbnail file,
/// and a boolean indicating whether the thumbnail was generated or not. If the thumbnail already
/// exists and `force_regen` is false, or the thumbnail was explicitly selected or uploaded,
/// the function will return the existing thumbnail data.
#[tracing::instrument(skip_all)]
async fn generate_book_thumbnail(
	book: &media::MediaThumbSelect,
	conn: &DatabaseConnection,
	GenerateThumbnailOptions {
		image_options,
		core_config,
		force_regen,
	}: GenerateThumbnailOptions,
) -> Result<GenerateOutput, ThumbnailGenerateError> {
	let file_name = format!("{}.generated", book.id);
	let file_path = book
		.thumbnail_path
		.as_ref()
		.map(PathBuf::from)
		.unwrap_or_else(|| {
			core_config.thumbnails_directory().join(format!(
				"{}.{}",
				file_name,
				image_options.format.extension()
			))
		});
	let preserve_existing = !force_regen || !is_generated_thumbnail(&file_path);

	if let Err(e) = fs::metadata(&file_path).await {
		// A `NotFound` error is expected here, but anything else is unexpected
		if e.kind() != std::io::ErrorKind::NotFound {
			tracing::error!(error = ?e, "IO error while checking for file existence?");
		}
	} else if preserve_existing {
		match fs::read(&file_path).await {
			Ok(thumbnail) => return Ok((thumbnail, PathBuf::from(&file_path), false)),
			Err(e) => {
				// Realistically, this shouldn't happen if we can grab the metadata, but it isn't a
				// big deal if it does. We can just regenerate the thumbnail in the event something
				// is wrong with the file.
				tracing::error!(error = ?e, "Failed to read thumbnail file from disk! Regenerating...");
			},
		}
	}

	let book_path = Path::new(&book.path);
	if !force_regen && is_known_coverless(&core_config, &book.id, book_path).await {
		tracing::trace!(book_id = %book.id, "Skipping a book known to have no cover image");
		return Err(ThumbnailGenerateError::NoCover);
	}

	let (thumbnail, thumbnail_path, thumbnail_metadata) =
		match generate_thumbnail_from_book(
			&book.path,
			&file_name,
			&core_config,
			image_options,
		)
		.await
		{
			Err(ThumbnailGenerateError::NoCover) => {
				remember_coverless(&core_config, &book.id, book_path).await;
				return Err(ThumbnailGenerateError::NoCover);
			},
			result => result?,
		};

	media::Entity::update_many()
		.filter(media::Column::Id.eq(&book.id))
		.col_expr(
			media::Column::ThumbnailPath,
			Expr::value(Some(thumbnail_path.to_string_lossy().to_string())),
		)
		.col_expr(
			media::Column::ThumbnailMeta,
			Expr::value(thumbnail_metadata),
		)
		.col_expr(
			media::Column::UpdatedAt,
			Expr::value(Some(DateTimeWithTimeZone::from(Utc::now()))),
		)
		.exec(conn)
		.await?;

	Ok((thumbnail, thumbnail_path, true))
}

/// Generate and write an image from a book page. The caller chooses the filename and
/// persists the returned path and metadata on the entity whose thumbnail is changing.
#[tracing::instrument(skip_all)]
pub async fn generate_thumbnail_from_book(
	book_path: &str,
	file_name: &str,
	core_config: &StumpConfig,
	image_options: ImageProcessorOptions,
) -> Result<(Vec<u8>, PathBuf, Option<ImageMetadata>), ThumbnailGenerateError> {
	let page = image_options.page.unwrap_or(1);
	let (_, page_data) = get_page(book_path, page, core_config).await?;

	let file_name_owned = file_name.to_owned();
	let core_config_clone = core_config.clone();
	let generate_result = spawn_blocking(move || {
		do_generate_book_thumbnail(
			page_data,
			&file_name_owned,
			&core_config_clone,
			image_options,
		)
	})
	.await
	.map_err(|e| ThumbnailGenerateError::Unknown(e.to_string()))??;

	let (thumbnail, thumbnail_path, _) = generate_result;
	fs::write(&thumbnail_path, &thumbnail).await?;

	let thumbnail_metadata =
		match generate_image_metadata_from_bytes(thumbnail.clone()).await {
			Ok(metadata) => Some(metadata),
			Err(e) => {
				tracing::error!(error = ?e, "Failed to generate thumbnail metadata");
				None
			},
		};

	Ok((thumbnail, thumbnail_path, thumbnail_metadata))
}

/// Sorts books that already have a thumbnail first, so a series or library takes its cover
/// from a book that has one (e.g. not from a first book without a cover image)
fn has_no_thumbnail() -> Expr {
	media::Column::ThumbnailPath.is_null()
}

/// Copy a book's thumbnail to a target entity (series or library) and update the database.
/// The happy path is assumed to be that the book already has a thumbnail generated, which should
/// be the case from the ordering of operations in the thumbnail generation job, however if not it
/// will attempt to generate the thumbnail from the book as a fallback.
#[tracing::instrument(skip(ctx, update_fn, first_book, options))]
async fn copy_thumbnail_to_entity<E>(
	entity_id: &str,
	first_book: media::MediaThumbSelect,
	ctx: &JobContext,
	options: GenerateThumbnailOptions,
	update_fn: impl FnOnce(String, Option<ImageMetadata>) -> sea_orm::UpdateMany<E>,
) -> Result<GenerateOutput, ThumbnailGenerateError>
where
	E: sea_orm::EntityTrait,
{
	let mut did_regenerate = false;
	let (source_path, thumbnail_data) = match &first_book.thumbnail_path {
		// Note: We don't currently use the data, so might be worth optimizing out in the future.
		Some(path) => (path.clone(), fs::read(path).await.unwrap_or_default()),
		None => {
			// Note that this shouldn't really happen but figured better to handle instead
			let book_path = Path::new(&first_book.path);
			// Not reopened even when forced: a forced job's book pass already retried it,
			// and an unchanged file still has no cover
			if is_known_coverless(&options.core_config, &first_book.id, book_path).await {
				return Err(ThumbnailGenerateError::NoCover);
			}
			let (data, path, _) =
				generate_book_thumbnail(&first_book, ctx.conn(), options.clone()).await?;
			did_regenerate = true;
			(path.to_string_lossy().to_string(), data)
		},
	};

	let source_path = std::path::PathBuf::from(&source_path);
	match fs::metadata(&source_path).await {
		Ok(_) => {},
		Err(error) => {
			tracing::warn!(
				?error,
				book_id = %first_book.id,
				?source_path,
				"Source thumbnail file does not exist or is inaccessible"
			);
			return Err(ThumbnailGenerateError::NothingToGenerate);
		},
	}

	let ext = source_path
		.extension()
		.and_then(|e| e.to_str())
		.ok_or(ThumbnailGenerateError::SourceMissingExtension)?;

	let dest_path = ctx
		.config()
		.thumbnails_directory()
		.join(format!("{}.generated.{}", entity_id, ext));

	fs::copy(&source_path, &dest_path).await?;
	tracing::debug!(
		entity_id,
		?source_path,
		?dest_path,
		"Copied book thumbnail to destination"
	);

	let thumbnail_metadata = match (
		did_regenerate || options.force_regen,
		first_book.thumbnail_meta,
	) {
		(true, _) | (false, None) => match generate_image_metadata(&dest_path).await {
			Ok(metadata) => Some(metadata),
			Err(e) => {
				tracing::error!(error = ?e, "Failed to generate thumbnail metadata");
				None
			},
		},
		(false, Some(meta)) => Some(meta),
	};

	update_fn(dest_path.to_string_lossy().to_string(), thumbnail_metadata)
		.exec(ctx.conn())
		.await?;

	Ok((thumbnail_data, dest_path, true))
}

#[tracing::instrument(skip_all)]
async fn generate_series_thumbnail(
	series: &series::SeriesThumbSelect,
	ctx: &JobContext,
	options: GenerateThumbnailOptions,
) -> Result<GenerateOutput, ThumbnailGenerateError> {
	if let Some(thumbnail_path) = &series.thumbnail_path {
		let preserve_existing =
			!options.force_regen || !is_generated_thumbnail(Path::new(thumbnail_path));

		if preserve_existing {
			match fs::metadata(thumbnail_path).await {
				Ok(_) => {
					tracing::debug!(
						series_id = %series.id,
						?thumbnail_path,
						"Thumbnail already exists, skipping generation"
					);
					let thumbnail_data = fs::read(thumbnail_path).await?;
					return Ok((thumbnail_data, PathBuf::from(thumbnail_path), false));
				},
				Err(error) => {
					tracing::debug!(
						?error,
						series_id = %series.id,
						?thumbnail_path,
						"Thumbnail path exists in DB but file may be missing, regenerating"
					);
				},
			}
		}
	}

	let first_book = media::Entity::find()
		.select_only()
		.columns(media::MediaThumbSelect::columns())
		.filter(media::Column::SeriesId.eq(&series.id))
		.order_by(has_no_thumbnail(), Order::Asc)
		.order_by_asc(media::Column::Name)
		.into_model::<media::MediaThumbSelect>()
		.one(ctx.conn())
		.await?;

	let Some(first_book) = first_book else {
		tracing::warn!(series_id = %series.id, "No books found in series");
		return Err(ThumbnailGenerateError::NothingToGenerate);
	};

	copy_thumbnail_to_entity(
		&series.id,
		first_book,
		ctx,
		options,
		|thumbnail_path, thumbnail_metadata| {
			series::Entity::update_many()
				.filter(series::Column::Id.eq(&series.id))
				.col_expr(
					series::Column::ThumbnailPath,
					Expr::value(Some(thumbnail_path)),
				)
				.col_expr(
					series::Column::ThumbnailMeta,
					Expr::value(thumbnail_metadata),
				)
				.col_expr(
					series::Column::UpdatedAt,
					Expr::value(Some(DateTimeWithTimeZone::from(Utc::now()))),
				)
		},
	)
	.await
}

#[tracing::instrument(skip_all)]
async fn generate_library_thumbnail(
	library: &library::LibraryThumbSelect,
	ctx: &JobContext,
	options: GenerateThumbnailOptions,
) -> Result<GenerateOutput, ThumbnailGenerateError> {
	if let Some(thumbnail_path) = &library.thumbnail_path {
		let preserve_existing =
			!options.force_regen || !is_generated_thumbnail(Path::new(thumbnail_path));

		if preserve_existing {
			match fs::metadata(thumbnail_path).await {
				Ok(_) => {
					tracing::debug!(
						library_id = %library.id,
						?thumbnail_path,
						"Thumbnail already exists, skipping generation"
					);
					let thumbnail_data = fs::read(thumbnail_path).await?;
					return Ok((thumbnail_data, PathBuf::from(thumbnail_path), false));
				},
				Err(error) => {
					tracing::debug!(
						?error,
						library_id = %library.id,
						?thumbnail_path,
						"Thumbnail path exists in DB but file may be missing, regenerating"
					);
				},
			}
		}
	}

	let first_book = media::Entity::find()
		.select_only()
		.columns(media::MediaThumbSelect::columns())
		.left_join(series::Entity)
		.filter(media::Column::LibraryId.eq(&library.id))
		.order_by(has_no_thumbnail(), Order::Asc)
		.order_by_asc(series::Column::Name)
		.order_by_asc(media::Column::Name)
		.into_model::<media::MediaThumbSelect>()
		.one(ctx.conn())
		.await?;

	let Some(first_book) = first_book else {
		tracing::warn!(library_id = %library.id, "No books found in library");
		return Err(ThumbnailGenerateError::NothingToGenerate);
	};

	copy_thumbnail_to_entity(
		&library.id,
		first_book,
		ctx,
		options,
		|thumbnail_path, thumbnail_metadata| {
			library::Entity::update_many()
				.filter(library::Column::Id.eq(&library.id))
				.col_expr(
					library::Column::ThumbnailPath,
					Expr::value(Some(thumbnail_path)),
				)
				.col_expr(
					library::Column::ThumbnailMeta,
					Expr::value(thumbnail_metadata),
				)
				.col_expr(
					library::Column::UpdatedAt,
					Expr::value(Some(DateTimeWithTimeZone::from(Utc::now()))),
				)
		},
	)
	.await
}

pub enum GenerateImageSource {
	Book(media::MediaThumbSelect),
	Series(series::SeriesThumbSelect),
	Library(library::LibraryThumbSelect),
}

#[tracing::instrument(skip_all)]
pub async fn safely_generate_batch(
	sources: Vec<GenerateImageSource>,
	ctx: &JobContext,
	options: GenerateThumbnailOptions,
	reporter: impl Fn(usize),
) -> JobTaskOutput<ThumbnailGenerationJob> {
	let mut output = ThumbnailGenerationOutput::default();
	let mut logs = vec![];

	let max_concurrency = options.core_config.thumbnail_concurrency_limit();
	let batch_size = max_concurrency;
	let total_sources = sources.len();
	tracing::debug!(
		batch_size,
		total_sources,
		"Processing thumbnails in batches"
	);

	let mut processed_count = 0;

	for (chunk_index, chunk) in sources.chunks(batch_size).enumerate() {
		let mut chunk_futures = FuturesUnordered::new();

		tracing::trace!(
			chunk_index,
			chunk_size = chunk.len(),
			"Processing thumbnail generation batch"
		);

		for (source_index, source) in chunk.iter().enumerate() {
			let options = options.clone();
			let path = match source {
				GenerateImageSource::Book(book) => Some(book.path.clone()),
				GenerateImageSource::Series(series) => series.path.clone(),
				GenerateImageSource::Library(library) => Some(library.path.clone()),
			};

			let future = async move {
				tracing::trace!(?path, "(Chunk {chunk_index}, Item {source_index}) Starting thumbnail generation");

				let result = match source {
					GenerateImageSource::Book(book) => {
						generate_book_thumbnail(book, ctx.conn(), options).await
					},
					GenerateImageSource::Series(series) => {
						generate_series_thumbnail(series, ctx, options).await
					},
					GenerateImageSource::Library(library) => {
						generate_library_thumbnail(library, ctx, options).await
					},
				}
				.map(|(_, path, did_generate)| (path, did_generate));

				result.map_err(|e| (e, path))
			};

			chunk_futures.push(future);
		}

		while let Some(gen_output) = chunk_futures.next().await {
			match gen_output {
				Ok((_, did_generate)) => {
					if did_generate {
						output.generated_thumbnails += 1;
					} else {
						output.skipped_files += 1;
					}
				},
				// A book without a cover image keeps the default cover, which isn't a failure
				Err((ThumbnailGenerateError::NoCover, _)) => {
					output.skipped_files += 1;
				},
				Err((error, path)) => {
					logs.push(
						JobExecuteLog::error(format!(
							"Failed to generate thumbnail: {:?}",
							error.to_string()
						))
						.with_ctx(format!("Media path: {path:?}")),
					);
				},
			}

			output.visited_files += 1;
			processed_count += 1;
			reporter(processed_count);
		}

		// TODO: Read up more on this, I added as an attempt to force garbage collection
		// between batches to help with memory usage, but it may not be necessary.
		if processed_count < total_sources {
			tokio::task::yield_now().await;
		}
	}

	JobTaskOutput {
		output,
		logs,
		subtasks: vec![],
	}
}

/// Reads a book's first page (its cover) for placeholder colors. Like thumbnail generation, a
/// book known to have no cover image isn't reopened unless forced, and a new one is remembered
async fn read_book_cover(
	book: &media::MediaThumbSelect,
	config: &StumpConfig,
	force_regen: bool,
) -> Result<Vec<u8>, ThumbnailGenerateError> {
	let book_path = Path::new(&book.path);
	if !force_regen && is_known_coverless(config, &book.id, book_path).await {
		return Err(ThumbnailGenerateError::NoCover);
	}
	match get_page(book_path, 1, config).await {
		Ok((_, data)) => Ok(data),
		Err(MediaProcessorError::CoverNotFound) => {
			remember_coverless(config, &book.id, book_path).await;
			Err(ThumbnailGenerateError::NoCover)
		},
		Err(error) => Err(error.into()),
	}
}

/// Generate placeholder metadata (ImageMetadata) for a book's existing thumbnail
#[tracing::instrument(skip_all)]
pub async fn generate_book_placeholder(
	book: &media::MediaThumbSelect,
	ctx: &JobContext,
	force_regen: bool,
) -> Result<(), ThumbnailGenerateError> {
	// Skip if metadata exists and not forcing regeneration
	if !force_regen && book.thumbnail_meta.is_some() {
		return Ok(());
	}

	// Pull the image data from either the existing thumb on disk or try to pull the full-res image
	// from the book. This way we don't require people to generate thumbnails in order to have good
	// placeholder metadata
	let image_data = match &book.thumbnail_path {
		Some(thumbnail_path) if fs::metadata(&thumbnail_path).await.is_ok() => {
			let path = PathBuf::from(thumbnail_path);
			fs::read(&path).await?
		},
		_ => read_book_cover(book, ctx.config(), force_regen).await?,
	};

	let thumbnail_metadata = match generate_image_metadata_from_bytes(image_data).await {
		Ok(metadata) => Some(metadata),
		Err(e) => {
			tracing::error!(error = ?e, "Failed to generate thumbnail metadata");
			None
		},
	};

	media::Entity::update_many()
		.filter(media::Column::Id.eq(&book.id))
		.col_expr(
			media::Column::ThumbnailMeta,
			Expr::value(thumbnail_metadata),
		)
		.exec(ctx.conn())
		.await?;

	Ok(())
}

async fn get_series_thumbnail_candidate(
	series: &series::SeriesThumbSelect,
	ctx: &JobContext,
) -> Result<Option<Vec<u8>>, ThumbnailGenerateError> {
	let Some(first_book) = media::Entity::find()
		.filter(media::Column::SeriesId.eq(series.id.clone()))
		.order_by(has_no_thumbnail(), Order::Asc)
		.order_by_asc(media::Column::Name)
		.into_model::<media::MediaThumbSelect>()
		.one(ctx.conn())
		.await?
	else {
		tracing::warn!(series_id = %series.id, "No books found in series");
		return Err(ThumbnailGenerateError::NothingToGenerate);
	};

	let image_data = match &first_book.thumbnail_path {
		Some(thumbnail_path) if fs::metadata(&thumbnail_path).await.is_ok() => {
			let path = PathBuf::from(thumbnail_path);
			fs::read(&path).await?
		},
		_ => read_book_cover(&first_book, ctx.config(), false).await?,
	};

	Ok(Some(image_data))
}

/// Generate placeholder metadata for a series's existing thumbnail
#[tracing::instrument(skip_all)]
async fn generate_series_placeholder(
	series: &series::SeriesThumbSelect,
	ctx: &JobContext,
	force_regen: bool,
) -> Result<(), ThumbnailGenerateError> {
	// Skip if metadata exists and not forcing regeneration
	if !force_regen && series.thumbnail_meta.is_some() {
		return Ok(());
	}

	let image_data = match &series.thumbnail_path {
		Some(thumbnail_path) if fs::metadata(&thumbnail_path).await.is_ok() => {
			let path = PathBuf::from(thumbnail_path);
			fs::read(&path).await?
		},
		_ => {
			tracing::debug!(series_id = %series.id, "Pulling thumbnail candidate from first book in series");
			let Some(data) = get_series_thumbnail_candidate(series, ctx).await? else {
				return Err(ThumbnailGenerateError::NothingToGenerate);
			};
			data
		},
	};

	let thumbnail_metadata = match generate_image_metadata_from_bytes(image_data).await {
		Ok(metadata) => Some(metadata),
		Err(e) => {
			tracing::error!(error = ?e, "Failed to generate thumbnail metadata");
			None
		},
	};

	series::Entity::update_many()
		.filter(series::Column::Id.eq(&series.id))
		.col_expr(
			series::Column::ThumbnailMeta,
			Expr::value(thumbnail_metadata),
		)
		.exec(ctx.conn())
		.await?;

	Ok(())
}

async fn get_library_thumbnail_candidate(
	library: &library::LibraryThumbSelect,
	ctx: &JobContext,
) -> Result<Option<Vec<u8>>, ThumbnailGenerateError> {
	let first_book = media::Entity::find()
		.left_join(series::Entity)
		.filter(media::Column::LibraryId.eq(&library.id))
		.order_by(has_no_thumbnail(), Order::Asc)
		.order_by_asc(series::Column::Name)
		.order_by_asc(media::Column::Name)
		.into_model::<media::MediaThumbSelect>()
		.one(ctx.conn())
		.await?;

	let Some(book) = first_book else {
		tracing::warn!(library_id = %library.id, "No books found in library");
		return Err(ThumbnailGenerateError::NothingToGenerate);
	};

	let image_data = match &book.thumbnail_path {
		Some(thumbnail_path) if fs::metadata(&thumbnail_path).await.is_ok() => {
			let path = PathBuf::from(thumbnail_path);
			fs::read(&path).await?
		},
		_ => read_book_cover(&book, ctx.config(), false).await?,
	};

	Ok(Some(image_data))
}

/// Generate placeholder metadata for a library's existing thumbnail
#[tracing::instrument(skip_all)]
async fn generate_library_placeholder(
	library: &library::LibraryThumbSelect,
	ctx: &JobContext,
	force_regen: bool,
) -> Result<(), ThumbnailGenerateError> {
	// Skip if metadata exists and not forcing regeneration
	if !force_regen && library.thumbnail_meta.is_some() {
		return Ok(());
	}

	let image_data = match &library.thumbnail_path {
		Some(thumbnail_path) if fs::metadata(&thumbnail_path).await.is_ok() => {
			let path = PathBuf::from(thumbnail_path);
			fs::read(&path).await?
		},
		_ => {
			tracing::debug!(library_id = %library.id, "Pulling thumbnail candidate from first book in library");
			let Some(data) = get_library_thumbnail_candidate(library, ctx).await? else {
				return Err(ThumbnailGenerateError::NothingToGenerate);
			};
			data
		},
	};

	let thumbnail_metadata = match generate_image_metadata_from_bytes(image_data).await {
		Ok(metadata) => Some(metadata),
		Err(e) => {
			tracing::error!(error = ?e, "Failed to generate thumbnail metadata");
			None
		},
	};

	library::Entity::update_many()
		.filter(library::Column::Id.eq(&library.id))
		.col_expr(
			library::Column::ThumbnailMeta,
			Expr::value(thumbnail_metadata),
		)
		.exec(ctx.conn())
		.await?;

	Ok(())
}

/// Batch generate placeholder metadata for multiple sources with concurrency control
#[tracing::instrument(skip_all)]
pub async fn safely_generate_placeholder_batch(
	sources: Vec<GenerateImageSource>,
	ctx: &JobContext,
	force_regen: bool,
	reporter: impl Fn(usize),
) -> JobTaskOutput<PlaceholderGenerationJob> {
	let mut output = PlaceholderGenerationOutput::default();
	let mut logs = vec![];

	let max_concurrency = ctx.config().cpu_concurrency_limit();
	let batch_size = max_concurrency;
	let total_sources = sources.len();
	tracing::debug!(
		batch_size,
		total_sources,
		"Processing placeholder colors in batches"
	);

	let mut processed_count = 0;

	for (chunk_index, chunk) in sources.chunks(batch_size).enumerate() {
		let mut chunk_futures = FuturesUnordered::new();

		for (source_index, source) in chunk.iter().enumerate() {
			let force_regen = force_regen;
			let path = match source {
				GenerateImageSource::Book(book) => Some(book.path.clone()),
				GenerateImageSource::Series(series) => series.path.clone(),
				GenerateImageSource::Library(library) => Some(library.path.clone()),
			};

			let future = async move {
				tracing::trace!(?path, "(Chunk {chunk_index}, Item {source_index}) Starting placeholder generation");

				let result = match source {
					GenerateImageSource::Book(book) => {
						generate_book_placeholder(book, ctx, force_regen).await
					},
					GenerateImageSource::Series(series) => {
						generate_series_placeholder(series, ctx, force_regen).await
					},
					GenerateImageSource::Library(library) => {
						generate_library_placeholder(library, ctx, force_regen).await
					},
				};

				result.map_err(|e| (e, path))
			};

			chunk_futures.push(future);
		}

		while let Some(gen_output) = chunk_futures.next().await {
			match gen_output {
				Ok(_) => {
					output.generated_metadata += 1;
				},
				Err((error, path)) => match error {
					ThumbnailGenerateError::NothingToGenerate
					| ThumbnailGenerateError::NoCover => {
						output.skipped_entities += 1;
					},
					_ => {
						logs.push(
							JobExecuteLog::error(format!(
								"Failed to generate placeholder metadata: {:?}",
								error.to_string()
							))
							.with_ctx(format!("Media path: {path:?}")),
						);
						output.skipped_entities += 1;
					},
				},
			}

			output.visited_entities += 1;
			processed_count += 1;
			reporter(processed_count);
		}

		// Force garbage collection between batches
		if processed_count < total_sources {
			tokio::task::yield_now().await;
		}
	}

	JobTaskOutput {
		output,
		logs,
		subtasks: vec![],
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::media::fixtures::write_test_epub;
	use ::tests::db::test_database;

	/// A config whose thumbnails live in `dir`, and a book there without a cover image
	fn coverless_book(dir: &Path) -> (StumpConfig, media::MediaThumbSelect) {
		let config = StumpConfig {
			config_dir: dir.to_string_lossy().to_string(),
			..StumpConfig::debug()
		};
		std::fs::create_dir_all(config.thumbnails_directory()).expect("thumbnails dir");
		let path = dir.join("book.epub");
		write_test_epub(&path, "No Cover", "");

		let book = media::MediaThumbSelect {
			id: "book-id".to_string(),
			path: path.to_string_lossy().to_string(),
			library_id: "library-id".to_string(),
			series_id: None,
			thumbnail_path: None,
			thumbnail_meta: None,
		};
		(config, book)
	}

	/// Replaces the book with zeros of the same size and modification time: reopening it
	/// would fail with another error, so a `NoCover` can only come from the marker
	fn break_book_keeping_signature(path: &Path) {
		let metadata = std::fs::metadata(path).expect("book metadata");
		std::fs::write(path, vec![0u8; metadata.len() as usize]).expect("rewrite book");
		set_modified(path, metadata.modified().expect("modification time"));
	}

	fn set_modified(path: &Path, time: std::time::SystemTime) {
		std::fs::File::options()
			.write(true)
			.open(path)
			.and_then(|file| file.set_modified(time))
			.expect("set modification time");
	}

	#[test]
	fn test_missing_cover_is_not_a_failure() {
		assert!(matches!(
			ThumbnailGenerateError::from(MediaProcessorError::CoverNotFound),
			ThumbnailGenerateError::NoCover
		));
		assert!(matches!(
			ThumbnailGenerateError::from(MediaProcessorError::FileNotFound),
			ThumbnailGenerateError::ImagePullFailed(_)
		));
	}

	#[tokio::test]
	async fn test_coverless_book_is_reopened_only_when_forced_or_changed() {
		let temp = tempfile::tempdir().expect("tempdir");
		let (core_config, book) = coverless_book(temp.path());
		let path = PathBuf::from(&book.path);
		let conn = test_database().await;
		let options = GenerateThumbnailOptions {
			image_options: ImageProcessorOptions::default(),
			core_config,
			force_regen: false,
		};

		let first = generate_book_thumbnail(&book, &conn, options.clone()).await;
		assert!(matches!(first, Err(ThumbnailGenerateError::NoCover)));

		break_book_keeping_signature(&path);
		let second = generate_book_thumbnail(&book, &conn, options.clone()).await;
		assert!(matches!(second, Err(ThumbnailGenerateError::NoCover)));

		// A forced regeneration reads the file again
		let forced = generate_book_thumbnail(
			&book,
			&conn,
			GenerateThumbnailOptions {
				force_regen: true,
				..options.clone()
			},
		)
		.await;
		assert!(matches!(
			forced,
			Err(ThumbnailGenerateError::ImagePullFailed(_))
		));

		// So does a changed file
		let modified = std::fs::metadata(&path)
			.and_then(|metadata| metadata.modified())
			.expect("modification time");
		set_modified(&path, modified + std::time::Duration::from_secs(10));
		let changed = generate_book_thumbnail(&book, &conn, options).await;
		assert!(matches!(
			changed,
			Err(ThumbnailGenerateError::ImagePullFailed(_))
		));
	}

	#[tokio::test]
	async fn test_placeholder_read_remembers_a_coverless_book() {
		let temp = tempfile::tempdir().expect("tempdir");
		let (config, book) = coverless_book(temp.path());
		let path = PathBuf::from(&book.path);

		let first = read_book_cover(&book, &config, false).await;
		assert!(matches!(first, Err(ThumbnailGenerateError::NoCover)));
		assert!(is_known_coverless(&config, &book.id, &path).await);

		break_book_keeping_signature(&path);
		let second = read_book_cover(&book, &config, false).await;
		assert!(matches!(second, Err(ThumbnailGenerateError::NoCover)));

		let forced = read_book_cover(&book, &config, true).await;
		assert!(matches!(
			forced,
			Err(ThumbnailGenerateError::ImagePullFailed(_))
		));
	}
}
