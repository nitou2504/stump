use async_graphql::{InputObject, Result};
use models::{entity::reading_list, shared::enums::EntityVisibility};
use sea_orm::{prelude::Uuid, ActiveValue::Set};

#[derive(InputObject)]
pub struct ReadingListInput {
	/// The list's ID, generated when omitted or empty
	pub id: Option<String>,
	pub name: String,
	pub description: Option<String>,
	/// Private (only its owner sees it) when omitted
	pub visibility: Option<EntityVisibility>,
	/// The list's first books, in order
	#[graphql(default)]
	pub media_ids: Vec<String>,
}

impl ReadingListInput {
	pub fn into_active_model(self, user_id: &str) -> Result<reading_list::ActiveModel> {
		let id = self
			.id
			.filter(|id| !id.trim().is_empty())
			.unwrap_or_else(|| Uuid::new_v4().to_string());

		Ok(reading_list::ActiveModel {
			id: Set(id),
			name: Set(reading_list_name(&self.name)?),
			description: Set(reading_list_description(self.description)),
			updated_at: Set(chrono::Utc::now().into()),
			visibility: Set(self
				.visibility
				.unwrap_or_default()
				.to_string()
				.to_uppercase()),
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
	pub visibility: Option<EntityVisibility>,
}

impl UpdateReadingListInput {
	pub fn apply(self, model: reading_list::Model) -> Result<reading_list::ActiveModel> {
		let mut active_model: reading_list::ActiveModel = model.into();
		active_model.name = Set(reading_list_name(&self.name)?);
		active_model.description = Set(reading_list_description(self.description));
		if let Some(visibility) = self.visibility {
			active_model.visibility = Set(visibility.to_string().to_uppercase());
		}
		active_model.updated_at = Set(chrono::Utc::now().into());
		Ok(active_model)
	}
}

/// A list's name, trimmed; it can't be empty
fn reading_list_name(name: &str) -> Result<String> {
	let name = name.trim();
	if name.is_empty() {
		return Err("A reading list needs a name".into());
	}
	Ok(name.to_string())
}

/// A list's description, trimmed; empty is none
fn reading_list_description(description: Option<String>) -> Option<String> {
	description
		.map(|description| description.trim().to_string())
		.filter(|description| !description.is_empty())
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
			.into_active_model(user_id)
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
			.into_active_model(user_id)
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
			let model = input(id, None).into_active_model("test_user").unwrap();
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
		.into_active_model("test_user")
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
		.into_active_model("test_user");

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
		.apply(model.clone())
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
		.apply(model.clone())
		.unwrap();
		assert_eq!(private.visibility.unwrap(), "PRIVATE");

		let unnamed = UpdateReadingListInput {
			name: String::new(),
			description: None,
			visibility: None,
		}
		.apply(model);
		assert!(unnamed.is_err());
	}
}
