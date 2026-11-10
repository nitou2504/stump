import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Route, Routes } from 'react-router'

import { READING_LIST_PAGE_SIZE } from '../graphql'
import ReadingListScene from '../ReadingListScene'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/components/book/BookCard', () => ({
	default: ({ fragment }: { fragment: { resolvedName: string } }) => (
		<span data-testid="book">{fragment.resolvedName}</span>
	),
}))
vi.mock('@/components/container', () => ({
	SceneContainer: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}))
vi.mock('@/components/container/DynamicCardGrid', () => ({
	default: ({
		count,
		renderItem,
	}: {
		count: number
		renderItem: (index: number) => React.ReactNode
	}) => <div>{Array.from({ length: count }, (_, index) => renderItem(index))}</div>,
}))
vi.mock('@/components/GenericEmptyState', () => ({
	default: ({ title, subtitle }: { title: string; subtitle?: string }) => (
		<div>
			<p>{title}</p>
			<p>{subtitle}</p>
		</div>
	),
}))
// the header is shown as its name and its menu items, which is what the page decides
vi.mock('@/components/sharedLayout', () => ({
	CollectionSceneHeader: ({ title }: { title: string }) => <h1>{title}</h1>,
	EntityHeader: ({
		name,
		actions,
		endActions,
	}: {
		name: string
		actions?: { items: { label: string; onClick: () => void }[] }[]
		endActions?: React.ReactNode
	}) => (
		<header>
			<h1>{name}</h1>
			<nav aria-label="menu">
				{actions
					?.flatMap((group) => group.items)
					.map((item) => (
						<button key={item.label} onClick={item.onClick}>
							{item.label}
						</button>
					))}
			</nav>
			{endActions}
		</header>
	),
}))

type Info = {
	name?: string
	description?: string | null
	visibility?: 'PUBLIC' | 'PRIVATE'
	canEdit?: boolean
	creatorId?: string
	bookCount?: number
}

const book = (number: number) => ({ id: `book-${number}`, resolvedName: `Book ${number}` })

function renderList(
	info: Info = {},
	{
		user = { id: 'me', isServerOwner: false },
		books = [book(1), book(2)],
		onBooks,
	}: {
		user?: { id: string; isServerOwner: boolean }
		books?: ReturnType<typeof book>[]
		onBooks?: (variables: Record<string, unknown>) => ReturnType<typeof book>[]
	} = {},
) {
	const canEdit = info.canEdit ?? true
	const header = {
		id: 'list',
		name: info.name ?? 'Recs',
		description: info.description ?? null,
		visibility: info.visibility ?? 'PRIVATE',
		canEdit,
		creatingUserId: info.creatorId ?? (canEdit ? 'me' : 'ana'),
		bookCount: info.bookCount ?? books.length,
		creator: canEdit
			? { id: 'me', username: 'me', avatarUrl: null }
			: { id: 'ana', username: 'ana', avatarUrl: null },
	}
	const { execute, Wrapper } = setup({
		user,
		path: '/lists/list',
		answers: (query, variables) => {
			if (query.includes('query ReadingListScene')) return { readingListById: header }
			if (query.includes('query ReadingListBooks'))
				return {
					readingListById: { id: 'list', books: onBooks ? onBooks(variables ?? {}) : books },
				}
			if (query.includes('query ReadingListBookIds')) {
				const offset = Number(variables?.offset ?? 0)
				const limit = Number(variables?.limit ?? 200)
				return {
					readingListById: { id: 'list', books: books.slice(offset, offset + limit) },
				}
			}
			if (query.includes('mutation CreateReadingList'))
				return { createReadingList: { id: 'copy', name: 'copy of recs' } }
			if (query.includes('mutation UpdateReadingList')) return { updateReadingList: { id: 'list' } }
			if (query.includes('mutation DeleteReadingList')) return { deleteReadingList: { id: 'list' } }
			if (query.includes('mutation RemoveBooksFromReadingList'))
				return { removeBooksFromReadingList: { id: 'list' } }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(
		<Routes>
			<Route path="/lists/:id" element={<ReadingListScene />} />
			<Route path="*" element={<p>Somewhere else</p>} />
		</Routes>,
		{ wrapper: ({ children }) => <Wrapper>{children}</Wrapper> },
	)
	return { execute }
}

const menu = () => within(screen.getByRole('navigation', { name: 'menu' }))

describe('ReadingListScene', () => {
	describe("the maker's own list", () => {
		it('can be edited, deleted and have its books removed', async () => {
			const user = userEvent.setup()
			const { execute } = renderList({ visibility: 'PRIVATE' })

			expect(await screen.findAllByTestId('book')).toHaveLength(2)
			expect(
				menu()
					.getAllByRole('button')
					.map((item) => item.textContent),
			).toEqual(['readingLists.actions.edit', 'readingLists.actions.delete'])
			expect(screen.queryByRole('button', { name: 'readingLists.actions.saveCopy' })).toBeNull()

			await user.click(
				screen.getByRole('button', { name: /readingLists.list.removeBook name=Book 1/ }),
			)
			await waitFor(() =>
				expect(sent(execute, 'RemoveBooksFromReadingList')).toEqual([
					{ id: 'list', mediaIds: ['book-1'] },
				]),
			)
		})

		it('says everyone on the server can see it when it is public', async () => {
			renderList({ visibility: 'PUBLIC' })

			expect(await screen.findByText('readingLists.list.publicYours')).toBeInTheDocument()
		})
	})

	describe("someone else's public list", () => {
		it('is read-only for a friend, who can save a copy of it', async () => {
			const user = userEvent.setup()
			const { execute } = renderList(
				{ canEdit: false, visibility: 'PUBLIC', name: 'Recs', description: 'From Ana' },
				{ books: [book(1), book(2), book(3)] },
			)

			expect(await screen.findAllByTestId('book')).toHaveLength(3)
			expect(await screen.findByText('readingLists.list.publicBy name=ana')).toBeInTheDocument()
			// nothing to change it with
			expect(screen.queryByRole('button', { name: /readingLists.list.removeBook/ })).toBeNull()
			expect(
				menu()
					.getAllByRole('button')
					.map((item) => item.textContent),
			).toEqual(['readingLists.actions.saveCopy'])

			await user.click(menu().getByRole('button', { name: 'readingLists.actions.saveCopy' }))

			// a private copy with every book the friend can see, named after the list
			await waitFor(() =>
				expect(sent(execute, 'CreateReadingList')).toEqual([
					{
						input: {
							name: 'readingLists.copy.name name=Recs',
							description: 'From Ana',
							mediaIds: ['book-1', 'book-2', 'book-3'],
						},
					},
				]),
			)
			// and opens it
			expect(await screen.findByText('Somewhere else')).toBeInTheDocument()
		})

		it('lets the server owner take it back to private, after asking', async () => {
			const user = userEvent.setup()
			const { execute } = renderList(
				{ canEdit: false, visibility: 'PUBLIC', name: 'Recs' },
				{ user: { id: 'me', isServerOwner: true } },
			)

			await screen.findAllByTestId('book')
			expect(
				menu()
					.getAllByRole('button')
					.map((item) => item.textContent),
			).toEqual([
				'readingLists.actions.saveCopy',
				'readingLists.actions.unpublish',
				'readingLists.actions.delete',
			])

			await user.click(menu().getByRole('button', { name: 'readingLists.actions.unpublish' }))
			expect(sent(execute, 'UpdateReadingList')).toEqual([])
			await user.click(
				await screen.findByRole('button', { name: 'readingLists.unpublish.confirm' }),
			)

			// only the visibility: the name is there because the API asks for one
			await waitFor(() =>
				expect(sent(execute, 'UpdateReadingList')).toEqual([
					{ id: 'list', input: { name: 'Recs', visibility: 'PRIVATE' } },
				]),
			)
		})

		it('lets the server owner delete it, after asking', async () => {
			const user = userEvent.setup()
			const { execute } = renderList(
				{ canEdit: false, visibility: 'PUBLIC' },
				{ user: { id: 'me', isServerOwner: true } },
			)

			await screen.findAllByTestId('book')
			await user.click(menu().getByRole('button', { name: 'readingLists.actions.delete' }))
			expect(sent(execute, 'DeleteReadingList')).toEqual([])
			await user.click(await screen.findByRole('button', { name: 'readingLists.delete.confirm' }))

			await waitFor(() => expect(sent(execute, 'DeleteReadingList')).toEqual([{ id: 'list' }]))
		})

		it('gives a friend no way to unpublish or delete it', async () => {
			renderList({ canEdit: false, visibility: 'PUBLIC' })

			await screen.findAllByTestId('book')
			expect(
				menu()
					.getAllByRole('button')
					.map((item) => item.textContent),
			).toEqual(['readingLists.actions.saveCopy'])
		})
	})

	describe('a long list', () => {
		const books = Array.from({ length: READING_LIST_PAGE_SIZE + 5 }, (_, index) => book(index + 1))
		const page = (variables: Record<string, unknown>) =>
			books.slice(Number(variables.offset), Number(variables.offset) + Number(variables.limit))

		it('is read a page at a time, the next on request', async () => {
			const user = userEvent.setup()
			const { execute } = renderList({ bookCount: books.length }, { books, onBooks: page })

			expect(await screen.findAllByTestId('book')).toHaveLength(READING_LIST_PAGE_SIZE)
			expect(
				screen.getByText(
					`readingLists.list.shown shown=${READING_LIST_PAGE_SIZE} total=${books.length}`,
				),
			).toBeInTheDocument()

			await user.click(screen.getByRole('button', { name: 'readingLists.list.loadMore' }))

			await waitFor(() => expect(screen.getAllByTestId('book')).toHaveLength(books.length))
			expect(sent(execute, 'ReadingListBooks')).toEqual([
				{ id: 'list', limit: READING_LIST_PAGE_SIZE, offset: 0 },
				{ id: 'list', limit: READING_LIST_PAGE_SIZE, offset: READING_LIST_PAGE_SIZE },
			])
			// that was the last page
			expect(screen.queryByRole('button', { name: 'readingLists.list.loadMore' })).toBeNull()
		})

		it('has no "load more" when it fits in a page', async () => {
			renderList({}, { books: [book(1), book(2)] })

			await screen.findAllByTestId('book')
			expect(screen.queryByRole('button', { name: 'readingLists.list.loadMore' })).toBeNull()
		})
	})

	it('says what is wrong when the list can not be opened', async () => {
		const { Wrapper } = setup({
			path: '/lists/list',
			answers: () => {
				throw new Error('Reading list not found')
			},
		})
		render(
			<Routes>
				<Route path="/lists/:id" element={<ReadingListScene />} />
			</Routes>,
			{ wrapper: Wrapper },
		)

		expect(await screen.findByText('readingLists.errors.loadList')).toBeInTheDocument()
		expect(screen.getByText('Reading list not found')).toBeInTheDocument()
	})
})
