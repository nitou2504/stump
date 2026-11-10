use async_graphql::{Enum, InputObject, OneofObject, Result};
use models::{
	entity::{
		smart_list::{SmartListGrouping, SmartListJoiner},
		user::AuthUser,
	},
	shared::enums::EntityVisibility,
};
use sea_orm::{prelude::*, ActiveValue::Set};
use serde::{Deserialize, Serialize};

use crate::filter::{
	library::LibraryFilterInput, media::MediaFilterInput,
	media_metadata::MediaMetadataFilterInput, series::SeriesFilterInput,
	series_metadata::SeriesMetadataFilterInput,
};

#[derive(InputObject)]
pub struct SmartListsInput {
	pub all: Option<bool>,
	pub mine: Option<bool>,
	pub search: Option<String>,
}

impl Default for SmartListsInput {
	fn default() -> Self {
		SmartListsInput {
			all: None,
			mine: Some(true),
			search: None,
		}
	}
}

/// The different filter joiners that can be used in smart lists
#[derive(
	PartialEq, Eq, Copy, Hash, Debug, Clone, Default, Enum, Serialize, Deserialize,
)]
pub enum SmartListGroupJoiner {
	#[default]
	And,
	Or,
	Not,
}

#[allow(clippy::large_enum_variant)]
#[derive(OneofObject, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartListFilterInput {
	Media(MediaFilterInput),
	MediaMetadata(MediaMetadataFilterInput),
	Series(SeriesFilterInput),
	SeriesMetadata(SeriesMetadataFilterInput),
	Library(LibraryFilterInput),
}

#[derive(InputObject, Clone, Serialize, Deserialize)]
pub struct SmartListFilterGroupInput {
	pub groups: Vec<SmartListFilterInput>,
	pub joiner: SmartListGroupJoiner,
}

/// What a user may put in smart lists. The server owner has no limits; everybody else is held to
/// them, so that nobody can fill the database with lists or with rules (a list's rules are read
/// on every visit, by everyone it is shown to)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartListLimits {
	/// How many lists a user may have
	pub lists: Option<u64>,
	/// How large a list's rules may be, in bytes of their JSON
	pub filters_bytes: Option<usize>,
	/// How long a list's name may be, in characters
	pub name_chars: Option<usize>,
	/// How long a list's description may be, in characters
	pub description_chars: Option<usize>,
}

impl SmartListLimits {
	pub const UNLIMITED: Self = Self {
		lists: None,
		filters_bytes: None,
		name_chars: None,
		description_chars: None,
	};

	pub const STANDARD: Self = Self {
		lists: Some(20),
		filters_bytes: Some(128 * 1024),
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

#[derive(Clone, InputObject)]
pub struct SaveSmartListInput {
	pub filters: Vec<SmartListFilterGroupInput>,
	pub name: String,
	pub description: Option<String>,
	pub joiner: SmartListJoiner,
	pub default_grouping: SmartListGrouping,
	/// `PUBLIC` shows the list to everyone on the server (each one with the books they may
	/// read); `PRIVATE` only to its creator. `SHARED` is not offered
	pub visibility: EntityVisibility,
}

impl SaveSmartListInput {
	pub fn into_active_model(
		self,
		user_id: &str,
		limits: &SmartListLimits,
	) -> Result<models::entity::smart_list::ActiveModel> {
		let filters = serde_json::to_vec(&self.filters)?;
		if limits.filters_bytes.is_some_and(|max| filters.len() > max) {
			return Err(format!(
				"A smart list's rules can take at most {} KB",
				limits.filters_bytes.unwrap_or_default() / 1024
			)
			.into());
		}

		Ok(models::entity::smart_list::ActiveModel {
			id: Set(Uuid::new_v4().to_string()),
			name: Set(smart_list_name(self.name, limits)?),
			description: Set(smart_list_description(self.description, limits)?),
			filters: Set(filters),
			joiner: Set(self.joiner),
			default_grouping: Set(self.default_grouping),
			visibility: Set(list_visibility(self.visibility)?),
			creator_id: Set(user_id.to_string()),
		})
	}
}

impl SaveSmartListInput {
	/// The list as the server owner changes it when it isn't theirs: the only thing they may do is
	/// take it back from the public (moderation). Its name, description, rules, grouping and saved
	/// views stay as its creator made them, whatever else the input says, so nothing the owner
	/// writes can end up in a friend's list (or show them what is hidden from them, or push the
	/// list over the limits that hold for them)
	pub fn apply_as_moderator(
		self,
		model: models::entity::smart_list::Model,
	) -> Result<models::entity::smart_list::ActiveModel> {
		if self.visibility != EntityVisibility::Private {
			return Err(ONLY_MAKE_PRIVATE.into());
		}

		let mut active_model: models::entity::smart_list::ActiveModel = model.into();
		active_model.visibility = Set(EntityVisibility::Private);
		Ok(active_model)
	}
}

/// What the server owner gets for changing a public list of someone else in any way but back to
/// private
pub const ONLY_MAKE_PRIVATE: &str =
	"A smart list of someone else can only be made private, or deleted";

/// A list is private or public: there's no one in between (nothing offers `SHARED`, so the
/// per-person access rules it needs are never written)
pub fn list_visibility(visibility: EntityVisibility) -> Result<EntityVisibility> {
	match visibility {
		EntityVisibility::Private | EntityVisibility::Public => Ok(visibility),
		EntityVisibility::Shared => {
			Err("A smart list is either private or public".into())
		},
	}
}

/// A list's name; it can't be longer than the limits allow
fn smart_list_name(name: String, limits: &SmartListLimits) -> Result<String> {
	if let Some(max) = limits.name_chars {
		if name.chars().count() > max {
			return Err(
				format!("A smart list's name can have at most {max} characters").into(),
			);
		}
	}
	Ok(name)
}

/// A list's description; it can't be longer than the limits allow
fn smart_list_description(
	description: Option<String>,
	limits: &SmartListLimits,
) -> Result<Option<String>> {
	if let (Some(description), Some(max)) = (&description, limits.description_chars) {
		if description.chars().count() > max {
			return Err(format!(
				"A smart list's description can have at most {max} characters"
			)
			.into());
		}
	}
	Ok(description)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn input(name: &str, visibility: EntityVisibility) -> SaveSmartListInput {
		SaveSmartListInput {
			filters: vec![],
			name: name.to_string(),
			description: None,
			joiner: SmartListJoiner::And,
			default_grouping: SmartListGrouping::ByBooks,
			visibility,
		}
	}

	#[test]
	fn a_list_is_private_or_public() {
		for visibility in [EntityVisibility::Private, EntityVisibility::Public] {
			let model = input("Some list", visibility)
				.into_active_model("42", &SmartListLimits::STANDARD)
				.unwrap();
			assert_eq!(model.visibility.unwrap(), visibility);
			assert_eq!(model.creator_id.unwrap(), "42");
		}

		let error = input("Some list", EntityVisibility::Shared)
			.into_active_model("42", &SmartListLimits::UNLIMITED)
			.err()
			.expect("SHARED is not offered, not even to the server owner");
		assert_eq!(error.message, "A smart list is either private or public");
	}

	#[test]
	fn names_and_descriptions_have_a_length_for_everyone_but_the_owner() {
		let long_name = "n".repeat(201);
		assert!(input(&long_name, EntityVisibility::Private)
			.into_active_model("42", &SmartListLimits::STANDARD)
			.is_err());
		assert!(input(&"n".repeat(200), EntityVisibility::Private)
			.into_active_model("42", &SmartListLimits::STANDARD)
			.is_ok());
		assert!(input(&long_name, EntityVisibility::Private)
			.into_active_model("42", &SmartListLimits::UNLIMITED)
			.is_ok());

		let with_description = |chars: usize| SaveSmartListInput {
			description: Some("d".repeat(chars)),
			..input("Some list", EntityVisibility::Private)
		};
		assert!(with_description(2_001)
			.into_active_model("42", &SmartListLimits::STANDARD)
			.is_err());
		assert!(with_description(2_000)
			.into_active_model("42", &SmartListLimits::STANDARD)
			.is_ok());
		assert!(with_description(2_001)
			.into_active_model("42", &SmartListLimits::UNLIMITED)
			.is_ok());
	}

	#[test]
	fn the_rules_of_a_list_have_a_size_for_everyone_but_the_owner() {
		let limits = SmartListLimits {
			filters_bytes: Some(64),
			..SmartListLimits::UNLIMITED
		};
		// an empty set of rules is `[]`; a group of rules is much bigger than 64 bytes
		let group = SmartListFilterGroupInput {
			groups: vec![],
			joiner: SmartListGroupJoiner::And,
		};
		let big = SaveSmartListInput {
			filters: vec![group.clone(), group.clone(), group.clone(), group],
			..input("Some list", EntityVisibility::Private)
		};
		assert!(serde_json::to_vec(&big.filters).unwrap().len() > 64);
		assert!(big.clone().into_active_model("42", &limits).is_err());
		assert!(big
			.into_active_model("42", &SmartListLimits::UNLIMITED)
			.is_ok());
		assert!(input("Some list", EntityVisibility::Private)
			.into_active_model("42", &limits)
			.is_ok());
	}

	fn stored_list(creator_id: &str) -> models::entity::smart_list::Model {
		models::entity::smart_list::Model {
			id: "list".to_string(),
			name: "Her shelf".to_string(),
			description: Some("What she reads".to_string()),
			filters: b"[]".to_vec(),
			joiner: SmartListJoiner::Or,
			default_grouping: SmartListGrouping::BySeries,
			visibility: EntityVisibility::Public,
			creator_id: creator_id.to_string(),
		}
	}

	#[test]
	fn the_owner_takes_a_list_of_someone_else_back_to_private_and_nothing_more() {
		// whatever else the input says, the list stays as its creator made it
		let rewritten = SaveSmartListInput {
			filters: vec![SmartListFilterGroupInput {
				groups: vec![],
				joiner: SmartListGroupJoiner::Or,
			}],
			description: Some("Written by the owner".to_string()),
			joiner: SmartListJoiner::And,
			default_grouping: SmartListGrouping::ByLibrary,
			..input("Taken", EntityVisibility::Private)
		};
		let model = rewritten.apply_as_moderator(stored_list("7")).unwrap();
		assert_eq!(model.visibility.clone().unwrap(), EntityVisibility::Private);
		// nothing but the visibility is written
		assert!(model.name.is_unchanged());
		assert!(model.description.is_unchanged());
		assert!(model.filters.is_unchanged());
		assert!(model.joiner.is_unchanged());
		assert!(model.default_grouping.is_unchanged());
		assert!(model.creator_id.is_unchanged());
		assert_eq!(model.name.unwrap(), "Her shelf");
	}

	#[test]
	fn the_owner_cannot_keep_a_list_of_someone_else_public_while_changing_it() {
		for visibility in [EntityVisibility::Public, EntityVisibility::Shared] {
			let error = input("Moderated", visibility)
				.apply_as_moderator(stored_list("7"))
				.err()
				.expect("only back to private is allowed");
			assert_eq!(error.message, ONLY_MAKE_PRIVATE);
		}
	}

	#[test]
	fn the_server_owner_has_no_limits() {
		let mut owner = crate::tests::common::get_default_user();
		owner.is_server_owner = true;
		assert_eq!(
			SmartListLimits::for_user(&owner),
			SmartListLimits::UNLIMITED
		);
		owner.is_server_owner = false;
		assert_eq!(SmartListLimits::for_user(&owner), SmartListLimits::STANDARD);
	}
}
