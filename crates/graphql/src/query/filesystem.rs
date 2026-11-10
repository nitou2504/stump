use std::path::{Path, PathBuf};

use crate::data::{AuthContext, CoreContext};
use crate::guard::PermissionGuard;
use crate::input::filesystem::{DirectoryListingIgnoreParams, DirectoryListingInput};
use crate::object::directory_listing::{DirectoryListing, DirectoryListingFile};
use crate::pagination::{
	OffsetPagination, OffsetPaginationInfo, PaginatedResponse, Pagination,
	PaginationInfo, PaginationValidator,
};
use async_graphql::{Context, Object, Result};
use itertools::Itertools;
use models::{
	entity::{library, user::AuthUser},
	shared::enums::UserPermission,
};
use sea_orm::{prelude::*, QuerySelect};
use std::fs::DirEntry;
use stump_core::fs_utils::PathUtils;

/// Where a user may look on the server's file system
enum BrowseScope {
	/// Anywhere: the server owner, and whoever may point a library at any folder (that
	/// folder is picked with this same explorer)
	Anywhere,
	/// Only inside the folders of the libraries the user can see (the file explorer
	/// permission alone isn't a reason to list `/`, the config folder or `/etc`)
	Libraries(Vec<PathBuf>),
}

impl BrowseScope {
	async fn for_user(user: &AuthUser, conn: &DatabaseConnection) -> Result<Self> {
		if user.is_server_owner || user.has_permission(UserPermission::EditLibrary) {
			return Ok(Self::Anywhere);
		}

		let roots = library::Entity::find_for_user(user)
			.select_only()
			.column(library::Column::Path)
			.into_tuple::<String>()
			.all(conn)
			.await?
			.into_iter()
			// a library whose folder isn't there has nothing to browse
			.filter_map(|path| std::fs::canonicalize(path).ok())
			.collect();

		Ok(Self::Libraries(roots))
	}

	/// Whether the folder is a library's folder or inside one. It is compared after
	/// resolving `..` and symbolic links, so a path can't climb out of a library
	fn allows(&self, path: &Path) -> bool {
		match self {
			Self::Anywhere => true,
			Self::Libraries(roots) => {
				path.is_absolute()
					&& std::fs::canonicalize(path).is_ok_and(|resolved| {
						roots.iter().any(|root| resolved.starts_with(root))
					})
			},
		}
	}

	fn is_restricted(&self) -> bool {
		matches!(self, Self::Libraries(_))
	}
}

#[derive(Default)]
pub struct FilesystemQuery;

#[Object]
impl FilesystemQuery {
	#[graphql(
		complexity = "crate::complexity::pagination_cost(&pagination, child_complexity)",
		guard = "PermissionGuard::one(UserPermission::FileExplorer)"
	)]
	async fn list_directory(
		&self,
		ctx: &Context<'_>,
		#[graphql(validator(custom = "PaginationValidator"))] pagination: Pagination,
		input: Option<DirectoryListingInput>,
	) -> Result<PaginatedResponse<DirectoryListing>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		let scope = BrowseScope::for_user(user, conn).await?;

		let params = input.unwrap_or_default();
		let list_root = params.path.clone().map(PathBuf::from);

		let pagination = match pagination {
			Pagination::Offset(p) => p,
			_ => return Err("Pagination must be offset".into()),
		};

		let start_path = match list_root {
			Some(path) => path,
			// there is no "everything" to start from for someone who can only look inside
			// libraries: they say which folder
			None if scope.is_restricted() => {
				return Err("Directory does not exist: /".into());
			},
			#[cfg(unix)]
			None => Path::new("/").into(),
			#[cfg(windows)]
			None => return windows_fs::get_drive_listing(pagination),
		};

		// For someone who can only look inside libraries, a folder that isn't there and one
		// out of their reach get the same answer, so this isn't a way to find what exists
		if !scope.allows(&start_path) {
			return Err(
				format!("Directory does not exist: {}", start_path.display()).into(),
			);
		}

		if !start_path.exists() {
			tracing::error!(?start_path, "Directory does not exist");
			return Err(
				format!("Directory does not exist: {}", start_path.display()).into(),
			);
		} else if !start_path.is_absolute() {
			return Err("Provided path must be absolute".into());
		}

		let files = read_and_filter_directory(&start_path, params.ignore_params)?
			.into_iter()
			.sorted_by(|a, b| {
				alphanumeric_sort::compare_path(
					a.name.to_lowercase(),
					b.name.to_lowercase(),
				)
			})
			.collect::<Vec<_>>();

		tracing::trace!(
			"{} files in directory listing for: {}",
			files.len(),
			start_path.display()
		);

		// no way up out of a library either
		let parent = start_path.parent().filter(|parent| scope.allows(parent));

		Ok(into_paginated_response(parent, files, pagination))
	}
}

fn into_paginated_response(
	parent: Option<&Path>,
	files: Vec<DirectoryListingFile>,
	pagination: OffsetPagination,
) -> PaginatedResponse<DirectoryListing> {
	let offset_info = OffsetPaginationInfo::new(pagination, files.len() as u64);

	let mut truncated_files = files;

	let start = offset_info.page_offset;
	let end = start + offset_info.page_size;

	if start > truncated_files.len() as u64 {
		truncated_files.clear();
	} else if end < truncated_files.len() as u64 {
		truncated_files = truncated_files
			.get((start as usize)..(end as usize))
			.unwrap_or_default()
			.to_vec();
	} else {
		truncated_files = truncated_files
			.get((start as usize)..)
			.unwrap_or_default()
			.to_vec();
	}

	PaginatedResponse {
		nodes: vec![DirectoryListing {
			parent: parent.map(|p| p.to_string_lossy().to_string()),
			files: truncated_files,
		}],
		page_info: PaginationInfo::Offset(offset_info),
	}
}

fn read_and_filter_directory(
	start_path: &Path,
	ignore_params: DirectoryListingIgnoreParams,
) -> Result<Vec<DirectoryListingFile>> {
	let listing = std::fs::read_dir(start_path)?;

	let files = listing
		.filter_map(Result::ok)
		.filter_map(|entry| filter_from_params(entry, &ignore_params))
		.map(|entry| DirectoryListingFile::from(entry.path()))
		.collect();

	Ok(files)
}

fn filter_from_params(
	entry: DirEntry,
	params: &DirectoryListingIgnoreParams,
) -> Option<DirEntry> {
	let path = entry.path();

	if params.ignore_files && path.is_file() {
		return None;
	}

	if params.ignore_directories && path.is_dir() {
		return None;
	}

	if params.ignore_hidden && path.is_hidden_file() {
		return None;
	}

	Some(entry)
}

#[cfg(target_os = "windows")]
mod windows_fs {
	use super::*;

	use std::{ffi::OsString, os::windows::ffi::OsStringExt};

	use windows::{core::PCWSTR, Win32::Foundation::MAX_PATH};

	pub fn get_drive_listing(
		pagination: OffsetPagination,
	) -> Result<PaginatedResponse<DirectoryListing>> {
		// Microsoft provides an unsafe interface to get a list of logical drive letters
		// docs: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getlogicaldrivestringsw
		let mut buffer: [u16; 256] = [0; 256];
		let len = unsafe {
			windows::Win32::Storage::FileSystem::GetLogicalDriveStringsW(Some(
				&mut buffer,
			))
		};

		// We check for a failed read
		if len == 0 {
			return Err("Failed to read Win32 list of logical drives".into());
		}

		// We can iterate over the buffer to fetch a list of drives with their info
		const MAX_VOL_NAME: usize = (MAX_PATH + 1) as usize;
		let mut volume_name_buffer: [u16; MAX_VOL_NAME] = [0; MAX_VOL_NAME];
		let drive_listing = buffer
			.split(|&c| c == 0)
			.filter(|s: &&[u16]| !s.is_empty())
			.map(|encoded_letter| {
				let drive_letter = OsString::from_wide(encoded_letter)
					.to_string_lossy()
					.to_string();

				// Now we get the names for each volume using the GetVolumeInformationW call
				// docs: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationw
				let volume_name_res = unsafe {
					windows::Win32::Storage::FileSystem::GetVolumeInformationW(
						PCWSTR(encoded_letter.as_ptr()),
						Some(&mut volume_name_buffer),
						None,
						None,
						None,
						None,
					)
				};

				let volume_name = match volume_name_res {
					Ok(_) => {
						let termination_index = volume_name_buffer
							.iter()
							.position(|&c| c == 0)
							.unwrap_or(volume_name_buffer.len());
						let os_str =
							OsString::from_wide(&volume_name_buffer[..termination_index]);

						if os_str.is_empty() {
							None
						} else {
							Some(os_str.to_string_lossy().to_string())
						}
					},
					Err(_) => None,
				};

				let drive_text = match volume_name {
					Some(name) => format!("{drive_letter} ({name})"),
					None => drive_letter.clone(),
				};

				DirectoryListingFile {
					is_directory: true,
					name: drive_text,
					path: drive_letter,
				}
			})
			.collect::<Vec<_>>();

		Ok(into_paginated_response(None, drive_listing, pagination))
	}
}
