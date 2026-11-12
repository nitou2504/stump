import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import PeopleSection from '../PeopleSection'
import { device, invite, K, overview, row, serverOf, signupLink } from './fixtures'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('sonner', () => ({ toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn() }) }))

const PEOPLE = [
	row('o', 'g3', { publicSignin: false }, { isServerOwner: true }),
	row('ana', 'ana', { publicSignin: true, activeDevices: 3, lastSignInAt: '2026-11-11T09:00:00Z' }),
	row(
		'ben',
		'ben',
		{ publicSignin: true, lastSignInAt: '2026-11-11T11:00:00Z', mustChangePassword: true },
		{ locked: true },
	),
	row('cy', 'cy', { publicSignin: true, beyondFriendSet: ['MANAGE_USERS'] }),
	row('dee', 'dee', { publicSignin: false }),
	row('gone', 'gone', { publicSignin: true }, { removed: true }),
]

function renderPeople(
	data: Parameters<typeof serverOf>[0] = {},
	answers: (query: string, variables: Record<string, unknown> | undefined) => unknown = () =>
		undefined,
) {
	const server = serverOf({ people: PEOPLE, ...data })
	const app = setup({
		answers: (query, variables) => {
			const own = answers(query, variables)
			return own === undefined ? server(query, variables) : own
		},
	})
	const props = { onInvite: vi.fn(), onOpenPerson: vi.fn() }
	render(<PeopleSection {...props} />, { wrapper: app.Wrapper })
	return { app, ...props }
}

describe('People', () => {
	it('lists who can sign in from the internet, the newest sign-in first, and leaves out the owner and anybody removed', async () => {
		renderPeople()

		const group = await screen.findByTestId('access-people-internet')
		const names = within(group)
			.getAllByTestId(/^access-person-/)
			.map((element) => element.getAttribute('data-testid'))
		expect(names).toEqual(['access-person-ben', 'access-person-ana', 'access-person-cy'])
		expect(group).toHaveTextContent(`${K('people.groups.internet')} count=3`)
		expect(screen.queryByTestId('access-person-o')).toBeNull()
		expect(screen.queryByTestId('access-person-gone')).toBeNull()
	})

	it('says how each one stands in a sentence, and what is wrong in a badge', async () => {
		renderPeople()

		const ana = await screen.findByTestId('access-person-ana')
		expect(ana).toHaveTextContent(`${K('people.devicesLine')} active=3 limit=3`)
		const ben = screen.getByTestId('access-person-ben')
		expect(within(ben).getByText(K('people.badges.locked'))).toBeInTheDocument()
		expect(within(ben).getByText(K('people.badges.mustChange'))).toBeInTheDocument()
		// the extras that keep somebody off the internet are a badge too
		expect(
			within(screen.getByTestId('access-person-cy')).getByText(K('people.badges.extras')),
		).toBeInTheDocument()
		// somebody who never signed in says so
		expect(screen.getByTestId('access-person-cy')).toHaveTextContent(K('people.devicesLineNever'))
	})

	it('has no control in a row: the whole row opens the person', async () => {
		const { onOpenPerson } = renderPeople()
		const user = userEvent.setup()

		const ana = await screen.findByTestId('access-person-ana')
		expect(within(ana).queryAllByRole('button')).toHaveLength(0)
		expect(ana.tagName).toBe('BUTTON')
		await user.click(ana)

		expect(onOpenPerson).toHaveBeenCalledWith('ana')
	})

	it('keeps the people who cannot sign in behind a button', async () => {
		renderPeople()
		const user = userEvent.setup()

		const button = await screen.findByRole('button', { name: `${K('people.groups.home')} count=1` })
		expect(screen.queryByTestId('access-person-dee')).toBeNull()
		await user.click(button)
		expect(screen.getByTestId('access-person-dee')).toBeInTheDocument()
	})

	it('has the one big button, and it asks to invite somebody', async () => {
		const { onInvite } = renderPeople()
		const user = userEvent.setup()

		const invite = await screen.findByRole('button', { name: K('people.invite') })
		await user.click(invite)

		expect(onInvite).toHaveBeenCalledOnce()
	})

	it('cannot invite anybody until public access is set up, and says why', async () => {
		renderPeople({ overview: overview({ configured: false }) })

		const invite = await screen.findByRole('button', { name: K('people.invite') })
		expect(invite).toBeDisabled()
		expect(invite).toHaveAttribute('title', K('people.inviteNeedsSetup'))
	})

	it('shows what was invited and has not come in yet, soonest to end first', async () => {
		renderPeople({
			links: [
				signupLink('club', 'Club de lectura', {
					kind: 'OPEN',
					maxUses: 5,
					uses: 2,
					expiresAt: '2099-01-01T00:00:00Z',
				}),
				signupLink('rosa', 'Tía Rosa', { expiresAt: '2098-01-01T00:00:00Z' }),
			],
			invites: [invite('phone', 'Ana’s phone', { expiresAt: '2097-01-01T00:00:00Z' })],
		})

		const group = await screen.findByTestId('access-people-invited')
		const ids = within(group)
			.getAllByTestId(/^access-invited-/)
			.map((element) => element.getAttribute('data-testid'))
		expect(ids).toEqual(['access-invited-phone', 'access-invited-rosa', 'access-invited-club'])
		expect(screen.getByTestId('access-invited-rosa')).toHaveTextContent(K('people.invitedOne'))
		expect(screen.getByTestId('access-invited-club')).toHaveTextContent(
			`${K('people.invitedOpen')} used=2 max=5`,
		)
		expect(screen.getByTestId('access-invited-phone')).toHaveTextContent(
			`${K('people.invitedDevice')} name=ana`,
		)
	})

	it('revokes an invitation after asking, and a signup link may take its accounts with it', async () => {
		const { app } = renderPeople(
			{
				links: [
					signupLink('club', 'Club de lectura', {
						kind: 'OPEN',
						maxUses: 5,
						uses: 2,
						people: [{ id: 'ana', username: 'ana', removed: false }],
					}),
				],
			},
			(query) =>
				query.includes('mutation PublicAccessRevokeSignupLink')
					? { revokeAccessSignupLink: 1 }
					: undefined,
		)
		const user = userEvent.setup()

		const line = await screen.findByTestId('access-invited-club')
		await user.click(within(line).getByRole('button', { name: K('people.moreActions') }))
		await user.click(await screen.findByRole('menuitem', { name: K('people.revoke') }))
		const dialog = await screen.findByTestId('access-revoke-link')
		expect(sent(app.execute, 'PublicAccessRevokeSignupLink')).toEqual([])
		await user.click(within(dialog).getByRole('checkbox'))
		await user.click(within(dialog).getByRole('button', { name: K('revoke.confirm') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessRevokeSignupLink')).toEqual([
				{ id: 'club', disableAccounts: true },
			]),
		)
	})

	it('revokes a device link too', async () => {
		const { app } = renderPeople({ invites: [invite('phone', 'Ana’s phone')] }, (query) =>
			query.includes('mutation PublicAccessRevokeInvite')
				? { revokeAccessInvite: true }
				: undefined,
		)
		const user = userEvent.setup()

		const line = await screen.findByTestId('access-invited-phone')
		await user.click(within(line).getByRole('button', { name: K('people.moreActions') }))
		await user.click(await screen.findByRole('menuitem', { name: K('people.revoke') }))
		await user.click(
			within(await screen.findByTestId('access-revoke-link')).getByRole('button', {
				name: K('revoke.confirm'),
			}),
		)

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessRevokeInvite')).toEqual([{ id: 'phone' }]),
		)
	})

	it('hides what ended until asked: links that were used up, and passes revoked long ago', async () => {
		renderPeople({
			links: [
				signupLink('old', 'Old link', { status: 'EXPIRED' }),
				signupLink('open', 'Open link'),
			],
			devices: [
				device('deploy-old', {
					legacy: true,
					person: null,
					status: 'REVOKED',
					revokedAt: '2026-11-12T00:00:00Z',
				}),
				device('ana-phone', { status: 'ACTIVE' }),
			],
		})
		const user = userEvent.setup()

		const show = await screen.findByTestId('access-show-ended')
		expect(show).toHaveTextContent(`${K('people.showEnded')} count=2`)
		expect(screen.queryByText('Old link')).toBeNull()
		expect(screen.queryByText('deploy-old')).toBeNull()
		expect(screen.getByText('Open link')).toBeInTheDocument()

		await user.click(show)
		const ended = await screen.findByTestId('access-ended')
		expect(within(ended).getByText('Old link')).toBeInTheDocument()
		expect(within(ended).getByText('deploy-old')).toBeInTheDocument()
		// an active device of somebody is theirs to show, not here
		expect(within(ended).queryByText('ana-phone')).toBeNull()
		expect(screen.getByTestId('access-show-ended')).toHaveTextContent(K('people.hideEnded'))
	})

	it('brings back a revoked pass', async () => {
		const { app } = renderPeople(
			{
				devices: [
					device('deploy-old', {
						legacy: true,
						person: null,
						status: 'REVOKED',
						revokedAt: '2026-11-12T00:00:00Z',
					}),
				],
			},
			(query) =>
				query.includes('mutation PublicAccessRestoreDevice')
					? { restoreAccessDevice: true }
					: undefined,
		)
		const user = userEvent.setup()

		await user.click(await screen.findByTestId('access-show-ended'))
		const ended = await screen.findByTestId('access-ended')
		await user.click(within(ended).getByRole('button', { name: K('person.restore') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessRestoreDevice')).toEqual([{ id: 'deploy-old' }]),
		)
	})

	it('says there is nobody when there is nobody', async () => {
		renderPeople({ people: [row('o', 'g3', {}, { isServerOwner: true })] })

		expect(await screen.findByText(K('people.empty'))).toBeInTheDocument()
	})
})
