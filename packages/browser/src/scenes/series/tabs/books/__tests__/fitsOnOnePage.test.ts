import { fitsOnOnePage } from '../fitsOnOnePage'

describe('fitsOnOnePage', () => {
	it('is true for a loaded list of one page that nothing narrows', () => {
		expect(fitsOnOnePage({ loaded: true, totalPages: 1, currentPage: 1, filters: {} })).toBe(true)
		expect(fitsOnOnePage({ loaded: true, totalPages: 0, currentPage: 1 })).toBe(true)
	})

	it('is false while the list is loading (the controls stay as they were)', () => {
		expect(fitsOnOnePage({ loaded: false, totalPages: 1, currentPage: 1 })).toBe(false)
	})

	it('is false when there is more than one page', () => {
		expect(fitsOnOnePage({ loaded: true, totalPages: 2, currentPage: 1 })).toBe(false)
	})

	it('is false while a search, a letter or a filter narrows the list, so it can be undone', () => {
		expect(fitsOnOnePage({ loaded: true, totalPages: 1, currentPage: 1, search: 'dune' })).toBe(false)
		expect(fitsOnOnePage({ loaded: true, totalPages: 1, currentPage: 1, startsWith: 'D' })).toBe(false)
		expect(
			fitsOnOnePage({ loaded: true, totalPages: 1, currentPage: 1, filters: { extension: 'epub' } }),
		).toBe(false)
	})

	it('is false on a page past the end, so its controls are the way back (page 2 of 1 after a bigger page size)', () => {
		expect(fitsOnOnePage({ loaded: true, totalPages: 1, currentPage: 2 })).toBe(false)
		expect(fitsOnOnePage({ loaded: true, totalPages: 0, currentPage: 3 })).toBe(false)
	})
})
