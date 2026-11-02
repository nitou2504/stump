/**
 * Finding a comic page's blank margins so they can be cropped away: Mihon's border detection
 * (its image decoder's `borders.cpp`), on a ~1 megapixel grey copy of the page, with a cap on
 * how much can go from each side (KCC never crops more than 10%; here 15%).
 *
 * Per side: the outermost line is sampled (every second pixel). If both dark and light pixels
 * are common on it, art runs off that side and it isn't cropped. Otherwise the edge's colour is
 * the margin's (white paper or black bleed) and lines are skipped inward until one has more
 * than 0.25% of the other colour.
 */

import type { Box } from './paged/pagedView'

/** A line has content when more than this share of its samples are 'filled' (Mihon) */
export const FILLED_RATIO = 0.0025
/** Darker than this is a dark pixel, lighter than `LIGHT_ABOVE` a light one (Mihon's 0.75) */
export const DARK_BELOW = 191
export const LIGHT_ABOVE = 63
/** The grey copy is about this many pixels: plenty to find an edge */
export const SAMPLE_PIXELS = 1_000_000
/** Never take more than this share of the page from one side */
export const MAX_CROP_PER_SIDE = 0.15
/** Crops smaller than this (each side, of the page) aren't worth a jump in size */
export const MIN_CROP = 0.01

/** A page's crop: the whole page, and its halves for a spread that may be split */
export type PageCrop = { whole: Box | null; left?: Box | null; right?: Box | null }

/** The size of the grey copy for a page of `w` × `h`, and the factor it's shrunk by */
export function sampleSize(w: number, h: number, pixels = SAMPLE_PIXELS) {
	const factor = Math.max(1, Math.ceil(Math.sqrt((w * h) / pixels)))
	return { w: Math.max(1, Math.floor(w / factor)), h: Math.max(1, Math.floor(h / factor)), factor }
}

/** RGBA pixels to their grey levels (BT.601, as Mihon) */
export function toLuma(rgba: Uint8ClampedArray | Uint8Array, count: number): Uint8Array {
	const grey = new Uint8Array(count)
	for (let index = 0, pixel = 0; index < count; index++, pixel += 4) {
		// a transparent pixel is paper, not ink
		const alpha = rgba[pixel + 3]!
		const r = alpha < 128 ? 255 : rgba[pixel]!
		const g = alpha < 128 ? 255 : rgba[pixel + 1]!
		const b = alpha < 128 ? 255 : rgba[pixel + 2]!
		grey[index] = (r * 77 + g * 150 + b * 29) >> 8
	}
	return grey
}

/**
 * The first line, counted inward from an edge, with content; 0 when the edge itself has
 * content (art runs off it) or when nothing is found (a blank page): don't crop that side
 */
function scanEdge(
	at: (line: number, position: number) => number,
	lines: number,
	from: number,
	to: number,
	lineLength: number,
): number {
	const limit = Math.round((lineLength * FILLED_RATIO) / 2)
	let dark = 0
	let light = 0
	for (let position = from; position < to; position += 2) {
		const value = at(0, position)
		if (value < DARK_BELOW) dark++
		else if (value > LIGHT_ABOVE) light++
	}
	if (dark > limit && light > limit) return 0
	// a dark edge is a black margin: content is what's light on it, and the other way round
	const filled = dark > limit ? (v: number) => v > LIGHT_ABOVE : (v: number) => v < DARK_BELOW
	for (let line = 1; line < lines; line++) {
		let count = 0
		for (let position = from; position < to; position += 2) {
			if (filled(at(line, position))) count++
		}
		if (count > limit) return line
	}
	return 0
}

/**
 * The content box of a grey image (`luma`, `w` × `h`), within the columns `[x0, x1)` (the whole
 * width by default, or one half of a spread), in the image's own pixels; `null` when there's
 * nothing worth cropping
 */
export function contentBox(
	luma: Uint8Array,
	w: number,
	h: number,
	x0 = 0,
	x1 = w,
	maxCropPerSide = MAX_CROP_PER_SIDE,
): Box | null {
	const width = x1 - x0
	if (width < 4 || h < 4) return null
	const pixel = (x: number, y: number) => luma[y * w + x]!

	const top = scanEdge((line, x) => pixel(x0 + x, line), h, 0, width, width)
	const bottomInset = scanEdge((line, x) => pixel(x0 + x, h - 1 - line), h, 0, width, width)
	const bottom = h - bottomInset
	if (bottom - top < 2) return null
	const left = scanEdge((line, y) => pixel(x0 + line, y), width, top, bottom, h)
	const rightInset = scanEdge((line, y) => pixel(x1 - 1 - line, y), width, top, bottom, h)
	const right = width - rightInset
	if (right - left < 2) return null

	// one pixel of slack (the samples skip every other pixel), then the cap per side
	let cropLeft = Math.max(0, left - 1)
	let cropTop = Math.max(0, top - 1)
	let cropRight = Math.max(0, width - right - 1)
	let cropBottom = Math.max(0, h - bottom - 1)
	cropLeft = Math.min(cropLeft, Math.floor(width * maxCropPerSide))
	cropRight = Math.min(cropRight, Math.floor(width * maxCropPerSide))
	cropTop = Math.min(cropTop, Math.floor(h * maxCropPerSide))
	cropBottom = Math.min(cropBottom, Math.floor(h * maxCropPerSide))
	// a sliver isn't worth changing the page's size for
	const worth =
		cropLeft > width * MIN_CROP ||
		cropRight > width * MIN_CROP ||
		cropTop > h * MIN_CROP ||
		cropBottom > h * MIN_CROP
	if (!worth) return null
	return {
		x: x0 + cropLeft,
		y: cropTop,
		w: width - cropLeft - cropRight,
		h: h - cropTop - cropBottom,
	}
}

/** A box found on the grey copy, in the page's own pixels */
export function scaleBox(box: Box, factor: number, natural: { w: number; h: number }): Box {
	const x = Math.max(0, Math.floor(box.x * factor))
	const y = Math.max(0, Math.floor(box.y * factor))
	const right = Math.min(natural.w, Math.ceil((box.x + box.w) * factor))
	const bottom = Math.min(natural.h, Math.ceil((box.y + box.h) * factor))
	return { x, y, w: right - x, h: bottom - y }
}

/**
 * A page's crop from its grey copy: the whole page, and for a spread each half on its own (a
 * split spread's halves are cropped separately, as Mihon splits before it crops)
 */
export function pageCrop(
	luma: Uint8Array,
	sample: { w: number; h: number; factor: number },
	natural: { w: number; h: number },
	withHalves: boolean,
): PageCrop {
	const scaled = (box: Box | null) => (box ? scaleBox(box, sample.factor, natural) : null)
	const crop: PageCrop = { whole: scaled(contentBox(luma, sample.w, sample.h)) }
	if (withHalves) {
		const half = Math.floor(sample.w / 2)
		crop.left = scaled(contentBox(luma, sample.w, sample.h, 0, half))
		crop.right = scaled(contentBox(luma, sample.w, sample.h, sample.w - half, sample.w))
	}
	return crop
}

/**
 * The page's background colour for the reader's 'auto' background: the colour of the outer
 * ring of the grey copy when most of it agrees (dark bleed or white paper), else `null`
 */
export function edgeTone(luma: Uint8Array, w: number, h: number): 'dark' | 'light' | null {
	const ring = Math.max(1, Math.round(Math.min(w, h) * 0.03))
	let dark = 0
	let light = 0
	let total = 0
	const count = (value: number) => {
		total++
		if (value < 40) dark++
		else if (value > 215) light++
	}
	for (let y = 0; y < h; y += 2) {
		const row = y * w
		if (y < ring || y >= h - ring) {
			// the top and bottom bands, across the whole width
			for (let x = 0; x < w; x += 2) count(luma[row + x]!)
		} else {
			// the left and right bands
			for (let x = 0; x < ring; x += 2) count(luma[row + x]!)
			for (let x = w - ring; x < w; x += 2) count(luma[row + x]!)
		}
	}
	if (!total) return null
	if (dark / total >= 0.6) return 'dark'
	if (light / total >= 0.6) return 'light'
	return null
}
