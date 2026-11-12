import { renderHook } from '@testing-library/react'

import { localeWrapper } from '@/__mocks__/locale'

import { type StatWord, useStatLabels } from '../useStatLabels'

const KINDS: StatWord[] = ['books', 'series', 'libraries', 'lists', 'authors', 'genres']

describe('useStatLabels', () => {
	it('puts the right plural after a count', () => {
		const { result } = renderHook(() => useStatLabels(), { wrapper: localeWrapper('en-US') })

		expect(result.current.count('books', 1)).toBe('1 book')
		expect(result.current.count('books', 8756)).toBe('8,756 books')
		expect(result.current.count('series', 1)).toBe('1 series')
		expect(result.current.count('libraries', 2)).toBe('2 libraries')
		expect(result.current.read()).toBe('read')
	})

	it('speaks Spanish to a Spanish reader', () => {
		const { result } = renderHook(() => useStatLabels(), { wrapper: localeWrapper('es-ES') })

		expect(result.current.count('books', 1)).toBe('1 libro')
		expect(result.current.count('books', 813)).toBe('813 libros')
		expect(result.current.count('series', 1)).toBe('1 serie')
		expect(result.current.count('libraries', 4)).toBe('4 bibliotecas')
		expect(result.current.word('authors', 3)).toBe('autores')
		expect(result.current.read()).toBe('leídos')
	})

	it.each(['en-US', 'es-ES'] as const)('has every word in %s, one and many', (locale) => {
		const { result } = renderHook(() => useStatLabels(), { wrapper: localeWrapper(locale) })

		for (const kind of KINDS) {
			for (const count of [1, 2]) {
				// a missing key would come back as the key itself
				expect(result.current.word(kind, count)).not.toContain('statChips')
			}
		}
	})
})
