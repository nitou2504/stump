import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PropsWithChildren } from 'react'
import { MemoryRouter } from 'react-router'

import { AppContext, IAppContext } from '@/context'

import AddToReadingList from '../AddToReadingList'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))

const lists = [
	{ id: 'recs', name: 'Recs from Ana', creatingUserId: 'me', bookCount: 0 },
	{ id: 'other', name: 'Not mine', creatingUserId: 'someone', bookCount: 5 },
	{ id: 'favs', name: 'Favourites', creatingUserId: 'me', bookCount: 2 },
]

function setup() {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	})
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async (document) => {
		const query = document.toString()
		if (query.includes('query BookReadingListIds')) {
			return { mediaById: { id: 'book', readingListIds: ['favs'] } }
		}
		if (query.includes('query PickerReadingLists')) {
			return { readingLists: { nodes: lists } }
		}
		if (query.includes('mutation AddBooksToReadingList')) {
			return { addBooksToReadingList: { id: 'recs' } }
		}
		if (query.includes('mutation RemoveBooksFromReadingList')) {
			return { removeBooksFromReadingList: { id: 'favs' } }
		}
		if (query.includes('mutation CreateReadingList')) {
			return { createReadingList: { id: 'new', name: 'Sci-fi' } }
		}
		throw new Error('Unexpected query')
	})
	const app = { user: { id: 'me' } } as unknown as IAppContext

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

const calls = (execute: ReturnType<typeof setup>['execute'], name: string) =>
	execute.mock.calls
		.filter(([document]) => document.toString().includes(`mutation ${name}`))
		.map(([, variables]) => variables)

const queried = (execute: ReturnType<typeof setup>['execute'], name: string) =>
	execute.mock.calls.some(([document]) => document.toString().includes(`query ${name}`))

describe('AddToReadingList', () => {
	it("lists the user's own lists by name, checked when they have the book", async () => {
		const user = userEvent.setup()
		const { execute, Wrapper } = setup()
		render(<AddToReadingList bookId="book" bookName="Dune" />, { wrapper: Wrapper })

		// the book page only asks which lists have the book; the lists load when the menu opens
		await waitFor(() => expect(queried(execute, 'BookReadingListIds')).toBe(true))
		expect(queried(execute, 'PickerReadingLists')).toBe(false)

		await user.click(await screen.findByRole('button', { name: 'readingLists.addToList.label' }))

		const rows = await screen.findAllByRole('checkbox')
		expect(rows.map((row) => row.textContent)).toEqual(['Favourites2', 'Recs from Ana0'])
		expect(rows.map((row) => row.getAttribute('aria-checked'))).toEqual(['true', 'false'])
		expect(screen.queryByText('Not mine')).not.toBeInTheDocument()
	})

	it('adds the book to a list or removes it with a tap', async () => {
		const user = userEvent.setup()
		const { execute, Wrapper } = setup()
		render(<AddToReadingList bookId="book" bookName="Dune" />, { wrapper: Wrapper })
		await user.click(await screen.findByRole('button', { name: 'readingLists.addToList.label' }))

		await user.click(await screen.findByRole('checkbox', { name: /Recs from Ana/ }))
		await waitFor(() =>
			expect(calls(execute, 'AddBooksToReadingList')).toEqual([{ id: 'recs', mediaIds: ['book'] }]),
		)

		await user.click(screen.getByRole('checkbox', { name: /Favourites/ }))
		await waitFor(() =>
			expect(calls(execute, 'RemoveBooksFromReadingList')).toEqual([
				{ id: 'favs', mediaIds: ['book'] },
			]),
		)
	})

	it('makes a new list with the book in it', async () => {
		const user = userEvent.setup()
		const { execute, Wrapper } = setup()
		render(<AddToReadingList bookId="book" bookName="Dune" />, { wrapper: Wrapper })
		await user.click(await screen.findByRole('button', { name: 'readingLists.addToList.label' }))

		await user.click(await screen.findByRole('button', { name: 'readingLists.addToList.newList' }))
		await user.type(screen.getByRole('textbox', { name: 'readingLists.form.name' }), '  Sci-fi ')
		await user.click(screen.getByRole('button', { name: 'shared.common.create' }))

		await waitFor(() =>
			expect(calls(execute, 'CreateReadingList')).toEqual([
				{ input: { name: 'Sci-fi', mediaIds: ['book'] } },
			]),
		)
		// back to the "New list" line
		expect(
			await screen.findByRole('button', { name: 'readingLists.addToList.newList' }),
		).toBeInTheDocument()
	})
})
