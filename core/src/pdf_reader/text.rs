//! The text layer of a PDF page: PDFium's chars grouped into runs (one per line or column part),
//! the shape the web reader draws as invisible, selectable text over the page picture.
//!
//! The grouping walks the chars in PDFium order and keeps consecutive chars on one baseline
//! together. Unlike `count_rects` + `get_text_bounded`, it never duplicates text where boxes
//! overlap and it keeps PDFium's generated line ends (see `/tmp/pdf-lane/PLAN.md`, "Text-run
//! algorithm").

use serde::{ser::SerializeTuple, Serialize, Serializer};

/// One char of a page as PDFium reports it, with its loose box already mapped to the displayed
/// page (crop box and rotation applied), in points, origin top-left, y down.
#[derive(Debug, Clone, PartialEq)]
pub struct PageChar {
	/// The PDFium char index on the page
	pub index: i32,
	pub ch: char,
	/// `[left, top, right, bottom]` in display points, or `None` when PDFium has no box
	pub bounds: Option<[f32; 4]>,
}

/// A run of text: consecutive chars on one baseline without a large gap
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
	/// `[left, top, right, bottom]` in display points
	pub bounds: [f32; 4],
	pub text: String,
	/// The PDFium char index of the run's first char; the run's chars are consecutive indices
	pub first_char: i32,
	/// The PDF's text has a line end after this run
	pub eol: bool,
	/// `[left, right]` of every char of `text` (in `text.chars()` order), in display points.
	/// Used to place search hits; not sent to the client.
	pub char_spans: Vec<[f32; 2]>,
}

/// The text layer of one page
#[derive(Debug, Clone, PartialEq)]
pub struct PageText {
	/// Displayed page width in points
	pub width: f32,
	/// Displayed page height in points
	pub height: f32,
	pub runs: Vec<TextRun>,
}

/// PDFium's markers for a hyphen at a line break
fn normalize_char(ch: char) -> char {
	match ch {
		'\u{2}' | '\u{FFFE}' => '-',
		other => other,
	}
}

struct RunBuilder {
	bounds: [f32; 4],
	text: String,
	first_char: i32,
	char_spans: Vec<[f32; 2]>,
}

impl RunBuilder {
	fn height(&self) -> f32 {
		(self.bounds[3] - self.bounds[1]).max(1.0)
	}
}

/// Groups a page's chars into runs. `width`/`height` are the displayed page size in points.
pub fn group_runs(chars: &[PageChar]) -> Vec<TextRun> {
	let mut runs: Vec<TextRun> = Vec::new();
	let mut current: Option<RunBuilder> = None;

	fn flush(current: &mut Option<RunBuilder>, runs: &mut Vec<TextRun>, eol: bool) {
		match current.take() {
			Some(run) if !run.text.trim().is_empty() => runs.push(TextRun {
				bounds: run.bounds,
				text: run.text,
				first_char: run.first_char,
				eol,
				char_spans: run.char_spans,
			}),
			Some(_) => {},
			None => {
				if eol {
					if let Some(last) = runs.last_mut() {
						last.eol = true;
					}
				}
			},
		}
	}

	for (position, page_char) in chars.iter().enumerate() {
		let ch = page_char.ch;
		if ch == '\r' || ch == '\n' {
			// PDFium marks a line end with a generated "\r\n": end the run once, on the '\n' (or
			// on a lone '\r')
			let next_is_newline =
				chars.get(position + 1).is_some_and(|next| next.ch == '\n');
			if ch == '\n' || !next_is_newline {
				flush(&mut current, &mut runs, true);
			}
			continue;
		}
		let ch = normalize_char(ch);

		let bounds = page_char
			.bounds
			.filter(|[l, t, r, b]| r > l || b > t || ch == ' ');
		let Some([left, top, right, bottom]) = bounds else {
			// No usable box: keep the char inside the current run so char indices stay
			// consecutive, without moving the run's box
			if let Some(run) = current.as_mut() {
				let x = run.bounds[2];
				run.text.push(ch);
				run.char_spans.push([x, x]);
			}
			continue;
		};

		if right - left <= 0.0 && ch == ' ' {
			if let Some(run) = current.as_mut() {
				let x = run.bounds[2];
				run.text.push(' ');
				run.char_spans.push([x, x]);
			}
			continue;
		}

		if let Some(run) = current.as_ref() {
			let height = run.height();
			let same_line = (bottom - run.bounds[3]).abs() < height * 0.5
				&& (top - run.bounds[1]).abs() < height * 0.6;
			let gap = left - run.bounds[2];
			if !same_line || gap > height * 1.5 || gap < -height {
				flush(&mut current, &mut runs, false);
			}
		}

		match current.as_mut() {
			None => {
				current = Some(RunBuilder {
					bounds: [left, top, right, bottom],
					text: ch.to_string(),
					first_char: page_char.index,
					char_spans: vec![[left, right]],
				});
			},
			Some(run) => {
				run.text.push(ch);
				run.char_spans.push([left, right]);
				run.bounds = [
					run.bounds[0].min(left),
					run.bounds[1].min(top),
					run.bounds[2].max(right),
					run.bounds[3].max(bottom),
				];
			},
		}
	}
	flush(&mut current, &mut runs, true);
	runs
}

fn round5(value: f32) -> f32 {
	(value * 100_000.0).round() / 100_000.0
}

fn fraction(value: f32, total: f32) -> f32 {
	if total <= 0.0 {
		return 0.0;
	}
	round5((value / total).clamp(0.0, 1.0))
}

impl PageText {
	/// A run's `[x, y, width, height]` as fractions of the displayed page
	pub fn run_rect(&self, run: &TextRun) -> [f32; 4] {
		let [left, top, right, bottom] = run.bounds;
		[
			fraction(left, self.width),
			fraction(top, self.height),
			fraction(right - left, self.width),
			fraction(bottom - top, self.height),
		]
	}

	/// The box of the chars `from..to` (positions in `run.text.chars()`) as page fractions
	pub fn span_rect(&self, run: &TextRun, from: usize, to: usize) -> [f32; 4] {
		let spans = &run.char_spans;
		let to = to.min(spans.len());
		if from >= to {
			return self.run_rect(run);
		}
		let left = spans[from..to]
			.iter()
			.map(|span| span[0])
			.fold(f32::INFINITY, f32::min);
		let right = spans[from..to]
			.iter()
			.map(|span| span[1])
			.fold(f32::NEG_INFINITY, f32::max);
		let (left, right) = if left.is_finite() && right > left {
			(left, right)
		} else {
			(run.bounds[0], run.bounds[2])
		};
		[
			fraction(left, self.width),
			fraction(run.bounds[1], self.height),
			fraction(right - left, self.width),
			fraction(run.bounds[3] - run.bounds[1], self.height),
		]
	}
}

/// The JSON sent by `GET /api/v2/pdf/{id}/text/{n}`:
/// `{"w": 612.0, "h": 792.0, "runs": [[x, y, w, h, text, firstChar, eol], …]}`
impl Serialize for PageText {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		struct Run<'a>(&'a PageText, &'a TextRun);
		impl Serialize for Run<'_> {
			fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
				let [x, y, w, h] = self.0.run_rect(self.1);
				let mut tuple = serializer.serialize_tuple(7)?;
				tuple.serialize_element(&x)?;
				tuple.serialize_element(&y)?;
				tuple.serialize_element(&w)?;
				tuple.serialize_element(&h)?;
				tuple.serialize_element(&self.1.text)?;
				tuple.serialize_element(&self.1.first_char)?;
				tuple.serialize_element(&u8::from(self.1.eol))?;
				tuple.end()
			}
		}

		#[derive(Serialize)]
		struct Body<'a> {
			w: f32,
			h: f32,
			runs: Vec<Run<'a>>,
		}

		Body {
			w: (self.width * 10.0).round() / 10.0,
			h: (self.height * 10.0).round() / 10.0,
			runs: self.runs.iter().map(|run| Run(self, run)).collect(),
		}
		.serialize(serializer)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn chars_of(
		line: &str,
		first_index: i32,
		x: f32,
		top: f32,
		size: f32,
	) -> Vec<PageChar> {
		line.chars()
			.enumerate()
			.map(|(i, ch)| {
				let left = x + i as f32 * size * 0.5;
				PageChar {
					index: first_index + i as i32,
					ch,
					bounds: Some([left, top, left + size * 0.5, top + size]),
				}
			})
			.collect()
	}

	fn line_end(index: i32) -> Vec<PageChar> {
		vec![
			PageChar {
				index,
				ch: '\r',
				bounds: None,
			},
			PageChar {
				index: index + 1,
				ch: '\n',
				bounds: None,
			},
		]
	}

	#[test]
	fn groups_lines_and_keeps_line_ends() {
		let mut chars = chars_of("Hello world", 0, 72.0, 100.0, 10.0);
		chars.extend(line_end(11));
		chars.extend(chars_of("second line", 13, 72.0, 112.0, 10.0));
		let runs = group_runs(&chars);
		assert_eq!(runs.len(), 2);
		assert_eq!(runs[0].text, "Hello world");
		assert_eq!(runs[0].first_char, 0);
		assert!(runs[0].eol);
		assert_eq!(runs[1].text, "second line");
		assert_eq!(runs[1].first_char, 13);
		assert!(runs[1].eol, "the last run of a page ends a line");
		assert_eq!(runs[0].char_spans.len(), runs[0].text.chars().count());
	}

	#[test]
	fn splits_columns_on_a_large_gap_without_duplicating_text() {
		let mut chars = chars_of("left", 0, 72.0, 100.0, 10.0);
		// the right column starts 200pt further on the same baseline
		chars.extend(chars_of("right", 4, 300.0, 100.0, 10.0));
		let runs = group_runs(&chars);
		let texts: Vec<_> = runs.iter().map(|run| run.text.as_str()).collect();
		assert_eq!(texts, vec!["left", "right"]);
		assert!(!runs[0].eol, "a column split is not a line end");
		let joined: String = runs.iter().map(|run| run.text.as_str()).collect();
		assert_eq!(joined.matches("right").count(), 1);
	}

	#[test]
	fn maps_hyphen_markers_and_keeps_zero_width_spaces() {
		let mut chars = chars_of("commensura", 0, 72.0, 100.0, 10.0);
		chars.push(PageChar {
			index: 10,
			ch: '\u{2}',
			bounds: Some([122.0, 100.0, 126.0, 110.0]),
		});
		chars.extend(line_end(11));
		let mut word = chars_of("bility of", 13, 72.0, 112.0, 10.0);
		// a generated space with no box between words keeps the indices consecutive
		word[6].bounds = Some([102.0, 112.0, 102.0, 122.0]);
		chars.extend(word);
		let runs = group_runs(&chars);
		assert_eq!(runs[0].text, "commensura-");
		assert_eq!(runs[1].text, "bility of");
		assert_eq!(runs[1].text.chars().count(), 9);
		assert_eq!(runs[1].first_char, 13);
	}

	#[test]
	fn drops_whitespace_only_runs_but_keeps_their_line_end() {
		let mut chars = chars_of("text", 0, 72.0, 100.0, 10.0);
		chars.extend(chars_of("   ", 4, 400.0, 300.0, 10.0));
		chars.extend(line_end(7));
		let runs = group_runs(&chars);
		assert_eq!(runs.len(), 1);
		assert_eq!(runs[0].text, "text");
	}

	#[test]
	fn serializes_the_contract_shape() {
		let chars = chars_of("Hi", 5, 61.2, 79.2, 12.0);
		let text = PageText {
			width: 612.0,
			height: 792.0,
			runs: group_runs(&chars),
		};
		// through the JSON text, as clients see it (f32 values print shortest: 0.1)
		let json: serde_json::Value =
			serde_json::from_str(&serde_json::to_string(&text).unwrap()).unwrap();
		assert_eq!(json["w"], 612.0);
		assert_eq!(json["h"], 792.0);
		let run = &json["runs"][0];
		assert_eq!(run[0].as_f64().unwrap(), 0.1);
		assert_eq!(run[1].as_f64().unwrap(), 0.1);
		assert_eq!(run[4], "Hi");
		assert_eq!(run[5], 5);
		assert_eq!(run[6], 1);
	}

	#[test]
	fn span_rects_cover_only_the_chars_asked_for() {
		let chars = chars_of("abcdef", 0, 100.0, 100.0, 10.0);
		let text = PageText {
			width: 1000.0,
			height: 1000.0,
			runs: group_runs(&chars),
		};
		let [x, _, w, _] = text.span_rect(&text.runs[0], 2, 4);
		assert!((x - 0.11).abs() < 1e-4, "x = {x}");
		assert!((w - 0.01).abs() < 1e-4, "w = {w}");
	}
}
