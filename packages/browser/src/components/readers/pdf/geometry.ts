/**
 * Page geometry for the PDF reader. Pages are measured in PDF points (`layout.pages`) and
 * shown at a CSS size from the fit mode and zoom. Everything drawn on a page (text runs,
 * annotation rects, search hits) is stored in fractions (0-1) of the page box, top-left
 * origin, so it lines up at any zoom.
 */

/** `[width, height]` in PDF points */
export type PageSize = [number, number]

export type FitMode = 'width' | 'page'

export type Size = { width: number; height: number }

export type FractionRect = { x: number; y: number; width: number; height: number }

/** A box in CSS pixels (e.g. from `getClientRects`) */
export type PixelRect = { left: number; top: number; width: number; height: number }

export const MIN_ZOOM = 0.5
export const MAX_ZOOM = 5
export const ZOOM_STEP = 1.2

export function clampZoom(zoom: number): number {
	if (!Number.isFinite(zoom)) return 1
	return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Math.round(zoom * 100) / 100))
}

export function zoomIn(zoom: number): number {
	return clampZoom(zoom * ZOOM_STEP)
}

export function zoomOut(zoom: number): number {
	return clampZoom(zoom / ZOOM_STEP)
}

/**
 * The CSS size of a page: `width` fit makes it as wide as the available width, `page` fit
 * makes the whole page fit in the available box; the zoom then scales that.
 */
export function pageCssSize(page: PageSize, available: Size, fit: FitMode, zoom = 1): Size {
	const [pageWidth, pageHeight] = page
	if (pageWidth <= 0 || pageHeight <= 0 || available.width <= 0) {
		return { width: 0, height: 0 }
	}
	const widthScale = available.width / pageWidth
	const scale =
		fit === 'page' && available.height > 0
			? Math.min(widthScale, available.height / pageHeight)
			: widthScale
	const factor = scale * clampZoom(zoom)
	return { width: Math.round(pageWidth * factor), height: Math.round(pageHeight * factor) }
}

/**
 * The picture width to ask the server for: the smallest tier at least as wide as the page
 * shows in device pixels, or the largest tier. The server snaps to its own tiers anyway;
 * asking for exact tiers keeps the URLs (and so the browser cache) stable.
 */
export function pickTier(cssWidth: number, devicePixelRatio: number, tiers: number[]): number {
	const wanted = Math.ceil(cssWidth * Math.max(1, devicePixelRatio || 1))
	const sorted = [...tiers].filter((tier) => tier > 0).sort((a, b) => a - b)
	if (!sorted.length) return Math.max(1, wanted)
	return sorted.find((tier) => tier >= wanted) ?? sorted[sorted.length - 1]!
}

/** The server's picture tiers before it has sent the layout (`layout.tiers` once known) */
export const DEFAULT_TIERS = [720, 1000, 1400, 2000, 2800]

/**
 * The first page's picture to ask for together with the layout, so the server starts it
 * right away. Only a `width` fit is known before the layout arrives (its size doesn't depend
 * on the page's shape), so other fits return `undefined` rather than a wrong guess that
 * would keep the server's render thread busy.
 */
export function firstPageHint({
	page,
	viewportWidth,
	devicePixelRatio,
	fit,
	zoom = 1,
	sidePadding = 0,
	tiers = DEFAULT_TIERS,
}: {
	page: number
	viewportWidth: number
	devicePixelRatio: number
	fit: FitMode
	zoom?: number
	sidePadding?: number
	tiers?: number[]
}): { page: number; w: number } | undefined {
	if (fit !== 'width' || !(viewportWidth > 0) || !(page >= 1)) return undefined
	const cssWidth = Math.max(0, viewportWidth - sidePadding * 2) * clampZoom(zoom)
	if (cssWidth <= 0) return undefined
	return { page: Math.round(page), w: pickTier(cssWidth, devicePixelRatio, tiers) }
}

/** A page-fraction rect as CSS percentages, for absolutely positioned overlays */
export function fractionRectStyle(rect: FractionRect) {
	return {
		left: `${rect.x * 100}%`,
		top: `${rect.y * 100}%`,
		width: `${rect.width * 100}%`,
		height: `${rect.height * 100}%`,
	}
}

/** Converts boxes in CSS pixels to fractions of a page box, clipped to the page */
export function toPageFractions(rects: PixelRect[], page: PixelRect): FractionRect[] {
	if (page.width <= 0 || page.height <= 0) return []
	const result: FractionRect[] = []
	for (const rect of rects) {
		const left = Math.max(rect.left, page.left)
		const top = Math.max(rect.top, page.top)
		const right = Math.min(rect.left + rect.width, page.left + page.width)
		const bottom = Math.min(rect.top + rect.height, page.top + page.height)
		if (right - left < 0.5 || bottom - top < 0.5) continue
		result.push({
			x: (left - page.left) / page.width,
			y: (top - page.top) / page.height,
			width: (right - left) / page.width,
			height: (bottom - top) / page.height,
		})
	}
	return result
}

/**
 * Merges the boxes of a selection into one box per line: boxes that overlap vertically by
 * at least half the smaller height and are close horizontally (gap up to `gapFactor` × the
 * line height) become one. The result is ordered top to bottom, left to right and rounded
 * to 5 decimals, the precision the server keeps.
 */
export function mergeLineRects(rects: FractionRect[], gapFactor = 1.5): FractionRect[] {
	const sorted = rects
		.filter((rect) => rect.width > 0 && rect.height > 0)
		.sort((a, b) => a.y + a.height / 2 - (b.y + b.height / 2) || a.x - b.x)

	const lines: FractionRect[] = []
	for (const rect of sorted) {
		const line = lines.find((candidate) => sameLine(candidate, rect, gapFactor))
		if (line) {
			const right = Math.max(line.x + line.width, rect.x + rect.width)
			const bottom = Math.max(line.y + line.height, rect.y + rect.height)
			line.x = Math.min(line.x, rect.x)
			line.y = Math.min(line.y, rect.y)
			line.width = right - line.x
			line.height = bottom - line.y
		} else {
			lines.push({ ...rect })
		}
	}

	return lines
		.sort((a, b) => a.y - b.y || a.x - b.x)
		.map((rect) => ({
			x: round5(rect.x),
			y: round5(rect.y),
			width: round5(rect.width),
			height: round5(rect.height),
		}))
}

function sameLine(a: FractionRect, b: FractionRect, gapFactor: number): boolean {
	const overlap = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y)
	const minHeight = Math.min(a.height, b.height)
	if (overlap < minHeight * 0.5) return false
	const gap = Math.max(a.x, b.x) - Math.min(a.x + a.width, b.x + b.width)
	return gap <= Math.max(a.height, b.height) * gapFactor
}

function round5(value: number): number {
	return Math.round(value * 100000) / 100000
}

/** Whether a point (page fractions) is inside a rect, with a small tolerance */
export function rectContains(rect: FractionRect, x: number, y: number, tolerance = 0.002): boolean {
	return (
		x >= rect.x - tolerance &&
		x <= rect.x + rect.width + tolerance &&
		y >= rect.y - tolerance &&
		y <= rect.y + rect.height + tolerance
	)
}

/** The bounding box of rects, or `null` for none */
export function boundingRect(rects: FractionRect[]): FractionRect | null {
	if (!rects.length) return null
	const left = Math.min(...rects.map((rect) => rect.x))
	const top = Math.min(...rects.map((rect) => rect.y))
	const right = Math.max(...rects.map((rect) => rect.x + rect.width))
	const bottom = Math.max(...rects.map((rect) => rect.y + rect.height))
	return { x: left, y: top, width: right - left, height: bottom - top }
}
