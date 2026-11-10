//! A ready-made world to check what a user with limits may see.
//!
//! What a user may read is decided by their account: the libraries hidden from them, an age
//! restriction, and whether a book was removed from the library. It has to hold wherever a book
//! is listed, counted, searched, served or written to, so every package that adds a surface (a
//! list, a page, a feed) checks it against the same world: [`VisibilityFixture`].
//!
//! The helpers at the top (`add_library`, `add_book`, `kid`, `hide_library`, ...) build smaller
//! worlds for the tests that need only a corner of it.

use std::{fs::File, io::Write, path::Path};

use graphql::input::user::AgeRestrictionInput;
use models::{
	entity::{
		library, library_exclusion, media, media_metadata, series, series_metadata,
	},
	shared::enums::UserPermission,
};
use sea_orm::{prelude::*, ActiveModelTrait, Set};
use serde_json::{json, Value};
use tests::fake_data;

use super::{
	account::{CreateTestUser, TestUser},
	TestApp,
};

/// What to put in a book's metadata (nothing is as good as no metadata)
#[derive(Default)]
pub struct Meta<'a> {
	pub age: Option<i32>,
	pub writer: Option<&'a str>,
	pub publisher: Option<&'a str>,
	pub genre: Option<&'a str>,
	pub number: Option<f64>,
}

pub async fn add_library(app: &TestApp, name: &str) -> library::Model {
	fake_data::Library {
		name: Some(name.to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await
}

pub async fn add_series(
	app: &TestApp,
	library: &library::Model,
	name: &str,
) -> series::Model {
	fake_data::Series {
		name: Some(name.to_string()),
		library_id: Some(library.id.clone()),
		..Default::default()
	}
	.insert(app.conn())
	.await
}

pub async fn add_book(
	app: &TestApp,
	series: &series::Model,
	name: &str,
	meta: Meta<'_>,
) -> media::Model {
	let book = fake_data::Media {
		series_id: series.id.clone(),
		name: Some(name.to_string()),
		pages: Some(10),
		..Default::default()
	}
	.insert(app.conn())
	.await;

	if meta.age.is_some()
		|| meta.writer.is_some()
		|| meta.publisher.is_some()
		|| meta.genre.is_some()
		|| meta.number.is_some()
	{
		media_metadata::ActiveModel {
			media_id: Set(Some(book.id.clone())),
			age_rating: Set(meta.age),
			writers: Set(meta.writer.map(str::to_string)),
			publisher: Set(meta.publisher.map(str::to_string)),
			genres: Set(meta.genre.map(str::to_string)),
			number: Set(meta
				.number
				.map(|number| Decimal::try_from(number).expect("a valid number"))),
			..Default::default()
		}
		.insert(app.conn())
		.await
		.expect("should insert the metadata");
	}

	book
}

/// A user who may read nothing rated above 13 (and has no permissions)
pub async fn kid(app: &TestApp) -> TestUser {
	CreateTestUser {
		username: "kid".to_string(),
		age_restriction: Some(AgeRestrictionInput {
			age: 13,
			restrict_on_unset: false,
		}),
		..Default::default()
	}
	.insert(app)
	.await
}

/// A user with no permissions and no restrictions (until a library is hidden from them)
pub async fn friend(app: &TestApp) -> TestUser {
	CreateTestUser {
		username: "friend".to_string(),
		..Default::default()
	}
	.insert(app)
	.await
}

pub async fn hide_library(app: &TestApp, user: &TestUser, library: &library::Model) {
	library_exclusion::ActiveModel {
		user_id: Set(user.id.clone()),
		library_id: Set(library.id.clone()),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.expect("should hide the library");
}

pub fn errors(response: &Value) -> String {
	response["errors"].to_string()
}

pub fn ids(nodes: &Value) -> Vec<String> {
	let mut ids = nodes
		.as_array()
		.expect("a list")
		.iter()
		.map(|node| node["id"].as_str().expect("an id").to_string())
		.collect::<Vec<_>>();
	ids.sort();
	ids
}

pub fn sorted(mut values: Vec<String>) -> Vec<String> {
	values.sort();
	values
}

pub fn strings(list: &Value, field: Option<&str>) -> Vec<String> {
	sorted(
		list.as_array()
			.expect("a list")
			.iter()
			.map(|item| match field {
				Some(field) => item[field].as_str().expect("a string").to_string(),
				None => item.as_str().expect("a string").to_string(),
			})
			.collect(),
	)
}

/// The smallest picture there is: a 1x1 PNG
const TINY_PNG: [u8; 70] = [
	0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
	0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
	0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78,
	0xda, 0x63, 0xfc, 0xcf, 0xc0, 0x50, 0x0f, 0x00, 0x04, 0x85, 0x01, 0x80, 0x84, 0xa9,
	0x8c, 0x21, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// Writes a comic with a single page (a real file, small enough to make one per book)
fn write_tiny_cbz(path: &Path) {
	let mut zip =
		zip::ZipWriter::new(File::create(path).expect("should create the file"));
	zip.start_file("page-1.png", zip::write::FileOptions::<()>::default())
		.expect("should start the page");
	zip.write_all(&TINY_PNG).expect("should write the page");
	zip.finish().expect("should finish the comic");
}

/// The world every access test asks for: two libraries, five books and four users.
///
/// - **Libraries and series.** *Open* holds "Open shelf" with `general`, `adult` and `removed`,
///   and "Loose shelf" with `unrated`. *Hidden* holds "Hidden shelf" with `hidden`. Open shelf is
///   rated 0 and Loose shelf has no rating: a book without a rating of its own takes its series'
///   (so `unrated` has to live in a series without one to stay unrated), and a series needs a
///   rating of its own to show for a user whose age restriction applies to unrated things.
/// - **Books.** `general` is rated 0. `adult` is rated 18. `unrated` has no age rating at all.
///   `removed` was taken out of the library (*Remove from library*), so nobody sees it. `hidden`
///   is in *Hidden*.
/// - **Users.** The owner (the app's default user). **ana** may read nothing above 16 and nothing
///   unrated (her age restriction applies to unset ratings too), *Hidden* is hidden from her, and
///   she may use smart lists and add books. **ben** has no limits and no permissions. **zoe** was
///   deleted.
///
/// So ana's hidden set, H, is `adult`, `unrated`, `removed` and `hidden` (see [`hidden_from_ana`]):
/// she must never get an ID, title, path, cover, count, facet value or search hit that comes from
/// them, through any surface ([`assert_none_hidden`] checks a response for that). What she sees
/// is `general`, in "Open shelf". The owner and ben see `general`, `adult`, `unrated` and
/// `hidden`; `removed` is out for everybody.
///
/// [`hidden_from_ana`]: VisibilityFixture::hidden_from_ana
/// [`assert_none_hidden`]: VisibilityFixture::assert_none_hidden
pub struct VisibilityFixture {
	pub open: library::Model,
	pub hidden_library: library::Model,
	pub open_series: series::Model,
	pub loose_series: series::Model,
	pub hidden_series: series::Model,
	pub general: media::Model,
	pub adult: media::Model,
	pub unrated: media::Model,
	pub removed: media::Model,
	pub hidden: media::Model,
	pub owner_token: String,
	pub ana: TestUser,
	pub ben: TestUser,
	pub zoe: TestUser,
}

impl VisibilityFixture {
	/// Builds the world in the app's database. The app needs its owner: use
	/// [`TestApp::new_with_default_user`]
	pub async fn build(app: &TestApp) -> Self {
		let owner_token = app.access_token.read().await.clone().expect(
			"the world needs its owner: start the app with TestApp::new_with_default_user()",
		);

		let open = add_library(app, "Open").await;
		let hidden_library = add_library(app, "Hidden").await;
		let open_series = add_series(app, &open, "Open shelf").await;
		let loose_series = add_series(app, &open, "Loose shelf").await;
		let hidden_series = add_series(app, &hidden_library, "Hidden shelf").await;
		// a series has to carry a rating of its own to be shown to a user whose age restriction
		// applies to unrated things, and its books without one take it
		series_metadata::ActiveModel {
			series_id: Set(open_series.id.clone()),
			age_rating: Set(Some(0)),
			..Default::default()
		}
		.insert(app.conn())
		.await
		.expect("should rate the series");

		let book = |age, number| Meta {
			age,
			number: Some(number),
			..Default::default()
		};
		let general =
			add_book(app, &open_series, "General book", book(Some(0), 1.0)).await;
		let adult = add_book(app, &open_series, "Adult book", book(Some(18), 2.0)).await;
		let unrated = add_book(app, &loose_series, "Unrated book", book(None, 1.0)).await;
		let removed =
			add_book(app, &open_series, "Removed book", book(Some(0), 4.0)).await;
		let hidden =
			add_book(app, &hidden_series, "Hidden book", book(Some(0), 1.0)).await;

		// removed the way a user removes a book, not by writing the column
		let response = app
			.execute_gql(
				r#"mutation RemoveBook($id: ID!) { deleteMedia(id: $id) { id } }"#,
				Some(json!({ "id": removed.id })),
			)
			.await;
		assert!(response.get("errors").is_none(), "{response}");

		let ana = limited_user(
			app,
			&hidden_library,
			"ana",
			vec![UserPermission::AccessSmartList, UserPermission::AddBooks],
		)
		.await;
		let ben = CreateTestUser {
			username: "ben".to_string(),
			..Default::default()
		}
		.insert(app)
		.await;
		let zoe = CreateTestUser {
			username: "zoe".to_string(),
			..Default::default()
		}
		.insert(app)
		.await;
		let response = app
			.execute_gql(
				r#"mutation DeleteUser($id: ID!) { deleteUser(id: $id) { id } }"#,
				Some(json!({ "id": zoe.id })),
			)
			.await;
		assert!(response.get("errors").is_none(), "{response}");

		// the removal is the owner's doing, and the books are as the database has them now
		let removed = media::Entity::find_by_id(removed.id.clone())
			.one(app.conn())
			.await
			.expect("should look for the removed book")
			.expect("the removed book is still there");

		Self {
			open,
			hidden_library,
			open_series,
			loose_series,
			hidden_series,
			general,
			adult,
			unrated,
			removed,
			hidden,
			owner_token,
			ana,
			ben,
			zoe,
		}
	}

	/// Another user with ana's limits (rated 16 or less, *Hidden* hidden) and the permissions
	/// given: e.g. one who may download, which ana may not
	pub async fn limited_like_ana(
		&self,
		app: &TestApp,
		username: &str,
		permissions: Vec<UserPermission>,
	) -> TestUser {
		limited_user(app, &self.hidden_library, username, permissions).await
	}

	/// What ana may not see, H: `adult`, `unrated`, `removed` and `hidden`
	pub fn hidden_from_ana(&self) -> Vec<&media::Model> {
		vec![&self.adult, &self.unrated, &self.removed, &self.hidden]
	}

	/// The five books, in the order of the world
	pub fn all_books(&self) -> Vec<&media::Model> {
		vec![
			&self.general,
			&self.adult,
			&self.unrated,
			&self.removed,
			&self.hidden,
		]
	}

	/// Gives every book a real file of its own (a comic with one page) and a saved thumbnail, in a
	/// folder that lasts as long as the returned guard, for the tests that serve a book's
	/// thumbnail, pages or file. The books in the fixture are updated to match.
	pub async fn give_books_files(&mut self, app: &TestApp) -> tempfile::TempDir {
		let dir = tempfile::tempdir().expect("should make a folder for the books");
		for book in [
			&mut self.general,
			&mut self.adult,
			&mut self.unrated,
			&mut self.removed,
			&mut self.hidden,
		] {
			let path = dir.path().join(format!("{}.cbz", book.name));
			write_tiny_cbz(&path);
			// the server makes a thumbnail in a folder of its own, which a test app doesn't have
			let thumbnail = dir.path().join(format!("{}.png", book.name));
			std::fs::write(&thumbnail, TINY_PNG).expect("should write the thumbnail");
			let mut active: media::ActiveModel = book.clone().into();
			active.path = Set(path.to_string_lossy().to_string());
			active.extension = Set("cbz".to_string());
			active.pages = Set(1);
			active.thumbnail_path = Set(Some(thumbnail.to_string_lossy().to_string()));
			*book = active
				.update(app.conn())
				.await
				.expect("should point the book at its file");
		}
		dir
	}

	/// What would give away a book of H, or the library and series that hold only hidden books:
	/// (what it is, how it looks in a response)
	fn hidden_markers(&self) -> Vec<(String, String)> {
		let mut markers = Vec::new();
		for book in self.hidden_from_ana() {
			markers.push((format!("the id of {}", book.name), book.id.clone()));
			markers.push((format!("the title {}", book.name), book.name.clone()));
			markers.push((format!("the path of {}", book.name), book.path.clone()));
		}
		let library_name = self.hidden_library.name.as_str();
		markers.push((
			"the id of the hidden library".to_string(),
			self.hidden_library.id.clone(),
		));
		markers.push((
			"the path of the hidden library".to_string(),
			self.hidden_library.path.clone(),
		));
		// a whole value, so that the word in a sentence or a key is not taken for the library
		markers.push((
			"the name of the hidden library".to_string(),
			format!("\"{library_name}\""),
		));
		markers.push((
			"the name of the hidden library".to_string(),
			format!(">{library_name}<"),
		));
		// the series that hold nothing she may read
		for series in [&self.loose_series, &self.hidden_series] {
			markers.push((format!("the id of {}", series.name), series.id.clone()));
			markers.push((format!("the name {}", series.name), series.name.clone()));
		}
		markers
	}

	/// Panics if `text` (a feed, a page, a body) holds anything that comes from H
	pub fn assert_text_none_hidden(&self, text: &str) {
		for (what, marker) in self.hidden_markers() {
			assert!(
				!text.contains(&marker),
				"a user who may not see it got {what} ({marker:?}) in:\n{text}"
			);
		}
	}

	/// Panics if `json` (a response) holds anything that comes from H
	pub fn assert_none_hidden(&self, json: &Value) {
		self.assert_text_none_hidden(&json.to_string());
	}
}

/// A user rated 16 or less, with *Hidden* hidden from them: ana's limits
async fn limited_user(
	app: &TestApp,
	hidden_library: &library::Model,
	username: &str,
	permissions: Vec<UserPermission>,
) -> TestUser {
	let user = CreateTestUser {
		username: username.to_string(),
		permissions,
		age_restriction: Some(AgeRestrictionInput {
			age: 16,
			restrict_on_unset: true,
		}),
		..Default::default()
	}
	.insert(app)
	.await;
	hide_library(app, &user, hidden_library).await;
	user
}
