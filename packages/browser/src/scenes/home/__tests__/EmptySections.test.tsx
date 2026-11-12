import '@/__mocks__/resizeObserver'

import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor } from '@testing-library/react'
import { PropsWithChildren } from 'react'
import { MemoryRouter } from 'react-router'

import ContinueReading from '../ContinueReading'
import { HomeEmptySectionsProvider } from '../emptySections'
import OnDeck from '../OnDeck'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))
// the list is shown as its title: what the section decides is whether it is there
vi.mock('@/components/HorizontalCardList', () => ({
	default: ({ title }: { title: string }) => <h2>{title}</h2>,
}))

const pageInfo = {
	__typename: 'OffsetPaginationInfo',
	currentPage: 1,
	totalPages: 1,
	pageSize: 20,
	pageOffset: 0,
	zeroBased: false,
}
const book = { id: 'book' }

function setup({ reading, deck }: { reading: unknown[]; deck: unknown[] }) {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
	vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		const query = document.toString()
		if (query.includes('query ContinueReadingMedia'))
			return { keepReading: { nodes: reading, pageInfo } }
		if (query.includes('query OnDeckBooksWeb')) return { onDeck: { nodes: deck, pageInfo } }
		throw new Error('Unexpected query')
	})
	const report = vi.fn()

	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>
						<MemoryRouter>
							<HomeEmptySectionsProvider value={report}>{children}</HomeEmptySectionsProvider>
						</MemoryRouter>
					</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}

	render(
		<>
			<ContinueReading />
			<OnDeck />
		</>,
		{ wrapper: Wrapper },
	)
	return { report }
}

describe('Continue reading and Your next read', () => {
	it('draw nothing when there is nothing to show, and say so to the page', async () => {
		const { report } = setup({ reading: [], deck: [] })

		await waitFor(() => {
			expect(report).toHaveBeenCalledWith('continueReading', true)
			expect(report).toHaveBeenCalledWith('onDeck', true)
		})
		expect(screen.queryByRole('heading')).toBeNull()
		// not the empty boxes they used to draw
		expect(screen.queryByText(/shared\.homeSections/)).toBeNull()
	})

	it('are drawn when there is something, and say they are not empty', async () => {
		const { report } = setup({ reading: [book], deck: [book] })

		expect(
			await screen.findByRole('heading', { name: 'shared.homeSections.continueReading.label' }),
		).toBeInTheDocument()
		expect(
			screen.getByRole('heading', { name: 'shared.homeSections.onDeck.label' }),
		).toBeInTheDocument()
		expect(report).toHaveBeenCalledWith('continueReading', false)
		expect(report).toHaveBeenCalledWith('onDeck', false)
		expect(report).not.toHaveBeenCalledWith('continueReading', true)
	})

	it('hide one at a time: what is in progress stays when nothing is on deck', async () => {
		setup({ reading: [book], deck: [] })

		expect(
			await screen.findByRole('heading', { name: 'shared.homeSections.continueReading.label' }),
		).toBeInTheDocument()
		await waitFor(() =>
			expect(
				screen.queryByRole('heading', { name: 'shared.homeSections.onDeck.label' }),
			).toBeNull(),
		)
	})
})
