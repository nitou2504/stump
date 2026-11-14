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

const book = (
	libraryType: LibraryType,
	lists: Partial<Record<'writers' | 'genres', string[]>> = {},
) =>
	({
		id: 'book',
		resolvedName: 'Dune',
		seriesPosition: null,
		series: null,
		library: { id: 'lib', name: 'Biblioteca', config: { libraryType } },
		metadata: {
			summary: null,
			writers: lists.writers ?? ['Frank Herbert'],
			pencillers: ['Moebius'],
			genres: lists.genres ?? [],
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

describe('BookOverviewDetails lists', () => {
	const renderBook = (lists: Parameters<typeof book>[1]) =>
		render(
			<MemoryRouter>
				<BookOverviewDetails fragment={book(LibraryType.Book, lists)} />
			</MemoryRouter>,
		)

	it('shows a chip for every genre and none for a blank one', () => {
		// what a server that keeps the empty first item of ", History" sends
		renderBook({ genres: ['', 'History', '   ', 'Form: Reportage'] })

		const chips = screen.getAllByRole('link').map((link) => link.textContent?.trim())
		expect(chips).toEqual(expect.arrayContaining(['History', 'Form: Reportage']))
		// every link has a name: no empty chip that would filter on nothing
		expect(chips.every(Boolean)).toBe(true)
	})

	it('names only the authors that have a name', () => {
		renderBook({ writers: ['', 'Charlotte Brontë'] })

		expect(screen.getByText(/bookOverviewScene\.by/)).toHaveTextContent('Charlotte Brontë')
		expect(screen.getByText(/bookOverviewScene\.by/).textContent).not.toMatch(/,/)
		expect(screen.getAllByRole('link').every((link) => link.textContent?.trim())).toBe(true)
	})
})
