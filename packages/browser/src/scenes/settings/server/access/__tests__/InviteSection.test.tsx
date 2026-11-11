import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import InviteSection from '../InviteSection'
import { page, person, sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('../copy', () => ({ copyText: vi.fn().mockResolvedValue(true) }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

import { toast } from 'sonner'

import { copyText } from '../copy'

const K = (key: string) => `settingsScene.server/access.${key}`

const LINK = 'https://g3.test:8443/api/_i/SECRETTOKEN123456'

const overview = (configured = true) => ({
	hasKey: configured,
	hasPublicUrl: configured,
	hasStateDir: configured,
	configured,
	requirePass: false,
	publicUrl: 'https://g3.test:8443',
	stateWrittenAt: null,
	activeDevices: 0,
	openInvites: 0,
})

const PEOPLE = [
	{
		person: person('o', 'g3', { isServerOwner: true }),
		lastSignInAt: null,
		mustChangePassword: false,
	},
	{ person: person('ana', 'ana'), lastSignInAt: null, mustChangePassword: false },
	{ person: person('ben', 'ben', { locked: true }), lastSignInAt: null, mustChangePassword: false },
	{ person: person('cy', 'cy', { removed: true }), lastSignInAt: null, mustChangePassword: false },
	{ person: person('dee', 'dee'), lastSignInAt: null, mustChangePassword: false },
]

const invite = (id: string, status: string, label = 'phone') => ({
	id,
	label,
	createdAt: '2026-11-10T10:00:00Z',
	createdBy: 'g3',
	expiresAt: '2026-11-13T10:00:00Z',
	maxUses: 3,
	uses: 1,
	status,
	person: { id: 'ana', username: 'ana', removed: false },
})

function renderSection({ configured = true, invites = [] as ReturnType<typeof invite>[] } = {}) {
	const app = setup({
		answers: (query) => {
			if (query.includes('query PublicAccessOverview'))
				return { accessOverview: overview(configured) }
			if (query.includes('query PublicAccessPeople')) return { accessPeople: PEOPLE }
			if (query.includes('query PublicAccessInvites')) return { accessInvites: page(invites) }
			if (query.includes('mutation PublicAccessCreateInvite'))
				return { createAccessInvite: { link: LINK, invite: { id: 'new' } } }
			if (query.includes('mutation PublicAccessRevokeInvite')) return { revokeAccessInvite: true }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<InviteSection />, { wrapper: app.Wrapper })
	return app
}

const person$ = () => screen.findByLabelText(K('invites.form.person'))

describe('Invite links', () => {
	beforeEach(() => {
		vi.clearAllMocks()
	})

	it('offers only the people who can use an invite: not the owner, nobody locked or removed', async () => {
		renderSection()

		const select = await person$()
		const offered = within(select)
			.getAllByRole('option')
			.map((option) => option.textContent)

		// the first is the placeholder
		expect(offered).toEqual([K('invites.form.choosePerson'), 'ana', 'dee'])
	})

	it('makes an invite for the person and device, for three days and three uses unless chosen otherwise', async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		await user.selectOptions(await person$(), 'ana')
		await user.type(screen.getByLabelText(K('invites.form.label')), "  Ana's phone ")
		await user.click(screen.getByRole('button', { name: K('invites.form.create') }))

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessCreateInvite')).toEqual([
				{ input: { personId: 'ana', label: "Ana's phone", validForHours: 72, maxUses: 3 } },
			]),
		)
	})

	it('sends the validity and the uses that were chosen', async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		await user.selectOptions(await person$(), 'dee')
		await user.type(screen.getByLabelText(K('invites.form.label')), 'laptop')
		await user.selectOptions(screen.getByLabelText(K('invites.form.validFor')), '1')
		await user.selectOptions(screen.getByLabelText(K('invites.form.uses')), '10')
		await user.click(screen.getByRole('button', { name: K('invites.form.create') }))

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessCreateInvite')).toEqual([
				{ input: { personId: 'dee', label: 'laptop', validForHours: 1, maxUses: 10 } },
			]),
		)
	})

	it("can't be sent without a person and a name", async () => {
		renderSection()
		const user = userEvent.setup()
		const create = await screen.findByRole('button', { name: K('invites.form.create') })

		expect(create).toBeDisabled()
		await user.selectOptions(await person$(), 'ana')
		expect(create).toBeDisabled()
		await user.type(screen.getByLabelText(K('invites.form.label')), '   ')
		expect(create).toBeDisabled()
		await user.type(screen.getByLabelText(K('invites.form.label')), 'phone')
		expect(create).toBeEnabled()
	})

	it("can't make one while the public side isn't set up", async () => {
		renderSection({ configured: false })
		const user = userEvent.setup()

		await user.selectOptions(await person$(), 'ana')
		await user.type(screen.getByLabelText(K('invites.form.label')), 'phone')

		expect(screen.getByRole('button', { name: K('invites.form.create') })).toBeDisabled()
	})

	it('shows the link once, with the words that it is the only copy, and copies it', async () => {
		renderSection()
		const user = userEvent.setup()

		await user.selectOptions(await person$(), 'ana')
		await user.type(screen.getByLabelText(K('invites.form.label')), 'phone')
		await user.click(screen.getByRole('button', { name: K('invites.form.create') }))

		const box = await screen.findByTestId('access-made-link')
		expect(within(box).getByLabelText(K('invites.created.linkLabel'))).toHaveValue(LINK)
		expect(within(box).getByText(K('invites.created.once'))).toBeInTheDocument()

		await user.click(within(box).getByRole('button', { name: K('invites.created.copy') }))
		expect(copyText).toHaveBeenCalledWith(LINK)
		expect(toast.success).toHaveBeenCalledWith(K('invites.created.copied'))

		// done with it: it is gone from the page
		await user.click(within(box).getByRole('button', { name: K('invites.created.dismiss') }))
		expect(screen.queryByTestId('access-made-link')).not.toBeInTheDocument()
		expect(screen.queryByDisplayValue(LINK)).not.toBeInTheDocument()
	})

	it('says so when it could not copy the link, which stays selected in the box', async () => {
		vi.mocked(copyText).mockResolvedValueOnce(false)
		renderSection()
		const user = userEvent.setup()

		await user.selectOptions(await person$(), 'ana')
		await user.type(screen.getByLabelText(K('invites.form.label')), 'phone')
		await user.click(screen.getByRole('button', { name: K('invites.form.create') }))
		await user.click(await screen.findByRole('button', { name: K('invites.created.copy') }))

		expect(toast.error).toHaveBeenCalledWith(K('invites.created.copyFailed'))
		expect(screen.getByLabelText(K('invites.created.linkLabel'))).toHaveValue(LINK)
	})

	it("never lets the link into a cache: not a query's data, not a kept mutation", async () => {
		const { client } = renderSection()
		const user = userEvent.setup()

		await user.selectOptions(await person$(), 'ana')
		await user.type(screen.getByLabelText(K('invites.form.label')), 'phone')
		await user.click(screen.getByRole('button', { name: K('invites.form.create') }))
		await screen.findByTestId('access-made-link')

		const queries = JSON.stringify(
			client
				.getQueryCache()
				.getAll()
				.map((query) => query.state.data),
		)
		expect(queries).not.toContain('SECRETTOKEN')
		// the mutation that made it is dropped, with its answer
		await waitFor(() => expect(client.getMutationCache().getAll()).toHaveLength(0))
		expect(
			JSON.stringify(
				client
					.getMutationCache()
					.getAll()
					.map((m) => m.state.data),
			),
		).not.toContain('SECRETTOKEN')
	})

	it('says it could not make the link, and shows none', async () => {
		const app = setup({
			answers: (query) => {
				if (query.includes('query PublicAccessOverview')) return { accessOverview: overview() }
				if (query.includes('query PublicAccessPeople')) return { accessPeople: PEOPLE }
				if (query.includes('query PublicAccessInvites')) return { accessInvites: page([]) }
				throw new Error('That person may not be invited')
			},
		})
		render(<InviteSection />, { wrapper: app.Wrapper })
		const user = userEvent.setup()

		await user.selectOptions(await person$(), 'ana')
		await user.type(screen.getByLabelText(K('invites.form.label')), 'phone')
		await user.click(screen.getByRole('button', { name: K('invites.form.create') }))

		await waitFor(() =>
			expect(toast.error).toHaveBeenCalledWith(K('invites.errors.create'), {
				description: 'That person may not be invited',
			}),
		)
		expect(screen.queryByTestId('access-made-link')).not.toBeInTheDocument()
	})

	it('lists the invites with their state, and takes back only those that are still open', async () => {
		const { execute } = renderSection({
			invites: [
				invite('i-unused', 'UNUSED', 'one'),
				invite('i-used', 'USED', 'two'),
				invite('i-up', 'USED_UP', 'three'),
				invite('i-old', 'EXPIRED', 'four'),
				invite('i-rev', 'REVOKED', 'five'),
			],
		})
		const user = userEvent.setup()

		const list = await screen.findByTestId('access-invite-list')
		const revokeLabel = K('invites.revoke')
		for (const id of ['i-unused', 'i-used']) {
			expect(
				within(screen.getByTestId(`access-invite-${id}`)).getByRole('button', {
					name: revokeLabel,
				}),
			).toBeEnabled()
		}
		for (const id of ['i-up', 'i-old', 'i-rev']) {
			expect(
				within(screen.getByTestId(`access-invite-${id}`)).queryByRole('button', {
					name: revokeLabel,
				}),
			).toBeNull()
		}
		expect(within(list).getByText(K('invites.status.usedUp'))).toBeInTheDocument()
		expect(within(list).getByText(K('invites.status.expired'))).toBeInTheDocument()

		await user.click(
			within(screen.getByTestId('access-invite-i-used')).getByRole('button', { name: revokeLabel }),
		)
		await waitFor(() =>
			expect(sent(execute, 'PublicAccessRevokeInvite')).toEqual([{ id: 'i-used' }]),
		)
	})

	it('says when there are no invites', async () => {
		renderSection()
		expect(await screen.findByText(K('invites.empty'))).toBeInTheDocument()
	})
})
