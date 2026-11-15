import { IntakeKind, IntakeSeriesKind, IntakeState, LibraryType } from '@stump/graphql'
import { fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import IntakeItemCard, { primaryLabel } from '../IntakeItemCard'
import type { IntakeItemData } from '../queries'
import { item } from './fixtures'

vi.mock('@/hooks/useTranslate', () => import('./english'))

// the pickers ask the server for folders and tags: not what is tested here
vi.mock('../FolderPicker', () => ({ default: () => <div>Folder picker</div> }))
vi.mock('../TagPicker', () => ({ default: () => <div>Tag picker</div> }))

/** The card keeps what few people need behind *More details*: open it to reach the rest */
function openDetails(root: HTMLElement = document.body) {
	const details = within(root).getByText('More details').closest('details')
	if (!details) throw new Error('the card has no “More details”')
	details.open = true
}

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
		expect(screen.getByText(/Biblioteca · rosa\.epub/)).toBeInTheDocument()
	})

	it('names the library on every card, also when there are several to choose from', () => {
		// the switch is behind “More details”: the card itself has to say where the book goes
		renderCard()

		expect(screen.getByLabelText('Library')).toBeInTheDocument()
		expect(screen.getByText(/Biblioteca · rosa\.epub/)).toBeInTheDocument()
	})

	it('names the library the book is in, whichever it is', () => {
		renderCard({ libraryId: 'manga', libraryName: 'Manga' })

		expect(screen.getByText(/Manga · rosa\.epub/)).toBeInTheDocument()
	})

	it('puts the library before the file name, so a long name is what gets cut short', () => {
		const name =
			'Does God exist_ _ an answer for today -- Kung, Hans -- 2013;2012 -- Knopf Doubleday Publishing Group -- isbn13 9780307826527 -- Anna’s Archive.epub'
		renderCard({ name })

		const line = screen.getByText(/^Biblioteca · Does God exist/)
		expect(line.textContent?.startsWith('Biblioteca · ')).toBe(true)
		// everything is on hover, in the same order
		expect(line).toHaveAttribute('title', `Biblioteca · ${name}`)
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
		openDetails()

		expect(screen.getByRole('radiogroup', { name: 'File it under' })).toBeInTheDocument()
	})

	it('has nothing to choose in a manga or light novel library, which holds one kind of series', () => {
		const { decision } = item()
		renderCard({
			decision: decision ? { ...decision, seriesKind: IntakeSeriesKind.Manga } : null,
			libraryType: LibraryType.Manga,
		})
		openDetails()

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
		openDetails()

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

	const withProposal = (change: Record<string, unknown>) => {
		const base = item()
		return base.proposal ? { ...base.proposal, ...change } : null
	}
	const duplicate = (reason: string) => ({
		fileName: 'rosa.epub',
		mediaId: 'm1',
		path: '',
		reason,
		title: 'El nombre de la rosa',
	})

	it('has nothing to send for a book that is the same file as one in the library, and says so', () => {
		const onDiscard = vi.fn()
		friendCard({ proposal: withProposal({ duplicates: [duplicate('SAME_FILE')] }) }, { onDiscard })

		expect(screen.getByTestId('intake-same-file')).toHaveTextContent('Already in the library.')
		// the server refuses it: there is no button that sends it, anyway or otherwise
		expect(screen.queryByRole('button', { name: /Send/ })).not.toBeInTheDocument()
		fireEvent.click(screen.getByRole('button', { name: 'Remove from my list' }))
		expect(onDiscard).toHaveBeenCalledWith([expect.any(String)])
	})

	it('leaves the owner’s own filing alone: the same file can still be added, as the server allows it', () => {
		renderCard({ proposal: withProposal({ duplicates: [duplicate('SAME_FILE')] }) })

		expect(screen.queryByTestId('intake-same-file')).not.toBeInTheDocument()
		expect(screen.getByRole('button', { name: /^Add/ })).toBeEnabled()
	})

	it('still lets a book with a similar title be sent anyway', () => {
		friendCard({ proposal: withProposal({ duplicates: [duplicate('SAME_TITLE')] }) })

		expect(screen.queryByTestId('intake-same-file')).not.toBeInTheDocument()
		expect(screen.getByRole('button', { name: /Send/ })).toBeEnabled()
	})

	it('never shows an approver what the online lookup says about itself (limits, keys, unreachable sources)', () => {
		renderCard(
			{
				proposal: withProposal({
					warnings: [
						'Open Library: unreachable',
						'Google Books: no answer in time',
						'Google Books: daily limit reached: add a Google Books API key in Settings → Metadata',
						'Hardcover: daily limit reached: try again in about 2 h',
						'Add a cover',
					],
				}),
			},
			{ mode: 'file' },
		)

		// what the online lookup says about itself is never shown, not even as a muted line
		expect(screen.queryByTestId('intake-source-failures')).not.toBeInTheDocument()
		expect(screen.queryByText(/Open Library|Google Books/)).not.toBeInTheDocument()
		// what is not about the lookup stays as it was
		expect(screen.getByText('Add a cover')).toBeInTheDocument()
	})

	it('says nothing of the places that could not be asked to the person who sent the book', () => {
		friendCard({ proposal: withProposal({ warnings: ['Open Library: unreachable'] }) })

		expect(screen.queryByTestId('intake-source-failures')).not.toBeInTheDocument()
		expect(screen.queryByText(/Open Library/)).not.toBeInTheDocument()
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
		openDetails()

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

	it('approves the book with the tags its card shows, not the weak guesses it leaves unticked', () => {
		// Fantasy is in the book; Horror is only an author vote at 0.6, which the card leaves
		// unticked: filing must not write what the approver was not shown as chosen
		const base = item()
		const metadata = base.decision?.metadata
		if (!base.decision || !metadata || !base.proposal) throw new Error('the fixture is complete')
		reviewCard({
			decision: { ...base.decision, metadata: { ...metadata, genres: ['Fantasy', 'Horror'] } },
			proposal: {
				...base.proposal,
				genreSuggestions: [
					{
						confidence: 0.9,
						kind: 'GENRE',
						libraryCount: 30,
						reasons: ["subject 'Fantasy fiction'"],
						selected: true,
						value: 'Fantasy',
					},
					{
						confidence: 0.6,
						kind: 'GENRE',
						libraryCount: 20,
						reasons: ['3 of 5 books by this author have it'],
						selected: true,
						value: 'Horror',
					},
				],
			},
		})

		fireEvent.click(screen.getByRole('button', { name: 'Approve' }))

		expect(onApprove).toHaveBeenCalledTimes(1)
		expect(onApprove).toHaveBeenCalledWith(expect.objectContaining({ id: 'a' }), ['Fantasy'])
	})

	it('approves a book whose only tag is a weak guess with no tags at all', () => {
		reviewCard()

		fireEvent.click(screen.getByRole('button', { name: 'Approve' }))

		expect(onApprove).toHaveBeenCalledWith(expect.objectContaining({ id: 'a' }), [])
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

describe('IntakeItemCard layout', () => {
	it('asks for the title, authors, series, tags and description up front', () => {
		renderCard()

		for (const field of [
			screen.getByLabelText(/^Title/),
			screen.getByLabelText(/Authors/),
			screen.getByLabelText(/^Series/),
			screen.getByLabelText(/Description/),
			screen.getByText('Tag picker'),
		]) {
			expect(field.closest('details')).toBeNull()
		}
	})

	it('keeps the folder, library, language, year, publisher and age rating behind “More details”', () => {
		renderCard()

		const details = screen.getByText('More details').closest('details')
		expect(details).not.toBeNull()
		// closed until somebody opens it
		expect(details).not.toHaveAttribute('open')
		for (const advanced of [
			screen.getByText('Folder picker'),
			screen.getByLabelText('Library'),
			screen.getByLabelText('Language'),
			screen.getByLabelText('Year'),
			screen.getByLabelText('Publisher'),
			screen.getByLabelText('Age rating'),
		]) {
			expect(advanced.closest('details')).toBe(details)
		}
	})

	it('stays closed for a book that nothing blocks', () => {
		renderCard({ problems: [] })

		const details = screen.getByText('More details').closest('details') as HTMLDetailsElement
		expect(details.open).toBe(false)
	})

	it('opens by itself when something that blocks the book is fixed in there', () => {
		// the folder is chosen behind “More details”: a closed fold would hide the way out of the alert
		renderCard({ problems: ['Choose a folder'] })

		const details = screen.getByText('More details').closest('details') as HTMLDetailsElement
		expect(details.open).toBe(true)
		expect(within(details).getByText('Folder picker')).toBeInTheDocument()
		expect(screen.getByText('Choose a folder', { selector: 'li' })).toBeInTheDocument()
	})

	it('says what is inside “More details” in a few words, so nobody opens it to find out', () => {
		renderCard()
		expect(screen.getByText(/folder, language, year, age rating/)).toBeInTheDocument()
	})

	it('keeps the destination in sight for whoever files the book, with the folder it goes in', () => {
		renderCard()

		const destination = screen.getByText('Umberto_Eco/')
		expect(destination.closest('details')).toBeNull()
		expect(screen.getByText('existing folder')).toBeInTheDocument()
	})

	it('puts the book’s cover, title and author first', () => {
		renderCard()

		expect(screen.getByRole('heading', { name: 'El nombre de la rosa' })).toBeInTheDocument()
		expect(screen.getByText('Umberto Eco', { selector: 'p' })).toBeInTheDocument()
		// a book with no cover of its own gets a tile with its monogram
		expect(screen.getByText('EN')).toBeInTheDocument()
	})

	it('says it has no author yet rather than leaving the line empty', () => {
		const base = item()
		renderCard({
			decision: base.decision
				? { ...base.decision, metadata: { ...base.decision.metadata, authors: [] } }
				: null,
		})

		expect(screen.getByText('No author yet')).toBeInTheDocument()
	})

	it('does not ask for an author of a volume filed under a manga series', () => {
		const base = item()
		renderCard({
			decision: base.decision
				? {
						...base.decision,
						metadata: { ...base.decision.metadata, authors: [], series: 'Berserk' },
						seriesKind: IntakeSeriesKind.Manga,
					}
				: null,
		})

		expect(screen.queryByText('No author yet')).not.toBeInTheDocument()
	})

	it('counts the volumes of a series where an author would be', () => {
		renderCard({
			files: [
				{ format: 'cbz', originalName: 'v1.cbz', size: 1 },
				{ format: 'cbz', originalName: 'v2.cbz', size: 1 },
			],
			kind: IntakeKind.Series,
		})

		expect(screen.getByText('2 volumes')).toBeInTheDocument()
	})

	it.each([
		[IntakeState.Ready, 'Ready'],
		[IntakeState.NeedsReview, 'Needs your details'],
		[IntakeState.Failed, 'Couldn’t read it'],
	])('says a %s book is “%s” in a chip', (state, label) => {
		renderCard({ state })

		expect(screen.getByText(label)).toBeInTheDocument()
	})

	it('says an approver’s book is to approve, whatever state it is in', () => {
		renderCard({ state: IntakeState.PendingApproval }, { mode: 'review', onApprove: vi.fn() })

		expect(screen.getByText('To approve')).toBeInTheDocument()
	})

	describe('a book that could not be read', () => {
		const failed = { decision: null, error: 'Not a book', state: IntakeState.Failed }

		it('says what went wrong, and has nothing to add', () => {
			renderCard(failed)

			expect(screen.getByText('What went wrong')).toBeInTheDocument()
			expect(screen.getByText('Not a book')).toBeInTheDocument()
			expect(screen.queryByRole('button', { name: /Add/ })).not.toBeInTheDocument()
			expect(screen.queryByRole('button', { name: /Send/ })).not.toBeInTheDocument()
		})

		it('has “Try again” as its one main button, which reads the book again', () => {
			const onReanalyse = vi.fn()
			renderCard(failed, { onReanalyse })

			// one button of that name: the main one, in the footer next to “Discard”
			const tryAgain = screen.getByRole('button', { name: /Try again/ })
			const footer = screen.getByRole('button', { name: 'Discard' }).parentElement as HTMLElement
			expect(footer).toContainElement(tryAgain)
			expect(tryAgain).toBeEnabled()
			fireEvent.click(tryAgain)

			expect(onReanalyse).toHaveBeenCalledWith('a')
		})

		it('keeps the way out out of the alert, which only says what went wrong', () => {
			renderCard(failed)

			const alert = screen.getByText('What went wrong').closest('[role="alert"]') as HTMLElement
			expect(within(alert).queryByRole('button')).toBeNull()
		})

		it('cannot be tried again while something is being saved', () => {
			renderCard(failed, { isBusy: true })

			expect(screen.getByRole('button', { name: /Try again/ })).toBeDisabled()
		})

		it('says “Try again” to somebody who needs approval, too', () => {
			const onReanalyse = vi.fn()
			renderCard(failed, { mode: 'submit', onReanalyse, onSubmit: vi.fn() })

			fireEvent.click(screen.getByRole('button', { name: /Try again/ }))

			expect(onReanalyse).toHaveBeenCalledWith('a')
			expect(screen.queryByRole('button', { name: /Send/ })).not.toBeInTheDocument()
		})

		it('is not the approver’s to read again: the main button stays “Approve”, and waits', () => {
			renderCard(failed, { mode: 'review', onApprove: vi.fn(), onSendBack: vi.fn() })

			expect(screen.getByRole('button', { name: 'Approve' })).toBeDisabled()
			expect(screen.queryByRole('button', { name: /Try again/ })).not.toBeInTheDocument()
		})
	})

	it('names what the main button does for every state it can be in', () => {
		const base = { attention: false, friend: false, ready: true, reviewing: false, sentBack: false }

		expect(primaryLabel({ ...base, state: IntakeState.Failed })).toBe('Try again')
		expect(primaryLabel({ ...base, friend: true, state: IntakeState.Failed })).toBe('Try again')
		expect(primaryLabel({ ...base, reviewing: true, state: IntakeState.Failed })).toBe('Approve')
		expect(primaryLabel({ ...base, state: IntakeState.Ready })).toBe('Add to library')
		expect(primaryLabel({ ...base, friend: true, state: IntakeState.Ready })).toBe(
			'Send for approval',
		)
	})

	it('keeps one main button, and the way to discard beside it', () => {
		renderCard()

		const footer = screen.getByRole('button', { name: 'Add to library' }).parentElement
		expect(footer).not.toBeNull()
		expect(
			within(footer as HTMLElement).getByRole('button', { name: 'Discard' }),
		).toBeInTheDocument()
	})

	it('puts the reason a book cannot go on right above its main button', () => {
		renderCard({ problems: ['Choose a folder'] })

		const footer = screen.getByRole('button', { name: /Add/ }).parentElement as HTMLElement
		const problems = screen.getByText('Before it can be added').closest('[role="alert"]')
		expect(problems).not.toBeNull()
		// the alert comes straight before the footer in the card
		expect(problems?.nextElementSibling).toBe(footer)
	})
})

describe('IntakeItemCard look up online', () => {
	it('has a visible “Look up online” button that looks the book up again', () => {
		const onReanalyse = vi.fn()
		renderCard({}, { onReanalyse })

		fireEvent.click(screen.getByRole('button', { name: 'Look up online' }))

		expect(onReanalyse).toHaveBeenCalledWith('a', true)
	})

	it('cannot be pressed while something is being saved', () => {
		renderCard({}, { isBusy: true })

		expect(screen.getByRole('button', { name: 'Look up online' })).toBeDisabled()
	})
})
