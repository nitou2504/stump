use async_graphql::{Context, Object, Result, ID};
use models::services::reading_history;

use crate::{
	data::{AuthContext, CoreContext},
	object::{
		reader_profile::ReaderProfile,
		reading_history::{ReadingHistory, ReadingPrivacy, READER_NOT_FOUND},
	},
	pagination::MAX_LIST_SIZE,
};

/// The queries of the *My reading* page: a person's reading history, and who lets the others see
/// theirs
#[derive(Default)]
pub struct ReadingHistoryQuery;

#[Object]
impl ReadingHistoryQuery {
	/// What the authenticated user lets the other people on the server see of their reading. It
	/// is nothing until they say so
	async fn my_reading_privacy(&self, ctx: &Context<'_>) -> Result<ReadingPrivacy> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		Ok(ReadingPrivacy {
			share_reading_history: reading_history::is_sharing_history(conn, &user.id)
				.await?,
		})
	}

	/// The people, other than the authenticated user, who let everyone on the server see what
	/// they are reading and have finished, by name. People who were deleted or locked are left
	/// out
	#[graphql(complexity = "crate::complexity::RELATED_LIST * child_complexity")]
	async fn readers_sharing_history(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<ReaderProfile>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		Ok(
			reading_history::readers_sharing_history(conn, user, MAX_LIST_SIZE)
				.await?
				.into_iter()
				.map(ReaderProfile::from)
				.collect(),
		)
	}

	/// The reading history of a person: the authenticated user's own when `userId` is left out,
	/// or the history of somebody who lets everyone see it. Everything in it is what the
	/// authenticated user may see: the books they may read, and nothing else.
	///
	/// # Returns
	///
	/// The history. If the person doesn't exist, never shared theirs, stopped sharing, or was
	/// deleted or locked, the same error is returned for all of them. Nobody can read a history
	/// that isn't shared, the server owner included.
	async fn reading_history(
		&self,
		ctx: &Context<'_>,
		user_id: Option<ID>,
	) -> Result<ReadingHistory> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let reader_id = user_id.map(String::from).unwrap_or_else(|| user.id.clone());
		let reader = reading_history::viewable_reader(conn, user, &reader_id)
			.await?
			.ok_or(READER_NOT_FOUND)?;

		Ok(ReadingHistory::new(
			ReaderProfile::from(reader),
			reader_id == user.id,
		))
	}
}
