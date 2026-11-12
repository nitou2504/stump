import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import LetInDialog from '../LetInDialog'
import { K, row, serverOf } from './fixtures'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/paths', () => ({
	usePaths: () => ({ updateUser: (id: string) => `/settings/users/${id}/manage` }),
}))
vi.mock('sonner', () => ({ toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn() }) }))

const USER = {
	id: 'ben',
	username: 'ben',
	permissions: ['CHANGE_PASSWORD', 'MANAGE_USERS', 'DOWNLOAD_FILE'],
	maxSessionsAllowed: 2,
	ageRestriction: { age: 12, restrictOnUnset: false },
}

function renderDialog(person: ReturnType<typeof row>) {
	const server = serverOf({})
	const app = setup({
		answers: (query, variables) => {
			if (query.includes('query PublicAccessUserToEdit')) return { userById: USER }
			if (query.includes('mutation PublicAccessUpdateUser'))
				return { updateUser: { id: 'ben', permissions: [] } }
			if (query.includes('mutation PublicAccessSetPersonAccess'))
				return {
					setPersonAccess: {
						publicSignin: true,
						maxDevices: 3,
						hiddenLibraries: 0,
						newLibrariesHidden: true,
					},
				}
			return server(query, variables)
		},
	})
	const props = { onClose: vi.fn(), onSetPassword: vi.fn() }
	render(<LetInDialog person={person as never} {...props} />, { wrapper: app.Wrapper })
	return { app, ...props }
}

describe('Letting somebody in from the internet', () => {
	beforeEach(() => vi.clearAllMocks())

	it('says what they will do and see, and lets them in narrowed to the default library', async () => {
		const { app, onSetPassword, onClose } = renderDialog(row('ben', 'ben', { beyondFriendSet: [] }))
		const user = userEvent.setup()

		const dialog = await screen.findByTestId('access-letin')
		expect(dialog).toHaveTextContent(`${K('letIn.title')} name=ben`)
		expect(dialog).toHaveTextContent(`${K('letIn.body')} limit=3`)
		await waitFor(() => expect(dialog).toHaveTextContent(K('letIn.can')))
		expect(dialog).toHaveTextContent(`${K('letIn.sees')} libraries=Biblioteca`)
		await user.click(within(dialog).getByRole('button', { name: K('letIn.letIn') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'ben', publicSignin: true, onlyLibraryIds: ['bib'] } },
			]),
		)
		// a new password is recommended and goes on from here
		expect(onSetPassword).toHaveBeenCalledWith({ id: 'ben', username: 'ben' })
		expect(onClose).toHaveBeenCalled()
		expect(sent(app.execute, 'PublicAccessUpdateUser')).toEqual([])
	})

	it('can leave what they see as it is, and skip the new password', async () => {
		const { app, onSetPassword } = renderDialog(row('ben', 'ben'))
		const user = userEvent.setup()

		const dialog = await screen.findByTestId('access-letin')
		await user.click(within(dialog).getByRole('radio', { name: K('letIn.keepLibraries') }))
		await user.click(within(dialog).getByRole('checkbox', { name: K('letIn.setPassword') }))
		await user.click(within(dialog).getByRole('button', { name: K('letIn.letIn') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'ben', publicSignin: true, onlyLibraryIds: undefined } },
			]),
		)
		expect(onSetPassword).not.toHaveBeenCalled()
	})

	it('does not cut the libraries of somebody who is already in: it starts on leaving them as they are', async () => {
		const { app } = renderDialog(
			row('ben', 'ben', { publicSignin: true, beyondFriendSet: ['MANAGE_USERS'] }),
		)
		const user = userEvent.setup()

		const dialog = await screen.findByTestId('access-letin')
		expect(within(dialog).getByRole('radio', { name: K('letIn.keepLibraries') })).toBeChecked()
		await user.click(within(dialog).getByRole('button', { name: K('letIn.removeAndLetIn') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'ben', publicSignin: true, onlyLibraryIds: undefined } },
			]),
		)
	})

	it('names what keeps an account off the internet, and takes it off before letting them in, leaving the rest of the account alone', async () => {
		const { app } = renderDialog(row('ben', 'ben', { beyondFriendSet: ['MANAGE_USERS'] }))
		const user = userEvent.setup()

		const dialog = await screen.findByTestId('access-letin')
		expect(within(dialog).getByTestId('access-letin-extras')).toHaveTextContent(
			`${K('letIn.extras')} name=ben extras=userPermissions.MANAGE_USERS.label`,
		)
		expect(within(dialog).getByRole('link', { name: K('person.editAccount') })).toHaveAttribute(
			'href',
			'/settings/users/ben/manage',
		)
		await user.click(within(dialog).getByRole('button', { name: K('letIn.removeAndLetIn') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessUpdateUser')).toEqual([
				{
					id: 'ben',
					input: {
						username: 'ben',
						permissions: ['CHANGE_PASSWORD', 'DOWNLOAD_FILE'],
						maxSessionsAllowed: 2,
						ageRestriction: { age: 12, restrictOnUnset: false },
					},
				},
			]),
		)
		await waitFor(() => expect(sent(app.execute, 'PublicAccessSetPersonAccess')).toHaveLength(1))
	})
})
