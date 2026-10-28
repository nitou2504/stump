use crate::{
	data::{AuthContext, CoreContext},
	object::media_metadata_overview::MediaMetadataOverview,
	query::smart_lists_builder::smart_list_media_ids,
};
use async_graphql::{Context, Object, Result, ID};
use models::{entity::smart_list, shared::enums::UserPermission};

#[derive(Default)]
pub struct MediaMetadataOverviewQuery;

#[Object]
impl MediaMetadataOverviewQuery {
	/// The distinct metadata values (genres, writers, ...) of the books visible to the
	/// user, optionally restricted to a series or to the books of a smart list
	async fn media_metadata_overview(
		&self,
		ctx: &Context<'_>,
		series_id: Option<ID>,
		smart_list_id: Option<ID>,
	) -> Result<MediaMetadataOverview> {
		let media_ids = match smart_list_id {
			Some(id) => {
				let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
				if !user.has_permission(UserPermission::AccessSmartList) {
					return Err("You do not have permission to access smart lists".into());
				}
				let conn = ctx.data::<CoreContext>()?.conn.as_ref();
				let list = smart_list::Entity::find_by_id(user, id)
					.one(conn)
					.await?
					.ok_or("Smart list not found")?;
				Some(smart_list_media_ids(user, &list)?)
			},
			None => None,
		};
		Ok(MediaMetadataOverview {
			series_id: series_id.map(|id| id.to_string()),
			media_ids,
		})
	}
}
