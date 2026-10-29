import { Media } from '@stump/graphql'

import {
	compilePattern,
	detectKeyedPrefixes,
	groupBooksByField,
	groupingLabel,
} from '../fieldGrouping'

const book = (
	id: string,
	metadata: Partial<NonNullable<Media['metadata']>> = {},
	tags: string[] = [],
): Media =>
	({
		id,
		metadata: { genres: [], writers: [], ...metadata },
		resolvedName: id,
		tags: tags.map((name, index) => ({ id: index, name })),
	}) as unknown as Media

describe('compilePattern', () => {
	it('captures the wildcard, ignoring case and spacing', () => {
		const match = compilePattern('Asignatura: *')!
		expect(match('Asignatura: Física')).toBe('Física')
		expect(match('asignatura:Historia')).toBe('Historia')
		expect(match('Origen: IB')).toBeNull()
	})

	it('treats a pattern without * as a prefix and trims separators', () => {
		expect(compilePattern('Asignatura')!('Asignatura: Química')).toBe('Química')
	})

	it('rejects empty patterns', () => {
		expect(compilePattern('  ')).toBeNull()
		expect(compilePattern('*')).toBeNull()
	})
})

describe('groupBooksByField', () => {
	it('groups by a pattern over genres and tags, keeping books without a value apart', () => {
		const books = [
			book('a', { genres: ['Asignatura: Física'] }),
			book('b', {}, ['Asignatura: Física', 'Origen: IB']),
			book('c', {}, ['Asignatura: Historia']),
			book('d'),
		]
		const { groups, ungrouped } = groupBooksByField(books, {
			field: 'pattern',
			pattern: 'Asignatura: *',
		})
		expect(groups.map((g) => [g.name, g.books.length])).toEqual([
			['Física', 2],
			['Historia', 1],
		])
		expect(ungrouped.map((b) => b.id)).toEqual(['d'])
	})

	it('puts books with several authors in each group', () => {
		const result = groupBooksByField(
			[book('a', { writers: ['Ann', 'Bob'] }), book('b', { writers: ['Bob'] })],
			{ field: 'writers' },
		)
		expect(result.groups.map((g) => [g.name, g.books.length])).toEqual([
			['Ann', 1],
			['Bob', 2],
		])
		expect(result.hasMultiMembership).toBe(true)
	})

	it('orders rating buckets best first and ages ascending', () => {
		const books = [
			book('a', { rating: 3.2 }),
			book('b', { rating: 4.7 }),
			book('c', { ageRating: 18 }),
			book('d', { ageRating: 0 }),
		]
		expect(groupBooksByField(books, { field: 'rating' }).groups.map((g) => g.key)).toEqual([
			'4.5',
			'3',
		])
		expect(groupBooksByField(books, { field: 'ageRating' }).groups.map((g) => g.name)).toEqual([
			'All ages',
			'18+',
		])
	})

	it('leaves Key: value labels out of the plain genre grouping', () => {
		const { groups } = groupBooksByField(
			[book('a', { genres: ['Novela', 'Asignatura: Literatura'] }, ['bcurator:age=All ages'])],
			{ field: 'genres' },
		)
		expect(groups.map((g) => g.name)).toEqual(['Novela'])
	})
})

describe('detectKeyedPrefixes', () => {
	it('suggests Key: value schemes, most common first, without internal ones', () => {
		const prefixes = detectKeyedPrefixes([
			book('a', { genres: ['Asignatura: Física', 'Origen: IB'] }, ['bcurator:age=18']),
			book('b', {}, ['Asignatura: Historia', 'Origen: IB']),
			book('c', {}, ['Asignatura: Historia']),
		])
		expect(prefixes).toEqual([
			{ books: 3, pattern: 'Asignatura: *', prefix: 'Asignatura', values: 2 },
			{ books: 2, pattern: 'Origen: *', prefix: 'Origen', values: 1 },
		])
	})
})

describe('groupingLabel', () => {
	it('names pattern groupings after their prefix', () => {
		expect(groupingLabel({ field: 'pattern', pattern: 'Asignatura: *' })).toBe('Asignatura')
		expect(groupingLabel({ field: 'language' })).toBe('Language')
	})
})
