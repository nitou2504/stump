import { IntakeState } from '@stump/graphql'
import { fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import IntakeScene from '../IntakeScene'
import type { IntakeItemData, IntakeLimitsData } from '../queries'
import { item } from './fixtures'

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
}))
// the page header sits in the app's sidebar layout
vi.mock('@/components/sharedLayout', () => ({
	CollectionSceneHeader: ({ title }: { title: string }) => <h1>{title}</h1>,
}))
// the pickers and the drop zone ask the server or the browser for more: not what is tested here
vi.mock('../FolderPicker', () => ({ default: () => <div>Folder picker</div> }))
vi.mock('../TagPicker', () => ({ TagChip: () => null, default: () => <div>Tag picker</div> }))
vi.mock('../IntakeDropZone', () => ({
	default: ({ disabled }: { disabled?: boolean }) => (
		<div data-testid="drop-zone" data-disabled={String(!!disabled)} />
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

		expect(screen.getByText(/look at the books you add before they join it/)).toBeInTheDocument()
		const usage = screen.getByLabelText('Your inbox')
		expect(usage).toHaveTextContent('512.0 MB of 2.0 GB')
		expect(usage).toHaveTextContent('3 of 100 books')
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
		expect(filing.getByText('being added')).toBeInTheDocument()
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
})

describe('IntakeScene for somebody who files books themselves', () => {
	beforeEach(() => {
		vi.clearAllMocks()
		state.limits = limits({ canFileDirectly: true, quotaApplies: false })
		state.items = []
	})

	it('adds what is ready to the library, and has no sections of an approval', () => {
		state.items = [book('a'), book('b')]

		renderScene()

		expect(screen.getByRole('region', { name: 'Ready to add' })).toBeInTheDocument()
		expect(screen.queryByText(/look at the books you add/)).not.toBeInTheDocument()
		expect(screen.queryByLabelText('Your inbox')).not.toBeInTheDocument()
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
