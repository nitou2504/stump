import type { Media } from '@stump/graphql'

import { bookFuzzySearch } from '../items/table/mediaColumns'

/** A book the way a reader gets it: its path is the path inside the library, not the server's */
const book = (path: string | null): Media =>
	({ id: 'b1', resolvedName: 'Untitled', path, metadata: null, tags: [] }) as unknown as Media

describe('the file name of a book in a smart list', () => {
	it('is found in the path inside the library, as readers get it', () => {
		// "/Author/Some File.epub": what the server sends to those who don't manage libraries
		expect(bookFuzzySearch(book('/Author/Some File.epub'), 'some file')).toBe(true)
		expect(bookFuzzySearch(book('/Author/Some File.epub'), 'epub')).toBe(true)
	})

	it('is the same for whoever manages libraries, who get the whole path', () => {
		expect(bookFuzzySearch(book('/biblioteca/biblioteca/Author/Some File.epub'), 'some file')).toBe(
			true,
		)
	})

	it('does not match the folders, for either', () => {
		expect(bookFuzzySearch(book('/Author/Some File.epub'), 'author')).toBe(false)
		expect(
			bookFuzzySearch(book('/biblioteca/biblioteca/Author/Some File.epub'), 'biblioteca'),
		).toBe(false)
	})

	it('copes with a book that has no path to give', () => {
		expect(bookFuzzySearch(book(null), 'some file')).toBe(false)
	})
})
