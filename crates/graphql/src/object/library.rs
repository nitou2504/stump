use std::collections::{HashMap, HashSet};

use async_graphql::{
	dataloader::DataLoader, ComplexObject, Context, Result, SimpleObject,
};

use models::{
	entity::{
		library, library_config, library_exclusion, library_scan_record, library_tag,
		media, media_metadata, series, tag, user,
	},
	shared::{enums::UserPermission, image::ImageRef, ordering::OrderDirection},
};
use sea_orm::{prelude::*, sea_query::Query, QueryOrder, QuerySelect, QueryTrait};

use crate::{
	data::{AuthContext, CoreContext, ServiceContext},
	guard::PermissionGuard,
	loader::favorite::{FavoriteLibraryLoaderKey, FavoritesLoader},
	object::{library_scan_record::LibraryScanRecord, media::Media, stats::LibraryStats},
	pagination::list_limit,
	utils::letter_counts,
};

use super::{
	author::Author,
	library_config::LibraryConfig,
	series::Series,
	server_paths::{only_for_those_who_may, requester_may_see_server_paths},
	tag::Tag,
	user::User,
};

#[derive(Clone, Debug, SimpleObject)]
#[graphql(complex)]
pub struct Library {
	#[graphql(flatten)]
	pub model: library::Model,
}

impl From<library::Model> for Library {
	fn from(model: library::Model) -> Self {
		Self { model }
	}
}

#[ComplexObject]
impl Library {
	/// The folder of the library on the server's disk. Only for whoever manages libraries (or
	/// browses the server's files); for everyone else it is empty
	async fn path(&self, ctx: &Context<'_>) -> Result<String> {
		Ok(if requester_may_see_server_paths(ctx)? {
			self.model.path.clone()
		} else {
			String::new()
		})
	}

	/// The path to the thumbnail image of the library on disk. Only for whoever manages libraries
	/// (or browses the server's files)
	async fn thumbnail_path(&self, ctx: &Context<'_>) -> Result<Option<String>> {
		only_for_those_who_may(ctx, self.model.thumbnail_path.clone())
	}

	#[graphql(complexity = "crate::complexity::RELATED_LIST * child_complexity")]
	async fn authors(&self, ctx: &Context<'_>) -> Result<Vec<Author>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		// the writers of the books the user can see
		let writers: Vec<String> = media::Entity::find_for_user(user)
			.select_only()
			.column(media_metadata::Column::Writers)
			.distinct()
			.filter(media::Column::LibraryId.eq(self.model.id.clone()))
			.filter(media_metadata::Column::Writers.is_not_null())
			.into_tuple()
			.all(conn)
			.await?;

		let unique_names: std::collections::BTreeSet<String> = writers
			.into_iter()
			.flat_map(|w| {
				w.split(',')
					.map(|s| s.trim().to_string())
					.filter(|s| !s.is_empty())
					.collect::<Vec<String>>()
			})
			.collect();

		let library_id = Some(self.model.id.clone());
		let authors = unique_names
			.into_iter()
			.map(|name| Author {
				name,
				// Note: This is a little fuzzy tbh and feels counterintuitive at a glance. My
				// rationale here is that in the context of a library, an author has no role. We only
				// care about surfacing all authors here.
				role: None,
				library_id: library_id.clone(),
			})
			.collect();

		Ok(authors)
	}

	async fn config(&self, ctx: &Context<'_>) -> Result<LibraryConfig> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let config = library_config::Entity::find()
			.filter(library_config::Column::Id.eq(self.model.config_id))
			.one(conn)
			.await?
			.ok_or("Library config not found")?;

		Ok(config.into())
	}

	#[graphql(
		complexity = "crate::complexity::RELATED_LIST * child_complexity",
		guard = "PermissionGuard::new(&[UserPermission::ReadUsers, UserPermission::ManageLibrary])"
	)]
	async fn excluded_users(&self, ctx: &Context<'_>) -> Result<Vec<User>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let users = user::Entity::find()
			.filter(
				user::Column::Id.in_subquery(
					Query::select()
						.column(library_exclusion::Column::UserId)
						.from(library_exclusion::Entity)
						.and_where(
							library_exclusion::Column::LibraryId
								.eq(self.model.id.clone()),
						)
						.to_owned(),
				),
			)
			.all(conn)
			.await?;

		Ok(users.into_iter().map(User::from).collect())
	}

	async fn is_favorite(&self, ctx: &Context<'_>) -> Result<bool> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let loader = ctx.data::<DataLoader<FavoritesLoader>>()?;

		let is_favorite = loader
			.load_one(FavoriteLibraryLoaderKey {
				user_id: user.id.clone(),
				library_id: self.model.id.clone(),
			})
			.await?;

		Ok(is_favorite.unwrap_or(false))
	}

	/// Get the details of the last scan job for this library, if any exists.
	async fn last_scan(&self, ctx: &Context<'_>) -> Result<Option<LibraryScanRecord>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let record = library_scan_record::Entity::find()
			.filter(library_scan_record::Column::LibraryId.eq(self.model.id.clone()))
			.order_by_desc(library_scan_record::Column::Timestamp)
			.one(conn)
			.await?;

		Ok(record.map(LibraryScanRecord::from))
	}

	// TODO(perf): We probably could put this behind a dataloader if used frequently
	/// Get media in this library
	#[graphql(complexity = "crate::complexity::list_cost(take, child_complexity)")]
	async fn media(
		&self,
		ctx: &Context<'_>,
		#[graphql(default, validator(minimum = 1))] take: Option<u64>,
	) -> Result<Vec<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let models = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::LibraryId.eq(self.model.id.clone()))
			// TODO: Consider allowing custom ordering?
			.order_by_asc(media::Column::Name)
			.limit(list_limit(take))
			.into_model::<media::ModelWithMetadata>()
			.all(conn)
			.await?;

		Ok(models.into_iter().map(Media::from).collect())
	}

	async fn media_alphabet(&self, ctx: &Context<'_>) -> Result<HashMap<String, i64>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		letter_counts(
			conn,
			media::Entity::find_for_user(user)
				.filter(media::Column::LibraryId.eq(self.model.id.clone())),
			"COALESCE(media_metadata.title, media.name)",
			"media.id",
		)
		.await
	}

	/// Get the full history of scan jobs for this library.
	async fn scan_history(&self, ctx: &Context<'_>) -> Result<Vec<LibraryScanRecord>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let records = library_scan_record::Entity::find()
			.filter(library_scan_record::Column::LibraryId.eq(self.model.id.clone()))
			.order_by_desc(library_scan_record::Column::Timestamp)
			.all(conn)
			.await?;

		Ok(records.into_iter().map(LibraryScanRecord::from).collect())
	}

	// TODO(perf): We probably could put this behind a dataloader if used frequently
	/// Get series in this library
	#[graphql(complexity = "crate::complexity::list_cost(take, child_complexity)")]
	async fn series(
		&self,
		ctx: &Context<'_>,
		#[graphql(default, validator(minimum = 1))] take: Option<u64>,
		#[graphql(default, validator(minimum = 0))] skip: Option<u64>,
	) -> Result<Vec<Series>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let models = series::ModelWithMetadata::find_for_user(user)
			.filter(series::Column::LibraryId.eq(Some(self.model.id.clone())))
			// TODO: Consider allowing custom ordering?
			.order_by_asc(series::Column::Name)
			.limit(list_limit(take))
			.apply_if(skip, |query, skip| query.offset(skip))
			.into_model::<series::ModelWithMetadata>()
			.all(conn)
			.await?;

		Ok(models.into_iter().map(Series::from).collect())
	}

	async fn series_alphabet(&self, ctx: &Context<'_>) -> Result<HashMap<String, i64>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		letter_counts(
			conn,
			series::Entity::with_metadata_for_user(
				series::Entity::find_for_user(user),
				user,
			)
			.filter(series::Column::LibraryId.eq(self.model.id.clone())),
			"COALESCE(series_metadata.title, series.name)",
			"series.id",
		)
		.await
	}

	/// The reading stats of the library's books the user can see: their own, or with `allUsers`
	/// everyone's (for users who may read users)
	async fn stats(
		&self,
		ctx: &Context<'_>,
		all_users: Option<bool>,
	) -> Result<LibraryStats> {
		let auth = ctx.data::<AuthContext>()?;
		let AuthContext { user, .. } = auth;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let for_all_users = all_users.unwrap_or(false);
		if for_all_users {
			auth.enforce_permissions(&[UserPermission::ReadUsers])?;
		}

		let stats =
			LibraryStats::fetch(conn, Some(self.model.id.clone()), user, for_all_users)
				.await?;

		Ok(stats)
	}

	async fn genres(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] sort: Option<OrderDirection>,
	) -> Result<Vec<String>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let genre_strings = get_unique_str_list_metadata_fields(
			self,
			user,
			media_metadata::Column::Genres,
			sort.unwrap_or(OrderDirection::Asc),
			conn,
		)
		.await?;

		Ok(genre_strings)
	}

	async fn publishers(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] sort: Option<OrderDirection>,
	) -> Result<Vec<String>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let publisher_strings = get_unique_metadata_fields(
			self,
			user,
			media_metadata::Column::Publisher,
			sort.unwrap_or(OrderDirection::Asc),
			conn,
		)
		.await?;

		Ok(publisher_strings)
	}

	async fn tags(&self, ctx: &Context<'_>) -> Result<Vec<Tag>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let models = tag::Entity::find()
			.filter(
				tag::Column::Id.in_subquery(
					Query::select()
						.column(library_tag::Column::TagId)
						.from(library_tag::Entity)
						.and_where(
							library_tag::Column::LibraryId.eq(self.model.id.clone()),
						)
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
				format!("/api/v2/library/{}/thumbnail", self.model.id),
				&last_modified,
			),
			height: dimensions.map(|(_, height)| height),
			width: dimensions.map(|(width, _)| width),
			metadata: self.model.thumbnail_meta.clone(),
			last_modified,
		})
	}
}

/// The distinct values of `column` over the library's books that `user` can see
async fn get_unique_metadata_fields(
	library: &Library,
	user: &user::AuthUser,
	column: media_metadata::Column,
	sort: OrderDirection,
	conn: &DatabaseConnection,
) -> Result<Vec<String>> {
	let values = media_metadata::Entity::find()
		.select_only()
		.column(column)
		.distinct()
		.filter(column.is_not_null())
		// just a lil inefficient
		.filter(
			media_metadata::Column::MediaId.in_subquery(
				media::Entity::find_for_user(user)
					.select_only()
					.column(media::Column::Id)
					.filter(media::Column::LibraryId.eq(library.model.id.clone()))
					.into_query(),
			),
		)
		.order_by(column, sort.into())
		.into_tuple::<String>()
		.all(conn)
		.await?;

	Ok(values)
}

/// Get unique values from fields which are string arrays
async fn get_unique_str_list_metadata_fields(
	library: &Library,
	user: &user::AuthUser,
	column: media_metadata::Column,
	sort: OrderDirection,
	conn: &DatabaseConnection,
) -> Result<Vec<String>> {
	let csv_list = media_metadata::Entity::find()
		.select_only()
		.column(column)
		.distinct()
		.filter(column.is_not_null())
		// just a lil inefficient
		.filter(
			media_metadata::Column::MediaId.in_subquery(
				media::Entity::find_for_user(user)
					.select_only()
					.column(media::Column::Id)
					.filter(media::Column::LibraryId.eq(library.model.id.clone()))
					.into_query(),
			),
		)
		.into_tuple::<String>()
		.all(conn)
		.await?;

	let mut unique_values = HashSet::new();
	for csv in csv_list {
		for value in csv.split(',') {
			if !value.trim().is_empty() {
				unique_values.insert(value.trim().to_string());
			}
		}
	}

	let sorted_values = {
		let mut vec: Vec<String> = unique_values.into_iter().collect();
		match sort {
			OrderDirection::Asc => vec.sort(),
			OrderDirection::Desc => vec.sort_by(|a, b| b.cmp(a)),
		}
		vec
	};

	Ok(sorted_values)
}
