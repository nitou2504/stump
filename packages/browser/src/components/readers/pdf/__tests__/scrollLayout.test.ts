import { spreadRows } from '../../shared/layout'
import {
	buildScrollLayout,
	dominantPage,
	KEEP_PAGES,
	mountRange,
	prefetchIndexes,
	progressionWithin,
	rowAt,
	scrollTopFor,
	slotAt,
	visibleRange,
} from '../scrollLayout'

const pages = (count: number, height = 1000, width = 700) =>
	Array.from({ length: count }, () => ({ width, height }))

describe('buildScrollLayout', () => {
	it('stacks pages with a gap before, between and after them', () => {
		const layout = buildScrollLayout(
			[
				{ width: 700, height: 1000 },
				{ width: 600, height: 500 },
			],
			16,
		)
		expect(layout.slots).toEqual([
			{ index: 0, top: 16, height: 1000, width: 700, left: 0, row: 0 },
			{ index: 1, top: 1032, height: 500, width: 600, left: 0, row: 1 },
		])
		expect(layout.totalHeight).toBe(1548)
		expect(layout.maxRowWidth).toBe(700)
	})

	it('puts two pages side by side in a spread, the cover alone', () => {
		const sizes = pages(5, 1000, 700)
		const layout = buildScrollLayout(sizes, 16, spreadRows(5, 'double'))
		expect(layout.rows.map((row) => row.pages)).toEqual([[0], [1, 2], [3, 4]])
		expect(layout.slots[2]).toEqual({
			index: 2,
			top: 1032,
			height: 1000,
			width: 700,
			left: 716,
			row: 1,
		})
		expect(layout.rows[1]).toMatchObject({ top: 1032, width: 1416, height: 1000 })
		expect(layout.maxRowWidth).toBe(1416)
		expect(layout.totalHeight).toBe(16 + 3 * 1016)
	})
})

describe('spreads', () => {
	const layout = buildScrollLayout(pages(9), 16, spreadRows(9, 'double'))

	it('finds rows and their pages at an offset', () => {
		expect(rowAt(layout, 0)).toBe(0)
		expect(rowAt(layout, 1032)).toBe(1)
		expect(slotAt(layout, 1032)).toBe(1)
		expect(visibleRange(layout, 500, 900)).toEqual([0, 2])
	})

	it('names the first page of the row filling the view as the current page', () => {
		expect(dominantPage(layout, 1032, 900)).toBe(1)
		expect(dominantPage(layout, 2048, 900)).toBe(3)
	})

	it('mounts whole rows', () => {
		const [start, end] = mountRange(layout, 2048, 900)!
		expect(start % 2).toBe(1)
		expect(end % 2).toBe(0)
		expect(end - start + 1).toBeLessThanOrEqual(KEEP_PAGES)
	})

	it('scrolls to the row of a page, and keeps progression across the row', () => {
		expect(scrollTopFor(layout, 2)).toBe(1016)
		expect(scrollTopFor(layout, 1)).toBe(1016)
		expect(progressionWithin(layout, 2, 1032 + 500)).toBe(0.5)
	})
})

describe('positions', () => {
	const layout = buildScrollLayout(pages(100), 16)

	it('finds the slot at an offset', () => {
		expect(slotAt(layout, 0)).toBe(0)
		expect(slotAt(layout, 1032)).toBe(1)
		expect(slotAt(layout, 1031)).toBe(0)
		expect(slotAt(layout, 99_999_999)).toBe(99)
	})

	it('lists the pages in view', () => {
		expect(visibleRange(layout, 0, 900)).toEqual([0, 0])
		expect(visibleRange(layout, 500, 900)).toEqual([0, 1])
		// in the gap after the first page, which isn't in view any more
		expect(visibleRange(layout, 1020, 900)).toEqual([1, 1])
	})

	it('picks the page filling most of the viewport, the upper one on ties', () => {
		expect(dominantPage(layout, 0, 900)).toBe(0)
		expect(dominantPage(layout, 700, 900)).toBe(1)
		expect(dominantPage(layout, 2032 * 10, 900)).toBe(20)
	})

	it('mounts the visible pages plus a margin, at most KEEP_PAGES', () => {
		expect(mountRange(layout, 0, 900)).toEqual([0, 2])
		const [start, end] = mountRange(buildScrollLayout(pages(100, 200), 8), 4000, 900)!
		expect(end - start + 1).toBeLessThanOrEqual(KEEP_PAGES)
		const center = dominantPage(buildScrollLayout(pages(100, 200), 8), 4000, 900)
		expect(start).toBeLessThanOrEqual(center)
		expect(end).toBeGreaterThanOrEqual(center)
	})

	it('scrolls to a page, or a point inside it, and back', () => {
		expect(scrollTopFor(layout, 0)).toBe(0)
		expect(scrollTopFor(layout, 3)).toBe(3 * 1016)
		expect(scrollTopFor(layout, 3, 0.5)).toBe(3 * 1016 + 16 + 500)
		expect(progressionWithin(layout, 3, 3 * 1016 + 16 + 250)).toBe(0.25)
		expect(scrollTopFor(layout, 500)).toBe(99 * 1016)
	})

	it('is empty without pages', () => {
		const empty = buildScrollLayout([], 16)
		expect(visibleRange(empty, 0, 900)).toBeNull()
		expect(mountRange(empty, 0, 900)).toBeNull()
		expect(dominantPage(empty, 0, 900)).toBe(0)
	})
})

describe('prefetchIndexes', () => {
	it('fetches the next two pages and the previous one, nearest first', () => {
		expect(prefetchIndexes(10, 100)).toEqual([11, 9, 12])
		expect(prefetchIndexes(0, 100)).toEqual([1, 2])
		expect(prefetchIndexes(99, 100)).toEqual([98])
	})
})
