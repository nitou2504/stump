//! What somebody who needs approval may put into their own item. An approver (the server owner, or
//! anyone who manages libraries) is trusted and has none of these limits; a friend's descriptions
//! end up shown on the owner's screens, so they are plain text, every text has a length, and an
//! age rating can be kept or raised, never lowered to slip a book past a restricted reader.

use models::shared::intake::IntakeReviewInput;

/// The longest title, series, publisher or one author's name
pub const MAX_NAME_CHARS: usize = 300;
/// The longest description
pub const MAX_DESCRIPTION_CHARS: usize = 20_000;
/// The most authors, and tags, of one item
pub const MAX_AUTHORS: usize = 30;
pub const MAX_TAGS: usize = 50;
/// The longest tag
pub const MAX_TAG_CHARS: usize = 100;
/// The longest note an approver writes when sending an item back
pub const MAX_NOTE_CHARS: usize = 1000;
/// The years a book can have been published in
pub const MIN_YEAR: i32 = 1000;
pub const MAX_YEAR: i32 = 2100;

/// Text without markup or control characters: what remains when everything between `<` and `>`
/// is removed (an unclosed `<` takes the rest of the text), then any stray `>`. Newlines and tabs
/// stay. The description of a book is shown as Markdown on screens that also render the HTML in
/// it, so a description from somebody else must not carry any.
pub fn plain_text(text: &str) -> String {
	let mut out = String::with_capacity(text.len());
	let mut in_tag = false;
	for c in text.chars() {
		match c {
			'<' => in_tag = true,
			'>' if in_tag => in_tag = false,
			'>' => {},
			_ if in_tag => {},
			'\n' | '\t' => out.push(c),
			c if c.is_control() => {},
			c => out.push(c),
		}
	}
	out.trim().to_string()
}

fn too_long(what: &str, max: usize, value: &str) -> Option<String> {
	(value.chars().count() > max)
		.then(|| format!("{what} can't be longer than {max} characters"))
}

/// The first thing wrong with what an uploader who needs approval sent, if any. Folders, file
/// names and the kind of series are not theirs to choose.
pub fn check_uploader_review(input: &IntakeReviewInput) -> Result<(), String> {
	if input.folder.is_some()
		|| input.file_names.is_some()
		|| input.series_kind.is_some()
		|| input.clear_series_kind
	{
		return Err(
			"The folder, the file names and the kind of series are chosen by whoever approves the book"
				.to_string(),
		);
	}
	let text = |what: &str, value: &Option<String>, max: usize| {
		value
			.as_deref()
			.and_then(|value| too_long(what, max, value))
	};
	for problem in [
		text("The title", &input.title, MAX_NAME_CHARS),
		text("The series", &input.series, MAX_NAME_CHARS),
		text("The publisher", &input.publisher, MAX_NAME_CHARS),
		text("The description", &input.description, MAX_DESCRIPTION_CHARS),
	]
	.into_iter()
	.flatten()
	{
		return Err(problem);
	}
	if let Some(authors) = &input.authors {
		if authors.len() > MAX_AUTHORS {
			return Err(format!("A book can't have more than {MAX_AUTHORS} authors"));
		}
		if let Some(problem) = authors
			.iter()
			.find_map(|a| too_long("An author's name", MAX_NAME_CHARS, a))
		{
			return Err(problem);
		}
	}
	if let Some(genres) = &input.genres {
		if genres.len() > MAX_TAGS {
			return Err(format!("A book can't have more than {MAX_TAGS} tags"));
		}
		if let Some(problem) = genres
			.iter()
			.find_map(|g| too_long("A tag", MAX_TAG_CHARS, g))
		{
			return Err(problem);
		}
	}
	if let Some(year) = input.year {
		if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
			return Err(format!(
				"The year must be between {MIN_YEAR} and {MAX_YEAR}"
			));
		}
	}
	Ok(())
}

/// Whether a book's age rating can become `new` for somebody who needs approval: it can stay or
/// go up (no rating counts as the lowest), never down
pub fn check_age_rating(current: Option<i32>, new: i32) -> Result<(), String> {
	if new < current.unwrap_or(0) {
		Err("The age rating can be kept or raised, but not lowered".to_string())
	} else {
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn markup_is_removed_from_a_description() {
		assert_eq!(plain_text("A plain description."), "A plain description.");
		assert_eq!(
			plain_text("Hello <b>world</b><script>alert(1)</script>!"),
			"Hello worldalert(1)!"
		);
		assert_eq!(plain_text(r#"<img src=x onerror="alert(1)">Text"#), "Text");
		// an unclosed tag takes the rest, a stray > goes
		assert_eq!(plain_text("before <img src=x onerror=alert(1)"), "before");
		assert_eq!(plain_text("1 > 0"), "1  0");
		// lines and tabs stay, other control characters go
		assert_eq!(plain_text("one\ntwo\tthree\u{0}\u{7}"), "one\ntwo\tthree");
		assert_eq!(plain_text("  padded  "), "padded");
		assert_eq!(plain_text("<!-- c -->kept"), "kept");
	}

	#[test]
	fn an_uploader_cannot_choose_where_the_book_goes() {
		for input in [
			IntakeReviewInput {
				folder: Some("Elsewhere".into()),
				..Default::default()
			},
			IntakeReviewInput {
				file_names: Some(vec!["x.epub".into()]),
				..Default::default()
			},
			IntakeReviewInput {
				clear_series_kind: true,
				..Default::default()
			},
		] {
			assert!(check_uploader_review(&input).is_err());
		}
		assert!(check_uploader_review(&IntakeReviewInput {
			title: Some("A title".into()),
			..Default::default()
		})
		.is_ok());
	}

	#[test]
	fn every_text_has_a_length() {
		let long = "x".repeat(MAX_NAME_CHARS + 1);
		let review = |input: IntakeReviewInput| check_uploader_review(&input);
		assert!(review(IntakeReviewInput {
			title: Some(long.clone()),
			..Default::default()
		})
		.is_err());
		assert!(review(IntakeReviewInput {
			description: Some("x".repeat(MAX_DESCRIPTION_CHARS + 1)),
			..Default::default()
		})
		.is_err());
		assert!(review(IntakeReviewInput {
			authors: Some(vec![long]),
			..Default::default()
		})
		.is_err());
		assert!(review(IntakeReviewInput {
			authors: Some(vec!["a".into(); MAX_AUTHORS + 1]),
			..Default::default()
		})
		.is_err());
		assert!(review(IntakeReviewInput {
			genres: Some(vec!["g".into(); MAX_TAGS + 1]),
			..Default::default()
		})
		.is_err());
		assert!(review(IntakeReviewInput {
			genres: Some(vec!["g".repeat(MAX_TAG_CHARS + 1)]),
			..Default::default()
		})
		.is_err());
		assert!(review(IntakeReviewInput {
			genres: Some(vec!["g".into(); MAX_TAGS]),
			description: Some("x".repeat(MAX_DESCRIPTION_CHARS)),
			..Default::default()
		})
		.is_ok());
	}

	#[test]
	fn a_year_is_a_plausible_one() {
		let year = |year| {
			check_uploader_review(&IntakeReviewInput {
				year: Some(year),
				..Default::default()
			})
		};
		assert!(year(999).is_err());
		assert!(year(1000).is_ok());
		assert!(year(2100).is_ok());
		assert!(year(2101).is_err());
		assert!(year(-5).is_err());
	}

	#[test]
	fn an_age_rating_is_kept_or_raised() {
		assert!(check_age_rating(Some(18), 0).is_err());
		assert!(check_age_rating(Some(18), 18).is_ok());
		assert!(check_age_rating(Some(0), 18).is_ok());
		assert!(check_age_rating(None, 0).is_ok());
		assert!(check_age_rating(None, 18).is_ok());
		assert!(check_age_rating(Some(13), 12).is_err());
	}
}
