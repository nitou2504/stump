use async_graphql::SimpleObject;
use sea_orm::entity::prelude::*;

use crate::shared::{enums::LibraryType, intake::IntakeSeriesKind};

/// How the "Add books" intake files books into a library. A library without a row uses
/// [`Model::defaults_for`]. An empty folder means that kind of series sits directly in the
/// library root, as in a manga or light novel library.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, SimpleObject)]
#[graphql(name = "LibraryIntakeSettings")]
#[sea_orm(table_name = "library_intake_settings")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
	pub library_id: String,
	/// Folder (relative to the library root) holding one folder per manga series
	#[sea_orm(column_type = "Text")]
	pub manga_dir: String,
	/// Folder (relative to the library root) holding one folder per light novel series
	#[sea_orm(column_type = "Text")]
	pub light_novel_dir: String,
	/// Look titles up on Open Library and Google Books while analysing
	pub online_lookup: bool,
}

pub const DEFAULT_MANGA_DIR: &str = "_Manga";
pub const DEFAULT_LIGHT_NOVEL_DIR: &str = "_Novela ligera";

impl Model {
	/// The settings of a library that has none stored. A manga or light novel library holds
	/// nothing but series, a folder each at its root (an empty folder setting); the other
	/// libraries keep their series in the two default sub-folders.
	pub fn defaults_for(library_id: &str, library_type: LibraryType) -> Self {
		let (manga_dir, light_novel_dir) = match typed_series_kind(library_type) {
			Some(_) => ("", ""),
			None => (DEFAULT_MANGA_DIR, DEFAULT_LIGHT_NOVEL_DIR),
		};
		Self {
			library_id: library_id.to_string(),
			manga_dir: manga_dir.to_string(),
			light_novel_dir: light_novel_dir.to_string(),
			online_lookup: true,
		}
	}
}

/// The kind of series a manga or light novel library holds at its root; `None` for any other
/// type of library
pub fn typed_series_kind(library_type: LibraryType) -> Option<IntakeSeriesKind> {
	match library_type {
		LibraryType::Manga => Some(IntakeSeriesKind::Manga),
		LibraryType::LightNovel => Some(IntakeSeriesKind::LightNovel),
		_ => None,
	}
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn manga_and_light_novel_libraries_hold_their_series_at_the_root() {
		for (library_type, kind) in [
			(LibraryType::Manga, IntakeSeriesKind::Manga),
			(LibraryType::LightNovel, IntakeSeriesKind::LightNovel),
		] {
			let settings = Model::defaults_for("lib", library_type);
			assert_eq!(settings.manga_dir, "");
			assert_eq!(settings.light_novel_dir, "");
			assert!(settings.online_lookup);
			assert_eq!(typed_series_kind(library_type), Some(kind));
		}
	}

	#[test]
	fn the_other_libraries_keep_their_series_in_sub_folders() {
		for library_type in [
			LibraryType::Book,
			LibraryType::Mixed,
			LibraryType::Comic,
			LibraryType::Manhwa,
			LibraryType::WebNovel,
			LibraryType::Webtoon,
		] {
			let settings = Model::defaults_for("lib", library_type);
			assert_eq!(settings.manga_dir, DEFAULT_MANGA_DIR);
			assert_eq!(settings.light_novel_dir, DEFAULT_LIGHT_NOVEL_DIR);
			assert_eq!(typed_series_kind(library_type), None);
		}
	}
}
