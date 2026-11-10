use async_graphql::{InputObject, Result};
use models::{
	entity::{reading_list, user::AuthUser},
	shared::enums::EntityVisibility,
};
use sea_orm::{prelude::Uuid, ActiveValue::Set};

use crate::object::reading_list::CANT_CHANGE;

/// What a user may put in reading lists. The server owner has no limits; everybody else is held
/// to them, so that nobody can fill the database with lists or books
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadingListLimits {
	/// How many lists a user may have
	pub lists: Option<u64>,
	/// How many books a list may hold
	pub books_per_list: Option<u64>,
	/// How long a list's name may be, in characters
	pub name_chars: Option<usize>,
	/// How long a list's description may be, in characters
	pub description_chars: Option<usize>,
}

impl ReadingListLimits {
	pub const UNLIMITED: Self = Self {
		lists: None,
		books_per_list: None,
		name_chars: None,
		description_chars: None,
	};

	pub const STANDARD: Self = Self {
		lists: Some(100),
		books_per_list: Some(2_000),
		name_chars: Some(200),
		description_chars: Some(2_000),
	};

	/// The limits of a user: none for the server owner
	pub fn for_user(user: &AuthUser) -> Self {
		if user.is_server_owner {
			Self::UNLIMITED
		} else {
			Self::STANDARD
		}
	}
}

#[derive(InputObject)]
pub struct ReadingListInput {
	/// The list's ID, generated when omitted or empty
	pub id: Option<String>,
	pub name: String,
	pub description: Option<String>,
	/// Private (only its owner sees it) when omitted. `PUBLIC` shows the list to everyone on
	/// the server; `SHARED` is not offered
	pub visibility: Option<EntityVisibility>,
	/// The list's first books, in order
	#[graphql(default)]
	pub media_ids: Vec<String>,
}

impl ReadingListInput {
	pub fn into_active_model(
		self,
		user_id: &str,
		limits: &ReadingListLimits,
	) -> Result<reading_list::ActiveModel> {
		let id = self
			.id
			.filter(|id| !id.trim().is_empty())
			.unwrap_or_else(|| Uuid::new_v4().to_string());

		Ok(reading_list::ActiveModel {
			id: Set(id),
			name: Set(reading_list_name(&self.name, limits)?),
			description: Set(reading_list_description(self.description, limits)?),
			updated_at: Set(chrono::Utc::now().into()),
			visibility: Set(list_visibility(self.visibility.unwrap_or_default())?),
			ordering: Set("MANUAL".to_string()),
			creating_user_id: Set(user_id.to_string()),
			..Default::default()
		})
	}
}

/// What `updateReadingList` changes: the name and description replace the list's, the
/// visibility stays when omitted. The books are changed with `addBooksToReadingList` and
/// `removeBooksFromReadingList`
#[derive(InputObject)]
pub struct UpdateReadingListInput {
	pub name: String,
	/// Omitted or empty clears it
	pub description: Option<String>,
	/// `PUBLIC` shows the list to everyone on the server; `SHARED` is not offered. The server
	/// owner can't change anyone else's list but can make it `PRIVATE`: the name and description
	/// sent are then ignored
	pub visibility: Option<EntityVisibility>,
}

impl UpdateReadingListInput {
	/// The list as its creator changes it
	pub fn apply(
		self,
		model: reading_list::Model,
		limits: &ReadingListLimits,
	) -> Result<reading_list::ActiveModel> {
		let mut active_model: reading_list::ActiveModel = model.into();
		active_model.name = Set(reading_list_name(&self.name, limits)?);
		active_model.description =
			Set(reading_list_description(self.description, limits)?);
		if let Some(visibility) = self.visibility {
			active_model.visibility = Set(list_visibility(visibility)?);
		}
		active_model.updated_at = Set(chrono::Utc::now().into());
		Ok(active_model)
	}

	/// The list as the server owner changes it when it isn't theirs: the only thing they may do
	/// is take it back from the public (moderation). The list keeps its name, description, books
	/// and the time its creator last changed it
	pub fn apply_as_moderator(
		self,
		model: reading_list::Model,
	) -> Result<reading_list::ActiveModel> {
		if self.visibility != Some(EntityVisibility::Private) {
			return Err(CANT_CHANGE.into());
		}

		let mut active_model: reading_list::ActiveModel = model.into();
		active_model.visibility = Set(reading_list::PRIVATE.to_string());
		Ok(active_model)
	}
}

/// A list is private or public: there's no one in between
fn list_visibility(visibility: EntityVisibility) -> Result<String> {
	match visibility {
		EntityVisibility::Private => Ok(reading_list::PRIVATE.to_string()),
		EntityVisibility::Public => Ok(reading_list::PUBLIC.to_string()),
		EntityVisibility::Shared => {
			Err("A reading list is either private or public".into())
		},
	}
}

/// A list's name, trimmed; it can't be empty or longer than the limits allow
fn reading_list_name(name: &str, limits: &ReadingListLimits) -> Result<String> {
	let name = name.trim();
	if name.is_empty() {
		return Err("A reading list needs a name".into());
	}
	if let Some(max) = limits.name_chars {
		if name.chars().count() > max {
			return Err(format!(
				"A reading list's name can have at most {max} characters"
			)
			.into());
		}
	}
	Ok(name.to_string())
}

/// A list's description, trimmed; empty is none, and it can't be longer than the limits allow
fn reading_list_description(
	description: Option<String>,
	limits: &ReadingListLimits,
) -> Result<Option<String>> {
	let description = description
		.map(|description| description.trim().to_string())
		.filter(|description| !description.is_empty());
	if let (Some(description), Some(max)) = (&description, limits.description_chars) {
		if description.chars().count() > max {
			return Err(format!(
				"A reading list's description can have at most {max} characters"
			)
			.into());
		}
	}
	Ok(description)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::common::*;

	fn input(id: Option<&str>, visibility: Option<EntityVisibility>) -> ReadingListInput {
		ReadingListInput {
			id: id.map(str::to_string),
			name: "Some Name".to_string(),
			description: None,
			visibility,
			media_ids: vec![],
		}
	}

	#[test]
	fn test_readinglistinput_into_activemodel_default() {
		let user_id = "test_user";
		let model = input(Some("some_id"), None)
			.into_active_model(user_id, &ReadingListLimits::UNLIMITED)
			.unwrap();

		assert_eq!(model.id.unwrap(), "some_id");
		assert_eq!(model.name.unwrap(), "Some Name");
		assert_eq!(model.description.unwrap(), None);
		assert!(is_close_to_now(model.updated_at.unwrap().into()));
		assert_eq!(model.visibility.unwrap(), "PRIVATE");
		assert_eq!(model.creating_user_id.unwrap(), user_id);
		assert_eq!(model.ordering.unwrap(), "MANUAL");
	}

	#[test]
	fn test_readinglistinput_into_activemodel_with_visibility() {
		let user_id = "test_user";
		let model = input(Some("some_id"), Some(EntityVisibility::Public))
			.into_active_model(user_id, &ReadingListLimits::UNLIMITED)
			.unwrap();

		assert_eq!(model.id.unwrap(), "some_id");
		assert_eq!(model.name.unwrap(), "Some Name");
		assert!(is_close_to_now(model.updated_at.unwrap().into()));
		assert_eq!(model.visibility.unwrap(), "PUBLIC");
		assert_eq!(model.creating_user_id.unwrap(), user_id);
	}

	#[test]
	fn test_readinglistinput_generates_an_id() {
		for id in [None, Some(""), Some("  ")] {
			let model = input(id, None)
				.into_active_model("test_user", &ReadingListLimits::UNLIMITED)
				.unwrap();
			let generated = model.id.unwrap();
			assert!(
				Uuid::parse_str(&generated).is_ok(),
				"{generated} is not a uuid"
			);
		}
	}

	#[test]
	fn test_readinglistinput_trims_the_name_and_description() {
		let model = ReadingListInput {
			name: "  Recs from Ana ".to_string(),
			description: Some(" From the book club ".to_string()),
			..input(None, None)
		}
		.into_active_model("test_user", &ReadingListLimits::UNLIMITED)
		.unwrap();

		assert_eq!(model.name.unwrap(), "Recs from Ana");
		assert_eq!(
			model.description.unwrap().as_deref(),
			Some("From the book club")
		);
	}

	#[test]
	fn test_readinglistinput_needs_a_name() {
		let result = ReadingListInput {
			name: "   ".to_string(),
			..input(None, None)
		}
		.into_active_model("test_user", &ReadingListLimits::UNLIMITED);

		assert!(result.is_err());
	}

	#[test]
	fn test_updatereadinglistinput_apply() {
		let model = reading_list::Model {
			id: "1".to_string(),
			name: "Old".to_string(),
			description: Some("Old description".to_string()),
			updated_at: "2021-08-01T00:00:00Z".parse().unwrap(),
			visibility: "PUBLIC".to_string(),
			ordering: "MANUAL".to_string(),
			creating_user_id: "42".to_string(),
		};

		let updated = UpdateReadingListInput {
			name: " New ".to_string(),
			description: Some("  ".to_string()),
			visibility: None,
		}
		.apply(model.clone(), &ReadingListLimits::UNLIMITED)
		.unwrap();
		assert_eq!(updated.name.clone().unwrap(), "New");
		assert_eq!(updated.description.clone().unwrap(), None);
		// kept when omitted
		assert_eq!(updated.visibility.clone().unwrap(), "PUBLIC");
		assert!(is_close_to_now(updated.updated_at.clone().unwrap().into()));

		let private = UpdateReadingListInput {
			name: "New".to_string(),
			description: None,
			visibility: Some(EntityVisibility::Private),
		}
		.apply(model.clone(), &ReadingListLimits::UNLIMITED)
		.unwrap();
		assert_eq!(private.visibility.unwrap(), "PRIVATE");

		let unnamed = UpdateReadingListInput {
			name: String::new(),
			description: None,
			visibility: None,
		}
		.apply(model, &ReadingListLimits::UNLIMITED);
		assert!(unnamed.is_err());
	}

	fn model(visibility: &str) -> reading_list::Model {
		reading_list::Model {
			id: "1".to_string(),
			name: "Old".to_string(),
			description: Some("Old description".to_string()),
			updated_at: "2021-08-01T00:00:00Z".parse().unwrap(),
			visibility: visibility.to_string(),
			ordering: "MANUAL".to_string(),
			creating_user_id: "42".to_string(),
		}
	}

	#[test]
	fn test_shared_is_not_offered() {
		let created = input(None, Some(EntityVisibility::Shared))
			.into_active_model("test_user", &ReadingListLimits::UNLIMITED);
		assert!(created.is_err());

		let updated = UpdateReadingListInput {
			name: "New".to_string(),
			description: None,
			visibility: Some(EntityVisibility::Shared),
		}
		.apply(model("PRIVATE"), &ReadingListLimits::UNLIMITED);
		assert!(updated.is_err());
	}

	#[test]
	fn test_limits_of_a_user() {
		let mut user = get_default_user();
		assert!(user.is_server_owner);
		assert_eq!(
			ReadingListLimits::for_user(&user),
			ReadingListLimits::UNLIMITED
		);

		user.is_server_owner = false;
		assert_eq!(
			ReadingListLimits::for_user(&user),
			ReadingListLimits::STANDARD
		);
		assert_eq!(ReadingListLimits::STANDARD.lists, Some(100));
		assert_eq!(ReadingListLimits::STANDARD.books_per_list, Some(2_000));
	}

	#[test]
	fn test_name_and_description_lengths_count_characters() {
		let limits = ReadingListLimits::STANDARD;
		let named = |name: String, description: Option<String>| ReadingListInput {
			name,
			description,
			..input(None, None)
		};

		// 200 characters of 2 bytes each fit; 201 don't
		assert!(named("é".repeat(200), None)
			.into_active_model("u", &limits)
			.is_ok());
		assert!(named("é".repeat(201), None)
			.into_active_model("u", &limits)
			.is_err());
		assert!(named("é".repeat(201), None)
			.into_active_model("u", &ReadingListLimits::UNLIMITED)
			.is_ok());

		assert!(named("x".to_string(), Some("y".repeat(2_000)))
			.into_active_model("u", &limits)
			.is_ok());
		assert!(named("x".to_string(), Some("y".repeat(2_001)))
			.into_active_model("u", &limits)
			.is_err());

		let long = UpdateReadingListInput {
			name: "n".repeat(201),
			description: None,
			visibility: None,
		};
		assert!(long.apply(model("PRIVATE"), &limits).is_err());
	}

	#[test]
	fn test_the_moderator_can_only_take_a_list_back_to_private() {
		let unpublished = UpdateReadingListInput {
			name: "Hijacked".to_string(),
			description: Some("Hijacked".to_string()),
			visibility: Some(EntityVisibility::Private),
		}
		.apply_as_moderator(model("PUBLIC"))
		.unwrap();
		assert_eq!(unpublished.visibility.unwrap(), "PRIVATE");
		// everything else stays as the creator left it
		assert_eq!(unpublished.name.unwrap(), "Old");
		assert_eq!(
			unpublished.description.unwrap().as_deref(),
			Some("Old description")
		);
		assert_eq!(
			unpublished.updated_at.unwrap().to_string(),
			"2021-08-01 00:00:00 +00:00"
		);

		for visibility in [
			None,
			Some(EntityVisibility::Public),
			Some(EntityVisibility::Shared),
		] {
			let refused = UpdateReadingListInput {
				name: "Hijacked".to_string(),
				description: None,
				visibility,
			}
			.apply_as_moderator(model("PUBLIC"));
			assert_eq!(refused.unwrap_err().message, CANT_CHANGE);
		}
	}
}
