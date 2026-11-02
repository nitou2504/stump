use async_graphql::SimpleObject;
use sea_orm::entity::prelude::*;

/// How the "Add books" intake files books into a library. A library without a row uses
/// [`Model::defaults_for`].
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
	/// Where the intake CSV is appended to. `None` = next to `catalogo.csv` when the
	/// library's parent folder has one, else in the library root
	#[sea_orm(column_type = "Text", nullable)]
	pub csv_path: Option<String>,
	/// Look titles up on Open Library and Google Books while analysing
	pub online_lookup: bool,
}

pub const DEFAULT_MANGA_DIR: &str = "_Manga";
pub const DEFAULT_LIGHT_NOVEL_DIR: &str = "_Novela ligera";
/// The name of the intake CSV
pub const INTAKE_CSV_NAME: &str = "stump_intake.csv";

impl Model {
	pub fn defaults_for(library_id: &str) -> Self {
		Self {
			library_id: library_id.to_string(),
			manga_dir: DEFAULT_MANGA_DIR.to_string(),
			light_novel_dir: DEFAULT_LIGHT_NOVEL_DIR.to_string(),
			csv_path: None,
			online_lookup: true,
		}
	}
}

impl ActiveModelBehavior for ActiveModel {}
