use async_graphql::{SimpleObject, Union};

use crate::object::{library::Library, media::Media, series::Series};

#[derive(Debug, Union)]
pub enum SmartListItemEntity {
	Series(Box<Series>),
	Library(Box<Library>),
}

#[derive(Debug, SimpleObject)]
pub struct SmartListGroupedItem {
	pub entity: Option<SmartListItemEntity>,
	pub books: Vec<Media>,
}

#[derive(Debug, SimpleObject)]
pub struct SmartListGrouped {
	pub items: Vec<SmartListGroupedItem>,
	/// The ID to pass as `after` for the next page of the list: the last book of this page, when
	/// the list has more. `null` on the last page. Unlike `offset`, it doesn't skip or repeat a
	/// book when the list changes between two pages
	pub next_cursor: Option<String>,
}

#[derive(Debug, SimpleObject)]
pub struct SmartListUngrouped {
	pub books: Vec<Media>,
	/// The ID to pass as `after` for the next page of the list: the last book of this page, when
	/// the list has more. `null` on the last page. Unlike `offset`, it doesn't skip or repeat a
	/// book when the list changes between two pages
	pub next_cursor: Option<String>,
}

#[derive(Debug, Union)]
pub enum SmartListItems {
	Grouped(SmartListGrouped),
	Ungrouped(SmartListUngrouped),
}
