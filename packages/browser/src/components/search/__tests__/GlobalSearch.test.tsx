import '@/__mocks__/resizeObserver'

import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PropsWithChildren } from 'react'
import { MemoryRouter, Route, Routes, useLocation } from 'react-router'

import GlobalSearch from '../GlobalSearch'

vi.mock('@/components/thumbnail/ThumbnailImage', () => ({ ThumbnailImage: () => null }))

// cmdk scrolls the highlighted item into view; jsdom has no layout
Element.prototype.scrollIntoView = vi.fn()

function Location() {
	const location = useLocation()
	return <p data-testid="location">{location.pathname + location.search}</p>
}

function setup() {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		const query = document.toString()
		if (query.includes('query GlobalSearch'))
			return {
				media: {
					nodes: [
						{
							id: 'b1',
							resolvedName: 'The Name of the Rose',
							thumbnail: { url: '' },
							metadata: { writers: ['Umberto Eco'], rating: 4.2 },
						},
					],
					pageInfo: { __typename: 'OffsetPaginationInfo', totalItems: 9 },
				},
				series: { nodes: [] },
			}
		if (query.includes('query SurpriseMe')) return { randomMedia: [{ id: 'r1' }] }
		throw new Error('Unexpected query')
	})
	function Wrapper({ children }: PropsWithChildren) {
		return (
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
		)
	}
	return { execute, Wrapper }
}

describe('GlobalSearch', () => {
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
		await user.click(screen.getAllByRole('button', { name: 'Search the library' })[0]!)
		await user.type(await screen.findByPlaceholderText('Search books, authors, series…'), 'rose')
		expect(await screen.findByText('The Name of the Rose')).toBeInTheDocument()
		expect(screen.getByText('Umberto Eco · ★ 4.2')).toBeInTheDocument()
		await user.click(screen.getByText('See all 9 books'))
		expect(screen.getByTestId('location')).toHaveTextContent('/books?search=rose')
		expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
	})
})
