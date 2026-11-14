use regex::Regex;
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

pub fn string_list_deserializer<'de, D>(
	deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
	D: Deserializer<'de>,
{
	let str_sequence =
		Option::<String>::deserialize(deserializer)?.filter(|s| !s.is_empty());

	// "A, , B" and ",A" have no empty items, and a list of blanks is no list at all
	match str_sequence {
		Some(s) => {
			let items = clean_list_items(s.split(','));
			Ok((!items.is_empty()).then_some(items))
		},
		None => Ok(None),
	}
}

fn empty_string_as_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
	D: Deserializer<'de>,
	T: FromStr,
{
	let opt = Option::<String>::deserialize(deserializer)?;
	match opt {
		Some(s) if s.trim().is_empty() => Ok(None),
		Some(s) => Ok(s.trim().parse().ok()),
		None => Ok(None),
	}
}

pub fn optional_f64_deserializer<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
	D: Deserializer<'de>,
{
	empty_string_as_none(deserializer)
}

pub fn optional_i32_deserializer<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
	D: Deserializer<'de>,
{
	empty_string_as_none(deserializer)
}

// https://anansi-project.github.io/docs/comicinfo/schemas/v2.1
/// Deserializes a string into an age rating. This isn't the fanciest deserializer,
/// but it's fine.
pub fn age_rating_deserializer<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
	D: Deserializer<'de>,
{
	let str_sequence = Option::<String>::deserialize(deserializer)?
		.filter(|s| !s.is_empty())
		.map(|s| s.trim().to_owned());

	let Some(str_sequence) = str_sequence else {
		return Ok(None);
	};

	Ok(parse_age_restriction(&str_sequence))
}

pub fn parse_age_restriction(str_sequence: &str) -> Option<i32> {
	// check for the first case G/PG/PG-13/R
	let movie_rating = match str_sequence.to_lowercase().as_str() {
		"g" | "pg" => Some(0),
		"pg-13" => Some(13),
		"r" => Some(17),
		_ => None,
	};

	if movie_rating.is_some() {
		return movie_rating;
	}

	// check for the second case All Ages/Teen/Teen+/Mature
	let common_phrase = match str_sequence.to_lowercase().as_str() {
		"all ages" => Some(0),
		"everyone" => Some(0),
		"early childhood" => Some(8), // TODO: this is a guess
		"everyone 10+" => Some(10),
		"teen" => Some(13),
		"teen+" => Some(16),
		"ma15+" => Some(15),
		"mature 17+" => Some(17),
		"m" => Some(18),
		"mature" => Some(18),
		"adults only 18+" => Some(18),
		"r18+" => Some(18),
		"x18+" => Some(18),
		_ => None,
	};

	if common_phrase.is_some() {
		return common_phrase;
	}

	// check for the third case \d and up
	if let Ok(re) = Regex::new(r"(\d+) and up") {
		let age = re
			.captures(str_sequence)
			.and_then(|c| c.get(1))
			.and_then(|m| m.as_str().parse().ok());

		if age.is_some() {
			return age;
		}
	} else {
		tracing::error!("Failed to create regex for age rating parsing");
	}

	// check for the fourth case \d+
	if let Ok(re) = Regex::new(r"(\d+)") {
		let age = re
			.captures(str_sequence)
			.and_then(|c| c.get(1))
			.and_then(|m| m.as_str().parse().ok());

		if age.is_some() {
			return age;
		}
	} else {
		tracing::error!("Failed to create regex for age rating parsing");
	}

	// check for the fifth case \d-\d, only the first number is used
	if let Ok(re) = Regex::new(r"(\d+)-(\d+)") {
		let age = re
			.captures(str_sequence)
			.and_then(|c| c.get(1))
			.and_then(|m| m.as_str().parse().ok());

		if age.is_some() {
			return age;
		}
	} else {
		tracing::error!("Failed to create regex for age rating parsing");
	}

	// check final case of just a number
	str_sequence.parse().ok()
}

/// The items of a stored comma-separated list (genres, writers, ...), trimmed. A blank item (a
/// leading, trailing or doubled comma, an empty `<dc:subject/>` in an EPUB that was joined into the
/// text) is never an item: it would show up as an empty chip that links to an empty filter
pub fn comma_separated_list_to_vec(list: String) -> Vec<String> {
	clean_list_items(list.split(','))
}

/// Trims every item and leaves the blank ones out, keeping the order
pub fn clean_list_items<I, S>(items: I) -> Vec<String>
where
	I: IntoIterator<Item = S>,
	S: AsRef<str>,
{
	items
		.into_iter()
		.map(|item| item.as_ref().trim().to_owned())
		.filter(|item| !item.is_empty())
		.collect()
}

/// A list as the text it is stored as: its items cleaned by [`clean_list_items`] and joined with
/// `, `. `None` when nothing is left, so a list of blanks is stored as no list, never as `""` or
/// `", "`
pub fn list_to_comma_separated<I, S>(items: I) -> Option<String>
where
	I: IntoIterator<Item = S>,
	S: AsRef<str>,
{
	let items = clean_list_items(items);
	(!items.is_empty()).then(|| items.join(", "))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_stored_list_has_no_blank_items() {
		for (stored, expected) in [
			("A, B", vec!["A", "B"]),
			(
				", History, History: Archaeology",
				vec!["History", "History: Archaeology"],
			),
			("History, ", vec!["History"]),
			("A,, B", vec!["A", "B"]),
			("A, , B", vec!["A", "B"]),
			("  ,  ", vec![]),
			(",", vec![]),
			("", vec![]),
			("  Solo  ", vec!["Solo"]),
		] {
			assert_eq!(
				comma_separated_list_to_vec(stored.to_string()),
				expected,
				"{stored:?}"
			);
		}
	}

	#[test]
	fn items_are_trimmed_and_blanks_dropped_in_order() {
		assert_eq!(
			clean_list_items(["", " Z ", "   ", "A", "\t"]),
			vec!["Z".to_string(), "A".to_string()]
		);
	}

	#[test]
	fn a_list_is_stored_without_blank_items_and_none_when_empty() {
		assert_eq!(
			list_to_comma_separated(vec!["".to_string(), "History".to_string()]),
			Some("History".to_string())
		);
		assert_eq!(
			list_to_comma_separated(["A", " ", "B"]),
			Some("A, B".to_string())
		);
		assert_eq!(list_to_comma_separated(vec!["".to_string()]), None);
		assert_eq!(list_to_comma_separated(Vec::<String>::new()), None);
	}

	#[test]
	fn what_is_stored_reads_back_the_same() {
		let items = vec!["Biology & Nature", "Form: Reportage"];
		let stored = list_to_comma_separated(items.clone()).unwrap();
		assert_eq!(comma_separated_list_to_vec(stored), items);
	}

	#[test]
	fn the_comma_separated_deserializer_skips_blank_items() {
		#[derive(Deserialize)]
		struct Row {
			#[serde(default, deserialize_with = "string_list_deserializer")]
			list: Option<Vec<String>>,
		}
		let row = |text: &str| -> Option<Vec<String>> {
			serde_json::from_value::<Row>(serde_json::json!({ "list": text }))
				.unwrap()
				.list
		};
		assert_eq!(row("A, , B"), Some(vec!["A".to_string(), "B".to_string()]));
		assert_eq!(row(", A"), Some(vec!["A".to_string()]));
		assert_eq!(row(" , "), None);
		assert_eq!(row(""), None);
	}
}
