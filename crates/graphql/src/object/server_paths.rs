//! What a user is told about where the server keeps its files.
//!
//! The folder of a library, the path of a book or a series on disk, where a thumbnail is cached
//! and the hashes of a book's file are the server's business. The people who run it need them
//! (the library settings, the file explorer, scripts that find books by path); a friend who reads
//! books has no use for `/biblioteca/biblioteca/Author/Book.epub`, and a hash is a handle on a
//! file they can't open.
//!
//! The fields stay in the schema, so clients keep compiling. Who may see the server's side of
//! them is [`may_see_server_paths`]; everybody else gets the path *inside* the library (what
//! `relativeLibraryPath` has always been), an empty text for a library's folder and nothing
//! (`null`) for the hashes and the thumbnails' places.

use async_graphql::{dataloader::DataLoader, Context, Result};
use models::{
	entity::user::AuthUser,
	shared::{enums::UserPermission, permission_set::user_has_all_permissions},
};

use crate::{data::AuthContext, loader::library::LibraryLoader};

/// Whether `user` may see where the files of the libraries are on the server: the owner, whoever
/// manages or edits libraries (the library settings show the folder and save it back) and whoever
/// may browse the server's files (the explorer lists the very same folders)
pub fn may_see_server_paths(user: &AuthUser) -> bool {
	[
		UserPermission::ManageLibrary,
		UserPermission::EditLibrary,
		UserPermission::FileExplorer,
	]
	.into_iter()
	.any(|permission| user_has_all_permissions(user, &[permission]))
}

/// Whether the user making this request may see the server's paths and hashes
pub fn requester_may_see_server_paths(ctx: &Context<'_>) -> Result<bool> {
	let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
	Ok(may_see_server_paths(user))
}

/// `value` for those who may see the server's paths and hashes, and nothing for the rest
pub fn only_for_those_who_may<T>(
	ctx: &Context<'_>,
	value: Option<T>,
) -> Result<Option<T>> {
	Ok(if requester_may_see_server_paths(ctx)? {
		value
	} else {
		None
	})
}

/// The name of a file or a folder: the last part of its path
fn name_of(path: &str) -> String {
	path.trim_end_matches('/')
		.rsplit('/')
		.next()
		.unwrap_or_default()
		.to_string()
}

/// `path` as seen from inside the library whose folder is `library_path`: `/Author/Book.epub`,
/// with the leading slash `relativeLibraryPath` has always had, and `""` for the folder itself.
///
/// A path that is not under the library's folder (the folder was changed after the last scan, or
/// the path is of another library) gives the name of the file, never the server's folders.
pub fn relative_to(library_path: &str, path: &str) -> String {
	let root = library_path.trim_end_matches('/');
	match path.strip_prefix(root) {
		Some("") => String::new(),
		Some(rest) if rest.starts_with('/') => rest.to_string(),
		_ => name_of(path),
	}
}

/// The path of a book or a series for the user making the request: the whole path for those who
/// may see the server's folders, the path inside its library for the rest
pub async fn path_for_requester(
	ctx: &Context<'_>,
	library_id: Option<&str>,
	path: &str,
) -> Result<String> {
	if requester_may_see_server_paths(ctx)? {
		return Ok(path.to_string());
	}

	let library = match library_id {
		Some(id) => {
			ctx.data::<DataLoader<LibraryLoader>>()?
				.load_one(id.to_string())
				.await?
		},
		None => None,
	};

	Ok(match library {
		Some(library) => relative_to(&library.model.path, path),
		None => name_of(path),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_path_is_seen_from_inside_its_library() {
		assert_eq!(
			relative_to(
				"/biblioteca/biblioteca",
				"/biblioteca/biblioteca/Author/Book.epub"
			),
			"/Author/Book.epub"
		);
		// a library folder written with a slash at its end is the same folder
		assert_eq!(
			relative_to("/books/", "/books/Author/Book.epub"),
			"/Author/Book.epub"
		);
		// a library at the root of the disk
		assert_eq!(relative_to("/", "/Author/Book.epub"), "/Author/Book.epub");
	}

	#[test]
	fn the_folder_of_the_library_is_empty_from_inside() {
		assert_eq!(relative_to("/books", "/books"), "");
		assert_eq!(relative_to("/books", "/books/"), "/");
	}

	#[test]
	fn a_path_outside_the_library_gives_its_name_and_nothing_of_the_server() {
		// a folder that only starts like the library's
		assert_eq!(
			relative_to("/books", "/books2/Author/Book.epub"),
			"Book.epub"
		);
		// another place altogether
		assert_eq!(relative_to("/books", "/config/thumbnails/x.webp"), "x.webp");
		// what is not a path
		assert_eq!(relative_to("/books", "Book.epub"), "Book.epub");
		assert_eq!(relative_to("/books", ""), "");
		// a folder, written with a slash at its end
		assert_eq!(relative_to("/books", "/srv/Series/"), "Series");
	}
}
