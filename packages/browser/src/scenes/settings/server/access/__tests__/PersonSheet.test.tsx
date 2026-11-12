import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import PersonSheet from '../PersonSheet'
import { device, K, row, serverOf } from './fixtures'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/paths', () => ({
	usePaths: () => ({
		bookApprovals: () => '/settings/book-approvals',
		updateUser: (id: string) => `/settings/users/${id}/manage`,
	}),
}))
vi.mock('../copy', () => ({ copyText: vi.fn().mockResolvedValue(true) }))
vi.mock('sonner', () => ({ toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn() }) }))

import { copyText } from '../copy'

const ANA = row('ana', 'ana', {
	publicSignin: true,
	activeDevices: 2,
	lastSignInAt: '2026-11-11T09:00:00Z',
})

const DEVICES = [
	device('ana-phone', { label: 'Ana’s phone' }),
	device('ana-kobo', { label: 'Kobo', kind: 'OPDS', lastSeenAt: null }),
	device('ana-old', { label: 'Old laptop', status: 'REVOKED', revokedAt: '2026-11-05T00:00:00Z' }),
]

function renderSheet(
	data: Parameters<typeof serverOf>[0] = {},
	answers: (query: string, variables: Record<string, unknown> | undefined) => unknown = () =>
		undefined,
	personId: string | null = 'ana',
) {
	const server = serverOf({ people: [ANA], devices: DEVICES, ...data })
	const app = setup({
		answers: (query, variables) => {
			const own = answers(query, variables)
			return own === undefined ? server(query, variables) : own
		},
	})
	const props = { onClose: vi.fn(), onLetIn: vi.fn(), onSetPassword: vi.fn() }
	render(<PersonSheet personId={personId} {...props} />, { wrapper: app.Wrapper })
	return { app, ...props }
}

const access = {
	setPersonAccess: {
		publicSignin: true,
		maxDevices: 3,
		hiddenLibraries: 0,
		newLibrariesHidden: true,
	},
}

describe('A person’s sheet', () => {
	beforeEach(() => vi.clearAllMocks())

	it('stays closed for nobody, and for somebody the list does not know', async () => {
		renderSheet({}, () => undefined, null)
		await waitFor(() => expect(screen.queryByTestId('access-sheet')).toBeNull())
	})

	it('shows the devices that work, and keeps the removed ones behind a button', async () => {
		renderSheet()

		const sheet = await screen.findByTestId('access-person-sheet', undefined, { timeout: 5000 })
		expect(within(sheet).getByTestId('access-device-ana-phone')).toBeInTheDocument()
		expect(within(sheet).getByTestId('access-device-ana-kobo')).toBeInTheDocument()
		expect(within(sheet).queryByTestId('access-device-ana-old')).toBeNull()
		expect(sheet).toHaveTextContent(`${K('person.devices')} active=2 limit=3`)
		// the kind of each, and the IP and the browser only when a row is opened
		expect(
			within(screen.getByTestId('access-device-ana-kobo')).getByText(K('person.kind.readingApp')),
		).toBeInTheDocument()
		expect(sheet).not.toHaveTextContent('198.51.100.7')
	})

	it('opens a device to show where it was first seen, from where and with what', async () => {
		renderSheet()
		const user = userEvent.setup()

		const phone = await screen.findByTestId('access-device-ana-phone')
		await user.click(within(phone).getAllByRole('button', { expanded: false })[0] as HTMLElement)

		const detail = within(phone).getByTestId('access-device-detail')
		expect(detail).toHaveTextContent('198.51.100.7')
		// what can be done with it is there too
		expect(
			within(phone).getByRole('button', { name: K('person.removeDevice') }),
		).toBeInTheDocument()
		expect(detail).toHaveTextContent('Chrome on Linux')
	})

	it('brings back a removed device', async () => {
		const { app } = renderSheet({}, (query) =>
			query.includes('mutation PublicAccessRestoreDevice')
				? { restoreAccessDevice: true }
				: undefined,
		)
		const user = userEvent.setup()

		await user.click(
			await screen.findByRole('button', { name: `${K('person.showRemoved')} count=1` }),
		)
		const old = await screen.findByTestId('access-device-ana-old')
		await user.click(within(old).getByRole('button', { name: K('person.restore') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessRestoreDevice')).toEqual([{ id: 'ana-old' }]),
		)
	})

	it('asks before it removes a device, and then removes that one', async () => {
		const { app } = renderSheet({}, (query) =>
			query.includes('mutation PublicAccessRevokeDevice')
				? { revokeAccessDevice: true }
				: undefined,
		)
		const user = userEvent.setup()

		const phone = await screen.findByTestId('access-device-ana-phone')
		await user.click(within(phone).getAllByRole('button', { expanded: false })[0] as HTMLElement)
		await user.click(within(phone).getByRole('button', { name: K('person.removeDevice') }))
		const dialog = await screen.findByTestId('access-confirm')
		expect(dialog).toHaveTextContent(`${K('person.removeDeviceConfirm')} label=Ana’s phone`)
		expect(sent(app.execute, 'PublicAccessRevokeDevice')).toEqual([])
		await user.click(within(dialog).getByRole('button', { name: K('person.removeDevice') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessRevokeDevice')).toEqual([{ id: 'ana-phone' }]),
		)
	})

	it('lets a device try again after too many wrong passwords', async () => {
		const { app } = renderSheet({}, (query) =>
			query.includes('mutation PublicAccessResetBudget')
				? { resetAccessDeviceBudget: true }
				: undefined,
		)
		const user = userEvent.setup()

		const phone = await screen.findByTestId('access-device-ana-phone')
		await user.click(within(phone).getAllByRole('button', { expanded: false })[0] as HTMLElement)
		await user.click(within(phone).getByRole('button', { name: K('person.allowMoreTries') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessResetBudget')).toEqual([{ id: 'ana-phone' }]),
		)
	})

	it('changes the number of devices they may have', async () => {
		const { app } = renderSheet({}, (query) =>
			query.includes('mutation PublicAccessSetPersonAccess') ? access : undefined,
		)
		const user = userEvent.setup()

		await user.selectOptions(await screen.findByLabelText(K('person.limit')), '5')

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'ana', maxDevices: 5 } },
			]),
		)
	})

	it('turns the internet off only after saying what that means, and keeps their devices', async () => {
		const { app } = renderSheet({}, (query) =>
			query.includes('mutation PublicAccessSetPersonAccess') ? access : undefined,
		)
		const user = userEvent.setup()

		await user.click(await screen.findByRole('switch', { name: K('person.internet') }))
		const dialog = await screen.findByTestId('access-confirm')
		expect(dialog).toHaveTextContent(`${K('letIn.turnOff')} name=ana`)
		expect(sent(app.execute, 'PublicAccessSetPersonAccess')).toEqual([])
		await user.click(within(dialog).getByRole('button', { name: K('person.turnOff') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'ana', publicSignin: false } },
			]),
		)
		expect(sent(app.execute, 'PublicAccessRevokePerson')).toEqual([])
	})

	it('hands turning the internet on to the dialog that says what they get', async () => {
		const off = row('ben', 'ben', { publicSignin: false })
		const { app, onLetIn } = renderSheet({ people: [off] }, () => undefined, 'ben')
		const user = userEvent.setup()

		await user.click(await screen.findByRole('switch', { name: K('person.internet') }))

		expect(onLetIn).toHaveBeenCalledWith(off)
		expect(sent(app.execute, 'PublicAccessSetPersonAccess')).toEqual([])
	})

	it('makes a link for another device and shows it once, with a copy button and the QR code only on request', async () => {
		const { app } = renderSheet({}, (query) =>
			query.includes('mutation PublicAccessCreateInvite')
				? {
						createAccessInvite: {
							link: 'https://books.example.org/api/_i/secret-token',
							invite: { id: 'i1' },
						},
					}
				: undefined,
		)
		const user = userEvent.setup()

		await user.click(await screen.findByRole('button', { name: K('person.addDeviceLink') }))
		// without a name nothing is sent
		await user.click(screen.getByRole('button', { name: K('invite.create') }))
		expect(screen.getByText(K('person.deviceNameMissing'))).toBeInTheDocument()
		expect(sent(app.execute, 'PublicAccessCreateInvite')).toEqual([])

		await user.type(screen.getByLabelText(K('person.deviceName')), 'Ana’s tablet')
		await user.click(screen.getByRole('button', { name: K('invite.create') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessCreateInvite')).toEqual([
				{ input: { personId: 'ana', label: 'Ana’s tablet', validForHours: 72, maxUses: 3 } },
			]),
		)
		const made = await screen.findByTestId('access-made-link')
		expect(within(made).getByLabelText(K('invite.linkLabel'))).toHaveValue(
			'https://books.example.org/api/_i/secret-token',
		)
		expect(screen.queryByTestId('access-qr')).toBeNull()
		await user.click(within(made).getByRole('button', { name: K('invite.copy') }))
		expect(copyText).toHaveBeenCalledWith('https://books.example.org/api/_i/secret-token')
		await user.click(within(made).getByRole('button', { name: K('invite.showQr') }))
		expect(await screen.findByTestId('access-qr')).toBeInTheDocument()
		// the address is forgotten when the block is closed
		await user.click(within(made).getByRole('button', { name: K('invite.done') }))
		expect(screen.queryByDisplayValue('https://books.example.org/api/_i/secret-token')).toBeNull()
	})

	it('shows the library they see, and changes it', async () => {
		const { app } = renderSheet({}, (query) =>
			query.includes('mutation PublicAccessSetPersonAccess') ? access : undefined,
		)
		const user = userEvent.setup()

		const sheet = await screen.findByTestId('access-person-sheet', undefined, { timeout: 5000 })
		// ana is kept out of Manga: she sees Biblioteca
		expect(sheet).toHaveTextContent(`${K('person.sees')} libraries=Biblioteca`)
		await user.click(within(sheet).getByRole('button', { name: K('person.change') }))
		await user.click(await screen.findByRole('checkbox', { name: 'Manga' }))
		await user.click(screen.getByRole('button', { name: 'shared.common.save' }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'ana', onlyLibraryIds: ['bib', 'manga'] } },
			]),
		)
	})

	it('hides libraries made from now on, or not', async () => {
		const { app } = renderSheet({}, (query) =>
			query.includes('mutation PublicAccessSetPersonAccess') ? access : undefined,
		)
		const user = userEvent.setup()

		await user.click(await screen.findByRole('switch', { name: K('person.hideNew') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'ana', newLibrariesHidden: false } },
			]),
		)
	})

	it('lets a locked account back in', async () => {
		const locked = row('ana', 'ana', { publicSignin: true }, { locked: true })
		const { app } = renderSheet(
			{
				people: [locked],
				blocked: [
					{
						failuresLast24H: 12,
						lastFailureAt: null,
						lockedBy: 'FAILURES',
						person: { id: 'ana', username: 'ana' },
					},
				],
			},
			(query) =>
				query.includes('mutation PublicAccessUnblock') ? { unblockUser: true } : undefined,
		)
		const user = userEvent.setup()

		const alert = await screen.findByTestId('access-person-locked')
		expect(alert).toHaveTextContent(`${K('person.locked')} count=12`)
		await user.click(within(alert).getByRole('button', { name: K('person.unblock') }))

		await waitFor(() => expect(sent(app.execute, 'PublicAccessUnblock')).toEqual([{ id: 'ana' }]))
	})

	it('signs them out everywhere after asking', async () => {
		const { app } = renderSheet({}, (query) =>
			query.includes('mutation PublicAccessSignOutEverywhere')
				? { deleteUserSessions: 2 }
				: undefined,
		)
		const user = userEvent.setup()

		await user.click(await screen.findByText(K('person.signOutAll')))
		const dialog = await screen.findByTestId('access-confirm')
		await user.click(within(dialog).getByRole('button', { name: K('person.signOutAll') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSignOutEverywhere')).toEqual([{ id: 'ana' }]),
		)
	})

	it('asks the page for the dialog that sets a password, and does not set one itself', async () => {
		const { app, onSetPassword } = renderSheet()
		const user = userEvent.setup()

		await user.click(await screen.findByText(K('person.setPassword')))

		expect(onSetPassword).toHaveBeenCalledWith({ id: 'ana', username: 'ana' })
		expect(sent(app.execute, 'PublicAccessSetPassword')).toEqual([])
	})

	it('can be asked, and then not: Cancel changes nothing', async () => {
		const { app } = renderSheet()
		const user = userEvent.setup()

		await user.click(await screen.findByRole('button', { name: K('person.removeAll') }))
		const ask = await screen.findByTestId('access-confirm')
		await user.click(within(ask).getByRole('button', { name: 'shared.common.cancel' }))

		expect(screen.queryByTestId('access-confirm')).toBeNull()
		expect(sent(app.execute, 'PublicAccessRevokePerson')).toEqual([])
	})

	it('cuts off every device after asking', async () => {
		const { app } = renderSheet({}, (query) =>
			query.includes('mutation PublicAccessRevokePerson')
				? { revokeAccessDevicesOfPerson: 2 }
				: undefined,
		)
		const user = userEvent.setup()

		await user.click(await screen.findByRole('button', { name: K('person.removeAll') }))
		const dialog = await screen.findByTestId('access-confirm')
		expect(dialog).toHaveTextContent(`${K('person.removeAllConfirm')} name=ana`)
		await user.click(within(dialog).getByRole('button', { name: K('person.removeAll') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessRevokePerson')).toEqual([{ personId: 'ana' }]),
		)
	})

	it('points at their upload limits and their account', async () => {
		renderSheet()

		const sheet = await screen.findByTestId('access-person-sheet', undefined, { timeout: 5000 })
		const links = within(sheet).getAllByRole('link')
		expect(links.map((link) => link.getAttribute('href'))).toEqual([
			'/settings/book-approvals?person=ana#limits',
			'/settings/users/ana/manage',
		])
	})
})
