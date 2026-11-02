import {
	canShowTwoPages,
	effectiveLayout,
	LAYOUT_OPTIONS,
	pageIndicatorText,
	rowOfPage,
	spreadRows,
} from '../layout'

describe('reader layouts', () => {
	it('offers the four layouts: single and double, paged and continuous', () => {
		expect(LAYOUT_OPTIONS).toEqual([
			{ spread: 'single', flow: 'paged' },
			{ spread: 'double', flow: 'paged' },
			{ spread: 'single', flow: 'continuous' },
			{ spread: 'double', flow: 'continuous' },
		])
	})

	it('shows one page on a narrow screen but keeps the preference', () => {
		const double = { spread: 'double', flow: 'paged' } as const
		expect(canShowTwoPages(390)).toBe(false)
		expect(canShowTwoPages(1440)).toBe(true)
		expect(effectiveLayout(double, 390)).toEqual({ spread: 'single', flow: 'paged' })
		expect(effectiveLayout(double, 1440)).toEqual(double)
		// not measured yet: as stored
		expect(effectiveLayout(double, 0)).toEqual(double)
	})
})

describe('spreadRows', () => {
	it('puts one page per row', () => {
		expect(spreadRows(3, 'single')).toEqual([[0], [1], [2]])
	})

	it('pairs pages after a lone cover, like an open book', () => {
		expect(spreadRows(6, 'double')).toEqual([[0], [1, 2], [3, 4], [5]])
	})

	it('pairs from the first page without the cover alone', () => {
		expect(spreadRows(5, 'double', { coverAlone: false })).toEqual([[0, 1], [2, 3], [4]])
	})

	it('is empty without pages', () => {
		expect(spreadRows(0, 'double')).toEqual([])
	})

	it('finds the row of a page', () => {
		const rows = spreadRows(9, 'double')
		expect(rowOfPage(rows, 0)).toBe(0)
		expect(rowOfPage(rows, 1)).toBe(1)
		expect(rowOfPage(rows, 2)).toBe(1)
		expect(rowOfPage(rows, 8)).toBe(4)
		expect(rowOfPage(rows, 9)).toBe(-1)
	})
})

describe('pageIndicatorText', () => {
	it('names the page, or the pages of a spread', () => {
		expect(pageIndicatorText([23], 327)).toBe('23 / 327')
		expect(pageIndicatorText([22, 23], 327)).toBe('22–23 / 327')
		expect(pageIndicatorText([], 327)).toBe('')
	})
})
