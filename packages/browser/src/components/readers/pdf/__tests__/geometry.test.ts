import {
	boundingRect,
	clampZoom,
	firstPageHint,
	fractionRectStyle,
	mergeLineRects,
	pageCssSize,
	pickTier,
	rectContains,
	toPageFractions,
	zoomIn,
	zoomOut,
} from '../geometry'

describe('pageCssSize', () => {
	const letter: [number, number] = [612, 792]

	it('makes the page as wide as the available width when fitting the width', () => {
		expect(pageCssSize(letter, { width: 390, height: 700 }, 'width')).toEqual({
			width: 390,
			height: 505,
		})
	})

	it('fits the whole page in the available box when fitting the page', () => {
		// height-bound on a wide screen
		expect(pageCssSize(letter, { width: 1408, height: 836 }, 'page')).toEqual({
			width: 646,
			height: 836,
		})
		// width-bound on a narrow one
		expect(pageCssSize(letter, { width: 300, height: 836 }, 'page')).toEqual({
			width: 300,
			height: 388,
		})
	})

	it('scales by the zoom, clamped to 50-500%', () => {
		expect(pageCssSize(letter, { width: 400, height: 0 }, 'width', 2)).toEqual({
			width: 800,
			height: 1035,
		})
		expect(pageCssSize(letter, { width: 400, height: 0 }, 'width', 10).width).toBe(2000)
	})

	it('is empty for unusable sizes', () => {
		expect(pageCssSize([0, 792], { width: 400, height: 400 }, 'width')).toEqual({
			width: 0,
			height: 0,
		})
		expect(pageCssSize(letter, { width: 0, height: 400 }, 'width')).toEqual({ width: 0, height: 0 })
	})
})

describe('zoom', () => {
	it('steps by 20% and stays within the limits', () => {
		expect(zoomIn(1)).toBe(1.2)
		expect(zoomOut(1.2)).toBe(1)
		expect(zoomIn(5)).toBe(5)
		expect(zoomOut(0.5)).toBe(0.5)
		expect(clampZoom(Number.NaN)).toBe(1)
	})
})

describe('pickTier', () => {
	const tiers = [720, 1000, 1400, 2000, 2800]

	it('picks the smallest tier covering the device pixels', () => {
		expect(pickTier(646, 1, tiers)).toBe(720)
		expect(pickTier(646, 2, tiers)).toBe(1400)
		expect(pickTier(390, 3, tiers)).toBe(1400)
		expect(pickTier(1000, 1, tiers)).toBe(1000)
	})

	it('falls back to the largest tier, and to the wanted width without tiers', () => {
		expect(pickTier(2000, 2, tiers)).toBe(2800)
		expect(pickTier(500, 1.5, [])).toBe(750)
	})
})

describe('firstPageHint', () => {
	it('asks for the tier a width-fit page will request', () => {
		// a 390px phone at 2x: the page is 390 css px wide -> 780 device px -> the 1000 tier
		expect(
			firstPageHint({ page: 12, viewportWidth: 390, devicePixelRatio: 2, fit: 'width' }),
		).toEqual({ page: 12, w: 1000 })
		// the same width the reader computes once the layout is there
		const [width] = [pageCssSize([612, 792], { width: 390, height: 800 }, 'width').width]
		expect(
			firstPageHint({ page: 1, viewportWidth: 390, devicePixelRatio: 2, fit: 'width' })?.w,
		).toBe(pickTier(width, 2, [720, 1000, 1400, 2000, 2800]))
	})

	it('applies the zoom and the side padding', () => {
		expect(
			firstPageHint({
				page: 3,
				viewportWidth: 1440,
				devicePixelRatio: 1,
				fit: 'width',
				zoom: 1.5,
				sidePadding: 16,
			}),
		).toEqual({ page: 3, w: 2800 })
	})

	it('gives no hint when the page size depends on the page shape or is unknown', () => {
		expect(
			firstPageHint({ page: 1, viewportWidth: 1440, devicePixelRatio: 1, fit: 'page' }),
		).toBeUndefined()
		expect(
			firstPageHint({ page: 1, viewportWidth: 0, devicePixelRatio: 1, fit: 'width' }),
		).toBeUndefined()
		expect(
			firstPageHint({ page: 0, viewportWidth: 390, devicePixelRatio: 1, fit: 'width' }),
		).toBeUndefined()
	})
})

describe('toPageFractions', () => {
	const page = { left: 100, top: 50, width: 400, height: 500 }

	it('converts boxes to page fractions and clips them to the page', () => {
		expect(
			toPageFractions(
				[
					{ left: 140, top: 100, width: 200, height: 20 },
					{ left: 450, top: 100, width: 100, height: 20 }, // runs off the right edge
					{ left: 0, top: 0, width: 20, height: 20 }, // outside
				],
				page,
			),
		).toEqual([
			{ x: 0.1, y: 0.1, width: 0.5, height: 0.04 },
			{ x: 0.875, y: 0.1, width: 0.125, height: 0.04 },
		])
	})
})

describe('mergeLineRects', () => {
	it('merges the boxes of one line and keeps lines apart', () => {
		const merged = mergeLineRects([
			{ x: 0.5, y: 0.1, width: 0.2, height: 0.02 },
			{ x: 0.1, y: 0.1005, width: 0.39, height: 0.019 },
			{ x: 0.1, y: 0.13, width: 0.6, height: 0.02 },
		])
		expect(merged).toEqual([
			{ x: 0.1, y: 0.1, width: 0.6, height: 0.02 },
			{ x: 0.1, y: 0.13, width: 0.6, height: 0.02 },
		])
	})

	it('keeps the columns of a two-column page apart', () => {
		const merged = mergeLineRects([
			{ x: 0.05, y: 0.2, width: 0.4, height: 0.02 },
			{ x: 0.55, y: 0.2, width: 0.4, height: 0.02 },
		])
		expect(merged).toHaveLength(2)
	})

	it('drops empty boxes', () => {
		expect(mergeLineRects([{ x: 0.1, y: 0.1, width: 0, height: 0.02 }])).toEqual([])
	})
})

describe('rect helpers', () => {
	it('tests points against rects and bounds them', () => {
		const rect = { x: 0.1, y: 0.2, width: 0.3, height: 0.05 }
		expect(rectContains(rect, 0.2, 0.22)).toBe(true)
		expect(rectContains(rect, 0.5, 0.22)).toBe(false)
		const bounds = boundingRect([rect, { x: 0.05, y: 0.3, width: 0.1, height: 0.02 }])
		expect(bounds?.x).toBeCloseTo(0.05)
		expect(bounds?.y).toBeCloseTo(0.2)
		expect(bounds?.width).toBeCloseTo(0.35)
		expect(bounds?.height).toBeCloseTo(0.12)
		expect(boundingRect([])).toBeNull()
		expect(fractionRectStyle({ x: 0.25, y: 0.5, width: 0.25, height: 0.125 })).toEqual({
			left: '25%',
			top: '50%',
			width: '25%',
			height: '12.5%',
		})
	})
})
