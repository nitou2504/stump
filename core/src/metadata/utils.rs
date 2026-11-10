use crate::{
	media::processor::error::MediaProcessorError, metadata::ProcessedMediaMetadata,
};
use models::shared::enums::OrganizationMode;
use quick_xml::de::from_str as xml_from_str;

pub(crate) fn metadata_from_buf(
	contents: &str,
) -> Result<ProcessedMediaMetadata, MediaProcessorError> {
	xml_from_str(contents.trim())
		.map_err(|error| MediaProcessorError::Metadata(error.to_string()))
}

pub(crate) fn comic_info_metadata(
	contents: &str,
	organization_mode: OrganizationMode,
) -> Result<Option<ProcessedMediaMetadata>, MediaProcessorError> {
	match metadata_from_buf(contents) {
		Ok(metadata) => Ok(Some(metadata)),
		Err(error) if organization_mode == OrganizationMode::Filesystem => {
			tracing::warn!(?error, "Ignoring invalid optional ComicInfo.xml");
			Ok(None)
		},
		Err(error) => Err(error),
	}
}

/// Parse a PDF format or general format date string into year, optional month, and optional day.
///
/// Note: Time and timezone components in the PDF date format (`D:YYYYMMDDHHmmSSOHH'mm'`)
/// are intentionally ignored for the purpose of media metadata year/month/day extraction.
pub(crate) fn parse_pdf_date(raw_date: &str) -> Option<(i32, Option<i32>, Option<i32>)> {
	use chrono::Datelike;

	let s = raw_date.trim();
	if s.is_empty() {
		return None;
	}

	// Normalize D: prefix (case-insensitive)
	let s = s
		.strip_prefix("D:")
		.or_else(|| s.strip_prefix("d:"))
		.unwrap_or(s);

	// The date comes from the book (an EPUB's `dc:date`, a PDF's Info dictionary), so it may
	// hold any text. Slicing a `str` at a byte offset inside a multi-byte character panics (and
	// the server aborts on a panic), so the digit checks look at bytes: a range of ASCII digits
	// always ends on a character boundary, which makes the `&s[..n]` slices below safe.
	let ascii_digits = |range: std::ops::Range<usize>| {
		s.as_bytes()
			.get(range)
			.is_some_and(|bytes| bytes.iter().all(u8::is_ascii_digit))
	};

	// 1. PDF spec: D:YYYYMMDDHHmmSS... parse digit-only prefixes
	if ascii_digits(0..4) {
		let has_digit_at_4 = s.chars().nth(4).is_some_and(|c| c.is_ascii_digit());
		let has_separator_at_4 = s
			.chars()
			.nth(4)
			.is_some_and(|c| c == '-' || c == '/' || c == '.' || c == 'T' || c == ' ');

		if !has_separator_at_4 && !has_digit_at_4 {
			if let Ok(y) = s[..4].parse::<i32>() {
				return Some((y, None, None));
			}
		} else if !has_separator_at_4 && has_digit_at_4 {
			if ascii_digits(4..8) {
				if let Ok(d) = chrono::NaiveDate::parse_from_str(&s[..8], "%Y%m%d") {
					return Some((
						d.year(),
						Some(d.month() as i32),
						Some(d.day() as i32),
					));
				}
			}
			if ascii_digits(4..6) {
				if let Ok(d) =
					chrono::NaiveDate::parse_from_str(&format!("{}01", &s[..6]), "%Y%m%d")
				{
					return Some((d.year(), Some(d.month() as i32), None));
				}
			}
		}
	}

	// 2. ISO 8601 / RFC 3339, handles YYYY-MM-DD, MM-DD-YYYY, etc.
	let prefix10 = s.get(..10).unwrap_or(s);
	let normalized10 = prefix10.replace(['/', '.'], "-");
	if let Ok(d) = chrono::NaiveDate::parse_from_str(&normalized10, "%Y-%m-%d") {
		return Some((d.year(), Some(d.month() as i32), Some(d.day() as i32)));
	}
	if let Ok(d) = chrono::NaiveDate::parse_from_str(&normalized10, "%m-%d-%Y") {
		return Some((d.year(), Some(d.month() as i32), Some(d.day() as i32)));
	}

	// Try parsing YYYY-MM or MM-YYYY formats by normalizing separators
	// `get` is None when byte 7 falls inside a character: no date starts with one of those
	if let Some(prefix7) = s.get(..7) {
		let normalized7 = prefix7.replace(['/', '.'], "-");
		// Try YYYY-MM (e.g. 2019-08)
		if let Ok(d) =
			chrono::NaiveDate::parse_from_str(&format!("{normalized7}-01"), "%Y-%m-%d")
		{
			return Some((d.year(), Some(d.month() as i32), None));
		}
		// Try MM-YYYY (e.g. 08-2019)
		if let Ok(d) =
			chrono::NaiveDate::parse_from_str(&format!("01-{normalized7}"), "%d-%m-%Y")
		{
			return Some((d.year(), Some(d.month() as i32), None));
		}
	}

	s.split(|c: char| !c.is_ascii_digit())
		.find(|tok| tok.len() == 4)
		.and_then(|tok| tok.parse::<i32>().ok())
		.map(|y| (y, None, None))
		.or_else(|| s.parse::<i32>().ok().map(|y| (y, None, None)))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_should_parse_incomplete_metadata() {
		let contents = "<?xml version=\"1.0\"?>\n<ComicInfo xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n  <Series>Delete</Series>\n  <Number>1</Number>\n  <Volume>2016</Volume>\n  <Summary>In the near future, where science can implant or remove human memories and the government uses brain scan technology in criminal investigations, a mute girl witnesses a multiple murder and must turn to a handyman for protection from the police and an army of killers. From the Harley Quinn team of writers Jimmy Palmiotti and Justin Grey and artist John Timms, with covers by Amanda Conner.\n\n\nNote: The digital edition (3/2/2016) for this issue was released before the print edition.</Summary>\n  <Notes>Tagged with ComicTagger 1.3.0-alpha.0 using info from Comic Vine on 2021-12-01 20:34:52.  [Issue ID 517895]</Notes>\n  <Year>2016</Year>\n  <Month>03</Month>\n  <Day>31</Day>\n  <Writer>Jimmy Palmiotti, Justin Gray</Writer>\n  <Penciller>John Timms, John Timms</Penciller>\n  <Inker>John Timms, John Timms</Inker>\n  <Colorist>David Curiel, Paul Mounts</Colorist>\n  <Letterer>Bill Tortolini</Letterer>\n  <CoverArtist>Amanda Conner, Paul Mounts</CoverArtist>\n  <Editor>Alex Wald, Joanne Starer</Editor>\n  <Publisher>1First Comics</Publisher>\n  <Web>https://comicvine.gamespot.com/delete-1/4000-517895/</Web>\n  <PageCount>27</PageCount>\n  <ScanInformation>(digital) (Son of Ultron-Empire)</ScanInformation>\n  <Pages>\n    <Page Image=\"0\" ImageSize=\"907332\" Type=\"FrontCover\" />\n    <Page Image=\"1\" ImageSize=\"431378\" />\n    <Page Image=\"2\" ImageSize=\"776720\" />\n    <Page Image=\"3\" ImageSize=\"524902\" />\n    <Page Image=\"4\" ImageSize=\"753942\" />\n    <Page Image=\"5\" ImageSize=\"607990\" />\n    <Page Image=\"6\" ImageSize=\"438880\" />\n    <Page Image=\"7\" ImageSize=\"504806\" />\n    <Page Image=\"8\" ImageSize=\"532746\" />\n    <Page Image=\"9\" ImageSize=\"542816\" />\n    <Page Image=\"10\" ImageSize=\"571650\" />\n    <Page Image=\"11\" ImageSize=\"626656\" />\n    <Page Image=\"12\" ImageSize=\"605810\" />\n    <Page Image=\"13\" ImageSize=\"585234\" />\n    <Page Image=\"14\" ImageSize=\"553270\" />\n    <Page Image=\"15\" ImageSize=\"440568\" />\n    <Page Image=\"16\" ImageSize=\"483816\" />\n    <Page Image=\"17\" ImageSize=\"492922\" />\n    <Page Image=\"18\" ImageSize=\"470748\" />\n    <Page Image=\"19\" ImageSize=\"644256\" />\n    <Page Image=\"20\" ImageSize=\"584142\" />\n    <Page Image=\"21\" ImageSize=\"425322\" />\n    <Page Image=\"22\" ImageSize=\"565166\" />\n    <Page Image=\"23\" ImageSize=\"582706\" />\n    <Page Image=\"24\" ImageSize=\"507370\" />\n    <Page Image=\"25\" ImageSize=\"489280\" />\n    <Page Image=\"26\" ImageSize=\"519906\" />\n  </Pages>\n</ComicInfo>";
		let metadata = metadata_from_buf(contents).unwrap();

		assert_eq!(metadata.series, Some("Delete".to_string()));
		assert_eq!(metadata.number, Some(1f64));
		assert_eq!(metadata.volume, Some(2016));
	}

	#[test]
	fn test_malformed_media_xml() {
		// An empty string
		let contents = "";
		let metadata = metadata_from_buf(contents);
		assert!(metadata.is_err());

		// Something JSON-ish instead of xml
		let contents = "metadata: { contents: oops }";
		let metadata = metadata_from_buf(contents);
		assert!(metadata.is_err());
	}

	// Asserts a fix for an issue reported wrt empty tags in ComicInfo.xml, e.g. <Number></Number> or <Number/>
	#[test]
	fn test_should_parse_metadata_with_empty_tags() {
		let contents = r#"<?xml version="1.0"?>
<ComicInfo xmlns:xsi="http://www.idk.bizbaz" xmlns:xsd="http://www.idk.bizbaz">
  <Title>Test Comic</Title>
  <Series>Test Series</Series>
  <Number></Number>
  <Volume></Volume>
  <Summary>A test summary</Summary>
</ComicInfo>"#;
		let metadata = metadata_from_buf(contents);
		assert!(metadata.is_ok(), "Should parse XML with empty tags");
		let metadata = metadata.unwrap();
		assert_eq!(metadata.title, Some("Test Comic".to_string()));
		assert_eq!(metadata.series, Some("Test Series".to_string()));
		assert_eq!(metadata.number, None);
		assert_eq!(metadata.volume, None);
	}

	// Asserts a fix for an issue reported wrt empty tags in ComicInfo.xml, e.g. <Number></Number> or <Number/>
	#[test]
	fn test_should_parse_metadata_with_self_closing_tags() {
		let contents = r#"<?xml version="1.0"?>
<ComicInfo xmlns:xsi="http://www.idk.bizbaz" xmlns:xsd="http://www.idk.bizbaz">
  <Title>Test Comic</Title>
  <Series>Test Series</Series>
  <Number/>
  <Volume/>
  <Summary>A test summary</Summary>
</ComicInfo>"#;
		let metadata = metadata_from_buf(contents);
		assert!(metadata.is_ok(), "Should parse XML with self-closing tags");
		let metadata = metadata.unwrap();
		assert_eq!(metadata.title, Some("Test Comic".to_string()));
		assert_eq!(metadata.series, Some("Test Series".to_string()));
		assert_eq!(metadata.number, None);
		assert_eq!(metadata.volume, None);
	}

	#[test]
	fn test_should_parse_comic_info_tags() {
		let contents = r#"<?xml version="1.0"?>
<ComicInfo>
  <Title>Test Comic</Title>
  <Tags>superhero, origin story, dark</Tags>
</ComicInfo>"#;
		let metadata = metadata_from_buf(contents).expect("should parse");
		assert_eq!(
			metadata.tags,
			Some(vec![
				"superhero".to_string(),
				"origin story".to_string(),
				"dark".to_string(),
			])
		);
	}

	#[test]
	fn test_should_parse_comic_info_with_empty_tags_element() {
		let contents = r#"<?xml version="1.0"?>
<ComicInfo>
  <Title>Test Comic</Title>
  <Tags></Tags>
</ComicInfo>"#;
		let metadata = metadata_from_buf(contents).expect("should parse");
		assert_eq!(metadata.tags, None);
	}

	#[test]
	fn test_parse_pdf_date() {
		let cases = vec![
			// PDF specific formats
			("D:20190101", Some((2019, Some(1), Some(1)))),
			("D:20190101123456Z", Some((2019, Some(1), Some(1)))),
			("d:20190101123456+01'00'", Some((2019, Some(1), Some(1)))),
			("20190101", Some((2019, Some(1), Some(1)))),
			("D:2019", Some((2019, None, None))),
			("201901", Some((2019, Some(1), None))),
			("2019010", Some((2019, Some(1), None))),
			("2019010a", Some((2019, Some(1), None))),
			// Standard ISO-like with separators
			("2019-08-31", Some((2019, Some(8), Some(31)))),
			("2019/08/31", Some((2019, Some(8), Some(31)))),
			("2019.08.31", Some((2019, Some(8), Some(31)))),
			("08-31-2019", Some((2019, Some(8), Some(31)))),
			("08/31/2019", Some((2019, Some(8), Some(31)))),
			("08.31.2019", Some((2019, Some(8), Some(31)))),
			// Trailing date-time details
			("2019-08-31T12:00:00Z", Some((2019, Some(8), Some(31)))),
			("2019-08-31 12:00:00", Some((2019, Some(8), Some(31)))),
			// Year-month only
			("2019-08", Some((2019, Some(8), None))),
			("08-2019", Some((2019, Some(8), None))),
			("2019/08", Some((2019, Some(8), None))),
			("08/2019", Some((2019, Some(8), None))),
			("2019.08", Some((2019, Some(8), None))),
			("08.2019", Some((2019, Some(8), None))),
			// Standalone years / non-isolated / parsing fallbacks
			("20241", Some((20241, None, None))),
			("Published in 2019", Some((2019, None, None))),
			("2019 year", Some((2019, None, None))),
			("2024", Some((2024, None, None))),
			("1995", Some((1995, None, None))),
			// Invalid formats
			("", None),
			("   ", None),
			("invalid-date", None),
		];

		for (raw, expected) in cases {
			assert_eq!(
				parse_pdf_date(raw),
				expected,
				"Failed parsing pdf date for: {:?}",
				raw
			);
		}
	}

	/// A book's date can be any text. Each of these used to slice a multi-byte character in
	/// half and panic, which aborts the whole server (`panic = "abort"`).
	#[test]
	fn test_parse_pdf_date_with_non_ascii_text() {
		let cases = vec![
			// the cut at byte 4 falls inside a character
			("日本語", None),
			("令和3年", None),
			("日付: 2019", Some((2019, None, None))),
			// four digits, then a character the 6- and 8-byte cuts fall inside
			("20201é", None),
			("20201éé", None),
			("2019é", Some((2019, None, None))),
			// the cut at byte 7 falls inside a character
			("xxxxxxé", None),
			("2020-0é", Some((2020, None, None))),
			// a year after non-ASCII text, and non-ASCII text after a full date
			("📅2019", Some((2019, None, None))),
			("D:20190101日本", Some((2019, Some(1), Some(1)))),
			("2019-08-31é", Some((2019, Some(8), Some(31)))),
			// full-width digits are not ASCII digits
			("２０１９", None),
		];

		for (raw, expected) in cases {
			assert_eq!(
				parse_pdf_date(raw),
				expected,
				"Failed parsing pdf date for: {:?}",
				raw
			);
		}
	}

	/// Every string up to six characters over an alphabet of digits, separators and 2, 3 and
	/// 4-byte characters: none may panic, wherever a character boundary falls.
	#[test]
	fn test_parse_pdf_date_never_panics() {
		const ALPHABET: [&str; 9] = ["2", "0", "-", "/", "D", ":", "é", "日", "📅"];

		fn walk(prefix: &mut String, depth: usize) {
			let _ = parse_pdf_date(prefix);
			if depth == 0 {
				return;
			}
			for symbol in ALPHABET {
				prefix.push_str(symbol);
				walk(prefix, depth - 1);
				prefix.truncate(prefix.len() - symbol.len());
			}
		}

		walk(&mut String::new(), 6);
	}
}
