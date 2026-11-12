import type { Ordering } from '@/components/filters/context'

import { relevanceFor } from '../relevance'

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
		const chosen = { orderBy: 'NAME', direction: 'asc' } as unknown as Ordering
		expect(relevanceFor('dune', chosen)).toBeUndefined()
		// half a sort is no sort
		expect(relevanceFor('dune', { orderBy: 'NAME' } as unknown as Ordering)).toBe('dune')
	})
})
