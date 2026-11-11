import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import DeviceSection, { groupByPerson } from '../DeviceSection'
import { page, sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

import { toast } from 'sonner'

const K = (key: string) => `settingsScene.server/access.${key}`

const device = (id: string, personId: string | null, extra: Record<string, unknown> = {}) => ({
	id,
	label: `${id} label`,
	legacy: personId === null,
	createdAt: '2026-11-10T10:00:00Z',
	firstSeenAt: '2026-11-10T10:05:00Z',
	lastSeenAt: '2026-11-10T12:00:00Z',
	lastIp: '203.0.113.9',
	lastAgent: 'Mozilla/5.0 (iPhone)',
	status: 'ACTIVE',
	revokedAt: null,
	revokedBy: null,
	person: personId ? { id: personId, username: personId, removed: false } : null,
	...extra,
})

const DEVICES = [
	device('ana-phone', 'ana'),
	device('ben-laptop', 'ben'),
	device('ana-tablet', 'ana', {
		status: 'REVOKED',
		revokedAt: '2026-11-10T13:00:00Z',
		revokedBy: 'g3',
	}),
	device('old-pass', null),
]

function renderSection(devices = DEVICES) {
	const app = setup({
		answers: (query) => {
			if (query.includes('query PublicAccessDevices')) return { accessDevices: page(devices) }
			if (query.includes('mutation PublicAccessRevokeDevice')) return { revokeAccessDevice: true }
			if (query.includes('mutation PublicAccessRestoreDevice')) return { restoreAccessDevice: true }
			if (query.includes('mutation PublicAccessRevokePerson'))
				return { revokeAccessDevicesOfPerson: 2 }
			if (query.includes('mutation PublicAccessResetBudget'))
				return { resetAccessDeviceBudget: true }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<DeviceSection />, { wrapper: app.Wrapper })
	return app
}

describe('groupByPerson', () => {
	it("puts a person's devices together, in the order the people first show up, and the terminal passes last as theirs", () => {
		const groups = groupByPerson(DEVICES as never)

		expect(groups.map((group) => [group.key, group.devices.map((d) => d.id)])).toEqual([
			['ana', ['ana-phone', 'ana-tablet']],
			['ben', ['ben-laptop']],
			['terminal', ['old-pass']],
		])
		expect(groups[2]!.person).toBeNull()
	})
})

describe('Devices with a pass', () => {
	beforeEach(() => {
		vi.clearAllMocks()
	})

	it("lists each person's devices, marking a terminal pass and a revoked device", async () => {
		renderSection()

		const ana = await screen.findByTestId('access-person-ana')
		expect(within(ana).getByTestId('access-device-ana-phone')).toBeInTheDocument()
		expect(within(ana).getByTestId('access-device-ana-tablet')).toBeInTheDocument()
		expect(
			within(screen.getByTestId('access-device-ana-tablet')).getByText(K('devices.revoked')),
		).toBeInTheDocument()

		const terminal = screen.getByTestId('access-person-terminal')
		expect(within(terminal).getByText(K('devices.terminalGroup'))).toBeInTheDocument()
		expect(within(terminal).getByText(K('devices.legacy'))).toBeInTheDocument()
	})

	it('shows when it was seen and from where, with the browser in its title', async () => {
		renderSection()

		const row = await screen.findByTestId('access-device-ana-phone')
		expect(
			within(row).getByText(/devices\.detail first=.* last=.* ip=203\.0\.113\.9/),
		).toBeInTheDocument()
		expect(within(row).getByTitle('Mozilla/5.0 (iPhone)')).toBeInTheDocument()
	})

	it('asks before it revokes a device, and changes nothing if you cancel', async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		const row = await screen.findByTestId('access-device-ben-laptop')
		await user.click(within(row).getByRole('button', { name: K('devices.revoke') }))

		const dialog = await screen.findByRole('alertdialog')
		expect(within(dialog).getByText(K('devices.confirmTitle'))).toBeInTheDocument()
		expect(
			within(dialog).getByText(`${K('devices.confirmDescription')} label=ben-laptop label`),
		).toBeInTheDocument()
		expect(sent(execute, 'PublicAccessRevokeDevice')).toEqual([])

		await user.click(within(dialog).getByRole('button', { name: 'Cancel' }))
		await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
		expect(sent(execute, 'PublicAccessRevokeDevice')).toEqual([])
	})

	it('revokes a device once it is confirmed', async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		const row = await screen.findByTestId('access-device-ben-laptop')
		await user.click(within(row).getByRole('button', { name: K('devices.revoke') }))
		await user.click(
			await within(await screen.findByRole('alertdialog')).findByRole('button', {
				name: K('devices.confirmRevoke'),
			}),
		)

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessRevokeDevice')).toEqual([{ id: 'ben-laptop' }]),
		)
		expect(toast.success).toHaveBeenCalledWith(
			`${K('devices.revokedDevice')} label=ben-laptop label`,
		)
		// every query of the page is loaded again
		await waitFor(() => expect(sent(execute, 'PublicAccessDevices').length).toBeGreaterThan(1))
	})

	it('restores a revoked device without asking', async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		const row = await screen.findByTestId('access-device-ana-tablet')
		// a revoked device has no revoke and no budget button
		expect(within(row).queryByRole('button', { name: K('devices.revoke') })).toBeNull()
		expect(within(row).queryByRole('button', { name: K('devices.resetBudget') })).toBeNull()
		await user.click(within(row).getByRole('button', { name: K('devices.restore') }))

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessRestoreDevice')).toEqual([{ id: 'ana-tablet' }]),
		)
	})

	it("resets a device's sign-in budget", async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		const row = await screen.findByTestId('access-device-ana-phone')
		await user.click(within(row).getByRole('button', { name: K('devices.resetBudget') }))

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessResetBudget')).toEqual([{ id: 'ana-phone' }]),
		)
	})

	it('cuts off a whole person after asking, and says how many devices that was', async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		const group = await screen.findByTestId('access-person-ana')
		await user.click(
			within(group).getByRole('button', { name: `${K('devices.revokeAll')} name=ana` }),
		)

		const dialog = await screen.findByRole('alertdialog')
		expect(within(dialog).getByText(K('devices.confirmPersonTitle'))).toBeInTheDocument()
		expect(sent(execute, 'PublicAccessRevokePerson')).toEqual([])
		await user.click(within(dialog).getByRole('button', { name: K('devices.confirmRevoke') }))

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessRevokePerson')).toEqual([{ personId: 'ana' }]),
		)
		expect(toast.info).toHaveBeenCalledWith(`${K('devices.revokedCount')} count=2`)
	})

	it('has no "revoke all" for a person with nothing active, nor for the terminal passes', async () => {
		renderSection([device('ana-tablet', 'ana', { status: 'REVOKED' }), device('old-pass', null)])

		await screen.findByTestId('access-person-ana')
		expect(screen.queryByRole('button', { name: /devices\.revokeAll/ })).toBeNull()
	})

	it('says it could not revoke it', async () => {
		const app = setup({
			answers: (query) => {
				if (query.includes('query PublicAccessDevices')) return { accessDevices: page(DEVICES) }
				throw new Error('The edge could not be told')
			},
		})
		render(<DeviceSection />, { wrapper: app.Wrapper })
		const user = userEvent.setup()

		const row = await screen.findByTestId('access-device-ana-phone')
		await user.click(within(row).getByRole('button', { name: K('devices.revoke') }))
		await user.click(
			await within(await screen.findByRole('alertdialog')).findByRole('button', {
				name: K('devices.confirmRevoke'),
			}),
		)

		await waitFor(() =>
			expect(toast.error).toHaveBeenCalledWith(K('devices.errors.revoke'), {
				description: 'The edge could not be told',
			}),
		)
	})

	it('says when no device holds a pass', async () => {
		renderSection([])
		expect(await screen.findByText(K('devices.empty'))).toBeInTheDocument()
	})
})
