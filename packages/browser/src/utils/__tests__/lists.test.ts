import { nonBlank } from '../lists'

describe('nonBlank', () => {
	it('drops empty, whitespace-only, null and undefined items and keeps the order', () => {
		expect(nonBlank(['', 'History', '   ', null, 'Form: Reportage', undefined])).toEqual([
			'History',
			'Form: Reportage',
		])
	})

	it('trims what it keeps', () => {
		expect(nonBlank(['  Biology & Nature '])).toEqual(['Biology & Nature'])
	})

	it('is an empty list for no list at all', () => {
		expect(nonBlank(null)).toEqual([])
		expect(nonBlank(undefined)).toEqual([])
		expect(nonBlank([])).toEqual([])
	})
})
