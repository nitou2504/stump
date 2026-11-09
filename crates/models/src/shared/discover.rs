use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};

/// The tag that keeps a book, series or library out of the picks by default
pub const NO_PICKS_TAG: &str = "No picks";

const MAX_ITEMS: usize = 200;

fn default_true() -> bool {
	true
}

fn default_excluded_tags() -> Vec<String> {
	vec![NO_PICKS_TAG.to_string()]
}

/// What the home's "Picks of the week" and "Surprise me" may pick, per user
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SimpleObject, InputObject)]
#[graphql(input_name = "DiscoverRulesInput")]
#[serde(rename_all = "camelCase")]
pub struct DiscoverRules {
	/// Only books the user hasn't started
	#[serde(default = "default_true")]
	#[graphql(default = true)]
	pub unread_only: bool,
	/// Books tagged with any of these (or in a series or library tagged so) are left out,
	/// ignoring case
	#[serde(default = "default_excluded_tags")]
	#[graphql(default_with = "default_excluded_tags()")]
	pub excluded_tags: Vec<String>,
	/// Books with any of these genres are left out: a whole genre (`Level: High School`),
	/// or a family ending in `*` (`Reference: *`)
	#[serde(default)]
	#[graphql(default)]
	pub excluded_genres: Vec<String>,
	/// Books in these libraries are left out
	#[serde(default)]
	#[graphql(default)]
	pub excluded_library_ids: Vec<String>,
	/// Books rated below this (0-5) are left out; books without a rating stay
	#[serde(default)]
	#[graphql(default)]
	pub min_rating: Option<f64>,
}

impl Default for DiscoverRules {
	fn default() -> Self {
		Self {
			unread_only: true,
			excluded_tags: default_excluded_tags(),
			excluded_genres: vec![],
			excluded_library_ids: vec![],
			min_rating: None,
		}
	}
}

/// Trimmed, without blanks or repeats (ignoring case), at most `MAX_ITEMS`
fn clean(values: Vec<String>) -> Vec<String> {
	let mut seen = std::collections::HashSet::new();
	values
		.into_iter()
		.map(|value| value.trim().to_string())
		.filter(|value| !value.is_empty() && seen.insert(value.to_lowercase()))
		.take(MAX_ITEMS)
		.collect()
}

impl DiscoverRules {
	/// The rules as saved: cleaned lists and a rating within 0-5 (0 is no minimum)
	pub fn normalized(self) -> Self {
		Self {
			unread_only: self.unread_only,
			excluded_tags: clean(self.excluded_tags),
			excluded_genres: clean(self.excluded_genres),
			excluded_library_ids: clean(self.excluded_library_ids),
			min_rating: self
				.min_rating
				.filter(|rating| rating.is_finite() && *rating > 0.0)
				.map(|rating| rating.min(5.0)),
		}
	}

	/// The saved JSON, or the defaults when there is none or it can't be read
	pub fn from_json(json: Option<&str>) -> Self {
		json.and_then(|json| serde_json::from_str::<Self>(json).ok())
			.map(Self::normalized)
			.unwrap_or_default()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn missing_or_unreadable_rules_are_the_defaults() {
		assert_eq!(DiscoverRules::from_json(None), DiscoverRules::default());
		assert_eq!(
			DiscoverRules::from_json(Some("[1]")),
			DiscoverRules::default()
		);
		let partial =
			DiscoverRules::from_json(Some(r#"{"excludedGenres": ["Form: Textbook"]}"#));
		assert!(partial.unread_only);
		assert_eq!(partial.excluded_tags, vec!["No picks"]);
		assert_eq!(partial.excluded_genres, vec!["Form: Textbook"]);
	}

	#[test]
	fn rules_are_cleaned() {
		let rules = DiscoverRules {
			unread_only: false,
			excluded_tags: vec![" No picks ".into(), "no PICKS".into(), "".into()],
			excluded_genres: vec!["Reference: *".into()],
			excluded_library_ids: vec![],
			min_rating: Some(9.0),
		}
		.normalized();
		assert_eq!(rules.excluded_tags, vec!["No picks"]);
		assert_eq!(rules.min_rating, Some(5.0));
		assert_eq!(
			DiscoverRules {
				min_rating: Some(0.0),
				..Default::default()
			}
			.normalized()
			.min_rating,
			None
		);
	}
}
