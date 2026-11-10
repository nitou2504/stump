use crate::{
	data::{AuthContext, CoreContext, ServiceContext},
	input::smart_lists::SmartListFilterGroupInput,
	loader::reader_profile::ReaderProfileLoader,
	object::{
		media::Media, reader_profile::ReaderProfile, smart_list_item::SmartListItems,
		smart_list_view::SmartListView,
	},
	pagination::list_limit,
	query::smart_lists_builder::{
		build_smart_list_items, smart_list_books, smart_list_meta,
	},
};
use async_graphql::{
	dataloader::DataLoader, ComplexObject, Context, Result, SimpleObject,
};
use models::{
	entity::{media, smart_list, smart_list_view},
	shared::image::ImageRef,
};
use sea_orm::{QueryOrder, QuerySelect, TransactionTrait};

#[derive(Debug, SimpleObject)]
pub struct SmartListMeta {
	pub matched_books: i64,
	pub matched_series: i64,
	pub matched_libraries: i64,
}

#[derive(Debug, SimpleObject)]
#[graphql(complex)]
pub struct SmartList {
	#[graphql(flatten)]
	pub model: smart_list::Model,
}

impl From<smart_list::Model> for SmartList {
	fn from(entity: smart_list::Model) -> Self {
		Self { model: entity }
	}
}

#[ComplexObject]
impl SmartList {
	/// The list's rules, as JSON. Only the user who made the list gets them: they name paths,
	/// libraries and tags, so they would show people a list is made of things they may not see.
	/// Anyone else, the server owner too, gets `[]`: what the list holds for them is what
	/// `books` and `items` give
	async fn filters(&self, ctx: &Context<'_>) -> Result<String> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		if !self.model.is_editable_by(user) {
			return Ok("[]".to_string());
		}

		let filters: Vec<SmartListFilterGroupInput> =
			serde_json::from_slice(&self.model.filters)?;
		Ok(serde_json::to_string(&filters)?)
	}

	/// Whether the user made the list, and so may change it (its rules, its name, who sees it,
	/// its saved views) and delete it. Anyone else sees it read-only (the server owner can only
	/// take a public list of someone else back to private, or delete it)
	async fn can_edit(&self, ctx: &Context<'_>) -> Result<bool> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		Ok(self.model.is_editable_by(user))
	}

	/// Who made the list: their name and avatar. None when they can't be shown any more (a
	/// deleted or locked account's lists aren't listed at all)
	async fn creator(&self, ctx: &Context<'_>) -> Result<Option<ReaderProfile>> {
		let loader = ctx.data::<DataLoader<ReaderProfileLoader>>()?;
		Ok(loader.load_one(self.model.creator_id.clone()).await?)
	}

	// TODO(thumb-placeholders): We need to refactor how non-book thumbs are handled so we can pull
	// dimensions/metadata here.
	async fn thumbnail(&self, ctx: &Context<'_>) -> Result<ImageRef> {
		let service = ctx.data::<ServiceContext>()?;
		Ok(ImageRef {
			// FIXME(graphql): Make thumbnails endpoint
			url: service
				.format_url(format!("/api/v2/smart-lists/{}/thumbnail", self.model.id)),
			..Default::default()
		})
	}

	async fn views(&self, ctx: &Context<'_>) -> Result<Vec<SmartListView>> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let views = smart_list_view::Entity::find_by_list_id(&self.model.id)
			.all(conn)
			.await?;
		views
			.into_iter()
			.map(SmartListView::try_from)
			.collect::<Result<Vec<_>, _>>()
	}

	#[graphql(complexity = "crate::complexity::list_cost(limit, child_complexity)")]
	async fn books(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] limit: Option<u64>,
	) -> Result<Vec<Media>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let smart_list =
			smart_list::Entity::find_by_id(user, self.model.id.clone().into())
				.one(conn)
				.await?
				.ok_or("Smart list not found".to_string())?;

		// the list's cached members, narrowed to what the user may read. At most MAX_LIST_SIZE
		// books, also without a `limit`, in the same order every time
		let books_query = smart_list_books(conn, user, &smart_list)
			.await?
			.order_by_asc(media::Column::Id)
			.limit(list_limit(limit));

		let models = books_query
			.into_model::<media::ModelWithMetadata>()
			.all(conn)
			.await?;

		let books: Vec<Media> = models.into_iter().map(Media::from).collect();

		Ok(books)
	}

	#[graphql(complexity = "crate::complexity::list_cost(limit, child_complexity)")]
	async fn items(
		&self,
		ctx: &Context<'_>,
		#[graphql(default)] limit: Option<u64>,
	) -> Result<SmartListItems> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let smart_list =
			smart_list::Entity::find_by_id(user, self.model.id.clone().into())
				.one(conn)
				.await?
				.ok_or("Smart list not found".to_string())?;

		// looked at before the transaction starts (see `smartListItems`)
		let books_query = smart_list_books(conn, user, &smart_list)
			.await?
			.order_by_asc(media::Column::Id)
			.limit(list_limit(limit));

		let txn = conn.begin().await?;
		let models = books_query
			.into_model::<media::ModelWithMetadata>()
			.all(&txn)
			.await?;

		let books: Vec<Media> = models.into_iter().map(Media::from).collect();
		let items =
			build_smart_list_items(user, smart_list.default_grouping, books, &txn)
				.await?;

		txn.commit().await?;

		Ok(items)
	}

	/// How many books, series and libraries the list holds for the user, counted by the database
	/// in one query over the list's cached members
	async fn meta(&self, ctx: &Context<'_>) -> Result<SmartListMeta> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let smart_list =
			smart_list::Entity::find_by_id(user, self.model.id.clone().into())
				.one(conn)
				.await?
				.ok_or("Smart list not found".to_string())?;

		let books = smart_list_books(conn, user, &smart_list).await?;
		smart_list_meta(conn, books).await
	}
}
