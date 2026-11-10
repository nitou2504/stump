//! `POST /api/v2/intake/upload`: who may upload, what is checked before the body is read, what the
//! limits do while it streams, and that an upload that stops early leaves nothing behind.

use std::{fs, path::PathBuf};

use models::{
	entity::intake_item,
	shared::{
		enums::UserPermission,
		intake::{IntakeKind, IntakeState},
	},
};
use sea_orm::prelude::*;
use serde_json::Value;
use tempfile::TempDir;
use tests::{fake_data, fixtures};

use crate::common::{
	account::{CreateTestUser, TestUser},
	upload::{message, post_upload, UploadBody},
	TestApp,
};

struct Rest {
	app: TestApp,
	friend: TestUser,
	library_id: String,
	config_dir: PathBuf,
	tmp: TempDir,
}

async fn rest() -> Rest {
	rest_with(|_| {}).await
}

async fn rest_with(change: impl FnOnce(&mut stump_core::config::StumpConfig)) -> Rest {
	let tmp = tempfile::tempdir().expect("tempdir");
	let config_dir = tmp.path().join("config");
	fs::create_dir_all(&config_dir).expect("config folder");
	let config = config_dir.to_string_lossy().to_string();
	let app = TestApp::new_with_default_user_and_config(move |c| {
		c.config_dir = config;
		change(c);
	})
	.await;
	let library_dir = tmp.path().join("library");
	fs::create_dir_all(&library_dir).expect("library folder");
	let library = fake_data::Library {
		path: Some(library_dir.to_string_lossy().to_string()),
		..Default::default()
	}
	.insert(app.conn())
	.await;
	let friend = CreateTestUser {
		username: "friend".to_string(),
		permissions: vec![UserPermission::AddBooks],
		..Default::default()
	}
	.insert(&app)
	.await;
	Rest {
		app,
		friend,
		library_id: library.id,
		config_dir,
		tmp,
	}
}

impl Rest {
	fn epub(&self, title: &str) -> Vec<u8> {
		let path = self
			.tmp
			.path()
			.join(format!("{}.epub", title.replace(' ', "_")));
		fixtures::write_test_epub(
			&path,
			title,
			"<dc:creator>Somebody, Some</dc:creator>",
		);
		fs::read(path).expect("epub bytes")
	}

	async fn upload(&self, body: &UploadBody) -> axum_test::TestResponse {
		post_upload(
			&self.app,
			Some(&self.friend.token),
			&self.library_id,
			None,
			body,
		)
		.await
	}

	async fn items(&self) -> Vec<intake_item::Model> {
		intake_item::Entity::find()
			.all(self.app.conn())
			.await
			.expect("the items should load")
	}

	/// the folders and files an upload staged for the friend, under the config folder
	fn staged(&self) -> Vec<String> {
		let root = self.config_dir.join("intake").join(&self.friend.id);
		let mut found = vec![];
		let mut stack = vec![root.clone()];
		while let Some(dir) = stack.pop() {
			let Ok(entries) = fs::read_dir(&dir) else {
				continue;
			};
			for entry in entries.filter_map(Result::ok) {
				let path = entry.path();
				found.push(
					path.strip_prefix(&root)
						.unwrap()
						.to_string_lossy()
						.into_owned(),
				);
				if path.is_dir() {
					stack.push(path);
				}
			}
		}
		found.sort();
		found
	}
}

fn pdf_bytes() -> Vec<u8> {
	b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n"
		.to_vec()
}

fn zip_bytes(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
	use std::io::Write;
	use zip::{write::SimpleFileOptions, CompressionMethod};

	let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
	for (name, bytes, deflated) in entries {
		let method = if *deflated {
			CompressionMethod::Deflated
		} else {
			CompressionMethod::Stored
		};
		writer
			.start_file(
				*name,
				SimpleFileOptions::default().compression_method(method),
			)
			.unwrap();
		writer.write_all(bytes).unwrap();
	}
	writer.finish().unwrap().into_inner()
}

/// a book arrives, is staged under a safe name, and is analysed like any other
#[tokio::test(flavor = "multi_thread")]
async fn test_a_book_arrives_and_is_analysed() {
	let rest = rest().await;
	let response = rest
		.upload(&UploadBody::new().file("rosa.epub", rest.epub("La rosa")))
		.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	let body: Value = response.json();
	let item = &body["items"][0];
	assert_eq!(item["state"], "ANALYSING");
	assert_eq!(item["kind"], "BOOK");
	assert_eq!(item["name"], "rosa.epub");
	assert_eq!(item["libraryId"], rest.library_id);

	let items = rest.items().await;
	assert_eq!(items.len(), 1);
	assert_eq!(items[0].user_id, rest.friend.id);
	assert_eq!(items[0].files.0[0].staged_name, "000_rosa.epub");
	assert_eq!(items[0].files.0[0].format, "epub");
	// the upload folder is gone: what arrived lives in the item's own
	assert!(
		!rest.staged().iter().any(|p| p.contains("_incoming/")),
		"{:?}",
		rest.staged()
	);
	assert!(PathBuf::from(&items[0].staging_dir)
		.join("000_rosa.epub")
		.is_file());
}

/// nobody, and nobody without the permission, uploads; and the answer about a library is the same
/// whether it is hidden or doesn't exist
#[tokio::test(flavor = "multi_thread")]
async fn test_who_may_upload() {
	let rest = rest().await;
	let body = UploadBody::new().file("rosa.epub", rest.epub("Who"));

	let response = post_upload(&rest.app, None, &rest.library_id, None, &body).await;
	assert_eq!(response.status_code(), 401);

	let reader = CreateTestUser {
		username: "reader".to_string(),
		permissions: vec![UserPermission::UploadFile],
		..Default::default()
	}
	.insert(&rest.app)
	.await;
	let response = post_upload(
		&rest.app,
		Some(&reader.token),
		&rest.library_id,
		None,
		&body,
	)
	.await;
	assert_eq!(response.status_code(), 403, "{}", response.text());

	// a locked account, whatever it holds
	let locked = CreateTestUser {
		username: "locked".to_string(),
		permissions: vec![UserPermission::AddBooks],
		..Default::default()
	}
	.insert(&rest.app)
	.await;
	models::entity::user::ActiveModel {
		id: sea_orm::Set(locked.id.clone()),
		is_locked: sea_orm::Set(true),
		..Default::default()
	}
	.update(rest.app.conn())
	.await
	.expect("should lock the account");
	let response = post_upload(
		&rest.app,
		Some(&locked.token),
		&rest.library_id,
		None,
		&body,
	)
	.await;
	assert!(
		matches!(response.status_code().as_u16(), 401 | 403),
		"{}",
		response.status_code()
	);

	// a library hidden from the friend, and one that isn't there
	let hidden = fake_data::Library::default().insert(rest.app.conn()).await;
	crate::common::visibility::hide_library(&rest.app, &rest.friend, &hidden).await;
	let hidden_answer =
		post_upload(&rest.app, Some(&rest.friend.token), &hidden.id, None, &body).await;
	let unknown_answer = post_upload(
		&rest.app,
		Some(&rest.friend.token),
		"no-such-library",
		None,
		&body,
	)
	.await;
	assert_eq!(hidden_answer.status_code(), 404);
	assert_eq!(unknown_answer.status_code(), 404);
	assert_eq!(message(&hidden_answer), message(&unknown_answer));
	assert!(rest.items().await.is_empty());
	assert!(rest.staged().is_empty(), "{:?}", rest.staged());

	// a mode that isn't one
	let response = post_upload(
		&rest.app,
		Some(&rest.friend.token),
		&rest.library_id,
		Some("everything"),
		&body,
	)
	.await;
	assert_eq!(response.status_code(), 400);
}

/// uploads are off when the server says so
#[tokio::test(flavor = "multi_thread")]
async fn test_uploads_can_be_turned_off() {
	let rest = rest_with(|c| c.enable_upload = false).await;
	let response = rest
		.upload(&UploadBody::new().file("rosa.epub", rest.epub("Off")))
		.await;
	assert_eq!(response.status_code(), 403, "{}", response.text());
	assert!(rest.items().await.is_empty());
}

/// the size is checked against what the request says before a byte of the body is read; a request
/// that doesn't say can't be given room
#[tokio::test(flavor = "multi_thread")]
async fn test_the_size_a_request_declares_is_checked_first() {
	let rest = rest_with(|c| c.intake_max_request_mb = 1).await;
	let response = rest
		.upload(
			&UploadBody::new()
				.file("rosa.epub", rest.epub("Small"))
				.claiming(5 * 1024 * 1024),
		)
		.await;
	assert_eq!(response.status_code(), 413, "{}", response.text());
	assert!(rest.items().await.is_empty());
	assert!(rest.staged().is_empty(), "{:?}", rest.staged());

	let response = rest
		.upload(
			&UploadBody::new()
				.file("rosa.epub", rest.epub("Unsaid"))
				.without_length(),
		)
		.await;
	assert_eq!(response.status_code(), 411, "{}", response.text());
	assert!(rest.items().await.is_empty());
}

/// a request that claims to be small and isn't is stopped while it streams, and what it had
/// staged goes with it
#[tokio::test(flavor = "multi_thread")]
async fn test_going_over_a_limit_while_streaming_leaves_nothing_behind() {
	let rest = rest_with(|c| c.intake_max_file_mb = 1).await;
	// a zip is not a book to read, but it has the right start: 3 MiB of it
	let big = zip_bytes(&[("big.epub", &vec![7u8; 3 * 1024 * 1024], false)]);
	let response = rest
		.upload(
			&UploadBody::new()
				.file("fits.epub", rest.epub("Fits"))
				.file("big.epub", big)
				.claiming(10 * 1024),
		)
		.await;
	assert_eq!(response.status_code(), 413, "{}", response.text());
	assert!(
		message(&response).contains("big.epub"),
		"{}",
		response.text()
	);
	assert!(
		rest.items().await.is_empty(),
		"no item for a request that failed"
	);
	assert!(
		rest.staged()
			.iter()
			.all(|p| !p.contains("000.upload") && !p.contains("001.upload")),
		"nothing of it stays: {:?}",
		rest.staged()
	);
	assert!(
		!rest.staged().iter().any(|p| p.ends_with(".upload")),
		"{:?}",
		rest.staged()
	);
}

/// a user's quota and their open items are held, and an upload says which one it ran into
#[tokio::test(flavor = "multi_thread")]
async fn test_the_quota_and_the_number_of_items_are_held() {
	let rest = rest_with(|c| {
		c.intake_user_quota_mb = 1;
		c.intake_user_max_items = 2;
	})
	.await;
	// ~0.7 MiB of book: it fits the quota of 1 MB once
	let book = |title: &str| {
		let mut bytes = rest.epub(title);
		bytes.extend(vec![0u8; 700 * 1024]);
		bytes
	};
	let response = rest
		.upload(&UploadBody::new().file("one.epub", book("One")))
		.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	let response = rest
		.upload(&UploadBody::new().file("two.epub", book("Two")))
		.await;
	assert_eq!(response.status_code(), 413, "{}", response.text());
	assert!(
		message(&response).contains("1 MB"),
		"{}",
		message(&response)
	);
	assert_eq!(rest.items().await.len(), 1);
	assert!(!rest.staged().iter().any(|p| p.contains("_incoming/")));

	// room again once the first is gone
	let item = rest.items().await.remove(0);
	let mut gone: intake_item::ActiveModel = item.into();
	gone.state = sea_orm::Set(IntakeState::Discarded);
	gone.update(rest.app.conn()).await.unwrap();
	let response = rest
		.upload(&UploadBody::new().file("two.epub", book("Two")))
		.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());

	// the number of items: a third small one is over the limit of two
	let rest = rest_with(|c| c.intake_user_max_items = 2).await;
	for title in ["A", "B"] {
		let response = rest
			.upload(&UploadBody::new().file(&format!("{title}.epub"), rest.epub(title)))
			.await;
		assert_eq!(response.status_code(), 200, "{}", response.text());
	}
	let response = rest
		.upload(&UploadBody::new().file("c.epub", rest.epub("C")))
		.await;
	assert_eq!(response.status_code(), 429, "{}", response.text());
	assert_eq!(rest.items().await.len(), 2);

	// and one request that would make too many
	let rest = rest_with(|c| c.intake_user_max_items = 2).await;
	let response = rest
		.upload(
			&UploadBody::new()
				.file("a.epub", rest.epub("A1"))
				.file("b.epub", rest.epub("B1"))
				.file("c.epub", rest.epub("C1")),
		)
		.await;
	assert_eq!(response.status_code(), 429, "{}", response.text());
	assert!(rest.items().await.is_empty());
	assert!(!rest
		.staged()
		.iter()
		.any(|p| p.contains("_incoming/") || p.ends_with(".upload")));
}

/// a request holds only so many files
#[tokio::test(flavor = "multi_thread")]
async fn test_a_request_holds_only_so_many_files() {
	let rest = rest_with(|c| c.intake_max_files_per_request = 2).await;
	let response = rest
		.upload(
			&UploadBody::new()
				.file("a.epub", rest.epub("Few1"))
				.file("b.epub", rest.epub("Few2"))
				.file("c.epub", rest.epub("Few3")),
		)
		.await;
	assert_eq!(response.status_code(), 429, "{}", response.text());
	assert!(rest.items().await.is_empty());
	assert!(!rest.staged().iter().any(|p| p.ends_with(".upload")));
	let response = rest
		.upload(
			&UploadBody::new()
				.file("a.epub", rest.epub("Few1"))
				.file("b.epub", rest.epub("Few2")),
		)
		.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	assert_eq!(rest.items().await.len(), 2);
}

/// a file is the kind of file its name says, in the formats Stump reads
#[tokio::test(flavor = "multi_thread")]
async fn test_a_file_is_what_its_name_says() {
	let rest = rest().await;
	// a format Stump can't read
	let response = rest
		.upload(&UploadBody::new().file("comic.cb7", b"7z\xBC\xAF\x27\x1C".to_vec()))
		.await;
	assert_eq!(response.status_code(), 415, "{}", response.text());
	assert!(
		message(&response).contains(".cb7"),
		"{}",
		message(&response)
	);
	// no extension at all
	let response = rest
		.upload(&UploadBody::new().file("README", b"hello".to_vec()))
		.await;
	assert_eq!(response.status_code(), 415);
	// a PDF called an EPUB, and an EPUB called a PDF
	let response = rest
		.upload(&UploadBody::new().file("lie.epub", pdf_bytes()))
		.await;
	assert_eq!(response.status_code(), 415, "{}", response.text());
	assert!(message(&response).contains("lie.epub"));
	let response = rest
		.upload(&UploadBody::new().file("lie.pdf", rest.epub("Lie")))
		.await;
	assert_eq!(response.status_code(), 415, "{}", response.text());
	// an empty file
	let response = rest
		.upload(&UploadBody::new().file("empty.epub", Vec::new()))
		.await;
	assert_eq!(response.status_code(), 400, "{}", response.text());
	assert!(rest.items().await.is_empty());
	assert!(!rest.staged().iter().any(|p| p.ends_with(".upload")));

	// every format Stump reads is accepted: EPUB, PDF, CBZ, and a CBR that is really a zip is
	// a CBZ
	let comic = zip_bytes(&[("page-1.png", b"png", false)]);
	let response = rest
		.upload(
			&UploadBody::new()
				.file("book.epub", rest.epub("Formats"))
				.file("paper.pdf", pdf_bytes())
				.file("comic.cbz", comic.clone())
				.file("liar.cbr", comic),
		)
		.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	let mut formats: Vec<String> = rest
		.items()
		.await
		.into_iter()
		.flat_map(|i| i.files.0.into_iter().map(|f| f.format))
		.collect();
	formats.sort();
	assert_eq!(formats, ["cbz", "cbz", "epub", "pdf"]);
}

/// a name with a path in it, or one that tries to leave the folder, is only a name
#[tokio::test(flavor = "multi_thread")]
async fn test_a_name_cannot_leave_the_staging_folder() {
	let rest = rest().await;
	let response = rest
		.upload(
			&UploadBody::new()
				.file("../../../escape.epub", rest.epub("Escape"))
				.file("..\\..\\windows.epub", rest.epub("Windows"))
				.file("/etc/absolute.epub", rest.epub("Absolute")),
		)
		.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	for item in rest.items().await {
		let staging = PathBuf::from(&item.staging_dir);
		assert!(
			staging.starts_with(rest.config_dir.join("intake").join(&rest.friend.id)),
			"{staging:?}"
		);
		for file in &item.files.0 {
			assert!(
				!file.staged_name.contains(['/', '\\'])
					&& !file.staged_name.starts_with('.'),
				"{}",
				file.staged_name
			);
			assert!(staging.join(&file.staged_name).is_file());
		}
	}
	// nothing outside the config folder, the library and the temporary folder was written
	assert!(!rest.tmp.path().join("escape.epub").exists());
	assert!(!PathBuf::from("/etc/absolute.epub").exists());
	assert!(!rest.config_dir.join("escape.epub").exists());
}

/// a folder of volumes is one series; loose files are books
#[tokio::test(flavor = "multi_thread")]
async fn test_a_folder_of_volumes_is_one_series() {
	let rest = rest().await;
	let comic = |n: u8| zip_bytes(&[("page-1.png", &[n], false)]);
	let response = rest
		.upload(
			&UploadBody::new()
				.file("Yotsuba/Yotsuba v01.cbz", comic(1))
				.file("Yotsuba/Yotsuba v02.cbz", comic(2))
				.file("Loose.epub", rest.epub("Loose")),
		)
		.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	let items = rest.items().await;
	assert_eq!(items.len(), 2);
	let series = items.iter().find(|i| i.kind == IntakeKind::Series).unwrap();
	assert_eq!(series.name, "Yotsuba");
	assert_eq!(series.files.0.len(), 2);
	assert!(items
		.iter()
		.any(|i| i.kind == IntakeKind::Book && i.name == "Loose.epub"));
}

/// every format, packs of books included, is taken from somebody who needs approval, within the
/// limits: a zip of books is one series, and a zip that inflates past them is refused with
/// nothing staged
#[tokio::test(flavor = "multi_thread")]
async fn test_a_zip_of_books_is_taken_and_a_bomb_is_not() {
	let rest = rest().await;
	let pack = zip_bytes(&[
		("Series/v01.epub", &rest.epub("Pack One"), false),
		("Series/v02.epub", &rest.epub("Pack Two"), false),
		("Series/notes.txt", b"not a book", false),
	]);
	let response = rest
		.upload(&UploadBody::new().file("Series.zip", pack))
		.await;
	assert_eq!(response.status_code(), 200, "{}", response.text());
	let items = rest.items().await;
	assert_eq!(items.len(), 1, "{items:?}");
	assert_eq!(items[0].kind, IntakeKind::Series);
	assert_eq!(items[0].name, "Series");
	assert_eq!(items[0].files.0.len(), 2);

	let before = rest.staged();
	let bomb = zip_bytes(&[("zeros.pdf", &vec![0u8; 8 * 1024 * 1024], true)]);
	let response = rest.upload(&UploadBody::new().file("bomb.zip", bomb)).await;
	assert_eq!(response.status_code(), 400, "{}", response.text());
	assert!(
		message(&response).contains("inflates far more"),
		"{}",
		response.text()
	);
	assert_eq!(rest.items().await.len(), 1);
	assert_eq!(rest.staged(), before, "nothing of the refused pack stays");
}

/// the way in works for somebody who files books themselves too, with no quota in the way
#[tokio::test(flavor = "multi_thread")]
async fn test_an_approver_is_not_held_to_the_quota() {
	let rest = rest_with(|c| {
		c.intake_user_quota_mb = 1;
		c.intake_user_max_items = 1;
	})
	.await;
	let owner = rest.app.access_token.read().await.clone().unwrap();
	for title in ["First", "Second", "Third"] {
		let mut bytes = rest.epub(title);
		bytes.extend(vec![0u8; 700 * 1024]);
		let response = post_upload(
			&rest.app,
			Some(&owner),
			&rest.library_id,
			None,
			&UploadBody::new().file(&format!("{title}.epub"), bytes),
		)
		.await;
		assert_eq!(response.status_code(), 200, "{title}: {}", response.text());
	}
	assert_eq!(rest.items().await.len(), 3);
}
