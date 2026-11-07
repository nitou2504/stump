import { fixedLayoutChapterPages, fixedLayoutPerPage } from '../fixedLayout'

describe('fixedLayoutPerPage', () => {
	it('shows two pages side by side only when two columns were asked for', () => {
		expect(fixedLayoutPerPage(2)).toBe(2)
		expect(fixedLayoutPerPage(1)).toBe(1)
		// continuous scroll (null) and unset preferences read as one page
		expect(fixedLayoutPerPage(null)).toBe(1)
		expect(fixedLayoutPerPage(undefined)).toBe(1)
	})
})

describe('fixedLayoutChapterPages', () => {
	// 10 pages; chapters start on pages 1, 4 and 9 (total progressions (p - 1) / 10)
	const starts = [0, 0.3, 0.8]

	it('counts the pages of the chapter the page is in', () => {
		expect(fixedLayoutChapterPages(starts, 1, 10)).toEqual({ page: 1, pages: 3, left: 2 })
		expect(fixedLayoutChapterPages(starts, 3, 10)).toEqual({ page: 3, pages: 3, left: 0 })
		expect(fixedLayoutChapterPages(starts, 4, 10)).toEqual({ page: 1, pages: 5, left: 4 })
		expect(fixedLayoutChapterPages(starts, 8, 10)).toEqual({ page: 5, pages: 5, left: 0 })
	})

	it('runs the last chapter to the end of the book', () => {
		expect(fixedLayoutChapterPages(starts, 9, 10)).toEqual({ page: 1, pages: 2, left: 1 })
		expect(fixedLayoutChapterPages(starts, 10, 10)).toEqual({ page: 2, pages: 2, left: 0 })
	})

	it('reads progressions that are not exact page fractions, and duplicate starts', () => {
		// (starts on pages 1 and 4, the second to the end of the book)
		expect(fixedLayoutChapterPages([0, 0.30000000004, 0.3], 5, 10)).toEqual({
			page: 2,
			pages: 7,
			left: 5,
		})
	})

	it('has no count before the first chapter or without a position', () => {
		expect(fixedLayoutChapterPages([0.5], 2, 10)).toBeNull()
		expect(fixedLayoutChapterPages(starts, undefined, 10)).toBeNull()
		expect(fixedLayoutChapterPages(starts, 1, 0)).toBeNull()
		expect(fixedLayoutChapterPages(starts, 11, 10)).toBeNull()
		expect(fixedLayoutChapterPages([], 1, 10)).toBeNull()
	})
})
