import { IntakeState } from '@stump/graphql'
import { fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter, Route, Routes, useLocation } from 'react-router'

import type { IntakeItemData } from '@/scenes/intake/queries'
import { item } from '@/scenes/intake/__tests__/fixtures'

import BookApprovalsRoute from '../index'

const flags = vi.hoisted(() => ({ available: true }))
// read when the route renders, so a test can turn the page off
vi.mock('../availability', () => ({
	get BOOK_APPROVALS_AVAILABLE() {
		return flags.available
	},
}))

const queue = vi.hoisted(() => ({
	isLoading: false,
	items: [] as unknown[],
	totalCount: 0,
}))
const actions = vi.hoisted(() => ({
	approve: vi.fn(),
	changeLibrary: vi.fn(),
	isApproving: false,
	isChangingLibrary: false,
	isRejecting: false,
	isReviewing: false,
	reanalyse: vi.fn(),
	reject: vi.fn(),
	reviewItem: vi.fn(),
}))
vi.mock('@/scenes/intake/hooks', () => ({
	useIntakeActions: () => actions,
	usePendingIntake: () => queue,
	useUploaderAgeLimit: () => null,
}))
vi.mock('@stump/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/client')>()),
	useGraphQL: () => ({
		data: {
			intakeLibraries: [
				{ id: 'lib', name: 'Biblioteca' },
				{ id: 'manga', name: 'Manga' },
			],
		},
	}),
}))
// the pickers ask the server for folders and tags: not what is tested here
vi.mock('@/scenes/intake/FolderPicker', () => ({ default: () => <div>Folder picker</div> }))
vi.mock('@/scenes/intake/TagPicker', () => ({ default: () => <div>Tag picker</div> }))

const pending = (id: string, overrides: Partial<IntakeItemData> = {}): IntakeItemData =>
	item({
		id,
		name: `${id}.epub`,
		state: IntakeState.PendingApproval,
		submittedAt: new Date(Date.now() - 2 * 3600 * 1000).toISOString(),
		uploader: { avatarUrl: null, id: 'u1', username: 'ana' },
		...overrides,
	})

const withTitle = (id: string, title: string, overrides: Partial<IntakeItemData> = {}) => {
	const base = item()
	return pending(id, {
		decision: base.decision
			? { ...base.decision, metadata: { ...base.decision.metadata, title } }
			: null,
		...overrides,
	})
}

function Where() {
	const location = useLocation()
	return <p data-testid="where">{location.pathname + location.search}</p>
}

function renderPage(path = '/settings/book-approvals') {
	render(
		<MemoryRouter initialEntries={[path]}>
			<Routes>
				<Route path="/" element={<p>Home page</p>} />
				<Route
					path="/settings/book-approvals"
					element={
						<>
							<BookApprovalsRoute />
							<Where />
						</>
					}
				/>
			</Routes>
		</MemoryRouter>,
	)
}

describe('Book approvals', () => {
	beforeEach(() => {
		vi.clearAllMocks()
		flags.available = true
		queue.items = []
		queue.totalCount = 0
		queue.isLoading = false
		actions.isApproving = false
	})

	it('sends the book approvals home while the page is turned off', async () => {
		flags.available = false

		renderPage()

		expect(await screen.findByText('Home page')).toBeInTheDocument()
	})

	it('says so when nothing waits', () => {
		renderPage()

		expect(screen.getByText('Nothing is waiting for approval.')).toBeInTheDocument()
		expect(screen.queryByRole('button', { name: /Approve all/ })).not.toBeInTheDocument()
	})

	it('shows each book somebody sent, with who sent it', () => {
		queue.items = [
			withTitle('a', 'Dune', { uploader: { avatarUrl: null, id: 'u1', username: 'ana' } }),
			withTitle('b', 'Emma', { uploader: { avatarUrl: null, id: 'u2', username: 'luis' } }),
		]
		queue.totalCount = 2

		renderPage()

		expect(
			screen.getByText('2 books are waiting. They join the library when you approve them.'),
		).toBeInTheDocument()
		expect(screen.getByRole('article', { name: 'Dune' })).toBeInTheDocument()
		expect(screen.getByRole('article', { name: 'Emma' })).toBeInTheDocument()
		expect(screen.getByText('ana')).toBeInTheDocument()
		expect(screen.getByText('luis')).toBeInTheDocument()
	})

	it('approves one book from its card', () => {
		queue.items = [withTitle('a', 'Dune'), withTitle('b', 'Emma')]
		queue.totalCount = 2
		renderPage()

		const card = within(screen.getByRole('article', { name: 'Emma' }))
		fireEvent.click(card.getByRole('button', { name: 'Approve' }))

		expect(actions.approve).toHaveBeenCalledWith(['b'])
	})

	it('sends a book back with the note, from its card', () => {
		queue.items = [withTitle('a', 'Dune')]
		queue.totalCount = 1
		renderPage()

		fireEvent.click(screen.getByRole('button', { name: 'Send back…' }))
		const dialog = within(screen.getByRole('dialog'))
		fireEvent.change(dialog.getByLabelText('What should they change?'), {
			target: { value: 'Wrong author' },
		})
		fireEvent.click(dialog.getByRole('button', { name: 'Send back' }))

		expect(actions.reject).toHaveBeenCalledTimes(1)
		expect(actions.reject.mock.calls[0]?.slice(0, 2)).toEqual(['a', 'Wrong author'])
	})

	it('approves a whole page after asking, leaving out the books with something to fix', () => {
		queue.items = [
			withTitle('a', 'Dune'),
			withTitle('b', 'Emma', { problems: ['Add an author'] }),
			withTitle('c', 'Ulysses'),
		]
		queue.totalCount = 3
		renderPage()

		fireEvent.click(screen.getByRole('button', { name: 'Approve all (2)' }))
		const dialog = within(screen.getByRole('dialog'))
		expect(dialog.getByRole('heading', { name: 'Approve 2 books' })).toBeInTheDocument()
		expect(dialog.getByText('Dune')).toBeInTheDocument()
		expect(dialog.getByText('Ulysses')).toBeInTheDocument()
		expect(dialog.queryByText('Emma')).not.toBeInTheDocument()
		expect(dialog.getByText(/1 book has something to fix and stays here/)).toBeInTheDocument()
		// nothing is approved by opening the dialog
		expect(actions.approve).not.toHaveBeenCalled()

		fireEvent.click(dialog.getByRole('button', { name: 'Approve 2 books' }))

		expect(actions.approve).toHaveBeenCalledWith(['a', 'c'])
		expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
	})

	it('has no "Approve all" for a single book', () => {
		queue.items = [withTitle('a', 'Dune')]
		queue.totalCount = 1
		renderPage()

		expect(screen.queryByRole('button', { name: /Approve all/ })).not.toBeInTheDocument()
	})

	it('filters by library, back from the first page', () => {
		queue.items = [withTitle('a', 'Dune')]
		queue.totalCount = 1
		renderPage('/settings/book-approvals?page=2&library=lib')

		fireEvent.change(screen.getByLabelText('Filter by library'), { target: { value: 'manga' } })

		const where = screen.getByTestId('where').textContent ?? ''
		expect(where).toContain('library=manga')
		expect(where).not.toContain('page=')
	})

	it('goes back to the last page when the one asked for is gone', () => {
		// 25 books are two pages of 20; page 5 is what an approved last page leaves behind
		queue.items = []
		queue.totalCount = 25

		renderPage('/settings/book-approvals?page=5')

		expect(screen.getByTestId('where').textContent).toContain('page=2')
	})

	it('shows a page of the pages, not the books of the others', () => {
		queue.items = [withTitle('a', 'Dune')]
		queue.totalCount = 45
		renderPage('/settings/book-approvals?page=2')

		expect(screen.getByRole('navigation')).toBeInTheDocument()
	})
})
