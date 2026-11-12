import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PropsWithChildren, Suspense } from 'react'
import { MemoryRouter } from 'react-router'

import SmartListSideBarSection from '../SmartListSideBarSection'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))
vi.mock('@/scenes/smartList/graphql', () => ({
	usePrefetchSmartList: () => ({ prefetch: vi.fn() }),
}))
vi.mock('../../../CollapsedSidebarItems', () => ({
	useIsSidebarIconMode: () => false,
	CollapsedSidebarSection: () => null,
}))

const lists = (count: number) =>
	Array.from({ length: count }, (_, index) => ({ id: `list-${index}`, name: `List ${index}` }))

function setup(count: number, at = '/') {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
	vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		if (document.toString().includes('query SmartListSideBarSection'))
			return { smartLists: lists(count) }
		throw new Error('Unexpected query')
	})

	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>
						<MemoryRouter initialEntries={[at]}>
							<Suspense fallback={null}>{children}</Suspense>
						</MemoryRouter>
					</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}

	return render(<SmartListSideBarSection />, { wrapper: Wrapper })
}

const trigger = () => screen.findByRole('button', { name: /shared\.common\.smartLists/ })

describe('SmartListSideBarSection', () => {
	it('lists a few smart lists, open, as it always did', async () => {
		setup(4)

		expect(await trigger()).toHaveAttribute('aria-expanded', 'true')
		expect(screen.getAllByRole('link', { name: /^List \d$/ })).toHaveLength(4)
		// nothing to count or fold
		expect(await trigger()).toHaveTextContent(/^shared\.common\.smartLists$/)
	})

	it('folds a long list of them under one entry that says how many there are', async () => {
		setup(45)

		const entry = await trigger()
		expect(entry).toHaveAttribute('aria-expanded', 'false')
		expect(entry).toHaveTextContent('shared.common.smartLists')
		expect(entry).toHaveTextContent('45')
		expect(screen.queryByRole('link', { name: /^List \d+$/ })).toBeNull()
	})

	it('opens the folded entry to all of them, with See all first', async () => {
		setup(45)

		await userEvent.click(await trigger())

		expect(screen.getAllByRole('link', { name: /^List \d+$/ })).toHaveLength(45)
		const links = screen.getAllByRole('link')
		expect(links[0]).toHaveTextContent('sidebar.buttons.seeAll')
		expect(links[0]).toHaveAttribute('href', '/smart-lists')
	})

	it('stays open on one of the lists, so the reader sees where they are', async () => {
		setup(45, '/smart-lists/list-7')

		expect(await trigger()).toHaveAttribute('aria-expanded', 'true')
		expect(screen.getByRole('link', { name: 'List 7' })).toBeInTheDocument()
	})
})
