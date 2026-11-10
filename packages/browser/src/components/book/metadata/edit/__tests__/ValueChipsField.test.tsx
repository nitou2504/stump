import '@/__mocks__/resizeObserver'

import { SDKContext, StumpClientContext } from '@stump/client'
import { MetadataFacetField } from '@stump/graphql'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PropsWithChildren } from 'react'

import ValueChipsField, { vocabularyLibraryId } from '../ValueChipsField'

// cmdk scrolls the highlighted item into view; jsdom has no layout
Element.prototype.scrollIntoView = vi.fn()

const BIBLIOTECA = 'library-biblioteca'
const MANGA = 'library-manga'

/** Each library's own genres: the server answers with one library's, or (without one) all of them */
const GENRES: Record<string, { value: string; count: number }[]> = {
	[BIBLIOTECA]: [{ value: 'Philosophy', count: 12 }],
	[MANGA]: [{ value: 'Shonen', count: 6 }],
	'library-light-novel': [{ value: 'Isekai', count: 4 }],
}
const AUTHORS: Record<string, { value: string; count: number }[]> = {
	[BIBLIOTECA]: [{ value: 'Umberto Eco', count: 3 }],
	[MANGA]: [{ value: 'Kentaro Miura', count: 2 }],
}

function setup() {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (_document, variables) => {
		const { field, libraryId } = variables as { field: string; libraryId?: string }
		const byLibrary = field === MetadataFacetField.Genres ? GENRES : AUTHORS
		return {
			metadataValueCounts: libraryId
				? (byLibrary[libraryId] ?? [])
				: Object.values(byLibrary).flat(),
		}
	})
	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>{children}</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}
	return { execute, Wrapper }
}

describe('vocabularyLibraryId', () => {
	it('keeps genres server-wide and every other field in the book’s library', () => {
		expect(vocabularyLibraryId(MetadataFacetField.Genres, BIBLIOTECA)).toBeUndefined()
		expect(vocabularyLibraryId(MetadataFacetField.Writers, BIBLIOTECA)).toBe(BIBLIOTECA)
		expect(vocabularyLibraryId(MetadataFacetField.Series, BIBLIOTECA)).toBe(BIBLIOTECA)
		expect(vocabularyLibraryId(MetadataFacetField.Publisher, BIBLIOTECA)).toBe(BIBLIOTECA)
		expect(vocabularyLibraryId(MetadataFacetField.Language, BIBLIOTECA)).toBe(BIBLIOTECA)
	})
})

describe('ValueChipsField vocabulary', () => {
	it('offers the genres of every library, asking the server without a library', async () => {
		const user = userEvent.setup()
		const { Wrapper, execute } = setup()
		render(
			<ValueChipsField
				field={MetadataFacetField.Genres}
				libraryId={MANGA}
				values={[]}
				onChange={vi.fn()}
				addLabel="Add a genre"
			/>,
			{ wrapper: Wrapper },
		)
		await user.click(screen.getByRole('button', { name: /Add a genre/ }))

		// editing a manga: its own genres and the other libraries' are all normal picks
		expect(await screen.findByText('Shonen')).toBeInTheDocument()
		expect(screen.getByText('Isekai')).toBeInTheDocument()
		expect(screen.getByText('Philosophy')).toBeInTheDocument()
		expect(execute).toHaveBeenCalledTimes(1)
		expect(execute.mock.calls[0]![1]).toEqual({
			field: MetadataFacetField.Genres,
			libraryId: undefined,
		})
	})

	it('picks a genre that only another library uses, without a new-genre warning', async () => {
		const user = userEvent.setup()
		const { Wrapper } = setup()
		const onChange = vi.fn()
		render(
			<ValueChipsField
				field={MetadataFacetField.Genres}
				libraryId={MANGA}
				values={['Shonen']}
				onChange={onChange}
				addLabel="Add a genre"
			/>,
			{ wrapper: Wrapper },
		)
		await user.click(screen.getByRole('button', { name: /Add a genre/ }))
		// Philosophy is only a Biblioteca genre
		await user.type(await screen.findByPlaceholderText('Search genres…'), 'philos')
		await user.click(await screen.findByText('Philosophy'))
		expect(screen.queryByText('Check the spelling')).not.toBeInTheDocument()
		expect(onChange).toHaveBeenCalledWith(['Shonen', 'Philosophy'])
	})

	it('keeps authors to the book’s own library', async () => {
		const user = userEvent.setup()
		const { Wrapper, execute } = setup()
		render(
			<ValueChipsField
				field={MetadataFacetField.Writers}
				libraryId={BIBLIOTECA}
				values={[]}
				onChange={vi.fn()}
				addLabel="Add an author"
			/>,
			{ wrapper: Wrapper },
		)
		await user.click(screen.getByRole('button', { name: /Add an author/ }))

		expect(await screen.findByText('Umberto Eco')).toBeInTheDocument()
		expect(execute.mock.calls[0]![1]).toEqual({
			field: MetadataFacetField.Writers,
			libraryId: BIBLIOTECA,
		})
	})
})
