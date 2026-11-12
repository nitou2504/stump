import type { Ordering } from '@/components/filters/context'

import { orderingInUrl, relevanceFor } from '../relevance'

describe('relevanceFor', () => {
	it('ranks a search by its own text, without the blanks around it', () => {
		expect(relevanceFor('dune', {})).toBe('dune')
		expect(relevanceFor('  Children of Dune ', {} as Ordering)).toBe('Children of Dune')
	})

	it('ranks nothing when nothing is searched', () => {
		expect(relevanceFor(undefined, {})).toBeUndefined()
		expect(relevanceFor('', {})).toBeUndefined()
		expect(relevanceFor('   ', {})).toBeUndefined()
	})

	it('leaves the order to a sort the reader chose', () => {
		const chosen = { orderBy: 'NAME', direction: 'ASC' } as unknown as Ordering
		expect(relevanceFor('dune', chosen)).toBeUndefined()
		// half a sort is no sort
		expect(relevanceFor('dune', { orderBy: 'NAME' } as unknown as Ordering)).toBe('dune')
	})
})

describe('orderingInUrl', () => {
	it('is the sort the address carries, both halves of it', () => {
		expect(orderingInUrl(new URLSearchParams('search=dune&orderBy=NAME&direction=ASC'))).toEqual({
			orderBy: 'NAME',
			direction: 'ASC',
		})
	})

	it('is no sort when the address has none or only half of one', () => {
		expect(orderingInUrl(new URLSearchParams('search=dune'))).toEqual({})
		expect(orderingInUrl(new URLSearchParams('orderBy=NAME'))).toEqual({})
		expect(orderingInUrl(new URLSearchParams('direction=ASC'))).toEqual({})
	})

	it('keeps the ranking on for a search whose address has no sort, whatever the browser saved', () => {
		// the sort saved by the Explore page is not in a search's address
		const search = new URLSearchParams('search=dune')
		expect(relevanceFor('dune', orderingInUrl(search))).toBe('dune')
		// the one chosen on the search is
		const chosen = new URLSearchParams('search=dune&orderBy=NAME&direction=DESC')
		expect(relevanceFor('dune', orderingInUrl(chosen))).toBeUndefined()
	})
})
