use async_graphql::{Context, Object, Result};
use models::services::reading_history;

use crate::{
	data::{AuthContext, CoreContext},
	object::reading_history::ReadingPrivacy,
};

/// The mutations of the *My reading* page
#[derive(Default)]
pub struct ReadingHistoryMutation;

#[Object]
impl ReadingHistoryMutation {
	/// Lets everyone on the server see what the authenticated user is reading now and has
	/// finished, or stops it (the next request of anyone else gets nothing). It changes the
	/// authenticated user's own setting and nobody else's: there is no way to name another user.
	/// A person decides it for themselves, so an API key can't
	async fn update_my_reading_privacy(
		&self,
		ctx: &Context<'_>,
		share_reading_history: bool,
	) -> Result<ReadingPrivacy> {
		let AuthContext { user, api_key } = ctx.data::<AuthContext>()?;
		if api_key.is_some() {
			return Err("An API key can't change who sees a person's reading".into());
		}
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let settings =
			reading_history::set_sharing_history(conn, &user.id, share_reading_history)
				.await?;

		Ok(ReadingPrivacy {
			share_reading_history: settings.share_reading_history,
		})
	}
}
