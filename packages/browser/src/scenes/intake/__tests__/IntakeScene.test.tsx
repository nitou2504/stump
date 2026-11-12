import { IntakeState } from '@stump/graphql'
import { fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import IntakeScene from '../IntakeScene'
import type { IntakeItemData, IntakeLimitsData } from '../queries'
import { item } from './fixtures'

vi.mock('@/hooks/useTranslate', () => import('./english'))

const state = vi.hoisted(() => ({
	items: [] as unknown[],
	limits: undefined as unknown,
}))
const actions = vi.hoisted(() => ({
	acceptItem: vi.fn(),
	acceptWithGenres: vi.fn(),
	changeLibrary: vi.fn(),
	discard: vi.fn(),
	isAccepting: false,
	isChangingLibrary: false,
	isReviewing: false,
	isSubmitting: false,
	isWithdrawing: false,
	reanalyse: vi.fn(),
	reviewItem: vi.fn(),
	submitItem: vi.fn(),
	submitWithGenres: vi.fn(),
	withdraw: vi.fn(),
}))
const uploader = vi.hoisted(() => ({ add: vi.fn(), clearFinished: vi.fn(), entries: [] }))
vi.mock('../hooks', () => ({
	useIntakeActions: () => actions,
	useIntakeInbox: () => ({ data: {}, isLoading: false, items: state.items }),
	useIntakeLimits: () => ({ limits: state.limits }),
	useIntakeUploader: () => uploader,
}))
vi.mock('@stump/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/client')>()),
	useGraphQL: () => ({ data: { intakeLibraries: [{ id: 'lib', name: 'Biblioteca' }] } }),
	// the cover of a book that is in the library is its thumbnail
	useSDK: () => ({
		sdk: { isTokenAuth: false, media: { thumbnailURL: (id: string) => `/thumbnail/${id}` } },
	}),
}))
// the page header sits in the app's sidebar layout
vi.mock('@/components/sharedLayout', () => ({
	CollectionSceneHeader: ({ title }: { title: string }) => <h1>{title}</h1>,
}))
// the pickers and the drop zone ask the server or the browser for more: not what is tested here
vi.mock('../FolderPicker', () => ({ default: () => <div>Folder picker</div> }))
vi.mock('../TagPicker', () => ({ TagChip: () => null, default: () => <div>Tag picker</div> }))
vi.mock('../IntakeDropZone', () => ({
	default: ({
		disabled,
		compact,
		acceptsZip,
	}: {
		disabled?: boolean
		compact?: boolean
		acceptsZip?: boolean
	}) => (
		<div
			data-testid="drop-zone"
			data-disabled={String(!!disabled)}
			data-compact={String(!!compact)}
			data-accepts-zip={String(!!acceptsZip)}
		/>
	),
}))

const MB = 1024 * 1024
const limits = (overrides: Partial<IntakeLimitsData> = {}): IntakeLimitsData => ({
	canFileDirectly: false,
	closedReason: null,
	maxFileBytes: 100 * MB,
	maxFilesPerRequest: 10,
	maxItems: 100,
	maxRequestBytes: 300 * MB,
	openItems: 3,
	quotaApplies: true,
	quotaBytes: 2048 * MB,
	uploadsOpen: true,
	usedBytes: 512 * MB,
	...overrides,
})

const book = (id: string, overrides: Partial<IntakeItemData> = {}) =>
	item({ id, name: `${id}.epub`, ...overrides })

const renderScene = () =>
	render(
		<MemoryRouter>
			<IntakeScene />
		</MemoryRouter>,
	)

describe('IntakeScene for somebody who needs approval', () => {
	beforeEach(() => {
		vi.clearAllMocks()
		state.limits = limits()
		state.items = []
		actions.isSubmitting = false
	})

	it('says what happens to their books, and how much of their inbox is used', () => {
		renderScene()

		expect(screen.getByText(/looks over your books before they join it/)).toBeInTheDocument()
		// a quarter full is nothing to say
		expect(screen.queryByLabelText(/almost full/)).not.toBeInTheDocument()
	})

	it('says so, with the numbers, only when the inbox is almost full', () => {
		state.limits = limits({ usedBytes: 1800 * MB, openItems: 90 })

		renderScene()

		const usage = screen.getByLabelText(/Your inbox is almost full/)
		expect(usage).toHaveTextContent('1.8 GB of 2.0 GB')
		expect(usage).toHaveTextContent('90 of 100 books')
	})

	it('says it also when the books, not the room, are nearly used up', () => {
		state.limits = limits({ usedBytes: 10 * MB, openItems: 85 })

		renderScene()

		expect(screen.getByLabelText(/Your inbox is almost full/)).toBeInTheDocument()
	})

	it('shows nothing of the inbox until it is known who is looking', () => {
		state.limits = undefined
		state.items = [book('a'), book('b', { state: IntakeState.NeedsReview })]

		renderScene()

		expect(screen.queryByRole('article')).not.toBeInTheDocument()
		expect(screen.queryByText('Ready to send')).not.toBeInTheDocument()
	})

	it('sends what is ready, not adds it', () => {
		state.items = [book('a'), book('b')]

		renderScene()

		expect(screen.getByRole('region', { name: 'Ready to send' })).toBeInTheDocument()
		expect(screen.queryByText('Ready to add')).not.toBeInTheDocument()
		expect(screen.queryByRole('button', { name: /Add all ready/ })).not.toBeInTheDocument()
		fireEvent.click(screen.getByRole('button', { name: 'Send all ready (2)' }))

		// the sheet asks before anything goes
		expect(screen.getByText('Send 2 books for approval')).toBeInTheDocument()
		expect(actions.submitWithGenres).not.toHaveBeenCalled()
		const sheet = within(screen.getByRole('dialog'))
		fireEvent.click(sheet.getByRole('button', { name: 'Send 2 books' }))
		expect(actions.submitWithGenres).toHaveBeenCalledTimes(1)
		expect(actions.acceptWithGenres).not.toHaveBeenCalled()
	})

	it('shows the books an approver sent back, with their note, apart from the ready ones', () => {
		state.items = [
			book('ready'),
			book('back', {
				reviewNote: 'The author is spelled wrong',
				reviewedAt: new Date().toISOString(),
				state: IntakeState.Rejected,
			}),
		]

		renderScene()

		const sentBack = within(screen.getByRole('region', { name: 'Sent back' }))
		expect(sentBack.getByText('The author is spelled wrong')).toBeInTheDocument()
		expect(sentBack.getByRole('button', { name: 'Send again' })).toBeInTheDocument()
		// only the ready one is in "send all ready"
		expect(screen.getByRole('button', { name: 'Send all ready (1)' })).toBeInTheDocument()
	})

	it('lists the books waiting for approval, which can be taken back', () => {
		state.items = [
			book('w', {
				destination: null,
				state: IntakeState.PendingApproval,
				submittedAt: new Date(Date.now() - 3600 * 1000).toISOString(),
			}),
		]

		renderScene()

		const waiting = within(screen.getByRole('region', { name: 'Waiting for approval' }))
		expect(waiting.getByText('El nombre de la rosa')).toBeInTheDocument()
		expect(waiting.getByText(/sent about 1 hour ago/)).toBeInTheDocument()
		fireEvent.click(waiting.getByRole('button', { name: /Take back/ }))
		expect(actions.withdraw).toHaveBeenCalledWith(['w'])
		// it is not a card to change: it is with the approver
		expect(screen.queryByRole('article')).not.toBeInTheDocument()
	})

	it('shows an approved book while it is being added, with nothing to take back', () => {
		state.items = [book('f', { state: IntakeState.Filing })]

		renderScene()

		const filing = within(screen.getByRole('region', { name: 'Being added' }))
		expect(filing.getByText('Adding…')).toBeInTheDocument()
		expect(filing.queryByRole('button', { name: /Take back/ })).not.toBeInTheDocument()
	})

	it('shows an added book by where it went inside the library, as it has no destination', () => {
		state.items = [
			book('d', {
				destination: null,
				filedAt: '2020-01-01T00:00:00Z',
				mediaIds: ['m1'],
				result: { notes: [], paths: ['Umberto_Eco/El_nombre_de_la_rosa.epub'] },
				state: IntakeState.Filed,
			}),
		]

		renderScene()

		const added = within(screen.getByRole('region', { name: 'Added' }))
		expect(added.getByText('Umberto_Eco/El_nombre_de_la_rosa.epub')).toBeInTheDocument()
		expect(added.getByRole('link', { name: 'Open' })).toBeInTheDocument()
	})

	it('closes the drop zone, and says why, while uploads are closed', () => {
		state.limits = limits({ closedReason: 'The server is full', uploadsOpen: false })

		renderScene()

		expect(screen.getByText('Uploads are closed')).toBeInTheDocument()
		expect(screen.getByText('The server is full')).toBeInTheDocument()
		expect(screen.getByTestId('drop-zone')).toHaveAttribute('data-disabled', 'true')
	})

	it('has an empty inbox say so', () => {
		renderScene()

		expect(screen.getByText('Your inbox is empty.')).toBeInTheDocument()
	})

	it('says nothing of an empty inbox until it is known who is looking', () => {
		// the last step is “Add them to the library” for who files and “Send them” for who doesn’t:
		// saying the wrong one for a moment would be wrong for somebody
		state.limits = undefined

		renderScene()

		expect(screen.queryByText('Your inbox is empty.')).not.toBeInTheDocument()
		expect(screen.queryByRole('list', { name: 'How adding books works' })).not.toBeInTheDocument()
	})

	it('does not offer zips of several books: they are for who files books themselves', () => {
		renderScene()

		expect(screen.getByTestId('drop-zone')).toHaveAttribute('data-accepts-zip', 'false')
	})

	it('offers no zips either until it is known who is looking', () => {
		state.limits = undefined

		renderScene()

		expect(screen.getByTestId('drop-zone')).toHaveAttribute('data-accepts-zip', 'false')
	})
})

describe('IntakeScene for somebody who files books themselves', () => {
	beforeEach(() => {
		vi.clearAllMocks()
		state.limits = limits({ canFileDirectly: true, quotaApplies: false })
		state.items = []
	})

	it('offers zips of several books: they are theirs to open and file', () => {
		renderScene()

		expect(screen.getByTestId('drop-zone')).toHaveAttribute('data-accepts-zip', 'true')
	})

	it('adds what is ready to the library, and has no sections of an approval', () => {
		state.items = [book('a'), book('b')]

		renderScene()

		expect(screen.getByRole('region', { name: 'Ready to add' })).toBeInTheDocument()
		expect(screen.queryByText(/looks over your books/)).not.toBeInTheDocument()
		expect(screen.queryByLabelText(/Your inbox is almost full/)).not.toBeInTheDocument()
		fireEvent.click(screen.getByRole('button', { name: 'Add all ready (2)' }))
		expect(screen.getByRole('heading', { name: 'Add 2 books' })).toBeInTheDocument()
		fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Add 2 books' }))

		expect(actions.acceptWithGenres).toHaveBeenCalledTimes(1)
		expect(actions.submitWithGenres).not.toHaveBeenCalled()
	})

	it('keeps the folder and the destination of each card', () => {
		state.items = [book('a')]

		renderScene()

		expect(screen.getByText('Folder picker')).toBeInTheDocument()
		expect(screen.getByRole('button', { name: 'Add to library' })).toBeInTheDocument()
	})
})

describe('IntakeScene layout', () => {
	const SECTIONS = [
		'Being read',
		'Sent back',
		'Needs your details',
		'Ready to add',
		'Ready to send',
		'Waiting for approval',
		'Being added',
		'Added',
	]
	const sectionOrder = () =>
		screen
			.getAllByRole('region')
			.map((region) => region.getAttribute('aria-label') ?? '')
			.filter((name) => SECTIONS.includes(name))

	beforeEach(() => {
		vi.clearAllMocks()
		state.limits = limits({ canFileDirectly: true, quotaApplies: false })
		state.items = []
	})

	it('has a big drop zone while the inbox is empty, and a slim one once books are on their way', () => {
		renderScene()
		expect(screen.getByTestId('drop-zone')).toHaveAttribute('data-compact', 'false')
	})

	it('steps the drop zone aside for the books on their way', () => {
		state.items = [book('a')]

		renderScene()

		expect(screen.getByTestId('drop-zone')).toHaveAttribute('data-compact', 'true')
	})

	it('keeps the drop zone big when the only books are ones that are already in the library', () => {
		state.items = [
			book('d', { destination: null, filedAt: '2020-01-01T00:00:00Z', state: IntakeState.Filed }),
		]

		renderScene()

		expect(screen.getByTestId('drop-zone')).toHaveAttribute('data-compact', 'false')
	})

	it('reads top to bottom as the books move: read, sent back, to check, ready, waiting, added', () => {
		state.limits = limits({ canFileDirectly: false })
		state.items = [
			book('added', {
				destination: null,
				filedAt: '2020-01-01T00:00:00Z',
				state: IntakeState.Filed,
			}),
			book('filing', { state: IntakeState.Filing }),
			book('waiting', { destination: null, state: IntakeState.PendingApproval }),
			book('ready'),
			book('check', { state: IntakeState.NeedsReview }),
			book('back', { reviewNote: 'Wrong author', state: IntakeState.Rejected }),
			book('reading', { decision: null, proposal: null, state: IntakeState.Analysing }),
		]

		renderScene()

		expect(sectionOrder()).toEqual([
			'Being read',
			'Sent back',
			'Needs your details',
			'Ready to send',
			'Waiting for approval',
			'Being added',
			'Added',
		])
	})

	it('shows a book being read with a bar that says something is happening', () => {
		state.items = [book('a', { decision: null, proposal: null, state: IntakeState.Analysing })]

		renderScene()

		const reading = within(screen.getByRole('region', { name: 'Being read' }))
		expect(reading.getByText('a.epub')).toBeInTheDocument()
		expect(reading.getByText('Reading…')).toBeInTheDocument()
		expect(reading.getByRole('progressbar', { name: 'Reading a.epub' })).toBeInTheDocument()
	})

	it('shows an added book with the cover the scan made of it', () => {
		state.items = [
			book('d', {
				destination: null,
				filedAt: '2020-01-01T00:00:00Z',
				mediaIds: ['m1'],
				result: { notes: [], paths: ['Umberto_Eco/El_nombre_de_la_rosa.epub'] },
				state: IntakeState.Filed,
			}),
		]

		const { container } = renderScene()

		expect(container.querySelector('img[src="/thumbnail/m1"]')).not.toBeNull()
	})

	it('shows a tile instead of a cover for an added book the scan has not reached', () => {
		state.items = [
			book('d', {
				destination: null,
				filedAt: new Date().toISOString(),
				result: { notes: [], paths: ['Umberto_Eco/El_nombre_de_la_rosa.epub'] },
				state: IntakeState.Filed,
			}),
		]

		const { container } = renderScene()

		expect(container.querySelector('img')).toBeNull()
		expect(screen.getByText('scanning')).toBeInTheDocument()
	})

	it('shows, in an empty inbox, what happens to a book: for those who add books themselves', () => {
		renderScene()

		const steps = within(screen.getByRole('list', { name: 'How adding books works' }))
		expect(steps.getByText('Drop your books')).toBeInTheDocument()
		expect(steps.getByText('Check the details')).toBeInTheDocument()
		expect(steps.getByText('Add them to the library')).toBeInTheDocument()
	})

	it('shows, in an empty inbox, what happens to a book: for those who send them for approval', () => {
		state.limits = limits({ canFileDirectly: false })

		renderScene()

		const steps = within(screen.getByRole('list', { name: 'How adding books works' }))
		expect(steps.getByText('Send them for approval')).toBeInTheDocument()
		expect(steps.queryByText('Add them to the library')).not.toBeInTheDocument()
	})

	it('has no steps once there are books', () => {
		state.items = [book('a')]

		renderScene()

		expect(screen.queryByRole('list', { name: 'How adding books works' })).not.toBeInTheDocument()
	})

	it('says each book of a section in the same words as its status chip', () => {
		state.items = [book('check', { state: IntakeState.NeedsReview })]

		renderScene()

		const section = within(screen.getByRole('region', { name: 'Needs your details' }))
		expect(section.getByRole('article', { name: 'El nombre de la rosa' })).toHaveTextContent(
			'Needs your details',
		)
	})
})
