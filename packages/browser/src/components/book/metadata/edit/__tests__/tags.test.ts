import {
	applyPicks,
	cleanValue,
	creatableValue,
	defaultPicks,
	groupVocabulary,
	isFacet,
	matchesSearch,
	orderTags,
	tagGroup,
} from '../tags'

describe('book tag helpers', () => {
	it('tells genres, subgenres and facets apart like the server', () => {
		expect(tagGroup('Philosophy')).toBe('genre')
		expect(tagGroup('Philosophy: Ethics')).toBe('subgenre')
		expect(tagGroup('Origin: French')).toBe('facet')
		expect(isFacet('Illustrated')).toBe(true)
		expect(isFacet('Estante: Ciencia')).toBe(false)
	})

	it('orders tags big, small, facets', () => {
		expect(
			orderTags(['Level: General Reader', 'Philosophy: Ethics', 'Philosophy', 'Philosophy']),
		).toEqual(['Philosophy', 'Philosophy: Ethics', 'Level: General Reader'])
	})

	it('matches word starts, accent-insensitively', () => {
		expect(matchesSearch('Biography & Memoir: Memoir', 'mem bio')).toBe(true)
		expect(matchesSearch('Filosofía', 'filosofia')).toBe(true)
		expect(matchesSearch('Philosophy', 'soph')).toBe(false)
	})

	it('groups the vocabulary, most used first, without the chosen tags', () => {
		const grouped = groupVocabulary(
			[
				{ count: 3, value: 'Philosophy: Ethics' },
				{ count: 480, value: 'Philosophy' },
				{ count: 9, value: 'Level: General Reader' },
				{ count: 404, value: 'Biography & Memoir' },
			],
			'',
			['philosophy'],
		)
		expect(grouped.genre.map((e) => e.value)).toEqual(['Biography & Memoir'])
		expect(grouped.subgenre.map((e) => e.value)).toEqual(['Philosophy: Ethics'])
		expect(grouped.facet.map((e) => e.value)).toEqual(['Level: General Reader'])
	})

	it('only offers to create a value that is new, without separators', () => {
		const entries = [{ count: 1, value: 'Philosophy' }]
		expect(creatableValue('philosophy', entries, [])).toBeNull()
		expect(creatableValue('  Philosophy:  Absurdism ', entries, [])).toBe('Philosophy: Absurdism')
		expect(creatableValue('Memoir', entries, ['Memoir'])).toBeNull()
		expect(cleanValue('a, b;c')).toBe('a b c')
		expect(cleanValue(' ,; ')).toBeNull()
	})

	it('picks the unsupported genres to drop and the default suggestions to add', () => {
		const current = [
			'Physics & Astronomy',
			'Physics & Astronomy: Astronomy',
			'Level: General Reader',
			'Collection: Owned',
		]
		const picks = defaultPicks(
			current,
			[
				{ fits: false, kind: 'GENRE', value: 'Physics & Astronomy' },
				{ fits: false, kind: 'GENRE', value: 'Physics & Astronomy: Astronomy' },
				{ fits: true, kind: 'FACET', value: 'Level: General Reader' },
				{ fits: false, kind: 'FACET', value: 'Collection: Owned' },
			],
			[
				{ kind: 'GENRE', selected: true, value: 'Literary Fiction' },
				{ kind: 'GENRE', selected: true, value: 'Literary Fiction: Philosophical' },
				{ kind: 'GENRE', selected: false, value: 'Science Fiction' },
				{ kind: 'FACET', selected: true, value: 'Level: General Reader' },
			],
		)
		expect(picks.remove).toEqual(['Physics & Astronomy', 'Physics & Astronomy: Astronomy'])
		expect(picks.add).toEqual(['Literary Fiction', 'Literary Fiction: Philosophical'])
		expect(applyPicks(current, picks.remove, picks.add)).toEqual([
			'Literary Fiction',
			'Literary Fiction: Philosophical',
			'Level: General Reader',
			'Collection: Owned',
		])
	})
})
