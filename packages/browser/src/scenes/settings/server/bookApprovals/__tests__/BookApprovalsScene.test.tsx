import { IntakeState } from '@stump/graphql'
import { fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter, Route, Routes, useLocation } from 'react-router'

import { item } from '@/scenes/intake/__tests__/fixtures'
import type { IntakeItemData } from '@/scenes/intake/queries'

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
	approveItem: vi.fn(),
	approveWithGenres: vi.fn(),
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
// the limits of the people who add books are for those who manage users: not what is tested here
vi.mock('@/context', async (importOriginal) => ({
	...(await importOriginal<typeof import('@/context')>()),
	useAppContext: () => ({ checkPermission: () => false }),
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
vi.mock('@/scenes/intake/TagPicker', () => ({
	default: () => <div>Tag picker</div>,
	// the chips of the "Approve all" sheet
	TagChip: ({
		chip,
		isOn,
		onToggle,
	}: {
		chip: { value: string }
		isOn: boolean
		onToggle: () => void
	}) => (
		<button type="button" aria-pressed={isOn} onClick={onToggle}>
			{chip.value}
		</button>
	),
}))

const pending = (id: string, overrides: Partial<IntakeItemData> = {}): IntakeItemData =>
	item({
		id,
		name: `${id}.epub`,
		state: IntakeState.PendingApproval,
		submittedAt: new Date(Date.now() - 2 * 3600 * 1000).toISOString(),
		uploader: { avatarUrl: null, id: 'u1', username: 'ana' },
		...overrides,
	})

const aDuplicate = {
	fileName: 'a.epub',
	mediaId: 'm1',
	path: '',
	reason: 'SAME_TITLE',
	title: 'Dune',
}

const withTitle = (id: string, title: string, overrides: Partial<IntakeItemData> = {}) => {
	const base = item()
	return pending(id, {
		decision: base.decision
			? { ...base.decision, metadata: { ...base.decision.metadata, title } }
			: null,
		...overrides,
	})
}

const pendingWithDuplicate = (id: string, title: string) => {
	const base = withTitle(id, title)
	return {
		...base,
		proposal: base.proposal ? { ...base.proposal, duplicates: [aDuplicate] } : null,
	}
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

	it('names the library each book would join, with several libraries to approve into', () => {
		// the switch for the library is behind “More details”: the card has to say it itself
		queue.items = [withTitle('a', 'Dune'), withTitle('b', 'Emma')]
		queue.totalCount = 2

		renderPage()

		expect(screen.getAllByText(/^Biblioteca · /)).toHaveLength(2)
	})

	it('approves one book from its card with the tags the card shows', () => {
		queue.items = [withTitle('a', 'Dune'), withTitle('b', 'Emma')]
		queue.totalCount = 2
		renderPage()

		const card = within(screen.getByRole('article', { name: 'Emma' }))
		fireEvent.click(card.getByRole('button', { name: 'Approve' }))

		// the one tag of the fixture is a weak guess the card leaves unticked, so none is written
		expect(actions.approveItem).toHaveBeenCalledTimes(1)
		expect(actions.approveItem).toHaveBeenCalledWith(expect.objectContaining({ id: 'b' }), [])
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
		expect(
			dialog.getByText(
				/1 book is left out: it has something to fix, may already be in the library/,
			),
		).toBeInTheDocument()
		// nothing is approved by opening the sheet
		expect(actions.approveWithGenres).not.toHaveBeenCalled()

		fireEvent.click(dialog.getByRole('button', { name: 'Approve 2 books' }))

		expect(actions.approveWithGenres).toHaveBeenCalledTimes(1)
		const entries = actions.approveWithGenres.mock.calls[0]?.[0] as Array<{
			item: { id: string }
			genres: string[]
		}>
		expect(entries.map(({ item }) => item.id)).toEqual(['a', 'c'])
		expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
	})

	it('leaves out of "Approve all" a book that may already be in the library, or that failed before', () => {
		queue.items = [
			withTitle('a', 'Dune'),
			// possibly a copy of a book the person who sent it may not read: it is looked at alone
			pendingWithDuplicate('b', 'Emma'),
			withTitle('c', 'Ulysses', { error: 'Could not be written' }),
			withTitle('d', 'Hamlet'),
		]
		queue.totalCount = 4
		renderPage()

		fireEvent.click(screen.getByRole('button', { name: 'Approve all (2)' }))
		const dialog = within(screen.getByRole('dialog'))

		expect(dialog.getByText('Dune')).toBeInTheDocument()
		expect(dialog.getByText('Hamlet')).toBeInTheDocument()
		expect(dialog.queryByText('Emma')).not.toBeInTheDocument()
		expect(dialog.queryByText('Ulysses')).not.toBeInTheDocument()
		expect(
			dialog.getByText(
				/2 books are left out: they have something to fix, may already be in the library, or failed/,
			),
		).toBeInTheDocument()
		fireEvent.click(dialog.getByRole('button', { name: 'Approve 2 books' }))
		const entries = actions.approveWithGenres.mock.calls[0]?.[0] as Array<{ item: { id: string } }>
		expect(entries.map(({ item }) => item.id)).toEqual(['a', 'd'])
	})

	it('can still approve a possible duplicate from its own card', () => {
		queue.items = [pendingWithDuplicate('b', 'Emma'), withTitle('a', 'Dune')]
		queue.totalCount = 2
		renderPage()

		const card = within(screen.getByRole('article', { name: 'Emma' }))
		fireEvent.click(card.getByRole('button', { name: 'Approve' }))

		expect(actions.approveItem).toHaveBeenCalledWith(expect.objectContaining({ id: 'b' }), [])
	})

	it('approves the tags the sheet shows: one untickable per book', () => {
		queue.items = [withTitle('a', 'Dune'), withTitle('b', 'Emma')]
		queue.totalCount = 2
		renderPage()

		fireEvent.click(screen.getByRole('button', { name: 'Approve all (2)' }))
		const dialog = within(screen.getByRole('dialog'))
		// the fixture's tag is a weak guess, so it is there but not ticked
		const tags = dialog.getAllByRole('button', { name: 'Historical Fiction' })
		expect(tags).toHaveLength(2)
		fireEvent.click(tags[0] as HTMLElement)
		fireEvent.click(dialog.getByRole('button', { name: 'Approve 2 books' }))

		const entries = actions.approveWithGenres.mock.calls[0]?.[0] as Array<{
			item: { id: string }
			genres: string[]
		}>
		expect(entries.find(({ item }) => item.id === 'a')?.genres).toEqual(['Historical Fiction'])
		expect(entries.find(({ item }) => item.id === 'b')?.genres).toEqual([])
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
