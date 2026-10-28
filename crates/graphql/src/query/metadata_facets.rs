//! Value + count listings for list-like metadata fields (genres, writers, language, ...).
//!
//! These power the "browse by genre/author" pages in the web client and the matching
//! OPDS navigation feeds, so the counting logic lives in a plain function that both can
//! call.

use std::collections::HashMap;

use async_graphql::{Context, Enum, Object, Result, SimpleObject, ID};
use models::entity::{media, media_metadata, user::AuthUser};
use sea_orm::{prelude::*, sea_query::SelectStatement, ConnectionTrait, QuerySelect};

use crate::data::{AuthContext, CoreContext};

/// The metadata fields which can be listed with their book counts
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetadataFacetField {
	Genres,
	Writers,
	Language,
	Publisher,
	Series,
}

impl MetadataFacetField {
	pub fn column(&self) -> media_metadata::Column {
		match self {
			Self::Genres => media_metadata::Column::Genres,
			Self::Writers => media_metadata::Column::Writers,
			Self::Language => media_metadata::Column::Language,
			Self::Publisher => media_metadata::Column::Publisher,
			Self::Series => media_metadata::Column::Series,
		}
	}

	/// Whether the column stores a comma-separated list of values
	pub fn is_list(&self) -> bool {
		matches!(self, Self::Genres | Self::Writers)
	}
}

#[derive(SimpleObject, Clone, Debug)]
pub struct MetadataValueCount {
	pub value: String,
	pub count: u64,
}

/// Lists every distinct value of `field` visible to `user`, with the number of books
/// carrying it. Values are de-duplicated case-insensitively (the first-seen casing wins)
/// and sorted alphabetically, case-insensitively.
///
/// * `library_id` restricts the books to a single library
/// * `media_ids` restricts the books to an arbitrary subquery of media IDs (e.g. a smart
///   list)
/// * `search` keeps only values containing the (case-insensitive) search string
pub async fn metadata_value_counts<C: ConnectionTrait>(
	conn: &C,
	user: &AuthUser,
	field: MetadataFacetField,
	library_id: Option<String>,
	media_ids: Option<SelectStatement>,
	search: Option<String>,
) -> std::result::Result<Vec<MetadataValueCount>, DbErr> {
	let column = field.column();
	let mut query = media::Entity::find_for_user(user)
		.select_only()
		.column(column)
		.filter(column.is_not_null());
	if let Some(library_id) = library_id {
		query = query.filter(media::Column::LibraryId.eq(library_id));
	}
	if let Some(media_ids) = media_ids {
		query = query.filter(media::Column::Id.in_subquery(media_ids));
	}
	let rows: Vec<String> = query.into_tuple().all(conn).await?;

	let search = search
		.map(|s| s.trim().to_lowercase())
		.filter(|s| !s.is_empty());

	// lowercase key -> (display value, count)
	let mut counts: HashMap<String, (String, u64)> = HashMap::new();
	for row in rows {
		let values: Vec<&str> = if field.is_list() {
			row.split(',').collect()
		} else {
			vec![row.as_str()]
		};
		// A book listing the same value twice should still only count once
		let mut seen_in_row = Vec::with_capacity(values.len());
		for value in values {
			let value = value.trim();
			if value.is_empty() {
				continue;
			}
			let key = value.to_lowercase();
			if seen_in_row.contains(&key) {
				continue;
			}
			if let Some(search) = &search {
				if !key.contains(search.as_str()) {
					continue;
				}
			}
			seen_in_row.push(key.clone());
			counts
				.entry(key)
				.and_modify(|(_, count)| *count += 1)
				.or_insert_with(|| (value.to_string(), 1));
		}
	}

	let mut result = counts
		.into_iter()
		.map(|(key, (value, count))| {
			(sort_key(&key), MetadataValueCount { value, count })
		})
		.collect::<Vec<_>>();
	result.sort_by(|(a, _), (b, _)| a.cmp(b));
	Ok(result.into_iter().map(|(_, v)| v).collect())
}

/// A cheap accent-insensitive sort key so that e.g. `Ética` sorts next to `Etnografía`
/// instead of after `Zoología`
pub fn sort_key(value: &str) -> String {
	value
		.to_lowercase()
		.chars()
		.map(|c| match c {
			'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
			'é' | 'è' | 'ë' | 'ê' => 'e',
			'í' | 'ì' | 'ï' | 'î' => 'i',
			'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'ø' => 'o',
			'ú' | 'ù' | 'ü' | 'û' => 'u',
			'ñ' => 'n',
			'ç' => 'c',
			_ => c,
		})
		.collect()
}

#[derive(Default)]
pub struct MetadataFacetQuery;

#[Object]
impl MetadataFacetQuery {
	/// List the distinct values of a metadata field with their book counts, e.g. every
	/// genre and how many books have it
	async fn metadata_value_counts(
		&self,
		ctx: &Context<'_>,
		field: MetadataFacetField,
		library_id: Option<ID>,
		search: Option<String>,
	) -> Result<Vec<MetadataValueCount>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		Ok(metadata_value_counts(
			conn,
			user,
			field,
			library_id.map(|id| id.to_string()),
			None,
			search,
		)
		.await?)
	}
}
