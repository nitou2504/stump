//! PDFium work for the reader: the layout, page pictures at a width tier, and text layers.
//!
//! Everything here runs on the render thread with PDFium's lock held (`worker_loop` in `super`).

use std::io::Cursor;

use pdfium_render::prelude::{
	PdfBitmap, PdfBitmapFormat, PdfBookmark, PdfColor, PdfDocument, PdfPage,
	PdfPageIndex, PdfPoints, PdfRenderConfig,
};
use serde::Serialize;

use super::text::{group_runs, PageChar, PageText};
use crate::{
	config::StumpConfig, fs_utils::ContentType, image::into_image_format,
	media::processor::error::MediaProcessorError,
};

/// Picture widths the reader can ask for (px)
pub const TIERS: [u32; 5] = [720, 1000, 1400, 2000, 2800];

/// Lossy WebP settings: quality 80, method 2 (method 4 doubles the encode time for ~2% smaller
/// files, see the research in `/tmp/pdf-lane/PLAN.md`)
const WEBP_QUALITY: f32 = 80.0;
const WEBP_METHOD: i32 = 2;

/// The outline (table of contents) is capped so a broken PDF can't loop forever
const MAX_OUTLINE_ENTRIES: usize = 5000;
const MAX_OUTLINE_DEPTH: usize = 12;
/// Pages sampled to tell whether the PDF has text
const TEXT_SAMPLES: i32 = 5;

/// The response of `GET /api/v2/pdf/{id}/layout`
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PdfLayout {
	pub v: String,
	pub page_count: i32,
	pub pages: Vec<[f32; 2]>,
	pub labels: Option<Vec<String>>,
	pub outline: Vec<OutlineEntry>,
	pub has_text: bool,
	pub tiers: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OutlineEntry {
	pub title: String,
	pub page: Option<i32>,
	pub children: Vec<OutlineEntry>,
}

/// The tiers allowed by the server's `pdf_max_tier`
pub fn allowed_tiers(max_tier: u32) -> Vec<u32> {
	let allowed: Vec<u32> = TIERS
		.iter()
		.copied()
		.filter(|tier| *tier <= max_tier.max(TIERS[0]))
		.collect();
	if allowed.is_empty() {
		vec![TIERS[0]]
	} else {
		allowed
	}
}

/// Snaps a requested width up to the next allowed tier (or the largest one)
pub fn snap_tier(width: u32, max_tier: u32) -> u32 {
	let tiers = allowed_tiers(max_tier);
	tiers
		.iter()
		.copied()
		.find(|tier| *tier >= width)
		.unwrap_or_else(|| *tiers.last().expect("never empty"))
}

fn round1(value: f32) -> f32 {
	(value * 10.0).round() / 10.0
}

pub fn read_layout(
	document: &PdfDocument<'_>,
	version: &str,
	max_tier: u32,
) -> Result<PdfLayout, MediaProcessorError> {
	let pages = document.pages();
	let page_count = pages.len() as i32;
	let mut sizes = Vec::with_capacity(page_count.max(0) as usize);
	for index in 0..page_count {
		let size = pages.page_size(index as PdfPageIndex)?;
		sizes.push([round1(size.width().value), round1(size.height().value)]);
	}

	let mut outline = Vec::new();
	let mut budget = MAX_OUTLINE_ENTRIES;
	if let Some(root) = document.bookmarks().root() {
		read_outline_level(Some(root), 0, &mut budget, &mut outline);
	}

	let has_text = sample_has_text(document, page_count);

	Ok(PdfLayout {
		v: version.to_string(),
		page_count,
		pages: sizes,
		// Page labels need every page loaded in pdfium-render, which is too slow for big books
		labels: None,
		outline,
		has_text,
		tiers: allowed_tiers(max_tier),
	})
}

fn read_outline_level(
	first: Option<PdfBookmark<'_>>,
	depth: usize,
	budget: &mut usize,
	into: &mut Vec<OutlineEntry>,
) {
	let mut current = first;
	while let Some(bookmark) = current {
		if *budget == 0 {
			return;
		}
		*budget -= 1;
		let title = bookmark
			.title()
			.map(|title| title.trim().to_string())
			.unwrap_or_default();
		let page = bookmark
			.destination()
			.and_then(|destination| destination.page_index().ok())
			.map(|index| index as i32 + 1);
		let mut children = Vec::new();
		if depth + 1 < MAX_OUTLINE_DEPTH {
			read_outline_level(bookmark.first_child(), depth + 1, budget, &mut children);
		}
		into.push(OutlineEntry {
			title,
			page,
			children,
		});
		current = bookmark.next_sibling();
	}
}

/// Whether any of a few pages spread over the book has text
fn sample_has_text(document: &PdfDocument<'_>, page_count: i32) -> bool {
	if page_count <= 0 {
		return false;
	}
	let samples = TEXT_SAMPLES.min(page_count);
	(0..samples).any(|sample| {
		let index = if samples == 1 {
			0
		} else {
			// spread evenly, skipping the first page (often a cover picture)
			((sample + 1) * page_count / (samples + 1)).min(page_count - 1)
		};
		document
			.pages()
			.get(index as PdfPageIndex)
			.ok()
			.and_then(|page| page.text().ok().map(|text| !text.all().trim().is_empty()))
			.unwrap_or(false)
	})
}

fn get_page<'a>(
	document: &PdfDocument<'a>,
	page: i32,
) -> Result<PdfPage<'a>, MediaProcessorError> {
	let count = document.pages().len() as i32;
	if page < 1 || page > count {
		return Err(MediaProcessorError::PageNotFound);
	}
	Ok(document.pages().get((page - 1) as PdfPageIndex)?)
}

/// A rendered page picture
#[derive(Debug, Clone)]
pub struct RenderedPage {
	pub bytes: Vec<u8>,
	pub width: u32,
	pub height: u32,
}

/// Renders a page `tier` pixels wide (aspect kept; very tall pages are capped at 4× the tier)
/// as lossy WebP
pub fn render_tier(
	document: &PdfDocument<'_>,
	page: i32,
	tier: u32,
) -> Result<RenderedPage, MediaProcessorError> {
	let page = get_page(document, page)?;
	let config = render_config()
		.set_target_width(tier as i32)
		.set_maximum_height((tier * 4) as i32)
		.render_form_data(true);
	let bitmap = page.render_with_config(&config)?;
	let (rgb, width, height) = rgb_pixels(&bitmap)?;
	drop(bitmap);
	let bytes = encode_webp(&rgb, width, height)?;
	Ok(RenderedPage {
		bytes,
		width,
		height,
	})
}

/// Opaque pages rendered straight to RGB (24-bit, byte order reversed from PDFium's BGR), so
/// there is no alpha channel to convert or encode
fn render_config() -> PdfRenderConfig {
	PdfRenderConfig::new()
		.set_format(PdfBitmapFormat::BGR)
		.set_reverse_byte_order(true)
		.use_print_quality(false)
		.set_text_smoothing(true)
		.set_image_smoothing(true)
		.set_path_smoothing(true)
		.set_clear_color(PdfColor::new(255, 255, 255, 255))
		.clear_before_rendering(true)
}

/// The bitmap's RGB pixels, tightly packed (PDFium pads rows to 4 bytes when `width * 3`
/// isn't a multiple of 4)
fn rgb_pixels(
	bitmap: &PdfBitmap<'_>,
) -> Result<(Vec<u8>, u32, u32), MediaProcessorError> {
	let width = bitmap.width().max(1) as usize;
	let height = bitmap.height().max(1) as usize;
	let raw = bitmap.as_raw_bytes();
	let row = width * 3;
	let stride = raw.len() / height;
	if stride < row {
		return Err(MediaProcessorError::PdfRenderFailed);
	}
	let pixels = if stride == row {
		raw
	} else {
		let mut packed = Vec::with_capacity(row * height);
		for line in raw.chunks_exact(stride).take(height) {
			packed.extend_from_slice(&line[..row]);
		}
		packed
	};
	Ok((pixels, width as u32, height as u32))
}

/// Lossy WebP at the reader's settings, from tightly packed RGB pixels
pub fn encode_webp(
	rgb: &[u8],
	width: u32,
	height: u32,
) -> Result<Vec<u8>, MediaProcessorError> {
	if rgb.len() < (width * height * 3) as usize {
		return Err(MediaProcessorError::PdfRenderFailed);
	}
	let mut config =
		webp::WebPConfig::new().map_err(|_| MediaProcessorError::PdfRenderFailed)?;
	config.lossless = 0;
	config.quality = WEBP_QUALITY;
	config.method = WEBP_METHOD;
	let encoded = webp::Encoder::from_rgb(rgb, width, height)
		.encode_advanced(&config)
		.map_err(|error| {
			tracing::warn!(?error, "Failed to encode a PDF page as WebP");
			MediaProcessorError::PdfRenderFailed
		})?;
	Ok(encoded.to_vec())
}

/// Maps PDF user-space points to the displayed page (crop box and rotation applied), in points
/// from its top-left corner. Built once per page from three PDFium conversions.
struct DisplayTransform {
	origin: (f64, f64),
	x_axis: (f64, f64),
	y_axis: (f64, f64),
}

/// The virtual bitmap width used for the conversions (big, for precision)
const VIRTUAL_WIDTH: i32 = 100_000;
const PROBE: f32 = 1000.0;

impl DisplayTransform {
	fn new(page: &PdfPage<'_>, width: f32) -> Result<Self, MediaProcessorError> {
		let config = PdfRenderConfig::new().set_target_width(VIRTUAL_WIDTH);
		let scale = width as f64 / VIRTUAL_WIDTH as f64;
		let to_display = |x: f32, y: f32| -> Result<(f64, f64), MediaProcessorError> {
			let (px, py) =
				page.points_to_pixels(PdfPoints::new(x), PdfPoints::new(y), &config)?;
			Ok((px as f64 * scale, py as f64 * scale))
		};
		let origin = to_display(0.0, 0.0)?;
		let x = to_display(PROBE, 0.0)?;
		let y = to_display(0.0, PROBE)?;
		Ok(Self {
			origin,
			x_axis: (
				(x.0 - origin.0) / PROBE as f64,
				(x.1 - origin.1) / PROBE as f64,
			),
			y_axis: (
				(y.0 - origin.0) / PROBE as f64,
				(y.1 - origin.1) / PROBE as f64,
			),
		})
	}

	fn apply(&self, x: f32, y: f32) -> (f32, f32) {
		let (x, y) = (x as f64, y as f64);
		(
			(self.origin.0 + self.x_axis.0 * x + self.y_axis.0 * y) as f32,
			(self.origin.1 + self.x_axis.1 * x + self.y_axis.1 * y) as f32,
		)
	}

	/// A PDF rect (left, bottom, right, top) → display `[left, top, right, bottom]`
	fn rect(&self, left: f32, bottom: f32, right: f32, top: f32) -> [f32; 4] {
		let corners = [
			self.apply(left, bottom),
			self.apply(right, bottom),
			self.apply(left, top),
			self.apply(right, top),
		];
		let xs = corners.map(|(x, _)| x);
		let ys = corners.map(|(_, y)| y);
		[
			xs.iter().copied().fold(f32::INFINITY, f32::min),
			ys.iter().copied().fold(f32::INFINITY, f32::min),
			xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
			ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
		]
	}
}

/// The page's text layer
pub fn extract_text(
	document: &PdfDocument<'_>,
	page_number: i32,
) -> Result<PageText, MediaProcessorError> {
	let page = get_page(document, page_number)?;
	let width = page.width().value;
	let height = page.height().value;
	let text = page.text()?;
	let chars = text.chars();
	let count = chars.len();
	if count == 0 {
		return Ok(PageText {
			width,
			height,
			runs: Vec::new(),
		});
	}
	let transform = DisplayTransform::new(&page, width)?;
	let mut page_chars = Vec::with_capacity(count);
	for index in 0..count {
		let Ok(page_char) = chars.get(index) else {
			continue;
		};
		let ch = page_char.unicode_char().unwrap_or('\u{FFFD}');
		let bounds = if ch == '\r' || ch == '\n' {
			None
		} else {
			page_char.loose_bounds().ok().map(|rect| {
				transform.rect(
					rect.left().value,
					rect.bottom().value,
					rect.right().value,
					rect.top().value,
				)
			})
		};
		page_chars.push(PageChar {
			index: index as i32,
			ch,
			bounds,
		});
	}
	Ok(PageText {
		width,
		height,
		runs: group_runs(&page_chars),
	})
}

/// The image reader's page (`GET /api/v2/media/{id}/page/{n}`), sized by `pdf_render_dpi` and
/// `pdf_max_dimension` as before, encoded lossy when the format is WebP
pub fn render_legacy(
	document: &PdfDocument<'_>,
	page_number: i32,
	config: &StumpConfig,
) -> Result<(ContentType, Vec<u8>), MediaProcessorError> {
	let page = get_page(document, page_number)?;
	let dpi_scale = config.pdf_render_dpi as f32 / 72.0;
	let mut target_width = page.width().value * dpi_scale;
	let mut target_height = page.height().value * dpi_scale;
	let max_dim = config.pdf_max_dimension as f32;
	if max_dim > 0.0 && (target_width > max_dim || target_height > max_dim) {
		let fit = (max_dim / target_width).min(max_dim / target_height);
		target_width *= fit;
		target_height *= fit;
	}
	let render_config = render_config()
		.set_target_width(target_width.max(1.0) as i32)
		.set_maximum_height(target_height.max(1.0) as i32);
	let bitmap = page.render_with_config(&render_config)?;
	let (rgb, width, height) = rgb_pixels(&bitmap)?;
	drop(bitmap);

	let output_format = config.get_pdf_render_format();
	let content_type = ContentType::from(output_format);
	if matches!(
		output_format,
		models::shared::image_processor_options::SupportedImageFormat::Webp
	) {
		return Ok((content_type, encode_webp(&rgb, width, height)?));
	}
	let image = image::RgbImage::from_raw(width, height, rgb)
		.ok_or(MediaProcessorError::PdfRenderFailed)?;
	let mut buffer = Cursor::new(vec![]);
	image.write_to(&mut buffer, into_image_format(output_format))?;
	Ok((content_type, buffer.into_inner()))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn snaps_widths_to_tiers() {
		assert_eq!(snap_tier(1, 2800), 720);
		assert_eq!(snap_tier(720, 2800), 720);
		assert_eq!(snap_tier(721, 2800), 1000);
		assert_eq!(snap_tier(1900, 2800), 2000);
		assert_eq!(snap_tier(9999, 2800), 2800);
		assert_eq!(snap_tier(9999, 2000), 2000, "capped by the max tier");
		assert_eq!(allowed_tiers(2000), vec![720, 1000, 1400, 2000]);
		assert_eq!(
			allowed_tiers(100),
			vec![720],
			"the smallest tier is always allowed"
		);
	}

	#[test]
	fn encodes_lossy_webp() {
		let (width, height) = (64, 32);
		let rgb: Vec<u8> = (0..width * height)
			.flat_map(|i| [(i % 256) as u8, 128, 255])
			.collect();
		let bytes = encode_webp(&rgb, width, height).unwrap();
		assert_eq!(&bytes[0..4], b"RIFF");
		assert_eq!(&bytes[8..12], b"WEBP");
		assert_eq!(&bytes[12..16], b"VP8 ", "lossy");
		let size = imagesize::blob_size(&bytes).unwrap();
		assert_eq!((size.width, size.height), (64, 32));
	}
}
