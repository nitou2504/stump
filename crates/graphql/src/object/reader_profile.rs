//! The public face of a user, for the places where people show each other what they did: who made
//! a list, whose reading history this is, who uploaded a book.
//!
//! It carries a name and an avatar, and nothing else. [`User`] has permissions, preferences, login
//! activity and more behind guards that protect it only when it's reached the usual way, so a
//! screen that shows *another* person goes through this type, never through `User`. What it never
//! includes (an e-mail address, the age restriction, whether the account is locked or the server
//! owner, when it was made) is what a friend has no business knowing about a friend.
//!
//! A deleted user has no profile: whoever builds one from a model checks `deleted_at` first (the
//! lists and histories that use it leave out the people they can't show).
//!
//! [`User`]: super::user::User

use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use models::entity::user;
use sea_orm::prelude::DateTimeWithTimeZone;

use crate::data::ServiceContext;

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
#[graphql(complex)]
pub struct ReaderProfile {
	pub id: String,
	pub username: String,
	#[graphql(skip)]
	has_avatar: bool,
	#[graphql(skip)]
	avatar_updated_at: Option<DateTimeWithTimeZone>,
}

impl From<user::Model> for ReaderProfile {
	fn from(model: user::Model) -> Self {
		Self {
			has_avatar: model.avatar_path.is_some(),
			avatar_updated_at: model.avatar_updated_at,
			id: model.id,
			username: model.username,
		}
	}
}

#[ComplexObject]
impl ReaderProfile {
	/// The address of the person's avatar, when they have one. It changes when they set a new one
	async fn avatar_url(&self, ctx: &Context<'_>) -> Result<Option<String>> {
		if !self.has_avatar {
			return Ok(None);
		}

		let service = ctx.data::<ServiceContext>()?;
		Ok(Some(service.cache_friendly_url(
			format!("/api/v2/users/{}/avatar", self.id),
			&self.avatar_updated_at,
		)))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use ::tests::{db::test_database, fake_data};
	use async_graphql::{EmptyMutation, EmptySubscription, Object, Request, Schema};
	use sea_orm::{ActiveModelTrait, Set};

	struct Query(user::Model);

	#[Object]
	impl Query {
		async fn reader(&self) -> ReaderProfile {
			ReaderProfile::from(self.0.clone())
		}
	}

	async fn ask(model: user::Model, query: &str) -> serde_json::Value {
		let schema = Schema::build(Query(model), EmptyMutation, EmptySubscription)
			.data(ServiceContext::new(
				"stump.example".to_string(),
				"https".to_string(),
			))
			.finish();
		let response = schema.execute(Request::new(query)).await;
		assert!(response.errors.is_empty(), "{:?}", response.errors);
		response.data.into_json().expect("should be json")
	}

	async fn reader() -> user::Model {
		let db = test_database().await;
		fake_data::User::new("ana").insert(&db).await
	}

	#[tokio::test]
	async fn a_profile_is_a_name_and_an_avatar() {
		let model = reader().await;
		let id = model.id.clone();

		let data = ask(model, "{ reader { id username avatarUrl } }").await;

		assert_eq!(data["reader"]["id"], id.as_str());
		assert_eq!(data["reader"]["username"], "ana");
		assert!(data["reader"]["avatarUrl"].is_null());
	}

	#[tokio::test]
	async fn the_avatar_is_where_the_server_serves_it_and_changes_with_it() {
		let db = test_database().await;
		let model = fake_data::User::new("ana").insert(&db).await;
		let id = model.id.clone();
		let updated: DateTimeWithTimeZone = chrono::Utc::now().into();
		let model = user::ActiveModel {
			avatar_path: Set(Some("/config/avatars/ana.webp".to_string())),
			avatar_updated_at: Set(Some(updated)),
			..model.into()
		}
		.update(&db)
		.await
		.expect("should set the avatar");

		let data = ask(model, "{ reader { avatarUrl } }").await;

		let url = data["reader"]["avatarUrl"].as_str().expect("an address");
		assert!(
			url.starts_with(&format!(
				"https://stump.example/api/v2/users/{id}/avatar?last_modified="
			)),
			"{url}"
		);
	}

	/// What a friend has no business knowing about a friend is not in the type at all: asking for
	/// it is an error, not an empty answer
	#[tokio::test]
	async fn nothing_else_about_the_user_can_be_asked() {
		let model = reader().await;

		let schema =
			Schema::build(Query(model), EmptyMutation, EmptySubscription).finish();
		let sdl = schema.sdl();
		let type_def = sdl
			.split("type ReaderProfile {")
			.nth(1)
			.and_then(|rest| rest.split('}').next())
			.expect("the type is in the schema");
		// the field lines, without the descriptions (a one-line "..." or a """ block)
		let mut fields = Vec::new();
		let mut in_description = false;
		for line in type_def.lines().map(str::trim) {
			if line == r#"""""# {
				in_description = !in_description;
			} else if !in_description && !line.is_empty() && !line.starts_with('"') {
				fields.push(line.split(':').next().expect("a field").trim());
			}
		}
		fields.sort_unstable();

		assert_eq!(fields, vec!["avatarUrl", "id", "username"]);
	}
}
