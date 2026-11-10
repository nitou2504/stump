use async_graphql::{Context, Object, Result};
use models::entity::{
	media::{self, MediaIdentSelect},
	reading_session,
};
use sea_orm::prelude::*;

use crate::{
	data::{AuthContext, CoreContext},
	object::reading_session::ReadingSession,
};

#[derive(Default)]
pub struct ReadingSessionQuery;

#[Object]
impl ReadingSessionQuery {
	/// Finds one of your own reading sessions by its ID. A reading session is its reader's: nobody
	/// else gets it, not through a shared book club and not with its ID, and it isn't returned
	/// either once its book is no longer visible to you (its library was hidden from you, it is
	/// above your age restriction, or it was removed), the way the book itself wouldn't be.
	async fn reading_session_by_id(
		&self,
		ctx: &Context<'_>,
		id: i32,
	) -> Result<Option<ReadingSession>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let Some(session) = reading_session::Entity::find_by_id(id)
			.filter(reading_session::Column::UserId.eq(user.id.clone()))
			.one(conn)
			.await?
		else {
			return Ok(None);
		};

		let book_is_visible =
			media::Entity::find_media_ids_for_user(session.media_id.clone(), user)
				.into_model::<MediaIdentSelect>()
				.one(conn)
				.await?
				.is_some();

		if !book_is_visible {
			return Ok(None);
		}

		Ok(Some(ReadingSession::from(session)))
	}
}
