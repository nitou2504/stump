import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import { PropsWithChildren } from 'react'
import { MemoryRouter } from 'react-router'

import { AppContext, IAppContext } from '@/context'

import { ContinueSeriesButton } from '../ContinueSeriesButton'

// the words are their keys, with the values asked for after them
vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({
		t: (key: string, options?: Record<string, unknown>) =>
			options ? `${key} ${Object.values(options).join(' ')}` : key,
		translate: (key: string) => key,
	}),
}))

const next = (extra: Record<string, unknown> = {}) => ({
	id: 'next',
	resolvedName: 'Choque de reyes',
	extension: 'epub',
	readProgress: null,
	readHistory: [],
	...extra,
})

function setup(series: unknown) {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		if (document.toString().includes('query SeriesContinue')) return { seriesById: series }
		throw new Error('Unexpected query')
	})
	const app = { user: { id: 'me' }, checkPermission: () => false } as unknown as IAppContext

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

	render(<ContinueSeriesButton seriesId="series" />, { wrapper: Wrapper })
	return { execute }
}

describe('ContinueSeriesButton', () => {
	it('continues the book in progress, from where the reader was', async () => {
		setup({
			id: 'series',
			percentageCompleted: 20,
			upNext: [next({ readProgress: { page: 4, locator: { href: 'ch3.xhtml' } } })],
		})

		const link = await screen.findByRole('link', { name: /shared\.common\.continue/ })
		expect(link).toHaveAttribute('href', '/books/next/epub-reader')
		expect(link).toHaveAttribute('title', 'seriesContinue.continueWith Choque de reyes')
	})

	it('continues with the next book when the one before it is read', async () => {
		setup({ id: 'series', percentageCompleted: 34, upNext: [next()] })

		const link = await screen.findByRole('link', { name: /shared\.common\.continue/ })
		expect(link).toHaveAttribute('href', '/books/next/epub-reader')
	})

	it('starts the series when nothing of it is read', async () => {
		setup({ id: 'series', percentageCompleted: 0, upNext: [next()] })

		const link = await screen.findByRole('link', { name: /seriesContinue\.start$/ })
		expect(link).toHaveAttribute('title', 'seriesContinue.startWith Choque de reyes')
	})

	it('is not there when every book is read (there is no next one)', async () => {
		const { execute } = setup({ id: 'series', percentageCompleted: 100, upNext: [] })

		await vi.waitFor(() => expect(execute).toHaveBeenCalled())
		await vi.waitFor(() => expect(screen.queryByRole('link')).toBeNull())
	})

	it('is not there for a series the reader cannot see', async () => {
		const { execute } = setup(null)

		await vi.waitFor(() => expect(execute).toHaveBeenCalled())
		expect(screen.queryByRole('link')).toBeNull()
	})
})
