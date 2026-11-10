use async_graphql::dataloader::Loader;
use models::entity::{media, user};
use sea_orm::{ColumnTrait, DatabaseConnection, QueryFilter, QuerySelect};
use std::{collections::HashMap, sync::Arc};

pub struct SeriesCountLoader {
	conn: Arc<DatabaseConnection>,
}

/// A loader for optimizing the loading of series counts for media items.
impl SeriesCountLoader {
	pub fn new(conn: Arc<DatabaseConnection>) -> Self {
		Self { conn }
	}
}

/// The series to count the books of, as seen by a user: the books in a library that is hidden from
/// them, above their age restriction or removed are not counted
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SeriesCountLoaderKey {
	pub user_id: String,
	pub series_id: String,
}

impl Loader<SeriesCountLoaderKey> for SeriesCountLoader {
	type Value = i64;
	type Error = Arc<sea_orm::error::DbErr>;

	async fn load(
		&self,
		keys: &[SeriesCountLoaderKey],
	) -> Result<HashMap<SeriesCountLoaderKey, Self::Value>, Self::Error> {
		let mut grouped: HashMap<String, Vec<&SeriesCountLoaderKey>> = HashMap::new();
		for key in keys {
			grouped.entry(key.user_id.clone()).or_default().push(key);
		}

		let login_users = user::LoginUser::find()
			.filter(user::Column::Id.is_in(grouped.keys().cloned().collect::<Vec<_>>()))
			.into_model::<user::LoginUser>()
			.all(self.conn.as_ref())
			.await?;
		// ^ realistically we'll only have one user

		let mut result = HashMap::new();
		for login_user in login_users {
			let Some(user_keys) = grouped.remove(&login_user.id) else {
				continue;
			};
			let auth_user = user::AuthUser::from(login_user);

			let series_counts = media::Entity::find_for_user(&auth_user)
				.filter(
					media::Column::SeriesId.is_in(
						user_keys
							.iter()
							.map(|k| k.series_id.clone())
							.collect::<Vec<_>>(),
					),
				)
				.select_only()
				.column(media::Column::SeriesId)
				.column_as(media::Column::Id.count(), "count")
				.group_by(media::Column::SeriesId)
				.into_tuple::<(String, i64)>()
				.all(self.conn.as_ref())
				.await?;

			for (series_id, count) in series_counts {
				result.insert(
					SeriesCountLoaderKey {
						user_id: auth_user.id.clone(),
						series_id,
					},
					count,
				);
			}
		}

		Ok(result)
	}
}
