import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import PeopleSection from '../PeopleSection'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/paths', () => ({
	usePaths: () => ({ updateUser: (id: string) => `/settings/users/${id}/manage` }),
}))
vi.mock('../copy', () => ({ copyText: vi.fn().mockResolvedValue(true) }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

import { toast } from 'sonner'

const K = (key: string) => `settingsScene.server/access.${key}`

const row = (
	id: string,
	username: string,
	rest: Record<string, unknown> = {},
	person: Record<string, unknown> = {},
) => ({
	person: { id, username, locked: false, removed: false, isServerOwner: false, ...person },
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
	row('o', 'g3', {}, { isServerOwner: true }),
	row('ana', 'ana', { publicSignin: true, activeDevices: 2, sessions: 4, hiddenLibraries: 3 }),
	row('dani', 'dani', { beyondFriendSet: ['UPLOAD_FILE', 'SCAN_LIBRARY'] }),
	row('eva', 'eva', { maxDevices: 1 }),
	row('max', 'max', { maxDevices: 10, publicSignin: true, activeDevices: 3 }),
]

function renderPeople() {
	const app = setup({
		answers: (query) => {
			if (query.includes('query PublicAccessPeople')) return { accessPeople: PEOPLE }
			if (query.includes('query PublicAccessPresetDefaults'))
				return {
					signupPresetDefaults: {
						permissions: [],
						maxDevices: 3,
						libraries: [{ id: 'bib', name: 'Biblioteca' }],
					},
				}
			if (query.includes('mutation PublicAccessSetPersonAccess'))
				return {
					setPersonAccess: {
						publicSignin: true,
						maxDevices: 3,
						hiddenLibraries: 0,
						newLibrariesHidden: true,
					},
				}
			if (query.includes('mutation PublicAccessRevokePerson')) return { revokeAccessDevicesOfPerson: 3 }
			if (query.includes('mutation PublicAccessSetPassword')) return { setUserPassword: true }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<PeopleSection />, { wrapper: app.Wrapper })
	return app
}

const personRow = (id: string) => screen.findByTestId(`access-people-${id}`)

describe('People: who may come in from the internet', () => {
	beforeEach(() => {
		vi.clearAllMocks()
	})

	it('shows each friend’s numbers and who is on the internet, and nothing of the kind for the owner', async () => {
		renderPeople()

		const ana = await personRow('ana')
		expect(within(ana).getByText(K('people.publicSignin.badge'))).toBeInTheDocument()
		expect(within(ana).getByTestId('access-people-ana-numbers')).toHaveTextContent(
			`${K('people.numbers')} active=2 max=3 sessions=4 hidden=3`,
		)
		expect(within(ana).getByRole('switch', { name: K('people.publicSignin.label') })).toBeChecked()
		expect(within(await personRow('eva')).getByRole('switch')).not.toBeChecked()

		// the owner never signs in from the internet: no switch, no numbers, no devices
		const owner = await personRow('o')
		expect(within(owner).queryByRole('switch')).toBeNull()
		expect(within(owner).queryByTestId('access-people-o-numbers')).toBeNull()
		expect(within(owner).queryByRole('group')).toBeNull()
	})

	it('names, as chips, what keeps an account out of the internet', async () => {
		renderPeople()

		const chips = await screen.findByTestId('access-people-dani-beyond')
		expect(within(chips).getByText('Upload file')).toBeInTheDocument()
		expect(within(chips).getByText('Scan library')).toBeInTheDocument()
		expect(screen.queryByTestId('access-people-ana-beyond')).toBeNull()
	})

	it('refuses to turn it on for an account that holds more than a friend may, naming what to remove', async () => {
		const { execute } = renderPeople()
		const user = userEvent.setup()

		await user.click(within(await personRow('dani')).getByRole('switch'))

		const dialog = await screen.findByRole('dialog')
		expect(within(dialog).getByTestId('access-beyond')).toHaveTextContent(
			`${K('people.publicSignin.beyond')} name=dani permissions=Upload file, Scan library`,
		)
		// there is no way to go on from here but to the account's page
		expect(within(dialog).queryByRole('button', { name: K('people.publicSignin.turnOn') })).toBeNull()
		expect(within(dialog).getByRole('link', { name: K('people.manage') })).toHaveAttribute(
			'href',
			'/settings/users/dani/manage',
		)
		expect(sent(execute, 'PublicAccessSetPersonAccess')).toEqual([])
	})

	it('turns it on, narrowed to the default library, and goes on to set a new password', async () => {
		const { execute } = renderPeople()
		const user = userEvent.setup()

		await user.click(within(await personRow('eva')).getByRole('switch'))
		const dialog = await screen.findByRole('dialog')
		expect(within(dialog).getByLabelText(K('people.publicSignin.libraries'))).toHaveValue('only')
		await user.click(within(dialog).getByRole('button', { name: K('people.publicSignin.turnOn') }))

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'eva', publicSignin: true, onlyLibraryIds: ['bib'] } },
			]),
		)
		expect(toast.success).toHaveBeenCalledWith(`${K('people.publicSignin.turnedOn')} name=eva`)
		// recommended for an account that existed before: the dialog that sets a password opens
		expect(await screen.findByLabelText(K('people.password.label'))).toBeInTheDocument()
	})

	it('can leave what the person sees as it is, and skip the new password', async () => {
		const { execute } = renderPeople()
		const user = userEvent.setup()

		await user.click(within(await personRow('eva')).getByRole('switch'))
		const dialog = await screen.findByRole('dialog')
		await user.selectOptions(within(dialog).getByLabelText(K('people.publicSignin.libraries')), 'leave')
		await user.click(within(dialog).getByRole('checkbox', { name: K('people.publicSignin.newPassword') }))
		await user.click(within(dialog).getByRole('button', { name: K('people.publicSignin.turnOn') }))

		await waitFor(() =>
			expect(sent(execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'eva', publicSignin: true, onlyLibraryIds: undefined } },
			]),
		)
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
		expect(screen.queryByLabelText(K('people.password.label'))).toBeNull()
	})

	it('turns it off, and removes their devices only when asked', async () => {
		const { execute } = renderPeople()
		const user = userEvent.setup()

		await user.click(within(await personRow('ana')).getByRole('switch'))
		let dialog = await screen.findByRole('dialog')
		await user.click(within(dialog).getByRole('button', { name: K('people.publicSignin.turnOff') }))
		await waitFor(() =>
			expect(sent(execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'ana', publicSignin: false } },
			]),
		)
		expect(sent(execute, 'PublicAccessRevokePerson')).toEqual([])
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())

		await user.click(within(await personRow('max')).getByRole('switch'))
		dialog = await screen.findByRole('dialog')
		await user.click(
			within(dialog).getByRole('switch', { name: `${K('people.publicSignin.removeDevices')} count=3` }),
		)
		await user.click(within(dialog).getByRole('button', { name: K('people.publicSignin.turnOff') }))
		await waitFor(() => expect(sent(execute, 'PublicAccessRevokePerson')).toEqual([{ personId: 'max' }]))
	})

	it('changes the number of devices a person may have, one at a time, between 1 and 10', async () => {
		const { execute } = renderPeople()
		const user = userEvent.setup()

		const ana = await personRow('ana')
		await user.click(within(ana).getByRole('button', { name: K('people.devices.more') }))
		await waitFor(() =>
			expect(sent(execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'ana', maxDevices: 4 } },
			]),
		)
		await user.click(within(ana).getByRole('button', { name: K('people.devices.fewer') }))
		await waitFor(() =>
			expect(sent(execute, 'PublicAccessSetPersonAccess')).toContainEqual({
				input: { personId: 'ana', maxDevices: 2 },
			}),
		)

		// at the ends the button that would go beyond is off
		expect(
			within(await personRow('eva')).getByRole('button', { name: K('people.devices.fewer') }),
		).toBeDisabled()
		expect(
			within(await personRow('max')).getByRole('button', { name: K('people.devices.more') }),
		).toBeDisabled()
	})
})
