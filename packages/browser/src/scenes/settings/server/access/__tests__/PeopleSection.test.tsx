import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { PASSWORD_ALPHABET } from '../password'
import PeopleSection from '../PeopleSection'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/paths', () => ({
	usePaths: () => ({ updateUser: (id: string) => `/settings/users/${id}/manage` }),
}))
vi.mock('../copy', () => ({ copyText: vi.fn().mockResolvedValue(true) }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

import { toast } from 'sonner'

import { copyText } from '../copy'

const K = (key: string) => `settingsScene.server/access.${key}`

const row = (
	id: string,
	username: string,
	extra: Record<string, unknown> = {},
	rest: Record<string, unknown> = {},
) => ({
	person: { id, username, locked: false, removed: false, isServerOwner: false, ...extra },
	lastSignInAt: null,
	mustChangePassword: false,
	publicSignin: false,
	maxDevices: 3,
	activeDevices: 0,
	sessions: 0,
	newLibrariesHidden: true,
	signupLinkId: null,
	beyondFriendSet: [],
	hiddenLibraries: 0,
	...rest,
})

const PEOPLE = [
	row('o', 'g3', { isServerOwner: true }, { lastSignInAt: '2026-11-10T12:00:00Z' }),
	row('ana', 'ana'),
	row('ben', 'ben', { locked: true }),
	row('cy', 'cy', {}, { mustChangePassword: true }),
]

function renderSection(
	setPassword: (variables: Record<string, unknown> | undefined) => unknown = () => ({
		setUserPassword: true,
	}),
) {
	const app = setup({
		answers: (query, variables) => {
			if (query.includes('query PublicAccessPeople')) return { accessPeople: PEOPLE }
			if (query.includes('mutation PublicAccessSetPassword')) return setPassword(variables)
			if (query.includes('mutation PublicAccessUnblock')) return { unblockUser: true }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<PeopleSection />, { wrapper: app.Wrapper })
	return app
}

const passwordField = () => screen.getByLabelText(K('people.password.label')) as HTMLInputElement

describe('People', () => {
	beforeEach(() => {
		vi.clearAllMocks()
	})

	it('shows each account with what is wrong with it', async () => {
		renderSection()

		const owner = await screen.findByTestId('access-people-o')
		expect(within(owner).getByText(K('people.owner'))).toBeInTheDocument()
		expect(
			within(screen.getByTestId('access-people-ben')).getByText(K('people.locked')),
		).toBeInTheDocument()
		expect(
			within(screen.getByTestId('access-people-cy')).getByText(K('people.mustChange')),
		).toBeInTheDocument()
		expect(
			within(screen.getByTestId('access-people-ana')).getByText(
				`${K('people.lastSignIn')} at=${K('people.never')}`,
			),
		).toBeInTheDocument()
	})

	it('never offers a password for the owner, since it is never used from the internet', async () => {
		renderSection()

		const owner = await screen.findByTestId('access-people-o')
		expect(within(owner).queryByRole('button', { name: K('people.setPassword') })).toBeNull()
		expect(
			within(screen.getByTestId('access-people-ana')).getByRole('button', {
				name: K('people.setPassword'),
			}),
		).toBeInTheDocument()
	})

	it("offers unblock only to a locked account, and links to the account's page", async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		const ben = await screen.findByTestId('access-people-ben')
		expect(
			within(screen.getByTestId('access-people-ana')).queryByRole('button', {
				name: K('signIns.unblock'),
			}),
		).toBeNull()
		expect(within(ben).getByRole('link', { name: K('people.manage') })).toHaveAttribute(
			'href',
			'/settings/users/ben/manage',
		)

		await user.click(within(ben).getByRole('button', { name: K('signIns.unblock') }))
		await waitFor(() => expect(sent(execute, 'PublicAccessUnblock')).toEqual([{ id: 'ben' }]))
	})

	describe('setting a password', () => {
		const open = async (user: ReturnType<typeof userEvent.setup>, id = 'ana') => {
			await user.click(
				within(await screen.findByTestId(`access-people-${id}`)).getByRole('button', {
					name: K('people.setPassword'),
				}),
			)
			await screen.findByRole('dialog')
		}

		it('starts with a password made for the owner: 20 characters, none that look alike', async () => {
			renderSection()
			const user = userEvent.setup()
			await open(user)

			const password = passwordField().value
			expect(password).toHaveLength(20)
			expect([...password].every((char) => PASSWORD_ALPHABET.includes(char))).toBe(true)
			// the person has to choose their own, unless the owner says otherwise
			expect(screen.getByRole('switch', { name: K('people.password.requireChange') })).toBeChecked()
		})

		it('can make another, and takes one that is typed', async () => {
			renderSection()
			const user = userEvent.setup()
			await open(user)

			const first = passwordField().value
			await user.click(screen.getByRole('button', { name: K('people.password.generate') }))
			expect(passwordField().value).not.toBe(first)

			await user.clear(passwordField())
			await user.type(passwordField(), 'short')
			expect(screen.getByText(`${K('people.password.length')} min=8 max=128`)).toBeInTheDocument()
			expect(screen.getByRole('button', { name: K('people.password.set') })).toBeDisabled()

			await user.type(passwordField(), 'er-and-longer')
			expect(screen.getByRole('button', { name: K('people.password.set') })).toBeEnabled()
		})

		it('sets it with the requirement that they choose their own, then shows it once, to copy', async () => {
			const { execute } = renderSection()
			const user = userEvent.setup()
			await open(user)
			const typed = passwordField().value

			await user.click(screen.getByRole('button', { name: K('people.password.set') }))

			await waitFor(() =>
				expect(sent(execute, 'PublicAccessSetPassword')).toEqual([
					{ id: 'ana', password: typed, requireChange: true },
				]),
			)
			const done = await screen.findByTestId('access-password-done')
			expect(within(done).getByLabelText(K('people.password.label'))).toHaveValue(typed)
			expect(within(done).getByText(K('people.password.doneMustChange'))).toBeInTheDocument()

			await user.click(within(done).getByRole('button', { name: K('people.password.copy') }))
			expect(copyText).toHaveBeenCalledWith(typed)
		})

		it('can leave them with the password that was set', async () => {
			const { execute } = renderSection()
			const user = userEvent.setup()
			await open(user)

			await user.click(screen.getByRole('switch', { name: K('people.password.requireChange') }))
			await user.click(screen.getByRole('button', { name: K('people.password.set') }))

			await waitFor(() =>
				expect(sent(execute, 'PublicAccessSetPassword')[0]).toMatchObject({ requireChange: false }),
			)
			expect(await screen.findByText(K('people.password.doneKept'))).toBeInTheDocument()
		})

		it('says what the server says when it refuses (a person who may not change their password, say), and sets nothing', async () => {
			renderSection(() => {
				throw new Error('That person may not change their password')
			})
			const user = userEvent.setup()
			await open(user)

			await user.click(screen.getByRole('button', { name: K('people.password.set') }))

			expect(
				await screen.findByText('That person may not change their password'),
			).toBeInTheDocument()
			expect(screen.queryByTestId('access-password-done')).not.toBeInTheDocument()
			// still the form: it can be tried again
			expect(screen.getByRole('button', { name: K('people.password.set') })).toBeInTheDocument()
		})

		it('keeps nothing of the password when it is closed: a new one is made the next time', async () => {
			renderSection()
			const user = userEvent.setup()
			await open(user)
			const first = passwordField().value

			await user.click(screen.getByRole('button', { name: 'shared.common.cancel' }))
			await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())

			await open(user, 'cy')
			expect(passwordField().value).not.toBe(first)
			expect(passwordField().value).toHaveLength(20)
		})

		it('warns that it signs them out everywhere', async () => {
			renderSection()
			const user = userEvent.setup()
			await open(user)

			expect(screen.getByText(K('people.password.signsOut'))).toBeInTheDocument()
			// and a toast is not where the password goes
			expect(toast.success).not.toHaveBeenCalled()
		})
	})
})
