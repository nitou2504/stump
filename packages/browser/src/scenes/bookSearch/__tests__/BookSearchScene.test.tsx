import '@/__mocks__/resizeObserver'

import { SDKContext, StumpClientContext } from '@stump/client'
import { MediaModelOrdering, OrderDirection } from '@stump/graphql'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, waitFor } from '@testing-library/react'
import { PropsWithChildren, Suspense } from 'react'
import { MemoryRouter } from 'react-router'

import { localeWrapper } from '@/__mocks__/locale'

import BookSearchScene from '../BookSearchScene'

// The page's own controls and cards are other tests' business: this one is about what the page asks for
vi.mock('@/hooks/useIsInView', () => ({ default: () => [{ current: null }, false] }))
vi.mock('@/hooks/usePreferences', () => ({
	usePreferences: () => ({
		preferences: { enableAlphabetSelect: false },
		update: vi.fn(),
		store: vi.fn(),
	}),
}))
vi.mock('@/components/filters', async (importOriginal) => ({
	...(await importOriginal<typeof import('@/components/filters')>()),
	FilterHeader: () => null,
	URLFilterContainer: ({ children }: PropsWithChildren) => <>{children}</>,
	URLFilterDrawer: () => null,
	URLOrdering: () => null,
}))
vi.mock('@/components/sharedLayout', () => ({
	PageHeader: Object.assign(({ children }: PropsWithChildren) => <>{children}</>, {
		Start: ({ children }: PropsWithChildren) => <>{children}</>,
	}),
}))
vi.mock('@/components/TableOrGridLayout', () => ({ default: () => null }))
vi.mock('@/components/container/GridSizeSlider', () => ({ GridSizeSlider: () => null }))
vi.mock('@/components/book', () => ({ BookCard: () => null, BookTable: () => null }))
vi.mock('../AuthorMatches', () => ({ default: () => null }))
vi.mock('../../book/BooksAlphabet', () => ({ default: () => null }))

/** What the Explore page saves in the browser when somebody sorts there (the layout store's own key) */
const SAVED_SORT = {
	orderBy: MediaModelOrdering.Name,
	direction: OrderDirection.Desc,
}

type Variables = {
	relevanceTo?: string | null
	orderBy: unknown[]
}

function setup(url: string) {
	localStorage.setItem(
		'stump-books-global-layout-store',
		JSON.stringify({ state: { ordering: SAVED_SORT }, version: 1 }),
	)
	const LocaleWrapper = localeWrapper('en-US')
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		if (document.toString().includes('query BookSearchScene')) {
			return {
				media: {
					nodes: [],
					pageInfo: {
						__typename: 'OffsetPaginationInfo',
						currentPage: 1,
						totalPages: 1,
						totalItems: 0,
					},
				},
			}
		}
		throw new Error('Unexpected query')
	})
	function Wrapper({ children }: PropsWithChildren) {
		return (
			<LocaleWrapper>
				<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
					<StumpClientContext.Provider value={{}}>
						<QueryClientProvider client={client}>
							<MemoryRouter initialEntries={[url]}>
								<Suspense fallback={null}>{children}</Suspense>
							</MemoryRouter>
						</QueryClientProvider>
					</StumpClientContext.Provider>
				</SDKContext.Provider>
			</LocaleWrapper>
		)
	}
	const asked = () =>
		execute.mock.calls
			.filter(([document]) => document.toString().includes('query BookSearchScene'))
			.map(([, variables]) => variables as unknown as Variables)
	return { Wrapper, asked }
}

describe('BookSearchScene: what a search asks for', () => {
	afterEach(() => localStorage.clear())

	it('ranks the books by the search, and a sort saved by another page only orders inside each rank', async () => {
		const { Wrapper, asked } = setup('/books?search=dune')
		render(<BookSearchScene />, { wrapper: Wrapper })
		await waitFor(() => expect(asked().length).toBeGreaterThan(0))

		for (const variables of asked()) {
			expect(variables.relevanceTo).toBe('dune')
			// the saved sort is still what orders the books of one rank
			expect(variables.orderBy).toEqual([
				{ media: { field: MediaModelOrdering.Name, direction: OrderDirection.Desc } },
			])
		}
	})

	it('leaves the order to a sort chosen on the search itself', async () => {
		const { Wrapper, asked } = setup(
			`/books?search=dune&orderBy=${MediaModelOrdering.CreatedAt}&direction=${OrderDirection.Asc}`,
		)
		render(<BookSearchScene />, { wrapper: Wrapper })
		await waitFor(() => expect(asked().length).toBeGreaterThan(0))

		for (const variables of asked()) {
			expect(variables.relevanceTo ?? null).toBeNull()
			expect(variables.orderBy).toEqual([
				{ media: { field: MediaModelOrdering.CreatedAt, direction: OrderDirection.Asc } },
			])
		}
	})

	it('ranks nothing when nothing is searched', async () => {
		const { Wrapper, asked } = setup('/books')
		render(<BookSearchScene />, { wrapper: Wrapper })
		await waitFor(() => expect(asked().length).toBeGreaterThan(0))

		for (const variables of asked()) {
			expect(variables.relevanceTo ?? null).toBeNull()
		}
	})
})
