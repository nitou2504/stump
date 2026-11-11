use crate::data::{AuthContext, CoreContext};
use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use epub::doc::{EpubDoc, NavPoint};
use models::entity::{bookmark, media, media_annotation};
use sea_orm::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek};
use std::{collections::HashMap, path::PathBuf};
use stump_core::media::processor::epub::open_epub;

use super::bookmark::Bookmark;
use super::media::Media;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpubContent {
	label: String,
	content: PathBuf,
	children: Vec<EpubContent>,
	play_order: u32,
}

impl From<NavPoint> for EpubContent {
	fn from(nav_point: NavPoint) -> EpubContent {
		EpubContent {
			label: nav_point.label,
			content: nav_point.content,
			children: nav_point
				.children
				.into_iter()
				.map(EpubContent::from)
				.collect(),
			play_order: nav_point.play_order.unwrap_or(0) as u32,
		}
	}
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SpineItem {
	pub idref: String,
	pub id: Option<String>,
	pub properties: Option<String>,
	pub linear: bool,
}

#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct Epub {
	pub media_id: String,
	pub spine: Vec<SpineItem>,
	pub resources: HashMap<String, (String, String)>,
	// Note: Not using [EpubContent] since it is recursive and selections would be a nightmare
	pub toc: Vec<String>,
	pub metadata: HashMap<String, Vec<String>>,
	pub root_base: String,
	pub root_file: String,
	pub extra_css: Vec<String>,
}

impl Epub {
	fn try_from_with_epub<R: Read + Seek>(
		ident: media::MediaIdentSelect,
		epub_file: EpubDoc<R>,
	) -> Result<Self> {
		// convert path buf to string, return error if any path fails to serialize
		let resources_serialized = epub_file
			.resources
			.into_iter()
			.map(|(k, v)| {
				Ok((
					k,
					(v.path.to_str().ok_or("Invalid path")?.to_string(), v.mime),
				))
			})
			.collect::<Result<HashMap<String, (String, String)>>>()?;

		// serialize toc to string, return error if any path fails to serialize
		let toc_serialized = epub_file
			.toc
			.into_iter()
			.map(|x| Ok(serde_json::to_string(&EpubContent::from(x))?))
			.collect::<Result<Vec<String>>>()?;

		let spine = epub_file
			.spine
			.into_iter()
			.map(|x| SpineItem {
				idref: x.idref,
				id: x.id,
				properties: x.properties,
				linear: x.linear,
			})
			.collect();

		let metadata_map = epub_file.metadata.into_iter().fold(
			HashMap::new(),
			|mut map: HashMap<String, Vec<String>>, item| {
				map.entry(item.property).or_default().push(item.value);
				map
			},
		);

		Ok(Self {
			media_id: ident.id,
			spine,
			resources: resources_serialized,
			toc: toc_serialized,
			metadata: metadata_map,
			root_base: epub_file
				.root_base
				.to_str()
				.ok_or("Invalid path")?
				.to_string(),
			root_file: epub_file
				.root_file
				.to_str()
				.ok_or("Invalid path")?
				.to_string(),
			extra_css: epub_file.extra_css,
		})
	}

	pub fn try_from(ident: media::MediaIdentSelect) -> Result<Self> {
		let epub_file = open_epub(ident.path.as_str()).map_err(|e| {
			tracing::error!("Error parsing epub file: {:?}", e);
			"Error parsing epub file"
		})?;
		Self::try_from_with_epub(ident, epub_file)
	}

	/// Opens the book and reads its package off the async workers. It is blocking work, as long as
	/// the book is big, and the workers are shared by every request.
	pub async fn try_from_async(ident: media::MediaIdentSelect) -> Result<Self> {
		tokio::task::spawn_blocking(move || Self::try_from(ident))
			.await
			.map_err(|error| {
				tracing::error!(?error, "The task that reads an epub failed");
				async_graphql::Error::new("Error parsing epub file")
			})?
	}
}

#[ComplexObject]
impl Epub {
	async fn annotations(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<media_annotation::Model>> {
		let user_id = ctx.data::<AuthContext>()?.id();
		let media_id = self.media_id.clone();
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let sessions = media_annotation::Entity::find()
			.filter(media_annotation::Column::UserId.eq(user_id))
			.filter(media_annotation::Column::MediaId.eq(media_id))
			.all(conn)
			.await?;

		Ok(sessions)
	}

	async fn bookmarks(&self, ctx: &Context<'_>) -> Result<Vec<Bookmark>> {
		let AuthContext { user, .. } = ctx.data::<AuthContext>()?;
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();
		if self.media_id.is_empty() {
			return Err("Media ID not set".into());
		}

		let id = self.media_id.clone();

		Ok(
			bookmark::Entity::find_for_user_and_media_id(user, id.as_ref())
				.into_model::<bookmark::Model>()
				.all(conn)
				.await?
				.into_iter()
				.map(Bookmark::from)
				.collect(),
		)
	}

	async fn media(&self, ctx: &Context<'_>) -> Result<Media> {
		let conn = ctx.data::<CoreContext>()?.conn.as_ref();

		let model = media::ModelWithMetadata::find_by_id(self.media_id.clone())
			.into_model::<media::ModelWithMetadata>()
			.one(conn)
			.await?
			.ok_or("Media not found")?;

		Ok(model.into())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use epub::doc::ResourceItem;
	use models::entity::media::MediaIdentSelect;
	use stump_core::media::processor::epub::MAX_READ_TOC_DEPTH;

	/// An EPUB 2 whose table of contents nests `depth` levels
	fn epub_with_toc_nested(depth: usize) -> Vec<u8> {
		use std::io::{Cursor, Write};
		use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

		let container = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
		let opf = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:1</dc:identifier><dc:title>Deep</dc:title><dc:language>en</dc:language></metadata><manifest><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/></manifest><spine toc="ncx"><itemref idref="c1"/></spine></package>"#;
		let chapter = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>c</title></head><body><p>hello</p></body></html>"#;
		let mut ncx = String::from(
			r#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head/><docTitle><text>Deep</text></docTitle><navMap>"#,
		);
		for i in 0..depth {
			ncx.push_str(&format!(
				r#"<navPoint id="n{i}" playOrder="{}"><navLabel><text>L{i}</text></navLabel><content src="c1.xhtml"/>"#,
				i + 1
			));
		}
		ncx.push_str(&"</navPoint>".repeat(depth));
		ncx.push_str("</navMap></ncx>");

		let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
		for (name, data, method) in [
			(
				"mimetype",
				&b"application/epub+zip"[..],
				CompressionMethod::Stored,
			),
			(
				"META-INF/container.xml",
				container.as_bytes(),
				CompressionMethod::Deflated,
			),
			(
				"OEBPS/content.opf",
				opf.as_bytes(),
				CompressionMethod::Deflated,
			),
			("OEBPS/toc.ncx", ncx.as_bytes(), CompressionMethod::Deflated),
			(
				"OEBPS/c1.xhtml",
				chapter.as_bytes(),
				CompressionMethod::Deflated,
			),
		] {
			writer
				.start_file(
					name,
					SimpleFileOptions::default().compression_method(method),
				)
				.expect("start a file");
			writer.write_all(data).expect("write a file");
		}
		writer.finish().expect("finish").into_inner()
	}

	/// The contents of a book are written to the reader as JSON, by recursion: what a file nests
	/// deeper than the readers walk is not in them, so no book can overflow the stack that does it
	#[tokio::test]
	async fn test_epub_toc_is_no_deeper_than_the_readers_walk() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("deep.epub");
		std::fs::write(&path, epub_with_toc_nested(200)).unwrap();
		let ident = || MediaIdentSelect {
			id: "deep".to_string(),
			path: path.to_string_lossy().into_owned(),
		};

		let epub = Epub::try_from_async(ident()).await.unwrap();
		assert_eq!(epub.toc.len(), 1);
		// one `\"children\":[{` for each level that has a level under it
		assert_eq!(
			epub.toc[0].matches("\"children\":[{").count(),
			MAX_READ_TOC_DEPTH - 1
		);
		// the sync way in is the same one
		let epub = Epub::try_from(ident()).unwrap();
		assert_eq!(
			epub.toc[0].matches("\"children\":[{").count(),
			MAX_READ_TOC_DEPTH - 1
		);
	}

	#[tokio::test]
	async fn test_epub_try_from() {
		let mut epub_doc = EpubDoc::mock().unwrap();
		epub_doc.resources.insert(
			"test.css".to_string(),
			ResourceItem {
				path: PathBuf::from("test.css"),
				mime: "text/css".to_string(),
				properties: None,
			},
		);
		epub_doc.toc = vec![NavPoint {
			label: "test".to_string(),
			content: PathBuf::from("test.html"),
			children: vec![],
			play_order: Some(0),
		}];
		epub_doc.root_base = PathBuf::from("/");
		epub_doc.root_file = PathBuf::from("test.html");
		let epub = Epub::try_from_with_epub(
			MediaIdentSelect {
				id: "test".to_string(),
				path: "test.epub".to_string(),
			},
			epub_doc,
		)
		.unwrap();
		assert_eq!(epub.media_id, "test");
		assert_eq!(epub.resources.get("test.css").unwrap().0, "test.css");
		assert_eq!(epub.resources.get("test.css").unwrap().1, "text/css");
		assert_eq!(epub.toc.len(), 1);
		assert_eq!(epub.toc[0], "{\"label\":\"test\",\"content\":\"test.html\",\"children\":[],\"play_order\":0}".to_string());
		assert_eq!(epub.root_base, "/");
		assert_eq!(epub.root_file, "test.html");
	}

	// Test for malformed epub file, non-utf8 path are os dependent so only run on linux or mac
	// since windows uses utf-16 for paths
	#[cfg(any(target_os = "linux", target_os = "macos"))]
	#[tokio::test]
	async fn test_epub_try_from_malformed() {
		use std::ffi::OsString;
		use std::os::unix::ffi::OsStringExt;
		let malformed_path = PathBuf::from(OsString::from_vec(vec![255]));
		let mut epub_doc = EpubDoc::mock().unwrap();
		epub_doc.resources.insert(
			"test.css".to_string(),
			ResourceItem {
				mime: "text/css".to_string(),
				path: malformed_path.clone(),
				properties: None,
			},
		);
		let epub = Epub::try_from_with_epub(
			MediaIdentSelect {
				id: "test".to_string(),
				path: "test.epub".to_string(),
			},
			epub_doc,
		);
		assert!(epub.is_err());
		// try with toc
		epub_doc = EpubDoc::mock().unwrap();
		epub_doc.toc = vec![NavPoint {
			label: "test".to_string(),
			content: malformed_path.clone(),
			children: vec![],
			play_order: Some(0),
		}];
		let epub = Epub::try_from_with_epub(
			MediaIdentSelect {
				id: "test".to_string(),
				path: "test.epub".to_string(),
			},
			epub_doc,
		);
		assert!(epub.is_err());
		// try with base file
		epub_doc = EpubDoc::mock().unwrap();
		epub_doc.root_base = malformed_path.clone();
		let epub = Epub::try_from_with_epub(
			MediaIdentSelect {
				id: "test".to_string(),
				path: "test.epub".to_string(),
			},
			epub_doc,
		);
		assert!(epub.is_err());
	}
}
