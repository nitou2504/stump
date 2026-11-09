import '@/__mocks__/resizeObserver'

import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PropsWithChildren } from 'react'
import { MemoryRouter } from 'react-router'

import { AppContext, IAppContext } from '@/context'

import DiscoverBooks, { isoWeek } from '../DiscoverBooks'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))
vi.mock('@/components/thumbnail/ThumbnailImage', () => ({ ThumbnailImage: () => null }))
// virtualised rows render nothing without layout: show every card
vi.mock('@/components/MultiRowHorizontalCardList', () => ({
	default: <T,>({
		items,
		renderItem,
		keyExtractor,
	}: {
		items: T[]
		renderItem: (item: T) => React.ReactNode
		keyExtractor: (item: T) => string
	}) => (
		<ul>
			{items.map((item) => (
				<li key={keyExtractor(item)}>{renderItem(item)}</li>
			))}
		</ul>
	),
}))

const book = (id: string, name: string, tags: string[] = []) => ({
	id,
	resolvedName: name,
	tags: tags.map((tag) => ({ name: tag })),
	metadata: { writers: ['Someone'], rating: null },
	thumbnail: { url: '', metadata: null },
})

function setup(canEdit = true) {
	localStorage.clear()
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	})
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		const query = document.toString()
		if (query.includes('query DiscoverBooks'))
			return {
				randomMedia: [book('a', 'Estudios Sociales, 9.º EGB', ['Escuela']), book('b', 'Dune')],
			}
		if (query.includes('mutation DiscoverBooksVeto')) return { setMediaTags: { id: 'a' } }
		throw new Error('Unexpected query')
	})
	const app = { checkPermission: () => canEdit } as unknown as IAppContext
	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>
						<AppContext.Provider value={app}>
							<MemoryRouter>{children}</MemoryRouter>
						</AppContext.Provider>
					</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}
	return { execute, Wrapper }
}

describe('DiscoverBooks', () => {
	it('seeds the picks with the week and asks for unread books', async () => {
		const { execute, Wrapper } = setup()
		render(<DiscoverBooks />, { wrapper: Wrapper })
		expect(await screen.findByText('Dune')).toBeInTheDocument()
		expect(execute.mock.calls[0]![1]).toEqual({
			filter: { readingStatus: { is: 'NOT_STARTED' } },
			count: 16,
			seed: isoWeek(),
		})
	})

	it('"Not for picks" tags the book No picks, keeping its tags, and drops it', async () => {
		const user = userEvent.setup()
		const { execute, Wrapper } = setup()
		render(<DiscoverBooks />, { wrapper: Wrapper })
		await user.click(
			await screen.findByRole('button', { name: 'Not for picks: Estudios Sociales, 9.º EGB' }),
		)
		await waitFor(() =>
			expect(screen.queryByText('Estudios Sociales, 9.º EGB')).not.toBeInTheDocument(),
		)
		const veto = execute.mock.calls.find(([document]) =>
			document.toString().includes('mutation DiscoverBooksVeto'),
		)
		expect(veto![1]).toEqual({ id: 'a', tags: ['Escuela', 'No picks'] })
		expect(screen.getByText('Dune')).toBeInTheDocument()
	})

	it('has no veto button for users who cannot edit tags', async () => {
		const { Wrapper } = setup(false)
		render(<DiscoverBooks />, { wrapper: Wrapper })
		await screen.findByText('Dune')
		expect(screen.queryByRole('button', { name: /Not for picks/ })).not.toBeInTheDocument()
	})
})

describe('isoWeek', () => {
	it('names the ISO week, so the picks change on Mondays', () => {
		expect(isoWeek(new Date(2026, 10, 8))).toBe('2026-W45')
		expect(isoWeek(new Date(2026, 10, 9))).toBe('2026-W46')
	})

	it('puts the days around New Year in the week they belong to', () => {
		expect(isoWeek(new Date(2026, 0, 1))).toBe('2026-W01')
		expect(isoWeek(new Date(2027, 0, 1))).toBe('2026-W53')
		expect(isoWeek(new Date(2024, 11, 30))).toBe('2025-W01')
	})
})
