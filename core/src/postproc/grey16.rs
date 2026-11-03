//! The 16-grey step for manga CBZ files (Stump's rules, tuned on the library's manga):
//!
//! - only PNG pages are converted (JPEG pages stay: as 4-bit PNGs they mostly grow);
//! - colour pages stay untouched: a page is colour when at least [`COLOUR_SHARE`] of its pixels,
//!   measured on a copy that fits 256 px, have a chroma (max - min of R, G, B) above
//!   [`CHROMA_THRESHOLD`] (covers, colour inserts, chapter openers);
//! - a grey page becomes `round(grey / 17) * 17` (the nearest of 0, 17, ..., 255, no dithering),
//!   stored as a 4-bit palette PNG; it is used only when it decodes to the same size, every pixel
//!   is within [`MAX_ERR`] levels of the original grey, and the file is smaller.
//!
//! Per volume, entry names, order, timestamps and compression method are kept, every other entry
//! (ComicInfo.xml, JPEG pages, ...) is copied byte for byte, and the new CBZ is read back in full
//! (CRC checks, same entry names) before it is accepted.

use std::{
	collections::BTreeMap,
	fs::File,
	io::{BufReader, BufWriter, Read, Seek, Write},
	path::Path,
	sync::atomic::{AtomicBool, Ordering},
};

use image::{DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

/// Share of strongly coloured pixels from which a page counts as colour (0.5 %)
pub const COLOUR_SHARE: f64 = 0.005;
/// Chroma (max - min of R, G, B) above which a pixel is strongly coloured
pub const CHROMA_THRESHOLD: i16 = 40;
/// The largest allowed difference between an original grey and its 16-level value
pub const MAX_ERR: i16 = 9;
/// The colour check looks at a copy of the page that fits in this many pixels
const COLOUR_BOX: u32 = 256;
/// zlib level of the new PNGs (level 9 was 7x slower for files 2 % smaller)
const ZLIB_LEVEL: u8 = 6;

/// What happened to one entry of a CBZ
#[derive(
	Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum PageOutcome {
	/// Converted to 16 grey levels
	Grey16,
	/// A colour page, kept as it is
	Colour,
	/// The 16-grey PNG was not smaller, the page was kept
	NotSmaller,
	/// The check of the 16-grey PNG failed (never expected), the page was kept
	CheckFailed,
	/// Not a PNG (JPEG pages, ComicInfo.xml, ...), copied as it is
	NotPng,
	/// Already a 4-bit palette PNG, copied as it is
	AlreadyGrey16,
	/// The PNG could not be decoded, copied as it is
	Unreadable,
}

impl PageOutcome {
	pub fn label(self) -> &'static str {
		match self {
			Self::Grey16 => "grey16",
			Self::Colour => "colour",
			Self::NotSmaller => "not smaller",
			Self::CheckFailed => "CHECK FAILED",
			Self::NotPng => "not png",
			Self::AlreadyGrey16 => "already 16 greys",
			Self::Unreadable => "unreadable",
		}
	}
}

#[derive(Debug, thiserror::Error)]
pub enum Grey16Error {
	#[error("{0}")]
	Io(#[from] std::io::Error),
	#[error("zip: {0}")]
	Zip(#[from] zip::result::ZipError),
	#[error("png: {0}")]
	Png(#[from] png::EncodingError),
	#[error("the new CBZ failed its check: {0}")]
	Check(String),
	#[error("cancelled")]
	Cancelled,
}

/// The PNG header fields the rules depend on: Pillow (which the Python step uses) treats grey
/// and grey+alpha PNGs as grey without looking at their pixels, and only composites alpha that
/// is a real channel (or a palette's transparency) over white
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PngHeader {
	bit_depth: u8,
	colour_type: u8,
	has_trns: bool,
}

fn png_header(raw: &[u8]) -> Option<PngHeader> {
	const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
	if raw.len() < 33 || &raw[..8] != SIG || &raw[12..16] != b"IHDR" {
		return None;
	}
	let mut has_trns = false;
	let mut pos = 8;
	// walk the chunks before the image data for tRNS
	while pos + 8 <= raw.len() {
		let len = u32::from_be_bytes([raw[pos], raw[pos + 1], raw[pos + 2], raw[pos + 3]])
			as usize;
		let kind = &raw[pos + 4..pos + 8];
		if kind == b"tRNS" {
			has_trns = true;
		}
		if kind == b"IDAT" || kind == b"IEND" {
			break;
		}
		pos += 12 + len;
	}
	Some(PngHeader {
		bit_depth: raw[24],
		colour_type: raw[25],
		has_trns,
	})
}

/// Pillow's RGB -> L conversion (ITU-R 601-2 luma, 16-bit fixed point with rounding)
#[inline]
fn luma(r: u8, g: u8, b: u8) -> u8 {
	((r as u32 * 19595 + g as u32 * 38470 + b as u32 * 7471 + 0x8000) >> 16) as u8
}

/// Pillow's `alpha_composite` of a pixel over opaque white (7 bits of extra precision)
#[inline]
fn over_white(c: u8, a: u8) -> u8 {
	if a == 0 {
		return 255;
	}
	const PRECISION_BITS: u32 = 7;
	let a = a as u32;
	let coef1 = a << PRECISION_BITS;
	let coef2 = (255 << PRECISION_BITS) - coef1;
	let tmp = c as u32 * coef1 + 255 * coef2 + (0x80 << PRECISION_BITS);
	let shift_div255 = ((tmp >> 8) + tmp) >> 8;
	(shift_div255 >> PRECISION_BITS) as u8
}

/// The size Pillow's `thumbnail((256, 256))` gives an image of this size
fn thumbnail_size(width: u32, height: u32) -> (u32, u32) {
	let (bw, bh) = (COLOUR_BOX as f64, COLOUR_BOX as f64);
	if width <= COLOUR_BOX && height <= COLOUR_BOX {
		return (width, height);
	}
	let aspect = width as f64 / height as f64;
	if bw / bh >= aspect {
		let w = (bh * aspect).round().max(1.0);
		(w as u32, COLOUR_BOX)
	} else {
		let h = (bw / aspect).round().max(1.0);
		(COLOUR_BOX, h as u32)
	}
}

/// Whether the page is a colour page by the chroma rule, measured like Pillow's thumbnail:
/// a box reduction by an integer factor (reducing gap 2), then a bicubic resize
fn is_colour(image: &DynamicImage, header: PngHeader) -> bool {
	// grey and grey+alpha PNGs are grey by definition
	if matches!(header.colour_type, 0 | 4) {
		return false;
	}
	let rgb = image.to_rgb8();
	let (w, h) = rgb.dimensions();
	let (tw, th) = thumbnail_size(w, h);
	let small = if (tw, th) == (w, h) {
		rgb
	} else {
		let factor = ((w as f64 / tw as f64 / 2.0).floor() as u32)
			.min((h as f64 / th as f64 / 2.0).floor() as u32)
			.max(1);
		let reduced = if factor > 1 {
			box_reduce(&rgb, factor)
		} else {
			rgb
		};
		image::imageops::resize(&reduced, tw, th, image::imageops::FilterType::CatmullRom)
	};
	let total = (small.width() * small.height()).max(1) as f64;
	let coloured = small
		.pixels()
		.filter(|p| {
			let [r, g, b] = p.0;
			let max = r.max(g).max(b) as i16;
			let min = r.min(g).min(b) as i16;
			max - min > CHROMA_THRESHOLD
		})
		.count() as f64;
	coloured / total >= COLOUR_SHARE
}

/// Average blocks of `factor` x `factor` pixels (the last row/column of blocks may be partial)
fn box_reduce(rgb: &image::RgbImage, factor: u32) -> image::RgbImage {
	let (w, h) = rgb.dimensions();
	let (nw, nh) = (w.div_ceil(factor), h.div_ceil(factor));
	let mut out = image::RgbImage::new(nw, nh);
	for by in 0..nh {
		for bx in 0..nw {
			let (x0, y0) = (bx * factor, by * factor);
			let (x1, y1) = ((x0 + factor).min(w), (y0 + factor).min(h));
			let mut sum = [0u64; 3];
			for y in y0..y1 {
				for x in x0..x1 {
					let p = rgb.get_pixel(x, y).0;
					for c in 0..3 {
						sum[c] += p[c] as u64;
					}
				}
			}
			let n = ((x1 - x0) * (y1 - y0)) as u64;
			let px = [
				((sum[0] + n / 2) / n) as u8,
				((sum[1] + n / 2) / n) as u8,
				((sum[2] + n / 2) / n) as u8,
			];
			out.put_pixel(bx, by, image::Rgb(px));
		}
	}
	out
}

/// The page as 8-bit grey the way Pillow's `convert("L")` gives it, after compositing real
/// alpha over white
fn to_grey(image: &DynamicImage, header: PngHeader) -> Vec<u8> {
	let composite = matches!(header.colour_type, 4 | 6)
		|| (header.colour_type == 3 && header.has_trns);
	match image {
		DynamicImage::ImageLuma8(img) => img.as_raw().clone(),
		DynamicImage::ImageLumaA8(img) => img
			.pixels()
			.map(|p| {
				let [l, a] = p.0;
				if composite {
					over_white(l, a)
				} else {
					l
				}
			})
			.collect(),
		DynamicImage::ImageLuma16(img) => {
			img.pixels().map(|p| (p.0[0] >> 8) as u8).collect()
		},
		DynamicImage::ImageLumaA16(img) => img
			.pixels()
			.map(|p| {
				let [l, a] = p.0;
				let (l, a) = ((l >> 8) as u8, (a >> 8) as u8);
				if composite {
					over_white(l, a)
				} else {
					l
				}
			})
			.collect(),
		_ if image.color().has_alpha() => {
			let rgba = image.to_rgba8();
			rgba.pixels()
				.map(|p| {
					let [r, g, b, a] = p.0;
					if composite {
						luma(over_white(r, a), over_white(g, a), over_white(b, a))
					} else {
						luma(r, g, b)
					}
				})
				.collect()
		},
		_ => {
			let rgb = image.to_rgb8();
			rgb.pixels().map(|p| luma(p.0[0], p.0[1], p.0[2])).collect()
		},
	}
}

/// The 16-level index (0-15) of a grey value: the nearest of 0, 17, ..., 255
#[inline]
fn level(grey: u8) -> u8 {
	((grey as u16 + 8) / 17) as u8
}

/// Encode 16-level indices as a 4-bit palette PNG (no row filters, zlib level 6, like Pillow)
fn encode_grey16(
	indices: &[u8],
	width: u32,
	height: u32,
) -> Result<Vec<u8>, Grey16Error> {
	let row_bytes = (width as usize).div_ceil(2);
	let mut packed = vec![0u8; row_bytes * height as usize];
	for (y, row) in indices.chunks(width as usize).enumerate() {
		let out = &mut packed[y * row_bytes..(y + 1) * row_bytes];
		for (x, &v) in row.iter().enumerate() {
			if x % 2 == 0 {
				out[x / 2] = v << 4;
			} else {
				out[x / 2] |= v;
			}
		}
	}
	let palette: Vec<u8> = (0..16u8).flat_map(|v| [v * 17; 3]).collect();
	let mut buffer = Vec::new();
	{
		let mut encoder = png::Encoder::new(&mut buffer, width, height);
		encoder.set_color(png::ColorType::Indexed);
		encoder.set_depth(png::BitDepth::Four);
		encoder.set_palette(palette);
		encoder.set_deflate_compression(png::DeflateCompression::Level(ZLIB_LEVEL));
		encoder.set_filter(png::Filter::NoFilter);
		let mut writer = encoder.write_header()?;
		writer.write_image_data(&packed)?;
		writer.finish()?;
	}
	Ok(buffer)
}

/// Convert one PNG page. Returns the new bytes when the page was converted
pub fn convert_page(raw: &[u8]) -> (Option<Vec<u8>>, PageOutcome) {
	let Some(header) = png_header(raw) else {
		return (None, PageOutcome::NotPng);
	};
	if header.colour_type == 3 && header.bit_depth <= 4 {
		return (None, PageOutcome::AlreadyGrey16);
	}
	let Ok(image) = image::load_from_memory_with_format(raw, ImageFormat::Png) else {
		return (None, PageOutcome::Unreadable);
	};
	if is_colour(&image, header) {
		return (None, PageOutcome::Colour);
	}
	let (width, height) = (image.width(), image.height());
	let grey = to_grey(&image, header);
	drop(image);
	let indices: Vec<u8> = grey.iter().map(|&g| level(g)).collect();
	let Ok(new) = encode_grey16(&indices, width, height) else {
		return (None, PageOutcome::CheckFailed);
	};
	drop(indices);
	if new.len() >= raw.len() {
		return (None, PageOutcome::NotSmaller);
	}
	if !check_page(&new, &grey, width, height) {
		return (None, PageOutcome::CheckFailed);
	}
	(Some(new), PageOutcome::Grey16)
}

/// The new PNG decodes to the same size and every pixel is within [`MAX_ERR`] of the original
fn check_page(new: &[u8], grey: &[u8], width: u32, height: u32) -> bool {
	let Ok(decoded) = image::load_from_memory_with_format(new, ImageFormat::Png) else {
		return false;
	};
	if decoded.width() != width || decoded.height() != height {
		return false;
	}
	let rgb = decoded.to_rgb8();
	rgb.pixels().zip(grey.iter()).all(|(p, &g)| {
		let v = luma(p.0[0], p.0[1], p.0[2]) as i16;
		(v - g as i16).abs() <= MAX_ERR
	})
}

/// What a volume conversion did
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct VolumeReport {
	pub bytes_in: u64,
	pub bytes_out: u64,
	/// Entries per outcome (labels as in the Python step: `grey16`, `colour`, `not png`, ...)
	pub pages: BTreeMap<String, u32>,
}

impl VolumeReport {
	pub fn count(&self, outcome: PageOutcome) -> u32 {
		self.pages.get(outcome.label()).copied().unwrap_or(0)
	}

	fn add(&mut self, outcome: PageOutcome) {
		*self.pages.entry(outcome.label().to_string()).or_default() += 1;
	}
}

fn is_png_name(name: &str) -> bool {
	name.to_ascii_lowercase().ends_with(".png")
}

/// Convert a CBZ into `dst` (a new file). Pages are converted `threads` at a time; every
/// conversion thread runs at idle priority. The output is checked before this returns Ok
pub fn convert_volume(
	src: &Path,
	dst: &Path,
	threads: usize,
	cancel: &AtomicBool,
) -> Result<VolumeReport, Grey16Error> {
	let bytes_in = std::fs::metadata(src)?.len();
	let mut archive = ZipArchive::new(BufReader::new(File::open(src)?))?;
	let mut writer = ZipWriter::new(BufWriter::new(File::create(dst)?));
	let mut report = VolumeReport {
		bytes_in,
		..Default::default()
	};
	let threads = threads.max(1);
	let mut batch: Vec<usize> = Vec::with_capacity(threads);
	let count = archive.len();
	let mut i = 0;
	while i < count || !batch.is_empty() {
		if cancel.load(Ordering::Relaxed) {
			return Err(Grey16Error::Cancelled);
		}
		let next_is_png = i < count && {
			let name = archive.name_for_index(i).unwrap_or_default();
			is_png_name(name) && !name.ends_with('/')
		};
		if next_is_png && batch.len() < threads {
			batch.push(i);
			i += 1;
			continue;
		}
		if !batch.is_empty() {
			convert_batch(&mut archive, &mut writer, &batch, &mut report)?;
			batch.clear();
			continue;
		}
		// a non-PNG entry: copied with its compressed bytes untouched
		let file = archive.by_index_raw(i)?;
		writer.raw_copy_file(file)?;
		report.add(PageOutcome::NotPng);
		i += 1;
	}
	let mut inner = writer.finish()?;
	inner.flush()?;
	inner.into_inner().map_err(|e| e.into_error())?.sync_all()?;
	report.bytes_out = std::fs::metadata(dst)?.len();
	check_volume(src, dst)?;
	Ok(report)
}

/// Read a batch of PNG entries, convert them on idle-priority threads, write them in order
fn convert_batch<R: Read + Seek, W: Write + Seek>(
	archive: &mut ZipArchive<R>,
	writer: &mut ZipWriter<W>,
	batch: &[usize],
	report: &mut VolumeReport,
) -> Result<(), Grey16Error> {
	let mut entries = Vec::with_capacity(batch.len());
	for &index in batch {
		let mut file = archive.by_index(index)?;
		let mut raw = Vec::with_capacity(file.size() as usize);
		file.read_to_end(&mut raw)?;
		let mut options = SimpleFileOptions::default()
			.compression_method(file.compression())
			.last_modified_time(file.last_modified())
			.large_file(file.size() >= u32::MAX as u64);
		if let Some(mode) = file.unix_mode() {
			options = options.unix_permissions(mode);
		}
		entries.push((file.name().to_string(), raw, options));
	}
	let results: Vec<(Option<Vec<u8>>, PageOutcome)> = std::thread::scope(|scope| {
		let handles: Vec<_> = entries
			.iter()
			.map(|(_, raw, _)| {
				scope.spawn(move || {
					super::priority::lower_current_thread();
					convert_page(raw)
				})
			})
			.collect();
		handles
			.into_iter()
			.map(|h| h.join().unwrap_or((None, PageOutcome::Unreadable)))
			.collect()
	});
	for ((name, raw, options), (new, outcome)) in entries.into_iter().zip(results) {
		writer.start_file(name, options)?;
		writer.write_all(new.as_deref().unwrap_or(&raw))?;
		report.add(outcome);
	}
	Ok(())
}

/// The new CBZ holds the same entry names in the same order and every entry reads back with
/// a valid CRC
fn check_volume(src: &Path, dst: &Path) -> Result<(), Grey16Error> {
	let src_names: Vec<String> = {
		let archive = ZipArchive::new(BufReader::new(File::open(src)?))?;
		archive.file_names_ordered()
	};
	let mut archive = ZipArchive::new(BufReader::new(File::open(dst)?))?;
	let dst_names = archive.file_names_ordered();
	if src_names != dst_names {
		return Err(Grey16Error::Check(
			"entry names or order differ".to_string(),
		));
	}
	let mut sink = std::io::sink();
	for i in 0..archive.len() {
		let mut file = archive.by_index(i)?;
		std::io::copy(&mut file, &mut sink)
			.map_err(|e| Grey16Error::Check(format!("{}: {e}", file.name())))?;
	}
	Ok(())
}

trait OrderedNames {
	fn file_names_ordered(&self) -> Vec<String>;
}

impl<R: Read + Seek> OrderedNames for ZipArchive<R> {
	fn file_names_ordered(&self) -> Vec<String> {
		(0..self.len())
			.filter_map(|i| self.name_for_index(i).map(str::to_string))
			.collect()
	}
}

/// A dry run on a few pages: convert up to `sample` PNG pages spread over the volume and
/// extrapolate the volume's size from them. Returns (expected bytes, pages sampled per outcome)
pub fn estimate_volume(
	src: &Path,
	sample: usize,
) -> Result<(u64, VolumeReport), Grey16Error> {
	let bytes_in = std::fs::metadata(src)?.len();
	let mut archive = ZipArchive::new(BufReader::new(File::open(src)?))?;
	let png_indices: Vec<usize> = (0..archive.len())
		.filter(|&i| archive.name_for_index(i).is_some_and(is_png_name))
		.collect();
	let mut png_compressed: u64 = 0;
	for &i in &png_indices {
		png_compressed += archive.by_index_raw(i)?.compressed_size();
	}
	let mut report = VolumeReport {
		bytes_in,
		..Default::default()
	};
	if png_indices.is_empty() {
		report.bytes_out = bytes_in;
		return Ok((bytes_in, report));
	}
	let n = sample.min(png_indices.len()).max(1);
	let picks: Vec<usize> = (0..n)
		.map(|k| png_indices[(png_indices.len() * (2 * k + 1)) / (2 * n)])
		.collect();
	let (mut sampled_in, mut sampled_out) = (0u64, 0u64);
	for index in picks {
		let mut file = archive.by_index(index)?;
		let stored = file.compressed_size();
		let mut raw = Vec::with_capacity(file.size() as usize);
		file.read_to_end(&mut raw)?;
		drop(file);
		let (new, outcome) = convert_page(&raw);
		report.add(outcome);
		sampled_in += stored;
		sampled_out += new.map(|n| n.len() as u64).unwrap_or(stored);
	}
	let ratio = sampled_out as f64 / sampled_in.max(1) as f64;
	let expected =
		bytes_in - png_compressed + (png_compressed as f64 * ratio).round() as u64;
	report.bytes_out = expected;
	Ok((expected, report))
}

#[cfg(test)]
mod tests {
	use super::*;
	use image::{GrayImage, Luma, Rgb, RgbImage};
	use std::io::Cursor;

	fn png_bytes(image: &DynamicImage) -> Vec<u8> {
		let mut out = Vec::new();
		image
			.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
			.unwrap();
		out
	}

	/// Grey noise (like screentone and scan grain): the 8-bit PNG barely compresses, so the
	/// 4-bit one is always smaller
	fn grey_page(width: u32, height: u32) -> DynamicImage {
		let img = GrayImage::from_fn(width, height, |x, y| {
			let h =
				(x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(40_503)).wrapping_mul(97);
			Luma([(h >> 13) as u8])
		});
		DynamicImage::ImageLuma8(img)
	}

	#[test]
	fn luma_matches_pillow() {
		// values from Pillow 11: Image.new("RGB", (1, 1), c).convert("L").getpixel((0, 0))
		assert_eq!(luma(255, 0, 0), 76);
		assert_eq!(luma(0, 255, 0), 150);
		assert_eq!(luma(0, 0, 255), 29);
		assert_eq!(luma(200, 200, 200), 200);
		assert_eq!(luma(10, 20, 30), 18);
	}

	#[test]
	fn levels_are_the_nearest_of_16() {
		assert_eq!(level(0), 0);
		assert_eq!(level(8), 0);
		assert_eq!(level(9), 1);
		assert_eq!(level(17), 1);
		assert_eq!(level(255), 15);
		for g in 0..=255u8 {
			let v = level(g) as i16 * 17;
			assert!((v - g as i16).abs() <= 8, "grey {g} -> {v}");
		}
	}

	#[test]
	fn over_white_matches_pillow() {
		assert_eq!(over_white(0, 255), 0);
		assert_eq!(over_white(0, 0), 255);
		assert_eq!(over_white(100, 128), 177);
	}

	#[test]
	fn thumbnail_size_matches_pillow() {
		assert_eq!(thumbnail_size(1500, 2250), (171, 256));
		assert_eq!(thumbnail_size(2250, 1500), (256, 171));
		assert_eq!(thumbnail_size(200, 100), (200, 100));
	}

	#[test]
	fn grey_page_becomes_16_levels() {
		let raw = png_bytes(&grey_page(600, 900));
		let (new, outcome) = convert_page(&raw);
		assert_eq!(outcome, PageOutcome::Grey16);
		let new = new.unwrap();
		assert!(new.len() < raw.len());
		let header = png_header(&new).unwrap();
		assert_eq!((header.colour_type, header.bit_depth), (3, 4));
		let decoded = image::load_from_memory(&new).unwrap().to_luma8();
		let original = grey_page(600, 900).to_luma8();
		for (a, b) in decoded.pixels().zip(original.pixels()) {
			assert_eq!(a.0[0] % 17, 0);
			assert!((a.0[0] as i16 - b.0[0] as i16).abs() <= 8);
		}
		// converting again finds nothing to do
		assert_eq!(convert_page(&new).1, PageOutcome::AlreadyGrey16);
	}

	#[test]
	fn colour_page_is_kept() {
		let img = RgbImage::from_fn(400, 600, |x, _| {
			if x < 40 {
				Rgb([220, 30, 30])
			} else {
				Rgb([128, 128, 128])
			}
		});
		let raw = png_bytes(&DynamicImage::ImageRgb8(img));
		assert_eq!(convert_page(&raw), (None, PageOutcome::Colour));
	}

	#[test]
	fn rgb_page_without_colour_is_converted() {
		let grey = grey_page(500, 700).to_luma8();
		let img = RgbImage::from_fn(500, 700, |x, y| {
			let v = grey.get_pixel(x, y).0[0];
			Rgb([v, v, v])
		});
		let raw = png_bytes(&DynamicImage::ImageRgb8(img));
		assert_eq!(convert_page(&raw).1, PageOutcome::Grey16);
	}

	#[test]
	fn jpeg_is_not_png() {
		let mut out = Vec::new();
		grey_page(64, 64)
			.write_to(&mut Cursor::new(&mut out), ImageFormat::Jpeg)
			.unwrap();
		assert_eq!(convert_page(&out), (None, PageOutcome::NotPng));
	}

	#[test]
	fn tiny_page_that_does_not_shrink_is_kept() {
		let raw = png_bytes(&DynamicImage::ImageLuma8(GrayImage::from_pixel(
			4,
			4,
			Luma([0]),
		)));
		assert_eq!(convert_page(&raw), (None, PageOutcome::NotSmaller));
	}

	fn write_cbz(path: &Path) {
		let mut writer = ZipWriter::new(File::create(path).unwrap());
		let deflated = SimpleFileOptions::default()
			.compression_method(zip::CompressionMethod::Deflated)
			.last_modified_time(
				zip::DateTime::from_date_and_time(2020, 5, 6, 7, 8, 10).unwrap(),
			);
		let stored = SimpleFileOptions::default()
			.compression_method(zip::CompressionMethod::Stored)
			.last_modified_time(
				zip::DateTime::from_date_and_time(2021, 1, 2, 3, 4, 6).unwrap(),
			);
		let colour = DynamicImage::ImageRgb8(RgbImage::from_fn(300, 450, |x, _| {
			if x < 100 {
				Rgb([20, 40, 230])
			} else {
				Rgb([250, 250, 250])
			}
		}));
		writer.start_file("ComicInfo.xml", deflated).unwrap();
		writer
			.write_all(b"<ComicInfo><Series>Test</Series></ComicInfo>")
			.unwrap();
		writer.start_file("001.png", stored).unwrap();
		writer.write_all(&png_bytes(&colour)).unwrap();
		for k in 2..=6 {
			writer.start_file(format!("{k:03}.png"), stored).unwrap();
			writer
				.write_all(&png_bytes(&grey_page(500 + k, 700)))
				.unwrap();
		}
		writer.start_file("007.jpg", deflated).unwrap();
		writer.write_all(b"not really a jpeg").unwrap();
		writer.finish().unwrap();
	}

	#[test]
	fn volume_round_trip_keeps_entries() {
		let dir = tempfile::tempdir().unwrap();
		let src = dir.path().join("vol.cbz");
		let dst = dir.path().join("vol.grey16.cbz");
		write_cbz(&src);
		let report = convert_volume(&src, &dst, 2, &AtomicBool::new(false)).unwrap();
		assert_eq!(report.count(PageOutcome::Grey16), 5);
		assert_eq!(report.count(PageOutcome::Colour), 1);
		assert_eq!(report.count(PageOutcome::NotPng), 2);
		assert!(report.bytes_out < report.bytes_in);

		let mut a = ZipArchive::new(File::open(&src).unwrap()).unwrap();
		let mut b = ZipArchive::new(File::open(&dst).unwrap()).unwrap();
		assert_eq!(a.file_names_ordered(), b.file_names_ordered());
		for i in 0..a.len() {
			let (mut fa, mut fb) = (a.by_index(i).unwrap(), b.by_index(i).unwrap());
			assert_eq!(fa.name(), fb.name());
			assert_eq!(fa.compression(), fb.compression());
			let stamp = |d: zip::DateTime| {
				(
					d.year(),
					d.month(),
					d.day(),
					d.hour(),
					d.minute(),
					d.second(),
				)
			};
			assert_eq!(stamp(fa.last_modified()), stamp(fb.last_modified()));
			let (mut da, mut db) = (Vec::new(), Vec::new());
			fa.read_to_end(&mut da).unwrap();
			fb.read_to_end(&mut db).unwrap();
			if fa.name().ends_with(".png") && fa.name() != "001.png" {
				assert_ne!(da, db, "{} should be converted", fa.name());
			} else {
				assert_eq!(da, db, "{} should be copied as is", fa.name());
			}
		}

		// a second pass has nothing left to convert
		let again = dir.path().join("again.cbz");
		let report = convert_volume(&dst, &again, 3, &AtomicBool::new(false)).unwrap();
		assert_eq!(report.count(PageOutcome::Grey16), 0);
		assert_eq!(report.count(PageOutcome::AlreadyGrey16), 5);
	}

	#[test]
	fn estimate_extrapolates_from_samples() {
		let dir = tempfile::tempdir().unwrap();
		let src = dir.path().join("vol.cbz");
		write_cbz(&src);
		let (expected, report) = estimate_volume(&src, 3).unwrap();
		assert!(expected < report.bytes_in);
		assert_eq!(report.pages.values().sum::<u32>(), 3);
	}

	#[test]
	fn cancelled_conversion_stops() {
		let dir = tempfile::tempdir().unwrap();
		let src = dir.path().join("vol.cbz");
		write_cbz(&src);
		let result =
			convert_volume(&src, &dir.path().join("out.cbz"), 1, &AtomicBool::new(true));
		assert!(matches!(result, Err(Grey16Error::Cancelled)));
	}
}
