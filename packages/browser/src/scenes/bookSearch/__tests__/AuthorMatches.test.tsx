import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor } from '@testing-library/react'
import { PropsWithChildren } from 'react'
import { MemoryRouter } from 'react-router'

import { localeWrapper } from '@/__mocks__/locale'

import AuthorMatches from '../AuthorMatches'

type Author = { value: string; count: number }

function setup(authors: Author[], locale: 'en-US' | 'es-ES' = 'en-US') {
	const LocaleWrapper = localeWrapper(locale)
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		if (document.toString().includes('query BookSearchAuthors')) return { searchAuthors: authors }
		throw new Error('Unexpected query')
	})
	function Wrapper({ children }: PropsWithChildren) {
		return (
			<LocaleWrapper>
				<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
					<StumpClientContext.Provider value={{}}>
						<QueryClientProvider client={client}>
							<MemoryRouter>{children}</MemoryRouter>
						</QueryClientProvider>
					</StumpClientContext.Provider>
				</SDKContext.Provider>
			</LocaleWrapper>
		)
	}
	return { execute, Wrapper }
}

describe('AuthorMatches', () => {
	it('links each author to their books, with how many the reader can see', async () => {
		const { Wrapper, execute } = setup([
			{ value: 'Isaac Asimov', count: 148 },
			{ value: 'Janet Asimov', count: 1 },
		])
		render(<AuthorMatches search="asimov" />, { wrapper: Wrapper })

		const isaac = await screen.findByRole('link', { name: /Isaac Asimov/ })
		expect(isaac).toHaveTextContent('148 books')
		expect(screen.getByRole('link', { name: /Janet Asimov/ })).toHaveTextContent('1 book')
		expect(screen.getByRole('region', { name: 'Authors' })).toBeInTheDocument()

		const filters = new URL(`http://stump${isaac.getAttribute('href')}`).searchParams.get('filters')
		expect(JSON.parse(filters ?? '')).toEqual({
			metadata: { writers: { hasAnyOf: ['Isaac Asimov'] } },
		})
		expect(execute).toHaveBeenCalledWith(expect.anything(), { search: 'asimov' })
	})

	it('shows nothing when no author answers', async () => {
		const { Wrapper, execute } = setup([])
		const { container } = render(<AuthorMatches search="zzzz" />, { wrapper: Wrapper })
		await waitFor(() => expect(execute).toHaveBeenCalled())
		expect(container).toBeEmptyDOMElement()
	})

	it('asks for nothing before there is something to search', () => {
		const { Wrapper, execute } = setup([{ value: 'Isaac Asimov', count: 3 }])
		for (const search of [undefined, '', ' a ']) {
			const { container, unmount } = render(<AuthorMatches search={search} />, { wrapper: Wrapper })
			expect(container).toBeEmptyDOMElement()
			unmount()
		}
		expect(execute).not.toHaveBeenCalled()
	})

	it('speaks Spanish', async () => {
		const { Wrapper } = setup([{ value: 'Gabriel García Márquez', count: 12 }], 'es-ES')
		render(<AuthorMatches search="garcia" />, { wrapper: Wrapper })
		expect(await screen.findByRole('region', { name: 'Autores' })).toBeInTheDocument()
		expect(screen.getByRole('link')).toHaveTextContent('12 libros')
	})
})
