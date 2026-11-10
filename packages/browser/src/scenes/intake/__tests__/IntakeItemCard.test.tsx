import { IntakeKind, IntakeSeriesKind, IntakeState, LibraryType } from '@stump/graphql'
import { fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import IntakeItemCard from '../IntakeItemCard'
import type { IntakeItemData } from '../queries'
import { item } from './fixtures'

// the pickers ask the server for folders and tags: not what is tested here
vi.mock('../FolderPicker', () => ({ default: () => <div>Folder picker</div> }))
vi.mock('../TagPicker', () => ({ default: () => <div>Tag picker</div> }))

const biblioteca = { id: 'lib', name: 'Biblioteca' }
const libraries = [biblioteca, { id: 'manga', name: 'Manga' }]

function renderCard(
	overrides: Partial<IntakeItemData> = {},
	props: Partial<Parameters<typeof IntakeItemCard>[0]> = {},
) {
	const onChangeLibrary = vi.fn()
	render(
		<MemoryRouter>
			<IntakeItemCard
				item={item(overrides)}
				libraries={libraries}
				onChangeLibrary={onChangeLibrary}
				onReview={vi.fn()}
				onAccept={vi.fn()}
				onDiscard={vi.fn()}
				onReanalyse={vi.fn()}
				{...props}
			/>
		</MemoryRouter>,
	)
	return { onChangeLibrary }
}

describe('IntakeItemCard library', () => {
	it('moves the item to the library picked in the switch', () => {
		const { onChangeLibrary } = renderCard()

		const select = screen.getByLabelText('Library')
		expect(select).toHaveValue('lib')
		fireEvent.change(select, { target: { value: 'manga' } })

		expect(onChangeLibrary).toHaveBeenCalledWith('a', 'manga')
	})

	it('does not move the item when the library it is in is picked again', () => {
		const { onChangeLibrary } = renderCard()

		fireEvent.change(screen.getByLabelText('Library'), { target: { value: 'lib' } })

		expect(onChangeLibrary).not.toHaveBeenCalled()
	})

	it('has no switch, and names the library, when there is nowhere else to go', () => {
		renderCard({}, { libraries: [biblioteca] })

		expect(screen.queryByLabelText('Library')).not.toBeInTheDocument()
		expect(screen.getByText(/rosa\.epub · Biblioteca/)).toBeInTheDocument()
	})

	it('offers the library of its own the server suggests, and moves the item to it', () => {
		const { onChangeLibrary } = renderCard({ suggestedLibrary: { id: 'manga', name: 'Manga' } })

		fireEvent.click(screen.getByRole('button', { name: 'Move to Manga' }))

		expect(onChangeLibrary).toHaveBeenCalledWith('a', 'manga')
	})

	it('offers nothing when the server suggests no library', () => {
		renderCard({ suggestedLibrary: null })

		expect(screen.queryByRole('button', { name: /^Move to/ })).not.toBeInTheDocument()
	})

	it('lets the uploader choose what a book is filed under in a book library', () => {
		renderCard({ libraryType: LibraryType.Book })

		expect(screen.getByRole('radiogroup', { name: 'File it under' })).toBeInTheDocument()
	})

	it('has nothing to choose in a manga or light novel library, which holds one kind of series', () => {
		const { decision } = item()
		renderCard({
			decision: decision ? { ...decision, seriesKind: IntakeSeriesKind.Manga } : null,
			libraryType: LibraryType.Manga,
		})

		expect(screen.queryByRole('radiogroup', { name: 'File it under' })).not.toBeInTheDocument()
		expect(screen.getByText('Manga series')).toBeInTheDocument()
	})
})

/** What an item looks like to somebody who needs approval: no folder, destination or file names */
const forFriend = (overrides: Partial<IntakeItemData> = {}): Partial<IntakeItemData> => {
	const base = item()
	return {
		decision: base.decision ? { ...base.decision, fileNames: [], folder: '' } : null,
		destination: null,
		...overrides,
	}
}

describe('IntakeItemCard for somebody who needs approval', () => {
	const onSubmit = vi.fn()
	const onAccept = vi.fn()
	const onDiscard = vi.fn()
	const friendCard = (
		overrides: Partial<IntakeItemData> = {},
		props: Partial<Parameters<typeof IntakeItemCard>[0]> = {},
	) => renderCard(forFriend(overrides), { mode: 'submit', onAccept, onDiscard, onSubmit, ...props })

	beforeEach(() => vi.clearAllMocks())

	it('has no folder, no kind to file it under and no destination: they are the approver’s', () => {
		friendCard()

		expect(screen.queryByText('Folder picker')).not.toBeInTheDocument()
		expect(screen.queryByRole('radiogroup', { name: 'File it under' })).not.toBeInTheDocument()
		expect(screen.queryByText(/existing folder|new folder/)).not.toBeInTheDocument()
		expect(screen.queryByText('Umberto_Eco/')).not.toBeInTheDocument()
	})

	it('asks for what they can fix: the title, the authors, the description and the publisher', () => {
		friendCard()

		// the label carries a badge saying where the value came from
		expect(screen.getByLabelText(/^Title/)).toBeInTheDocument()
		expect(screen.getByLabelText(/Authors/)).toBeInTheDocument()
		expect(screen.getByLabelText('Publisher')).toBeInTheDocument()
		expect(screen.getByLabelText(/Description/)).toBeInTheDocument()
		// the age rating is the approver's
		expect(screen.queryByLabelText('Age rating')).not.toBeInTheDocument()
	})

	it('saves a description they wrote when they leave the field', () => {
		const onReview = vi.fn()
		friendCard({}, { onReview })

		const field = screen.getByLabelText(/Description/)
		fireEvent.change(field, { target: { value: 'A medieval murder mystery' } })
		fireEvent.blur(field)

		expect(onReview).toHaveBeenCalledWith('a', { description: 'A medieval murder mystery' })
	})

	it('sends the book for approval with the tags chosen, and never adds it', () => {
		friendCard()

		fireEvent.click(screen.getByRole('button', { name: 'Send for approval' }))

		expect(onSubmit).toHaveBeenCalledTimes(1)
		expect(onSubmit.mock.calls[0]?.[0]).toMatchObject({ id: 'a' })
		// the one tag is a guess from other books by the author: it is not ticked until they tick it
		expect(onSubmit.mock.calls[0]?.[1]).toEqual([])
		expect(onAccept).not.toHaveBeenCalled()
		expect(
			screen.queryByRole('button', { name: /Add to library|Add anyway/ }),
		).not.toBeInTheDocument()
	})

	it('says "Looks right" for a book the server is not sure about, as when adding one', () => {
		friendCard({ state: IntakeState.NeedsReview })

		expect(screen.getByRole('button', { name: 'Looks right, send it' })).toBeInTheDocument()
	})

	it('cannot send a book with something left to fix, and says what', () => {
		friendCard({ problems: ['Add an author'] })

		expect(screen.getByRole('button', { name: /Send/ })).toBeDisabled()
		expect(screen.getByText('Before it can be sent')).toBeInTheDocument()
		expect(screen.getByText('Add an author')).toBeInTheDocument()
	})

	it('names a possible duplicate without a link to it', () => {
		const base = item()
		const proposal = base.proposal
			? {
					...base.proposal,
					duplicates: [
						{
							fileName: 'rosa.epub',
							mediaId: 'm1',
							path: '',
							reason: 'SAME_TITLE',
							title: 'El nombre de la rosa',
						},
					],
				}
			: null
		friendCard({ proposal })

		const alert = within(screen.getByRole('alert'))
		expect(alert.getByText('Already in the library?')).toBeInTheDocument()
		expect(alert.getByText('El nombre de la rosa')).toBeInTheDocument()
		expect(alert.queryByRole('link')).not.toBeInTheDocument()
	})

	it('shows the note of an approver who sent the book back, and sends it again', () => {
		friendCard({
			reviewNote: 'The author is spelled wrong',
			reviewedAt: new Date().toISOString(),
			state: IntakeState.Rejected,
		})

		expect(screen.getAllByText('Sent back').length).toBeGreaterThan(0)
		expect(screen.getByText('The author is spelled wrong')).toBeInTheDocument()
		fireEvent.click(screen.getByRole('button', { name: 'Send again' }))
		expect(onSubmit).toHaveBeenCalledTimes(1)
	})

	it('keeps the note on a sent back book they have changed since', () => {
		friendCard({ reviewNote: 'Add the series', state: IntakeState.Ready })

		expect(screen.getByText('Add the series')).toBeInTheDocument()
		expect(screen.getByRole('button', { name: 'Send again' })).toBeInTheDocument()
	})

	it('can still discard its own book and move it to a library of its own', () => {
		const { onChangeLibrary } = friendCard({ suggestedLibrary: { id: 'manga', name: 'Manga' } })

		fireEvent.click(screen.getByRole('button', { name: 'Discard' }))
		fireEvent.click(screen.getByRole('button', { name: 'Move to Manga' }))

		expect(onDiscard).toHaveBeenCalledWith(['a'])
		expect(onChangeLibrary).toHaveBeenCalledWith('a', 'manga')
	})

	it('lists the files of a series by their own names, with a volume number each', () => {
		friendCard({
			files: [
				{ format: 'cbz', originalName: 'v1.cbz', size: 1 },
				{ format: 'cbz', originalName: 'v2.cbz', size: 1 },
			],
			kind: IntakeKind.Series,
		})

		const volumes = screen.getByRole('region', { name: 'Volumes' })
		expect(within(volumes).getAllByLabelText('Volume number')).toHaveLength(2)
		expect(within(volumes).getAllByText('v1.cbz').length).toBeGreaterThan(0)
	})
})

describe('IntakeItemCard for an approver with a book somebody sent', () => {
	const onApprove = vi.fn()
	const onSendBack = vi.fn()
	const onDiscard = vi.fn()
	const reviewCard = (
		overrides: Partial<IntakeItemData> = {},
		props: Partial<Parameters<typeof IntakeItemCard>[0]> = {},
	) =>
		renderCard(
			{
				state: IntakeState.PendingApproval,
				submittedAt: new Date(Date.now() - 3 * 3600 * 1000).toISOString(),
				uploader: { avatarUrl: null, id: 'u1', username: 'ana' },
				...overrides,
			},
			{ mode: 'review', onApprove, onDiscard, onSendBack, ...props },
		)

	beforeEach(() => vi.clearAllMocks())

	it('says who sent it and how long ago', () => {
		reviewCard()

		expect(screen.getByText('ana')).toBeInTheDocument()
		expect(screen.getByText(/sent about 3 hours ago/)).toBeInTheDocument()
	})

	it('keeps the whole card: where it goes, and the age rating to set', () => {
		reviewCard()

		expect(screen.getByText('Folder picker')).toBeInTheDocument()
		expect(screen.getByLabelText('Age rating')).toBeInTheDocument()
		expect(screen.getByText('Umberto_Eco/')).toBeInTheDocument()
	})

	it('approves the book', () => {
		reviewCard()

		fireEvent.click(screen.getByRole('button', { name: 'Approve' }))

		expect(onApprove).toHaveBeenCalledWith(['a'])
	})

	it('cannot approve a book with something left to fix', () => {
		reviewCard({ problems: ['Choose a folder'] })

		expect(screen.getByRole('button', { name: 'Approve' })).toBeDisabled()
		expect(screen.getByText('Before it can be approved')).toBeInTheDocument()
	})

	it('has no way to discard what somebody else sent', () => {
		reviewCard()

		expect(screen.queryByRole('button', { name: 'Discard' })).not.toBeInTheDocument()
	})

	it('sends the book back with a note, and only with one', () => {
		reviewCard()

		fireEvent.click(screen.getByRole('button', { name: 'Send back…' }))
		const dialog = screen.getByRole('dialog')
		const confirm = within(dialog).getByRole('button', { name: 'Send back' })
		expect(confirm).toBeDisabled()

		fireEvent.change(within(dialog).getByLabelText('What should they change?'), {
			target: { value: '  The series number is missing  ' },
		})
		expect(confirm).toBeEnabled()
		fireEvent.click(confirm)

		expect(onSendBack).toHaveBeenCalledTimes(1)
		expect(onSendBack.mock.calls[0]?.slice(0, 2)).toEqual(['a', 'The series number is missing'])
	})

	it('closes the note’s dialog when the book is sent back', () => {
		onSendBack.mockImplementation((_id: string, _note: string, done: () => void) => done())
		reviewCard()

		fireEvent.click(screen.getByRole('button', { name: 'Send back…' }))
		const dialog = screen.getByRole('dialog')
		fireEvent.change(within(dialog).getByLabelText('What should they change?'), {
			target: { value: 'Fix the title' },
		})
		fireEvent.click(within(dialog).getByRole('button', { name: 'Send back' }))

		expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
	})

	it('warns that a rating would hide the book from the person who sent it', () => {
		const base = item()
		const decision = base.decision
			? { ...base.decision, metadata: { ...base.decision.metadata, ageRating: 18 } }
			: null
		reviewCard({ decision }, { uploaderAge: { age: 13, restrictOnUnset: false } })

		expect(screen.getByText('ana couldn’t read this book')).toBeInTheDocument()
		expect(screen.getByText(/age limit is 13, and the book is rated 18\+/)).toBeInTheDocument()
	})

	it('says nothing about ratings when the uploader’s limit is not known', () => {
		const base = item()
		const decision = base.decision
			? { ...base.decision, metadata: { ...base.decision.metadata, ageRating: 18 } }
			: null
		reviewCard({ decision }, { uploaderAge: null })

		expect(screen.queryByText(/couldn’t read this book|may not be able/)).not.toBeInTheDocument()
	})

	it('does not warn when the rating is within the limit', () => {
		const base = item()
		const decision = base.decision
			? { ...base.decision, metadata: { ...base.decision.metadata, ageRating: 13 } }
			: null
		reviewCard({ decision }, { uploaderAge: { age: 13, restrictOnUnset: false } })

		expect(screen.queryByText(/couldn’t read this book/)).not.toBeInTheDocument()
	})

	it('sets the age rating through a review', () => {
		const onReview = vi.fn()
		reviewCard({}, { onReview })

		fireEvent.change(screen.getByLabelText('Age rating'), { target: { value: '18' } })

		expect(onReview).toHaveBeenCalledWith('a', { ageRating: 18 })
	})
})

describe('IntakeItemCard for somebody who files books themselves', () => {
	it('keeps the buttons of adding a book, and offers the age rating', () => {
		renderCard()

		expect(screen.getByRole('button', { name: 'Add to library' })).toBeInTheDocument()
		expect(screen.getByRole('button', { name: 'Discard' })).toBeInTheDocument()
		expect(screen.getByLabelText('Age rating')).toBeInTheDocument()
		expect(screen.queryByText(/^from /)).not.toBeInTheDocument()
	})
})
