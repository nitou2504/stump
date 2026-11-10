import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router'

import BookOverviewScene from '../BookOverviewScene'

const mocks = vi.hoisted(() => ({ canEditMetadata: true }))

vi.mock('@/components/book', () => ({
	useBookOverview: () => ({
		data: {
			mediaById: {
				id: 'book',
				resolvedName: 'Dune',
				seriesId: null,
				metadata: { writers: [] },
				thumbnail: { url: '/cover.png', metadata: null },
			},
		},
	}),
}))

vi.mock('@/components/book/useBookPermissions', () => ({
	useBookPermissions: () => ({ canEditMetadata: mocks.canEditMetadata }),
}))

// the parts are tested where they are; here is how the page wires them together
vi.mock('@/components/book/BookHeader', () => ({ BookHeader: () => null }))
vi.mock('@/components/book/BookInfoSheet', () => ({ BookInfoSheet: () => null }))
vi.mock('@/components/container', () => ({
	SceneContainer: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}))
vi.mock('@/components/thumbnail', () => ({ ProminentThumbnailImage: () => null }))

vi.mock('@/components/book/BookActions', () => ({
	BookActions: ({ onEditDetails }: { onEditDetails: () => void }) => (
		<button onClick={onEditDetails}>from the menu</button>
	),
}))

vi.mock('@/components/book/BookOverviewDetails', () => ({
	BookOverviewDetails: ({ onEditGenres }: { onEditGenres?: () => void }) =>
		onEditGenres ? <button onClick={onEditGenres}>pencil by the genres</button> : null,
}))

vi.mock('@/components/book/metadata/edit', () => ({
	EditBookDialog: ({
		isOpen,
		onClose,
		focus,
	}: {
		isOpen: boolean
		onClose: () => void
		focus?: string
	}) =>
		isOpen ? (
			<div data-testid="edit-book-dialog" data-focus={focus ?? ''}>
				<button onClick={onClose}>close</button>
			</div>
		) : null,
}))

const renderScene = () =>
	render(
		<MemoryRouter initialEntries={['/books/book']}>
			<Routes>
				<Route path="/books/:id" element={<BookOverviewScene />} />
			</Routes>
		</MemoryRouter>,
	)

describe('BookOverviewScene', () => {
	beforeEach(() => {
		mocks.canEditMetadata = true
	})

	it('has the dialog to edit the details closed until it is asked for', () => {
		renderScene()

		expect(screen.queryByTestId('edit-book-dialog')).not.toBeInTheDocument()
	})

	it('opens the one dialog from the More menu, and closes it', async () => {
		const user = userEvent.setup()
		renderScene()

		await user.click(screen.getByRole('button', { name: 'from the menu' }))
		expect(screen.getAllByTestId('edit-book-dialog')).toHaveLength(1)
		expect(screen.getByTestId('edit-book-dialog')).toHaveAttribute('data-focus', '')

		await user.click(screen.getByRole('button', { name: 'close' }))
		expect(screen.queryByTestId('edit-book-dialog')).not.toBeInTheDocument()
	})

	it('opens the same one dialog from the pencil by the genres, with the genres in view', async () => {
		const user = userEvent.setup()
		renderScene()

		await user.click(screen.getByRole('button', { name: 'pencil by the genres' }))

		expect(screen.getAllByTestId('edit-book-dialog')).toHaveLength(1)
		expect(screen.getByTestId('edit-book-dialog')).toHaveAttribute('data-focus', 'genres')
	})

	it('has no dialog and no pencil for a user who cannot edit the details', () => {
		mocks.canEditMetadata = false
		renderScene()

		expect(screen.queryByRole('button', { name: 'pencil by the genres' })).not.toBeInTheDocument()
		expect(screen.queryByTestId('edit-book-dialog')).not.toBeInTheDocument()
	})
})
