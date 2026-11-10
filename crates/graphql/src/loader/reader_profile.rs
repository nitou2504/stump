use std::{collections::HashMap, sync::Arc};

use async_graphql::dataloader::Loader;
use models::entity::user;
use sea_orm::{prelude::*, DatabaseConnection};

use crate::object::reader_profile::ReaderProfile;

/// Loads the public face of users ([`ReaderProfile`]) by their ids, one query for all the ids a
/// request asks for. A user who is deleted or locked has no profile: they are left out of what it
/// loads, and whoever asks for one gets `None`
pub struct ReaderProfileLoader {
	conn: Arc<DatabaseConnection>,
}

impl ReaderProfileLoader {
	pub fn new(conn: Arc<DatabaseConnection>) -> Self {
		Self { conn }
	}
}

impl Loader<String> for ReaderProfileLoader {
	type Value = ReaderProfile;
	type Error = Arc<sea_orm::error::DbErr>;

	async fn load(
		&self,
		keys: &[String],
	) -> Result<HashMap<String, Self::Value>, Self::Error> {
		let users = user::Entity::find()
			.filter(user::Column::Id.is_in(keys.iter().cloned().collect::<Vec<_>>()))
			.filter(user::Column::DeletedAt.is_null())
			.filter(user::Column::IsLocked.eq(false))
			.all(self.conn.as_ref())
			.await?;

		Ok(users
			.into_iter()
			.map(|model| (model.id.clone(), ReaderProfile::from(model)))
			.collect())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use ::tests::{db::test_database, fake_data};
	use async_graphql::dataloader::DataLoader;
	use sea_orm::{ActiveModelTrait, Set};

	#[tokio::test]
	async fn loads_the_people_who_can_be_shown_and_leaves_the_others_out() {
		let db = Arc::new(test_database().await);
		let ana = fake_data::User::new("ana").insert(&db).await;
		let ben = fake_data::User::new("ben").insert(&db).await;
		let zoe = fake_data::User::new("zoe").insert(&db).await;
		let kim = fake_data::User::new("kim").insert(&db).await;
		user::ActiveModel {
			deleted_at: Set(Some(chrono::Utc::now().into())),
			..zoe.clone().into()
		}
		.update(db.as_ref())
		.await
		.unwrap();
		user::ActiveModel {
			is_locked: Set(true),
			..kim.clone().into()
		}
		.update(db.as_ref())
		.await
		.unwrap();

		let loader = DataLoader::new(ReaderProfileLoader::new(db.clone()), tokio::spawn);
		let profiles = loader
			.load_many([
				ana.id.clone(),
				ben.id.clone(),
				zoe.id.clone(),
				kim.id.clone(),
				"nobody".to_string(),
			])
			.await
			.unwrap();

		let mut names = profiles
			.values()
			.map(|profile| profile.username.clone())
			.collect::<Vec<_>>();
		names.sort();
		assert_eq!(names, vec!["ana", "ben"]);
		assert!(!profiles.contains_key(&zoe.id));
		assert!(!profiles.contains_key(&kim.id));
	}
}
