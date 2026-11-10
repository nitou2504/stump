use std::path::PathBuf;

use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use models::entity::media;
use sea_orm::prelude::*;
use stump_core::fs_utils::PathUtils;

use crate::{
	data::{AuthContext, CoreContext},
	object::media::Media,
};

#[derive(Debug, Clone, SimpleObject)]
pub struct DirectoryListing {
	pub parent: Option<String>,
	pub files: Vec<DirectoryListingFile>,
}

#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct DirectoryListingFile {
	pub is_directory: bool,
	pub name: String,
	pub path: String,
}

impl DirectoryListingFile {
	pub fn new(is_directory: bool, name: &str, path: &str) -> Self {
		Self {
			is_directory,
			name: name.to_string(),
			path: path.to_string(),
		}
	}

	pub fn file(name: &str, path: &str) -> Self {
		Self::new(false, name, path)
	}

	pub fn directory(name: &str, path: &str) -> Self {
		Self::new(true, name, path)
	}
}

impl From<PathBuf> for DirectoryListingFile {
	fn from(path: PathBuf) -> Self {
		Self {
			name: path.file_parts().file_name,
			is_directory: path.is_dir(),
			path: path.to_string_lossy().to_string(),
		}
	}
}

#[ComplexObject]
impl DirectoryListingFile {
	async fn media(&self, ctx: &Context<'_>) -> Result<Option<Media>> {
		if self.is_directory {
			return Ok(None);
		}

		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		// Only a book the viewer may see: one in a library hidden from them, above their age
		// restriction or removed from the library isn't shown, however the file got listed
		let book = media::ModelWithMetadata::find_for_user(user)
			.filter(media::Column::Path.eq(self.path.clone()))
			.into_model::<media::ModelWithMetadata>()
			.one(conn)
			.await?;

		Ok(book.map(Media::from))
	}
}
