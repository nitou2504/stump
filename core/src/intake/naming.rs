//! Folder and file naming of the curated library, and matching of author names to the
//! folders that already exist.
//!
//! The curated library keeps `<Author_Folder>/<Title_File>.<ext>`, both ASCII-folded with
//! underscores: `A. C. Grayling` -> `A_C_Grayling`, `José Joaquín de Olmedo` ->
//! `Jose_Joaquin_de_Olmedo`, `Russell: A Very Short Introduction` ->
//! `Russell_A_Very_Short_Introduction`. Hyphens are kept (`Adam_Hart-Davis`), anything else
//! that is not a letter or a digit becomes one underscore, and names stop at 90 characters.
//! Older imports dropped hyphens (`Arturo_PerezReverte`), which the matching keys absorb.
//! Manga and light novel series keep their readable name: `<Series>/<Series> vNN.<ext>`.

use std::sync::LazyLock;

use regex::Regex;

/// The longest file or folder name the curated library uses
pub const MAX_NAME_LEN: usize = 90;

/// ASCII-folds a name the way the curated library names its folders and files
pub fn fold_name(name: &str) -> String {
	let ascii = unidecode::unidecode(&name.replace(['’', '‘', '`'], ""));
	let mut out = String::with_capacity(ascii.len());
	let mut pending_sep = false;
	for c in ascii.chars() {
		if c.is_ascii_alphanumeric() || c == '-' {
			if pending_sep && !out.is_empty() {
				out.push('_');
			}
			pending_sep = false;
			out.push(c);
		} else {
			pending_sep = true;
		}
	}
	let trimmed = out.trim_matches(|c| c == '_' || c == '-');
	truncate_name(trimmed)
}

/// Cuts a folded name at [`MAX_NAME_LEN`], preferring a word boundary
fn truncate_name(name: &str) -> String {
	if name.len() <= MAX_NAME_LEN {
		return name.to_string();
	}
	let cut = &name[..MAX_NAME_LEN];
	let at_word = cut
		.rfind('_')
		.filter(|i| *i > MAX_NAME_LEN / 2)
		.map(|i| &cut[..i])
		.unwrap_or(cut);
	at_word.trim_matches(|c| c == '_' || c == '-').to_string()
}

/// The lowercase alphanumeric words of a name, accents removed
pub fn name_tokens(name: &str) -> Vec<String> {
	unidecode::unidecode(name)
		.to_lowercase()
		.split(|c: char| !c.is_ascii_alphanumeric())
		.filter(|t| !t.is_empty())
		.map(str::to_string)
		.collect()
}

/// `Eco, Umberto` -> `Umberto Eco`. Names with more than one comma are left alone.
pub fn reorder_last_first(name: &str) -> String {
	let parts: Vec<&str> = name.split(',').map(str::trim).collect();
	match parts.as_slice() {
		[last, first] if !last.is_empty() && !first.is_empty() => {
			format!("{first} {last}")
		},
		_ => name.trim().to_string(),
	}
}

/// How the library writes an author: `Eco, Umberto` -> `Umberto Eco` (Stump splits writers
/// on commas), keeping suffixes such as `King, Jr.` and names with more than one comma
pub fn display_name(name: &str) -> String {
	let trimmed = name.trim();
	let parts: Vec<&str> = trimmed.split(',').map(str::trim).collect();
	if let [last, first] = parts.as_slice() {
		let suffix = ["jr", "jr.", "sr", "sr.", "ii", "iii", "iv"].contains(&first.to_lowercase().as_str());
		if !suffix && !last.is_empty() && !first.is_empty() && last.split_whitespace().count() <= 3 {
			return format!("{first} {last}");
		}
	}
	trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The keys an author name or folder can be matched by: the words squashed together in
/// order (`umbertoeco`, also absorbing dropped hyphens and spaces) and in sorted order
/// (`ecoumberto`, absorbing `Last First` without a comma)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameKeys {
	pub ordered: String,
	pub sorted: String,
}

pub fn name_keys(name: &str) -> NameKeys {
	let tokens = name_tokens(&reorder_last_first(name));
	let ordered = tokens.concat();
	let mut sorted_tokens = tokens;
	sorted_tokens.sort();
	NameKeys {
		ordered,
		sorted: sorted_tokens.concat(),
	}
}

/// The key a folder name is matched by (folders never carry `Last, First`)
pub fn folder_keys(folder: &str) -> NameKeys {
	let tokens = name_tokens(folder);
	let ordered = tokens.concat();
	let mut sorted_tokens = tokens;
	sorted_tokens.sort();
	NameKeys {
		ordered,
		sorted: sorted_tokens.concat(),
	}
}

/// The file name for a book: the folded title plus the extension
pub fn book_file_name(title: &str, extension: &str) -> String {
	let stem = fold_name(title);
	let stem = if stem.is_empty() { "Untitled".to_string() } else { stem };
	format!("{stem}.{}", extension.to_lowercase())
}

/// A series folder name: readable, only characters that are unsafe in paths removed
pub fn series_folder_name(series: &str) -> String {
	let cleaned: String = series
		.chars()
		.map(|c| match c {
			'/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
			c if c.is_control() => ' ',
			c => c,
		})
		.collect();
	let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
	collapsed.trim_matches('.').trim().to_string()
}

/// `<Series> vNN.<ext>` (two digits, more when needed; decimals kept: `v10.5`)
pub fn series_volume_file_name(series: &str, volume: Option<f64>, extension: &str) -> String {
	let series = series_folder_name(series);
	let ext = extension.to_lowercase();
	match volume {
		Some(v) if v.fract() == 0.0 => format!("{series} v{:02}.{ext}", v as i64),
		Some(v) => format!("{series} v{v}.{ext}"),
		None => format!("{series}.{ext}"),
	}
}

/// What a file name alone says about a book
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedFileName {
	pub title: Option<String>,
	pub authors: Vec<String>,
	pub series: Option<String>,
	pub volume: Option<f64>,
}

static VOLUME_RE: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?ix)
		(?:^|[\s_\-\.(\[,])
		(?:v|vol|vol\.|volume|volumen|tomo|t\.|book|libro|\#)
		[\s_\.]*
		(\d{1,4}(?:\.\d)?)
		(?:$|[\s_\-\.)\],])",
	)
	.expect("valid volume regex")
});

static SERIES_PAREN_RE: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"\(([^()]+?)\s*#\s*(\d{1,4}(?:\.\d+)?)\)").expect("valid series regex")
});

static TRAILING_NUMBER_RE: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"^(.*?)[\s_\-]+(\d{1,3})$").expect("valid trailing number regex")
});

/// Reads a file name like `Author - Title`, `Title (Series #2)`, `Series v03` or
/// `Series - 03`. The extension must already be removed.
pub fn parse_file_name(stem: &str) -> ParsedFileName {
	let mut parsed = ParsedFileName::default();
	// underscores stand for spaces in most library-style names
	let spaced = if stem.contains(' ') {
		stem.to_string()
	} else {
		stem.replace('_', " ")
	};
	let mut rest = spaced.trim().to_string();

	if let Some(caps) = SERIES_PAREN_RE.captures(&rest) {
		parsed.series = Some(caps[1].trim().to_string());
		parsed.volume = caps[2].parse().ok();
		rest = SERIES_PAREN_RE.replace(&rest, "").trim().to_string();
	}

	if parsed.volume.is_none() {
		if let Some(caps) = VOLUME_RE.captures(&rest) {
			let whole = caps.get(0).expect("match");
			parsed.volume = caps[1].parse().ok();
			let before = rest[..whole.start()].trim().trim_end_matches(['-', ',']).trim();
			if !before.is_empty() {
				parsed.series = Some(before.to_string());
			}
			let after = rest[whole.end()..].trim().trim_start_matches(['-', ':']).trim();
			rest = if after.is_empty() {
				before.to_string()
			} else {
				format!("{before} - {after}")
			};
		} else if let Some(caps) = TRAILING_NUMBER_RE.captures(&rest) {
			let before = caps[1].trim().trim_end_matches('-').trim().to_string();
			// "Series - 03": a bare trailing number reads as a volume only after a dash
			if rest.contains(" - ") || rest.contains(" -") {
				parsed.volume = caps[2].parse().ok();
				parsed.series = Some(before.clone());
				rest = before;
			}
		}
	}

	// "Author - Title": the first part is an author when it looks like a name
	if let Some((left, right)) = rest.split_once(" - ") {
		let left = left.trim();
		let right = right.trim();
		if looks_like_person(left) && !right.is_empty() {
			parsed.authors = split_authors(left);
			rest = right.to_string();
		}
	}

	let title = rest.trim().trim_matches(['-', ',']).trim().to_string();
	if !title.is_empty() {
		parsed.title = Some(title);
	}
	parsed
}

/// Two to five capitalised words (initials allowed), or `Last, First`
fn looks_like_person(text: &str) -> bool {
	if text.contains(',') {
		let parts: Vec<&str> = text.split(',').collect();
		return parts.len() == 2 && parts.iter().all(|p| looks_like_person(p.trim()) || p.trim().split_whitespace().count() == 1);
	}
	let words: Vec<&str> = text
		.split(['&', ';'])
		.next()
		.unwrap_or("")
		.split_whitespace()
		.collect();
	(2..=5).contains(&words.len())
		&& words.iter().all(|w| {
			let first = w.chars().next().unwrap_or('a');
			first.is_uppercase()
				|| ["de", "del", "la", "van", "von", "da", "di", "y", "le"].contains(&w.to_lowercase().as_str())
		})
		&& !words.iter().any(|w| w.chars().any(|c| c.is_ascii_digit()))
}

/// `A & B`, `A; B`, `A and B`, `A y B` -> `[A, B]`
pub fn split_authors(text: &str) -> Vec<String> {
	let normalised = text
		.replace(" and ", " & ")
		.replace(" y ", " & ")
		.replace(';', "&");
	normalised
		.split('&')
		.map(|a| a.trim().to_string())
		.filter(|a| !a.is_empty())
		.collect()
}

/// Two-letter language code from the forms found in files and online records
pub fn language_code(value: &str) -> Option<String> {
	let v = value.trim().to_lowercase();
	if v.is_empty() || v == "und" || v == "unknown" {
		return None;
	}
	let mapped = match v.as_str() {
		"eng" | "english" | "inglés" | "ingles" => "en",
		"spa" | "esl" | "spanish" | "español" | "espanol" | "castellano" => "es",
		"jpn" | "japanese" | "japonés" | "japones" => "ja",
		"fre" | "fra" | "french" | "francés" | "frances" => "fr",
		"ger" | "deu" | "german" | "alemán" | "aleman" => "de",
		"ita" | "italian" | "italiano" => "it",
		"por" | "portuguese" | "portugués" | "portugues" => "pt",
		"cat" | "catalan" | "català" | "catalán" => "ca",
		other => {
			let first = other.split(['-', '_', ' ']).next().unwrap_or(other);
			if first.len() == 2 && first.chars().all(|c| c.is_ascii_alphabetic()) {
				return Some(first.to_string());
			}
			return None;
		},
	};
	Some(mapped.to_string())
}

/// Whether a relative name is one plain path component (no separators, no `..`)
pub fn is_plain_component(name: &str) -> bool {
	let trimmed = name.trim();
	!trimmed.is_empty()
		&& trimmed != "."
		&& trimmed != ".."
		&& !trimmed.contains(['/', '\\', '\0'])
		&& !trimmed.starts_with('.')
}

#[cfg(test)]
mod tests {
	use super::*;

	/// real rows of the curated library's catalogo.csv: (authors, folder) and (title, file stem)
	#[test]
	fn folds_like_the_curated_library() {
		let authors = [
			("A. A. Kotov", "A_A_Kotov"),
			("A. C. Grayling", "A_C_Grayling"),
			("José Joaquín de Olmedo", "Jose_Joaquin_de_Olmedo"),
			("Adam Hart-Davis", "Adam_Hart-Davis"),
			("Auður Ava Ólafsdóttir", "Audur_Ava_Olafsdottir"),
			("Bjørn Lomborg", "Bjorn_Lomborg"),
			("Albert-László Barabási", "Albert-Laszlo_Barabasi"),
			("Gioconda Belli", "Gioconda_Belli"),
		];
		for (name, folder) in authors {
			assert_eq!(fold_name(name), folder, "author {name}");
		}
		let titles = [
			("Think Like a Grandmaster", "Think_Like_a_Grandmaster"),
			(
				"Russell: A Very Short Introduction",
				"Russell_A_Very_Short_Introduction",
			),
			("El país de las mujeres", "El_pais_de_las_mujeres"),
			("Hands-On Machine Learning", "Hands-On_Machine_Learning"),
			("Física cuántica para filo-sofos", "Fisica_cuantica_para_filo-sofos"),
			(
				"System Design Interview - Vol. 1",
				"System_Design_Interview_-_Vol_1",
			),
			(
				"Tahafut al-Tahafut (The Incoherence of the Incoherence)",
				"Tahafut_al-Tahafut_The_Incoherence_of_the_Incoherence",
			),
		];
		for (title, stem) in titles {
			assert_eq!(fold_name(title), stem, "title {title}");
		}
	}

	#[test]
	fn long_names_stop_at_ninety_characters_on_a_word() {
		let folded = fold_name(
			"The Experience of Warfare in Britain: Crimea, Boer and the First World War 1854–1929 (Access to History)",
		);
		assert!(folded.len() <= MAX_NAME_LEN);
		assert!(folded.starts_with("The_Experience_of_Warfare_in_Britain_Crimea_Boer"));
		assert!(!folded.ends_with('_'));
	}

	#[test]
	fn author_keys_match_folders_written_differently() {
		let folder = folder_keys("Umberto_Eco");
		for name in ["Umberto Eco", "Eco, Umberto", "ECO UMBERTO", "Umberto  Eco"] {
			let keys = name_keys(name);
			assert!(
				keys.ordered == folder.ordered || keys.sorted == folder.sorted,
				"{name} should match Umberto_Eco"
			);
		}
		// initials with or without dots and spaces
		assert_eq!(name_keys("A.C. Grayling").ordered, folder_keys("A_C_Grayling").ordered);
		assert_eq!(name_keys("Grayling, A. C.").ordered, folder_keys("A_C_Grayling").ordered);
		// older folders dropped the hyphen
		assert_eq!(
			name_keys("Arturo Pérez-Reverte").ordered,
			folder_keys("Arturo_PerezReverte").ordered
		);
		assert_ne!(name_keys("Umberto Eco").ordered, folder_keys("Umberto_Ecos").ordered);
	}

	#[test]
	fn names_series_volumes_like_the_library() {
		assert_eq!(series_volume_file_name("Yotsuba&!", Some(3.0), "CBZ"), "Yotsuba&! v03.cbz");
		assert_eq!(
			series_volume_file_name("Spice and Wolf", Some(12.0), "epub"),
			"Spice and Wolf v12.epub"
		);
		assert_eq!(series_volume_file_name("Dr. Stone", Some(100.0), "cbz"), "Dr. Stone v100.cbz");
		assert_eq!(series_volume_file_name("Re:Zero", Some(1.5), "epub"), "Re Zero v1.5.epub");
		assert_eq!(book_file_name("¿Por qué E=mc²?", "EPUB"), "Por_que_E_mc2.epub");
	}

	#[test]
	fn parses_file_names() {
		let p = parse_file_name("Umberto Eco - El nombre de la rosa");
		assert_eq!(p.authors, vec!["Umberto Eco"]);
		assert_eq!(p.title.as_deref(), Some("El nombre de la rosa"));

		let p = parse_file_name("Eco, Umberto - El péndulo de Foucault");
		assert_eq!(p.authors, vec!["Eco, Umberto"]);
		assert_eq!(p.title.as_deref(), Some("El péndulo de Foucault"));

		let p = parse_file_name("Yotsuba&! v15");
		assert_eq!(p.series.as_deref(), Some("Yotsuba&!"));
		assert_eq!(p.volume, Some(15.0));

		let p = parse_file_name("Spice and Wolf, Vol. 7");
		assert_eq!(p.series.as_deref(), Some("Spice and Wolf"));
		assert_eq!(p.volume, Some(7.0));

		let p = parse_file_name("Dune (Dune Chronicles #1)");
		assert_eq!(p.series.as_deref(), Some("Dune Chronicles"));
		assert_eq!(p.volume, Some(1.0));
		assert_eq!(p.title.as_deref(), Some("Dune"));

		let p = parse_file_name("Kaguya-sama - Love Is War - 05");
		assert_eq!(p.volume, Some(5.0));
		assert_eq!(p.series.as_deref(), Some("Kaguya-sama - Love Is War"));

		let p = parse_file_name("El_nombre_de_la_rosa");
		assert_eq!(p.title.as_deref(), Some("El nombre de la rosa"));
		assert!(p.authors.is_empty());
	}

	#[test]
	fn writes_authors_first_name_first() {
		assert_eq!(display_name("Eco, Umberto"), "Umberto Eco");
		assert_eq!(display_name("Grayling, A. C."), "A. C. Grayling");
		assert_eq!(display_name("Umberto  Eco"), "Umberto Eco");
		assert_eq!(display_name("Martin Luther King, Jr."), "Martin Luther King, Jr.");
	}

	#[test]
	fn reads_language_codes() {
		assert_eq!(language_code("eng").as_deref(), Some("en"));
		assert_eq!(language_code("es-ES").as_deref(), Some("es"));
		assert_eq!(language_code("Spanish").as_deref(), Some("es"));
		assert_eq!(language_code("und"), None);
	}

	#[test]
	fn rejects_unsafe_components() {
		assert!(is_plain_component("Umberto_Eco"));
		assert!(is_plain_component("Spice and Wolf"));
		assert!(!is_plain_component("../etc"));
		assert!(!is_plain_component("a/b"));
		assert!(!is_plain_component(".hidden"));
		assert!(!is_plain_component(""));
	}
}
