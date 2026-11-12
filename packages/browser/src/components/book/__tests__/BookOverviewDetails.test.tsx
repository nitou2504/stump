import { LibraryType } from '@stump/graphql'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import { BookOverviewDetails } from '../BookOverviewDetails'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))
vi.mock('@/paths', () => ({
	usePaths: () => new Proxy({}, { get: () => () => '/somewhere' }),
}))
vi.mock('@stump/graphql', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/graphql')>()),
	useFragment: (_: unknown, data: unknown) => data,
}))
vi.mock('@/components/ReadMore', () => ({ default: () => null }))
vi.mock('@/components/tags/TagList', () => ({ default: () => null }))
vi.mock('../BookStatRow', () => ({ default: () => null }))

const book = (libraryType: LibraryType) =>
	({
		id: 'book',
		resolvedName: 'Dune',
		seriesPosition: null,
		series: null,
		library: { id: 'lib', name: 'Biblioteca', config: { libraryType } },
		metadata: {
			summary: null,
			writers: ['Frank Herbert'],
			pencillers: ['Moebius'],
			genres: [],
			links: [],
			number: null,
			volume: null,
		},
		tags: [],
		readProgress: null,
		readHistory: [],
	}) as never

describe('BookOverviewDetails credits', () => {
	it('names the author through the translations (not "by" in English)', () => {
		render(
			<MemoryRouter>
				<BookOverviewDetails fragment={book(LibraryType.Book)} />
			</MemoryRouter>,
		)
		expect(screen.getByText(/bookOverviewScene\.by/)).toHaveTextContent('Frank Herbert')
		expect(screen.queryByText(/^by /)).toBeNull()
	})

	it('names the story and art credits of a comic through the translations', () => {
		render(
			<MemoryRouter>
				<BookOverviewDetails fragment={book(LibraryType.Comic)} />
			</MemoryRouter>,
		)
		expect(screen.getByText(/bookOverviewScene\.credits\.story/)).toHaveTextContent('Frank Herbert')
		expect(screen.getByText(/bookOverviewScene\.credits\.art/)).toHaveTextContent('Moebius')
		expect(screen.queryByText(/^(Story|Art) /)).toBeNull()
	})
})
