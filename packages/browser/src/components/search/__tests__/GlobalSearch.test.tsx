import '@/__mocks__/resizeObserver'

import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PropsWithChildren } from 'react'
import { MemoryRouter, Route, Routes, useLocation } from 'react-router'

import { localeWrapper } from '@/__mocks__/locale'

import GlobalSearch from '../GlobalSearch'
import { GLOBAL_SEARCH_SHORTCUT } from '../shortcut'

vi.mock('@/components/thumbnail/ThumbnailImage', () => ({ ThumbnailImage: () => null }))

// cmdk scrolls the highlighted item into view; jsdom has no layout
Element.prototype.scrollIntoView = vi.fn()

function Location() {
	const location = useLocation()
	return <p data-testid="location">{location.pathname + location.search}</p>
}

type Author = { value: string; count: number }

const ROSE = {
	id: 'b1',
	resolvedName: 'The Name of the Rose',
	thumbnail: { url: '' },
	metadata: { writers: ['Umberto Eco'], rating: 4.2 },
}

type SetupOptions = {
	/** The authors the search names (none unless a test asks) */
	authors?: Author[]
	/** The books it finds */
	books?: (typeof ROSE)[]
}

function setup(
	locale: 'en-US' | 'es-ES' = 'en-US',
	{ authors = [], books = [ROSE] }: SetupOptions = {},
) {
	const LocaleWrapper = localeWrapper(locale)
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		const query = document.toString()
		if (query.includes('query GlobalSearch'))
			return {
				media: {
					nodes: books,
					pageInfo: { __typename: 'OffsetPaginationInfo', totalItems: books.length ? 9 : 0 },
				},
				series: { nodes: [] },
				searchAuthors: authors,
			}
		if (query.includes('query SurpriseMe')) return { randomMedia: [{ id: 'r1' }] }
		throw new Error('Unexpected query')
	})
	function Wrapper({ children }: PropsWithChildren) {
		return (
			<LocaleWrapper>
				<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
					<StumpClientContext.Provider value={{}}>
						<QueryClientProvider client={client}>
							<MemoryRouter>
								<Routes>
									<Route
										path="*"
										element={
											<>
												{children}
												<Location />
											</>
										}
									/>
								</Routes>
							</MemoryRouter>
						</QueryClientProvider>
					</StumpClientContext.Provider>
				</SDKContext.Provider>
			</LocaleWrapper>
		)
	}
	return { execute, Wrapper }
}

describe('GlobalSearch', () => {
	it('is one icon button at every width, and its tooltip names the shortcut', () => {
		const { Wrapper } = setup()
		render(<GlobalSearch />, { wrapper: Wrapper })
		const buttons = screen.getAllByRole('button', { name: 'Search the library' })
		expect(buttons).toHaveLength(1)
		expect(buttons[0]).toHaveAttribute('title', `Search · ${GLOBAL_SEARCH_SHORTCUT}`)
		// the old search box and its hint are gone
		expect(screen.queryByText('Search books, authors…')).not.toBeInTheDocument()
	})

	it('opens with Ctrl+K and offers Surprise me before typing', async () => {
		const user = userEvent.setup()
		const { Wrapper, execute } = setup()
		render(<GlobalSearch />, { wrapper: Wrapper })
		await user.keyboard('{Control>}k{/Control}')
		await user.click(await screen.findByText('Surprise me'))
		await waitFor(() => expect(screen.getByTestId('location')).toHaveTextContent('/books/r1'))
		expect(execute).toHaveBeenCalledTimes(1)
	})

	it('shows books as you type and opens the full results', async () => {
		const user = userEvent.setup()
		const { Wrapper } = setup()
		render(<GlobalSearch />, { wrapper: Wrapper })
		await user.click(screen.getByRole('button', { name: 'Search the library' }))
		await user.type(await screen.findByPlaceholderText('Search books, authors, series…'), 'rose')
		expect(await screen.findByText('The Name of the Rose')).toBeInTheDocument()
		expect(screen.getByText('Umberto Eco · ★ 4.2')).toBeInTheDocument()
		await user.click(screen.getByText('See all 9 books'))
		expect(screen.getByTestId('location')).toHaveTextContent('/books?search=rose')
		expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
	})

	it('speaks Spanish: the icon, the suggestions, the results and the footer', async () => {
		const user = userEvent.setup()
		const { Wrapper } = setup('es-ES')
		render(<GlobalSearch />, { wrapper: Wrapper })

		const icon = screen.getByRole('button', { name: 'Buscar en la biblioteca' })
		expect(icon).toHaveAttribute('title', `Buscar · ${GLOBAL_SEARCH_SHORTCUT}`)
		await user.click(icon)

		// before typing
		const input = await screen.findByPlaceholderText('Busca libros, autores, series…')
		for (const text of [
			'Descubrir',
			'Sorpréndeme',
			'un libro al azar sin leer',
			'Todos los libros',
			'Autores',
			'Géneros',
		]) {
			expect(screen.getByText(text)).toBeInTheDocument()
		}
		expect(screen.getByText('↑↓ para moverte')).toBeInTheDocument()
		expect(screen.getByText(`${GLOBAL_SEARCH_SHORTCUT} en cualquier parte`)).toBeInTheDocument()
		expect(screen.queryByText('Discover')).toBeNull()

		// results
		await user.type(input, 'rose')
		expect(await screen.findByText('The Name of the Rose')).toBeInTheDocument()
		expect(screen.getByText('Libros')).toBeInTheDocument()
		expect(screen.getByText('Ver los 9 libros')).toBeInTheDocument()
		expect(screen.queryByText('See all 9 books')).toBeNull()
	})

	describe('authors', () => {
		const ECO = { value: 'Umberto Eco', count: 12 }
		const FISHER = { value: 'Eco Fisher', count: 1 }

		it('asks for the best title first and for the authors with the text that was typed', async () => {
			const user = userEvent.setup()
			const { Wrapper, execute } = setup()
			render(<GlobalSearch />, { wrapper: Wrapper })
			await user.click(screen.getByRole('button', { name: 'Search the library' }))
			await user.type(await screen.findByPlaceholderText('Search books, authors, series…'), 'rose')
			await screen.findByText('The Name of the Rose')

			const call = execute.mock.calls.find(([doc]) =>
				doc.toString().includes('query GlobalSearch'),
			)!
			const [doc, variables] = call
			expect(variables).toEqual({
				media: { search: 'rose' },
				series: { search: 'rose' },
				// the same text ranks the books and finds the authors
				term: 'rose',
			})
			expect(doc.toString()).toContain('relevanceTo: $term')
			expect(doc.toString()).toContain('searchAuthors(search: $term')
		})

		it('lists the authors a search names, with how many books each has', async () => {
			const user = userEvent.setup()
			const { Wrapper } = setup('en-US', { authors: [ECO, FISHER] })
			render(<GlobalSearch />, { wrapper: Wrapper })
			await user.click(screen.getByRole('button', { name: 'Search the library' }))
			await user.type(await screen.findByPlaceholderText('Search books, authors, series…'), 'eco')

			expect(await screen.findByText('Umberto Eco')).toBeInTheDocument()
			expect(screen.getByText('Authors')).toBeInTheDocument()
			expect(screen.getByText('12 books')).toBeInTheDocument()
			expect(screen.getByText('Eco Fisher')).toBeInTheDocument()
			expect(screen.getByText('1 book')).toBeInTheDocument()
			// the books group is still there, first: Enter opens the best book
			expect(screen.getByText('The Name of the Rose')).toBeInTheDocument()
			expect(
				screen.getByText('Books').compareDocumentPosition(screen.getByText('Authors')) &
					Node.DOCUMENT_POSITION_FOLLOWING,
			).toBeTruthy()
		})

		it("opens an author's books on the books page, filtered like the Browse page does", async () => {
			const user = userEvent.setup()
			const { Wrapper } = setup('en-US', { authors: [ECO] })
			render(<GlobalSearch />, { wrapper: Wrapper })
			await user.click(screen.getByRole('button', { name: 'Search the library' }))
			await user.type(await screen.findByPlaceholderText('Search books, authors, series…'), 'eco')
			await user.click(await screen.findByText('Umberto Eco'))

			const location = screen.getByTestId('location').textContent ?? ''
			expect(location.startsWith('/books?')).toBe(true)
			const filters = new URL(`http://stump${location}`).searchParams.get('filters')
			expect(JSON.parse(filters ?? '')).toEqual({
				metadata: { writers: { hasAnyOf: ['Umberto Eco'] } },
			})
			expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
		})

		it('says nothing matches only when there are no books, authors or series', async () => {
			const user = userEvent.setup()
			// only an author answers: it is a result, not "nothing"
			const { Wrapper } = setup('en-US', { books: [], authors: [ECO] })
			const { unmount } = render(<GlobalSearch />, { wrapper: Wrapper })
			await user.click(screen.getByRole('button', { name: 'Search the library' }))
			await user.type(
				await screen.findByPlaceholderText('Search books, authors, series…'),
				'umberto',
			)
			expect(await screen.findByText('Umberto Eco')).toBeInTheDocument()
			expect(screen.queryByText(/Nothing matches/)).not.toBeInTheDocument()
			unmount()

			const empty = setup('en-US', { books: [], authors: [] })
			render(<GlobalSearch />, { wrapper: empty.Wrapper })
			await user.click(screen.getByRole('button', { name: 'Search the library' }))
			await user.type(await screen.findByPlaceholderText('Search books, authors, series…'), 'zzzz')
			expect(await screen.findByText('Nothing matches “zzzz”')).toBeInTheDocument()
		})

		it('speaks Spanish: the group and the counts', async () => {
			const user = userEvent.setup()
			const { Wrapper } = setup('es-ES', { authors: [ECO, FISHER] })
			render(<GlobalSearch />, { wrapper: Wrapper })
			await user.click(screen.getByRole('button', { name: 'Buscar en la biblioteca' }))
			await user.type(await screen.findByPlaceholderText('Busca libros, autores, series…'), 'eco')

			expect(await screen.findByText('Umberto Eco')).toBeInTheDocument()
			expect(screen.getByText('Autores')).toBeInTheDocument()
			expect(screen.getByText('12 libros')).toBeInTheDocument()
			expect(screen.getByText('1 libro')).toBeInTheDocument()
		})
	})
})
