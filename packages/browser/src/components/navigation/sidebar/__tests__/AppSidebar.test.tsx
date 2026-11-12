import '@/__mocks__/resizeObserver'

import { SDKContext, StumpClientContext } from '@stump/client'
import { SystemArrangement } from '@stump/graphql'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import { PropsWithChildren, Suspense } from 'react'
import { MemoryRouter } from 'react-router'

import { AppContext, IAppContext } from '@/context'

import AppSidebar from '../AppSidebar'

const flags = vi.hoisted(() => ({ myReading: false }))

// read when the sidebar renders, so a test can turn the page on
vi.mock('@/scenes/readingHistory/availability', () => ({
	get MY_READING_AVAILABLE() {
		return flags.myReading
	},
}))
vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))
vi.mock('@/hooks', () => ({ useTheme: () => ({ shouldUseGradient: false }) }))
vi.mock('@/stores/user', () => ({
	useLocalPreferencesStore: (select: (store: unknown) => unknown) =>
		select({ sidebar: { variant: 'sidebar', collapsible: 'offcanvas' } }),
}))
vi.mock('@/scenes/home/arrangement', () => ({ useHomeArrangementKey: () => ['home'] }))
vi.mock('@/components/UserMenu', () => ({ default: () => null }))
vi.mock('../SideBarFooter', () => ({ default: () => null }))
vi.mock('../CollapsedSidebarItems', () => ({ useIsSidebarIconMode: () => false }))
vi.mock('../base', () => {
	function Sidebar({ children }: PropsWithChildren) {
		return <nav>{children}</nav>
	}
	Sidebar.Content = function SidebarContent({ children }: PropsWithChildren) {
		return <div>{children}</div>
	}
	Sidebar.Rail = function SidebarRail() {
		return null
	}
	return { Sidebar, useSidebar: () => ({ isMobile: false, setOpenMobile: vi.fn() }) }
})
// every section is there for a test to find (the book clubs one included: it must stay out)
vi.mock('../sections', () => ({
	LibrarySideBarSection: () => <p>Libraries section</p>,
	SmartListSideBarSection: () => <p>Smart lists section</p>,
	BookClubSideBarSection: () => <p>Book clubs section</p>,
}))

const variants = [
	SystemArrangement.Home,
	SystemArrangement.Explore,
	SystemArrangement.Libraries,
	SystemArrangement.SmartLists,
	SystemArrangement.BookClubs,
]

function setup(route = '/') {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	})
	vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		if (document.toString().includes('query SideBarQuery')) {
			return {
				me: {
					id: 'me',
					preferences: {
						navigationArrangement: {
							locked: false,
							sections: variants.map((variant) => ({
								visible: true,
								config: { __typename: 'SystemArrangementConfig', variant, links: [] },
							})),
						},
					},
				},
			}
		}
		throw new Error('Unexpected query')
	})
	// someone who may do everything, so that only what the sidebar leaves out is missing
	const app = { checkPermission: () => true } as unknown as IAppContext

	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>
						<AppContext.Provider value={app}>
							<MemoryRouter initialEntries={[route]}>
								<Suspense fallback={<p>Loading</p>}>{children}</Suspense>
							</MemoryRouter>
						</AppContext.Provider>
					</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}
	return { Wrapper }
}

beforeEach(() => {
	vi.clearAllMocks()
	flags.myReading = false
})

describe('AppSidebar', () => {
	it('has no book clubs, even for someone who may use them, and no My reading while it is not built', async () => {
		const { Wrapper } = setup()
		render(<AppSidebar />, { wrapper: Wrapper })

		// the sections that are there
		expect(await screen.findByText('Libraries section')).toBeInTheDocument()
		expect(screen.getByText('Smart lists section')).toBeInTheDocument()
		expect(screen.getByRole('link', { name: 'readingLists.title' })).toBeInTheDocument()

		expect(screen.queryByText('Book clubs section')).not.toBeInTheDocument()
		expect(screen.queryByText('sidebar.buttons.myReading')).not.toBeInTheDocument()
		expect(screen.queryByRole('link', { name: /reading$/ })).not.toBeInTheDocument()
	})

	it('offers My reading, right after the reading lists, once it is built', async () => {
		flags.myReading = true
		const { Wrapper } = setup()
		render(<AppSidebar />, { wrapper: Wrapper })

		const link = await screen.findByRole('link', { name: 'sidebar.buttons.myReading' })
		expect(link).toHaveAttribute('href', '/reading')
		expect(screen.queryByText('Book clubs section')).not.toBeInTheDocument()
	})

	// the entry the page is on has the highlight (the link sits in a box that carries it)
	const isHighlighted = (link: HTMLElement) => link.parentElement?.classList.contains('bg-sidebar-accent')

	it('lights up only the page you are on: Reading lists is not My reading', async () => {
		flags.myReading = true
		const { Wrapper } = setup('/reading-lists')
		render(<AppSidebar />, { wrapper: Wrapper })

		expect(isHighlighted(await screen.findByRole('link', { name: 'readingLists.title' }))).toBe(true)
		expect(isHighlighted(screen.getByRole('link', { name: 'sidebar.buttons.myReading' }))).toBe(false)
	})

	it('lights up My reading on its own pages and on the ones below it', async () => {
		flags.myReading = true
		for (const route of ['/reading', '/reading/someone']) {
			const { Wrapper } = setup(route)
			const { unmount } = render(<AppSidebar />, { wrapper: Wrapper })

			expect(isHighlighted(await screen.findByRole('link', { name: 'sidebar.buttons.myReading' }))).toBe(true)
			expect(isHighlighted(screen.getByRole('link', { name: 'readingLists.title' }))).toBe(false)
			unmount()
		}
	})
})
