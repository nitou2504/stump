import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import SignupLinkSection from '../SignupLinkSection'
import { page, person, sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('../copy', () => ({ copyText: vi.fn().mockResolvedValue(true) }))
// the QR code is drawn by a library that loads when it is first needed: a stand-in shows what it was given
vi.mock('../QrCode', () => ({
	default: ({ value }: { value: string }) => <div data-testid="qr-stand-in">{value}</div>,
}))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

import { toast } from 'sonner'

import { copyText } from '../copy'

const K = (key: string) => `settingsScene.server/access.${key}`

const URL_OF_LINK = 'https://g3.test:8443/api/_i/SECRETSIGNUPTOKEN1'

const overview = (extra: Record<string, unknown> = {}) => ({
	hasKey: true,
	hasPublicUrl: true,
	hasStateDir: true,
	configured: true,
	requirePass: true,
	publicUrl: 'https://g3.test:8443',
	stateWrittenAt: null,
	activeDevices: 0,
	openInvites: 0,
	publicSignin: true,
	activeSignupLinks: 0,
	signinStats: { failuresLast10Min: 0, namesWaiting: 0, globalBudgetTripped: false },
	...extra,
})

const LIBRARIES = [
	{ id: 'bib', name: 'Biblioteca' },
	{ id: 'manga', name: 'Manga' },
	{ id: 'guides', name: 'Guides' },
]

const DEFAULTS = {
	permissions: ['CHANGE_PASSWORD', 'CHANGE_AVATAR', 'DOWNLOAD_FILE', 'ADD_BOOKS'],
	maxDevices: 3,
	libraries: [LIBRARIES[0]],
}

const link = (id: string, status: string, extra: Record<string, unknown> = {}) => ({
	id,
	label: `note ${id}`,
	kind: 'OPEN',
	createdAt: '2026-11-10T10:00:00Z',
	createdBy: 'g3',
	expiresAt: '2026-11-13T10:00:00Z',
	maxUses: 5,
	uses: 2,
	status,
	revokedAt: null,
	preset: { permissions: DEFAULTS.permissions, maxDevices: 3, ageRestriction: null, libraries: [LIBRARIES[0]] },
	people: [person('ana-id', 'ana'), person('ben-id', 'ben')],
	...extra,
})

function renderSection({
	over = {},
	links = [] as ReturnType<typeof link>[],
}: { over?: Record<string, unknown>; links?: ReturnType<typeof link>[] } = {}) {
	const app = setup({
		answers: (query) => {
			if (query.includes('query PublicAccessOverview')) return { accessOverview: overview(over) }
			if (query.includes('query PublicAccessPresetDefaults')) return { signupPresetDefaults: DEFAULTS }
			if (query.includes('query PublicAccessLibraries')) return { libraries: { nodes: LIBRARIES } }
			if (query.includes('query PublicAccessSignupLinks')) return { accessSignupLinks: page(links) }
			if (query.includes('mutation PublicAccessCreateSignupLink'))
				return { createAccessSignupLink: { url: URL_OF_LINK, link: { id: 'new' } } }
			if (query.includes('mutation PublicAccessRevokeSignupLink'))
				return { revokeAccessSignupLink: 2 }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<SignupLinkSection />, { wrapper: app.Wrapper })
	return app
}

const note = () => screen.findByLabelText(K('signups.form.label'))
const create = () => screen.getByRole('button', { name: K('signups.form.create') })

describe('Signup links', () => {
	beforeEach(() => {
		vi.clearAllMocks()
	})

	it('offers one person for three days with the friend preset, and says in words what they get', async () => {
		renderSection()

		await note()
		expect(screen.getByLabelText(K('signups.form.kind'))).toHaveValue('SINGLE')
		expect(screen.getByLabelText(K('signups.form.validFor'))).toHaveValue('72')
		expect(screen.getByLabelText(K('signups.form.uses'))).toBeDisabled()

		const summary = screen.getByTestId('access-preset-summary')
		for (const permission of ['changePassword', 'changeAvatar', 'downloadFile', 'addBooks']) {
			expect(summary.textContent).toContain(K(`signups.permission.${permission}`))
		}
		// the permissions a link never gives are not in what it says
		expect(summary.textContent).not.toContain(K('signups.permission.changeUsername'))
		expect(summary.textContent).toContain('Biblioteca')
		expect(summary.textContent).not.toContain('Manga')
		expect(summary.textContent).toContain(`${K('signups.summary.devices')} count=3`)
		expect(summary.textContent).toContain(K('signups.summary.later'))
		expect(create()).toBeDisabled()
	})

	it('makes a link for one person with the preset it showed', async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		await user.type(await note(), '  Ana  ')
		await user.click(create())

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessCreateSignupLink')).toEqual([
				{
					input: {
						label: 'Ana',
						kind: 'SINGLE',
						validForHours: 72,
						maxUses: 1,
						preset: {
							permissions: DEFAULTS.permissions,
							libraryIds: ['bib'],
							ageRestriction: null,
							maxDevices: 3,
						},
					},
				},
			]),
		)
	})

	it('makes an open link with a budget, and warns that an open link with Add books is for people you trust', async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		await user.type(await note(), 'club')
		expect(screen.queryByTestId('access-open-addbooks')).toBeNull()
		await user.selectOptions(screen.getByLabelText(K('signups.form.kind')), 'OPEN')
		expect(screen.getByLabelText(K('signups.form.uses'))).toBeEnabled()
		expect(screen.getByLabelText(K('signups.form.uses'))).toHaveValue('5')
		expect(screen.getByTestId('access-open-addbooks')).toBeInTheDocument()

		await user.selectOptions(screen.getByLabelText(K('signups.form.uses')), '10')
		await user.selectOptions(screen.getByLabelText(K('signups.form.validFor')), '168')
		await user.click(create())

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessCreateSignupLink')).toMatchObject([
				{ input: { kind: 'OPEN', maxUses: 10, validForHours: 168 } },
			]),
		)
	})

	it('lets the owner change the preset, and offers only what a friend may have', async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		await user.type(await note(), 'club')
		await user.click(screen.getByRole('button', { name: K('signups.form.change') }))
		const editor = await screen.findByTestId('access-preset-editor')

		// the six permissions of the friend set, and nothing that scans, uploads or manages
		const boxes = within(editor)
			.getAllByRole('checkbox')
			.map((box) => box.id)
			.filter((id) => id.startsWith('access-signup-') && !id.startsWith('access-signup-library-'))
		expect(boxes.sort()).toEqual(
			[
				'CHANGE_AVATAR',
				'CHANGE_PASSWORD',
				'CHANGE_USERNAME',
				'DOWNLOAD_FILE',
				'ADD_BOOKS',
				'ACCESS_SMART_LIST',
			]
				.map((name) => `access-signup-${name}`)
				.sort(),
		)
		for (const never of ['UPLOAD_FILE', 'SCAN_LIBRARY', 'MANAGE_LIBRARY', 'MANAGE_USERS']) {
			expect(editor.querySelector(`[id="access-signup-${never}"]`)).toBeNull()
		}
		expect(within(editor).getByText(K('signups.editor.notOffered'))).toBeInTheDocument()

		// take away Add books, give Smart lists, add Manga, five devices, an age limit
		await user.click(within(editor).getByLabelText(K('signups.permission.addBooks')))
		await user.click(within(editor).getByLabelText(K('signups.permission.accessSmartList')))
		await user.click(within(editor).getByLabelText('Manga'))
		await user.selectOptions(within(editor).getByLabelText(K('signups.editor.devices')), '5')
		await user.type(within(editor).getByLabelText(K('signups.editor.age')), '13')
		await user.click(create())

		await waitFor(() => expect(sent(execute, 'PublicAccessCreateSignupLink')).toHaveLength(1))
		const [{ input }] = sent(execute, 'PublicAccessCreateSignupLink') as [
			{ input: { preset: Record<string, unknown> } },
		]
		expect([...(input.preset.permissions as string[])].sort()).toEqual(
			['ACCESS_SMART_LIST', 'CHANGE_AVATAR', 'CHANGE_PASSWORD', 'DOWNLOAD_FILE'].sort(),
		)
		expect([...(input.preset.libraryIds as string[])].sort()).toEqual(['bib', 'manga'])
		expect(input.preset.maxDevices).toBe(5)
		expect(input.preset.ageRestriction).toEqual({ age: 13, restrictOnUnset: false })
	})

	it('cannot make a link for no library, or with an age that is not a number', async () => {
		renderSection()
		const user = userEvent.setup()

		await user.type(await note(), 'club')
		await user.click(screen.getByRole('button', { name: K('signups.form.change') }))
		const editor = await screen.findByTestId('access-preset-editor')
		expect(create()).toBeEnabled()

		await user.click(within(editor).getByLabelText('Biblioteca'))
		expect(within(editor).getByText(K('signups.editor.noLibraries'))).toBeInTheDocument()
		expect(create()).toBeDisabled()
		await user.click(within(editor).getByLabelText('Guides'))
		expect(create()).toBeEnabled()

		// the field takes digits only; zero is not an age
		const age = within(editor).getByLabelText(K('signups.editor.age'))
		await user.type(age, 'ab0')
		expect(age).toHaveValue('0')
		expect(within(editor).getByText(K('signups.editor.ageInvalid'))).toBeInTheDocument()
		expect(create()).toBeDisabled()
	})

	it('shows the link once, with copy and a QR code, and keeps it nowhere', async () => {
		const app = renderSection()
		const user = userEvent.setup()

		await user.type(await note(), 'Ana')
		await user.click(create())

		const made = await screen.findByTestId('access-made-signup')
		expect(within(made).getByLabelText(K('signups.created.linkLabel'))).toHaveValue(URL_OF_LINK)
		expect(within(made).getByTestId('qr-stand-in')).toHaveTextContent(URL_OF_LINK)
		expect(within(made).getByText(K('signups.created.once'))).toBeInTheDocument()

		await user.click(within(made).getByRole('button', { name: K('signups.created.copy') }))
		expect(copyText).toHaveBeenCalledWith(URL_OF_LINK)
		expect(toast.success).toHaveBeenCalledWith(K('signups.created.copied'))

		// no query of the page holds it: it was in the answer of the mutation, and that was dropped
		const cached = JSON.stringify(
			app.client
				.getQueryCache()
				.getAll()
				.map((query) => query.state.data),
		)
		expect(cached).not.toContain('SECRETSIGNUPTOKEN1')
		expect(
			JSON.stringify(
				app.client
					.getMutationCache()
					.getAll()
					.map((mutation) => mutation.state.data),
			),
		).not.toContain('SECRETSIGNUPTOKEN1')

		await user.click(within(made).getByRole('button', { name: K('signups.created.dismiss') }))
		expect(screen.queryByTestId('access-made-signup')).toBeNull()
		expect(document.body.textContent).not.toContain('SECRETSIGNUPTOKEN1')
	})

	it('says why it can not be made when public sign-in is off', async () => {
		renderSection({ over: { publicSignin: false } })
		const user = userEvent.setup()

		expect(await screen.findByTestId('access-signup-off')).toHaveTextContent(K('signups.off'))
		await user.type(await note(), 'Ana')
		expect(create()).toBeDisabled()
	})

	it('lists the links with their state, who signed up with each, and takes an open one back', async () => {
		const { execute } = renderSection({
			links: [
				link('open', 'ACTIVE'),
				link('done', 'USED_UP'),
				link('gone', 'REVOKED', { people: [] }),
			],
		})
		const user = userEvent.setup()

		const open = await screen.findByTestId('access-signup-open')
		expect(within(open).getByText(K('signups.status.active'))).toBeInTheDocument()
		expect(within(open).getByText('ana')).toBeInTheDocument()
		expect(within(open).getByText('ben')).toBeInTheDocument()
		expect(
			within(screen.getByTestId('access-signup-done')).getByText(K('signups.status.usedUp')),
		).toBeInTheDocument()
		// only a link that can still make accounts can be revoked; a spent one with people can have them turned off
		expect(within(screen.getByTestId('access-signup-gone')).queryByRole('button')).toBeNull()
		expect(
			within(screen.getByTestId('access-signup-done')).getByRole('button', {
				name: K('signups.turnOffAccounts'),
			}),
		).toBeInTheDocument()

		await user.click(within(open).getByRole('button', { name: K('signups.revoke') }))
		const dialog = await screen.findByRole('dialog')
		await user.click(within(dialog).getByRole('switch', { name: `${K('signups.confirm.alsoAccounts')} count=2` }))
		await user.click(within(dialog).getByRole('button', { name: K('signups.confirm.revoke') }))

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessRevokeSignupLink')).toEqual([
				{ id: 'open', disableAccounts: true },
			]),
		)
		await waitFor(() =>
			expect(toast.success).toHaveBeenCalledWith(
				`${K('signups.revoked')} label=note open`,
				expect.objectContaining({ description: `${K('signups.revokedAccounts')} count=2` }),
			),
		)
	})

	it('revokes without touching the accounts unless asked', async () => {
		const { execute } = renderSection({ links: [link('open', 'ACTIVE')] })
		const user = userEvent.setup()

		await user.click(
			within(await screen.findByTestId('access-signup-open')).getByRole('button', {
				name: K('signups.revoke'),
			}),
		)
		const dialog = await screen.findByRole('dialog')
		await user.click(within(dialog).getByRole('button', { name: K('signups.confirm.revoke') }))

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessRevokeSignupLink')).toEqual([
				{ id: 'open', disableAccounts: false },
			]),
		)
	})
})
