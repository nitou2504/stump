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

/** `inProgress` is what the server answers to "the books of this series the reader is in the middle of" */
function setup(series: unknown, placement?: 'header' | 'bar', inProgress?: unknown[]) {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		if (document.toString().includes('query SeriesContinue')) {
			return { seriesById: series, ...(inProgress ? { media: { nodes: inProgress } } : {}) }
		}
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

	render(<ContinueSeriesButton seriesId="series" placement={placement} />, { wrapper: Wrapper })
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

	it('continues the book in progress even when an earlier book is not started', async () => {
		// the companion volume (number 0) is the first unread one, but the reader is in book 2
		setup(
			{
				id: 'series',
				percentageCompleted: 0,
				upNext: [next({ id: 'companion', resolvedName: 'El mundo de Hielo y Fuego' })],
			},
			undefined,
			[next({ id: 'book-2', resolvedName: 'Choque de reyes', readProgress: { page: 7 } })],
		)

		const link = await screen.findByRole('link', { name: /shared\.common\.continue/ })
		expect(link).toHaveAttribute('href', '/books/book-2/epub-reader')
		expect(link).toHaveAttribute('title', 'seriesContinue.continueWith Choque de reyes')
		expect(screen.queryByText('seriesContinue.start')).toBeNull()
	})

	it("asks only for the reader's own books in progress in this series, first one first", async () => {
		const { execute } = setup({ id: 'series', percentageCompleted: 0, upNext: [next()] })

		await vi.waitFor(() => expect(execute).toHaveBeenCalled())
		const variables = execute.mock.calls[0]?.[1] as unknown as {
			id: string
			inProgress: { seriesId: { eq: string }; readingStatus: { is: string } }
			orderBy: unknown[]
		}
		expect(variables.id).toBe('series')
		expect(variables.inProgress).toEqual({
			seriesId: { eq: 'series' },
			readingStatus: { is: 'READING' },
		})
		// the series' own order, as Series.upNext lists it: the number, then the name
		expect(variables.orderBy).toEqual([
			{ metadata: { field: 'NUMBER', direction: 'ASC' } },
			{ media: { field: 'NAME', direction: 'ASC' } },
		])
	})

	it('goes to the next unread book when no book is in progress', async () => {
		setup({ id: 'series', percentageCompleted: 34, upNext: [next()] }, undefined, [])

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

	it('sits in the header from a small screen up, and is hidden on a phone', async () => {
		setup({ id: 'series', percentageCompleted: 0, upNext: [next()] }, 'header')

		const link = await screen.findByRole('link', { name: /seriesContinue\.start$/ })
		expect(link.parentElement).toHaveClass('hidden', 'sm:block')
	})

	it('is a full-width bar on a phone, and hidden from a small screen up', async () => {
		setup({ id: 'series', percentageCompleted: 0, upNext: [next()] }, 'bar')

		const link = await screen.findByRole('link', { name: /seriesContinue\.start$/ })
		expect(link).toHaveClass('w-full')
		expect(link.parentElement).toHaveClass('sm:hidden')
	})

	it('leaves no gap behind when there is nothing to continue', async () => {
		const { execute } = setup({ id: 'series', percentageCompleted: 100, upNext: [] }, 'bar')

		await vi.waitFor(() => expect(execute).toHaveBeenCalled())
		await vi.waitFor(() => expect(document.body.querySelector('.sm\\:hidden')).toBeNull())
	})
})
