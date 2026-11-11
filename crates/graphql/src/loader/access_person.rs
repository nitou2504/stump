use std::{collections::HashMap, sync::Arc};

use async_graphql::dataloader::Loader;
use models::entity::user;
use sea_orm::{prelude::*, DatabaseConnection};

use crate::object::access::AccessPerson;

/// Loads the people of the *Public access* page by their ids, one query for all the ids a request
/// asks for. Unlike [`ReaderProfileLoader`](super::reader_profile::ReaderProfileLoader) it also
/// loads the people who are locked or removed: those are exactly the ones the owner has to see
/// there. It is only reachable behind the owner-only guards of the page
pub struct AccessPersonLoader {
	conn: Arc<DatabaseConnection>,
}

impl AccessPersonLoader {
	pub fn new(conn: Arc<DatabaseConnection>) -> Self {
		Self { conn }
	}
}

impl Loader<String> for AccessPersonLoader {
	type Value = AccessPerson;
	type Error = Arc<sea_orm::error::DbErr>;

	async fn load(
		&self,
		keys: &[String],
	) -> Result<HashMap<String, Self::Value>, Self::Error> {
		let users = user::Entity::find()
			.filter(user::Column::Id.is_in(keys.iter().cloned().collect::<Vec<_>>()))
			.all(self.conn.as_ref())
			.await?;

		Ok(users
			.into_iter()
			.map(|model| (model.id.clone(), AccessPerson::from(model)))
			.collect())
	}
}
