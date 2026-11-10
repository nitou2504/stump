use std::collections::HashMap;

use async_graphql::{
	dataloader::DataLoader, ComplexObject, Context, Result, SimpleObject,
};

use models::{
	entity::{library, media, media_metadata, reading_session, series, series_tag, tag},
	shared::{
		enums::{ReadingStatus, UserPermission},
		image::ImageRef,
	},
};
use sea_orm::{
	prelude::*,
	sea_query::{Expr, Query},
	Condition, ExprTrait, JoinType, QuerySelect, QueryTrait,
};

use crate::{
	data::{AuthContext, CoreContext, ServiceContext},
	loader::{
		favorite::{FavoriteSeriesLoaderKey, FavoritesLoader},
		series_count::{SeriesCountLoader, SeriesCountLoaderKey},
		series_finished_count::{FinishedCountLoaderKey, SeriesFinishedCountLoader},
		series_thumbnail_media::{
			SeriesThumbnailMediaLoader, SeriesThumbnailMediaLoaderKey,
		},
	},
	object::{series_metadata::SeriesMetadata, stats::SeriesStats},
	order::series_position_spec,
	pagination::list_limit,
	utils::letter_counts,
};

use super::{
	library::Library,
	media::Media,
	server_paths::{only_for_those_who_may, path_for_requester},
	tag::Tag,
};

#[derive(Clone, Debug, SimpleObject)]
#[graphql(complex)]
pub struct Series {
	#[graphql(flatten)]
	pub model: series::Model,
	pub metadata: Option<SeriesMetadata>,
}

impl From<series::ModelWithMetadata> for Series {
	fn from(entity: series::ModelWithMetadata) -> Self {
		Self {
			model: entity.series,
			metadata: entity.metadata.map(SeriesMetadata::from),
		}
	}
}

#[ComplexObject]
impl Series {
	/// The path of the series' folder on disk (none for a series made from the books' metadata).
	/// Whoever manages libraries (or browses the server's files) gets the whole path; everyone
	/// else the path inside the library
	async fn path(&self, ctx: &Context<'_>) -> Result<Option<String>> {
		match &self.model.path {
			Some(path) => path_for_requester(ctx, self.model.library_id.as_deref(), path)
				.await
				.map(Some),
			None => Ok(None),
		}
	}

	/// The path to the thumbnail image of the series on disk. Only for whoever manages libraries
	/// (or browses the server's files)
	async fn thumbnail_path(&self, ctx: &Context<'_>) -> Result<Option<String>> {
		only_for_those_who_may(ctx, self.model.thumbnail_path.clone())
	}

	async fn is_favorite(&self, ctx: &Context<'_>) -> Result<bool> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let loader = ctx.data::<DataLoader<FavoritesLoader>>()?;

		let is_favorite = loader
			.load_one(FavoriteSeriesLoaderKey {
				user_id: user.id.clone(),
				series_id: self.model.id.clone(),
			})
			.await?;

		Ok(is_favorite.unwrap_or(false))
	}

	async fn resolved_name(&self) -> String {
		self.metadata
			.as_ref()
			.and_then(|m| m.model.title.clone())
			.unwrap_or_else(|| self.model.name.clone())
	}

	async fn resolved_description(&self) -> Option<String> {
		self.metadata
			.as_ref()
			.and_then(|m| m.model.summary.clone())
			.or_else(|| self.model.description.clone())
	}

	async fn library(&self, ctx: &Context<'_>) -> Result<Library> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let library_id = self.model.library_id.clone().ok_or("Library ID not set")?;
		let model = library::Entity::find()
			.filter(library::Column::Id.eq(library_id))
			.one(conn)
			.await?
			.ok_or("Library not found")?;

		Ok(Library::from(model))
	}

	async fn oneshot_book(&self, ctx: &Context<'_>) -> Result<Option<Media>> {
		if !self.model.is_oneshot {
			return Ok(None);
		}

		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		// only if the user may see the book (a one-shot above their age restriction isn't given away)
		let model = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::SeriesId.eq(self.model.id.clone()))
			.filter(media::Column::IsOneshot.eq(true))
			.into_model::<media::ModelWithMetadata>()
			.one(conn)
			.await?;

		Ok(model.map(Media::from))
	}

	// TODO(perf): We probably could put this behind a dataloader if used frequently
	// At most MAX_LIST_SIZE books (200), also without `take`: page with `take` and `skip`
	/// Get media in this series
	#[graphql(complexity = "crate::complexity::list_cost(take, child_complexity)")]
	async fn media(
		&self,
		ctx: &Context<'_>,
		#[graphql(default, validator(minimum = 1))] take: Option<u64>,
		#[graphql(default, validator(minimum = 0))] skip: Option<u64>,
	) -> Result<Vec<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let models = media::Entity::order_in_series(
			media::ModelWithMetadata::find_for_user(user)
				.filter(media::Column::SeriesId.eq(self.model.id.clone())),
		)
		.limit(list_limit(take))
		.apply_if(skip, |query, skip| query.offset(skip))
		.into_model::<media::ModelWithMetadata>()
		.all(conn)
		.await?;

		Ok(models.into_iter().map(Media::from).collect())
	}

	// Note: originally i did this (kinda) client-side, where i just skipped the first
	// and assumed the others were good. the problem i observed was it wasn't perfect,
	// a few instances where we'd have duplicated thumbs in the stack

	/// Thumbnails for a stacked series card, in order of:
	///
	/// 1. the series thumbnail (e.g., if manually set)
	/// 2. the first `take` amount of books, excluding the one which the thumbnail
	///    was derived from
	async fn stacked_thumbnails(
		&self,
		ctx: &Context<'_>,
		#[graphql(default = 3, validator(minimum = 1, maximum = 5))] take: usize,
	) -> Result<Vec<ImageRef>> {
		let mut thumbnails = vec![self.thumbnail(ctx).await?];

		if take == 1 {
			return Ok(thumbnails);
		}

		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let loader = ctx.data::<DataLoader<SeriesThumbnailMediaLoader>>()?;

		let skip = usize::from(self.model.thumbnail_path.is_none());
		let books = loader
			.load_one(SeriesThumbnailMediaLoaderKey {
				user_id: user.id.clone(),
				series_id: self.model.id.clone(),
				limit: take - 1 + skip,
			})
			.await?
			.unwrap_or_default();

		for book in books.iter().skip(skip) {
			thumbnails.push(book.thumbnail(ctx).await?);
		}

		Ok(thumbnails)
	}

	/// How many books of the series the user can see
	async fn media_count(&self, ctx: &Context<'_>) -> Result<i64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let loader = ctx.data::<DataLoader<SeriesCountLoader>>()?;
		let media_count = loader
			.load_one(SeriesCountLoaderKey {
				user_id: user.id.clone(),
				series_id: self.model.id.clone(),
			})
			.await?
			.unwrap_or(0i64);

		Ok(media_count)
	}

	async fn media_alphabet(&self, ctx: &Context<'_>) -> Result<HashMap<String, i64>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		letter_counts(
			conn,
			media::Entity::find_for_user(user)
				.filter(media::Column::SeriesId.eq(self.model.id.clone())),
			"COALESCE(media_metadata.title, media.name)",
			"media.id",
		)
		.await
	}

	#[graphql(complexity = "crate::complexity::list_cost(Some(take), child_complexity)")]
	async fn up_next(
		&self,
		ctx: &Context<'_>,
		#[graphql(default = 1, validator(minimum = 1))] take: u64,
		cursor: Option<String>,
	) -> Result<Vec<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let user_id = user.id.clone();
		let newer_exists = reading_session::Entity::newer_session_exists_subquery();
		let latest_only = Expr::expr(Expr::exists(newer_exists)).not();

		let spec = series_position_spec();

		let mut query = media::ModelWithMetadata::find_for_user(user)
			.join_rev(
				JoinType::LeftJoin,
				reading_session::Entity::belongs_to(media::Entity)
					.from(reading_session::Column::MediaId)
					.to(media::Column::Id)
					.on_condition(move |_left, _right| {
						Condition::all()
							.add(reading_session::Column::UserId.eq(user_id.clone()))
					})
					.into(),
			)
			.filter(media::Column::SeriesId.eq(self.model.id.clone()))
			// We only want to consider media that the user hasn't started or is in progress
			.filter(
				Condition::any()
					// not started
					.add(reading_session::Column::Id.is_null())
					// in progress (latest is reading + no newer sessions)
					.add(
						Condition::all()
							.add(
								reading_session::Column::Status
									.eq(ReadingStatus::Reading),
							)
							.add(latest_only.clone()),
					),
			)
			.group_by(media::Column::Id)
			.group_by(media_metadata::Column::Id); // pgsql requires addtl grouping

		if let Some(id) = cursor {
			let cursor_row = media::ModelWithMetadata::find_by_id_for_user(id, user)
				.filter(media::Column::SeriesId.eq(self.model.id.clone()))
				.into_model::<media::ModelWithMetadata>()
				.one(conn)
				.await?
				.ok_or("Cursor not found")?;
			let number = match &cursor_row.metadata {
				Some(metadata) => metadata.get(media_metadata::Column::Number),
				None => Option::<Decimal>::None.into(),
			};
			query = query.filter(spec.sorts_after(&[
				number,
				cursor_row.media.get(media::Column::Name),
				cursor_row.media.get(media::Column::Id),
			]));
		}

		let books = spec
			.apply(query)
			.limit(list_limit(Some(take)))
			.into_model::<media::ModelWithMetadata>()
			.all(conn)
			.await?;

		Ok(books.into_iter().map(Media::from).collect())
	}

	async fn is_complete(&self, ctx: &Context<'_>) -> Result<bool> {
		let (media_count, finished_count) =
			get_series_progress(ctx, self.model.id.clone()).await?;

		Ok(finished_count >= media_count)
	}

	// TODO: support this for series
	// async fn last_completed_at(
	// 	&self,
	// 	ctx: &Context<'_>,
	// ) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
	// 	let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
	// 	let conn = ctx.data::<CoreContext>()?.conn.as_ref();

	// 	// i think we need a loader for this, but that does:
	// 	// get the media_count, finished_count, and the most recent finished_at all in one go
	// 	// the tricky part here is that it doesn't account for rereads well, so if i complete
	// 	// a series and then reread a random book in the middle months later, it would
	// 	// update the last_completed_at to the most recent reread completion
	// 	// this requires more thought, but leaving here as a reminder
	// }

	async fn percentage_completed(&self, ctx: &Context<'_>) -> Result<f32> {
		let (media_count, finished_count) =
			get_series_progress(ctx, self.model.id.clone()).await?;

		if media_count == 0 {
			return Ok(0.0);
		}

		let percentage = (finished_count as f32 / media_count as f32) * 100.0;

		Ok(percentage)
	}

	async fn read_count(&self, ctx: &Context<'_>) -> Result<i64> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let finished_loader = ctx.data::<DataLoader<SeriesFinishedCountLoader>>()?;
		let finished_count = finished_loader
			.load_one(FinishedCountLoaderKey {
				user_id: user.id.clone(),
				series_id: self.model.id.clone(),
			})
			.await?
			.unwrap_or(0i64);

		Ok(finished_count)
	}

	async fn unread_count(&self, ctx: &Context<'_>) -> Result<i64> {
		let (media_count, finished_count) =
			get_series_progress(ctx, self.model.id.clone()).await?;

		Ok(std::cmp::max(0, media_count - finished_count))
	}

	async fn tags(&self, ctx: &Context<'_>) -> Result<Vec<Tag>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let models = tag::Entity::find()
			.filter(
				tag::Column::Id.in_subquery(
					Query::select()
						.column(series_tag::Column::TagId)
						.from(series_tag::Entity)
						.and_where(series_tag::Column::SeriesId.eq(self.model.id.clone()))
						.to_owned(),
				),
			)
			.all(conn)
			.await?;

		Ok(models.into_iter().map(Tag::from).collect())
	}

	/// A reference to the thumbnail image for the thumbnail. This will be a fully
	/// qualified URL to the image.
	async fn thumbnail(&self, ctx: &Context<'_>) -> Result<ImageRef> {
		let service = ctx.data::<ServiceContext>()?;
		let last_modified = self.model.updated_at;

		let dimensions = self
			.model
			.thumbnail_meta
			.as_ref()
			.and_then(|meta| meta.dimensions.as_ref())
			.map(|dim| (dim.width, dim.height));

		Ok(ImageRef {
			url: service.cache_friendly_url(
				format!("/api/v2/series/{}/thumbnail", self.model.id),
				&last_modified,
			),
			height: dimensions.as_ref().map(|dim| dim.1),
			width: dimensions.as_ref().map(|dim| dim.0),
			metadata: self.model.thumbnail_meta.clone(),
			last_modified,
		})
	}

	/// The reading stats of the series' books the user can see: their own, or with `allUsers`
	/// everyone's (for users who may read users)
	async fn stats(
		&self,
		ctx: &Context<'_>,
		all_users: Option<bool>,
	) -> Result<SeriesStats> {
		let auth = ctx.data::<AuthContext>()?;
		let AuthContext { user, .. } = auth;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let for_all_users = all_users.unwrap_or(false);
		if for_all_users {
			auth.enforce_permissions(&[UserPermission::ReadUsers])?;
		}

		let stats =
			SeriesStats::fetch(conn, self.model.id.clone(), user, for_all_users).await?;

		Ok(stats)
	}
}

async fn get_series_progress(ctx: &Context<'_>, series_id: String) -> Result<(i64, i64)> {
	let AuthContext { user, .. } = ctx.data::<AuthContext>()?;

	let loader = ctx.data::<DataLoader<SeriesCountLoader>>()?;
	let media_count = loader
		.load_one(SeriesCountLoaderKey {
			user_id: user.id.clone(),
			series_id: series_id.clone(),
		})
		.await?
		.unwrap_or(0i64);

	let finished_loader = ctx.data::<DataLoader<SeriesFinishedCountLoader>>()?;
	let finished_count = finished_loader
		.load_one(FinishedCountLoaderKey {
			user_id: user.id.clone(),
			series_id,
		})
		.await?
		.unwrap_or(0i64);

	Ok((media_count, finished_count))
}
