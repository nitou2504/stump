import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import InviteDialog from '../InviteDialog'
import { K, overview, serverOf } from './fixtures'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('../copy', () => ({ copyText: vi.fn().mockResolvedValue(true) }))
vi.mock('sonner', () => ({ toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn() }) }))

import { toast } from 'sonner'

import { copyText } from '../copy'

const URL = 'https://books.example.org/api/_i/join-token'

function renderInvite(
	data: Parameters<typeof serverOf>[0] = {},
	answers: (query: string, variables: Record<string, unknown> | undefined) => unknown = () =>
		undefined,
	prefill: Parameters<typeof InviteDialog>[0]['prefill'] = null,
) {
	const server = serverOf(data)
	const app = setup({
		answers: (query, variables) => {
			if (query.includes('mutation PublicAccessCreateSignupLink'))
				return { createAccessSignupLink: { url: URL, link: { id: 'l1' } } }
			const own = answers(query, variables)
			return own === undefined ? server(query, variables) : own
		},
	})
	const onClose = vi.fn()
	render(<InviteDialog open onClose={onClose} prefill={prefill} />, { wrapper: app.Wrapper })
	return { app, onClose }
}

const PRESET = {
	permissions: ['CHANGE_PASSWORD', 'CHANGE_AVATAR', 'DOWNLOAD_FILE', 'ADD_BOOKS'],
	libraryIds: ['bib'],
	ageRestriction: null,
	maxDevices: 3,
}

const create = (user: ReturnType<typeof userEvent.setup>) =>
	user.click(screen.getByRole('button', { name: K('invite.create') }))

describe('Invite someone', () => {
	beforeEach(() => vi.clearAllMocks())

	it('starts with one question, one person for three days, and says what they get', async () => {
		renderInvite()

		const form = await screen.findByTestId('access-invite-form')
		expect(within(form).getByLabelText(K('invite.who'))).toHaveValue('')
		expect(within(form).getByRole('button', { name: K('invite.one') })).toHaveAttribute(
			'aria-pressed',
			'true',
		)
		expect(within(form).getByLabelText(K('invite.worksFor'))).toHaveValue('72')
		expect(within(form).queryByLabelText(K('invite.howMany'))).toBeNull()
		const summary = within(form).getByTestId('access-preset-summary')
		expect(summary).toHaveTextContent('Biblioteca')
		expect(summary).toHaveTextContent(`${K('signups.summary.devices')} count=3`)
	})

	it('asks who it is for, and sends nothing without it', async () => {
		const { app } = renderInvite()
		const user = userEvent.setup()

		await screen.findByTestId('access-invite-form')
		await create(user)

		expect(screen.getByText(K('invite.whoMissing'))).toBeInTheDocument()
		expect(sent(app.execute, 'PublicAccessCreateSignupLink')).toEqual([])
	})

	it('makes a link for one person with the preset it showed, and shows it once', async () => {
		const { app } = renderInvite()
		const user = userEvent.setup()

		await user.type(await screen.findByLabelText(K('invite.who')), 'Tía Rosa')
		await create(user)

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessCreateSignupLink')).toEqual([
				{
					input: {
						label: 'Tía Rosa',
						kind: 'SINGLE',
						validForHours: 72,
						maxUses: 1,
						preset: PRESET,
					},
				},
			]),
		)
		const made = await screen.findByTestId('access-made-link')
		expect(made).toHaveTextContent(`${K('invite.sendTo')} note=Tía Rosa`)
		expect(within(made).getByLabelText(K('invite.linkLabel'))).toHaveValue(URL)
		expect(made).toHaveTextContent(K('invite.onlyNow'))
		// the form is gone: the address is on screen only here
		expect(screen.queryByTestId('access-invite-form')).toBeNull()
		expect(screen.queryByTestId('access-qr')).toBeNull()
	})

	it('copies the link, shows its QR code on request, and forgets it when closed', async () => {
		const { onClose } = renderInvite()
		const user = userEvent.setup()

		await user.type(await screen.findByLabelText(K('invite.who')), 'Rosa')
		await create(user)
		const made = await screen.findByTestId('access-made-link')

		await user.click(within(made).getByRole('button', { name: K('invite.copy') }))
		expect(copyText).toHaveBeenCalledWith(URL)
		expect(toast.success).toHaveBeenCalledWith(K('invite.copied'))
		await user.click(within(made).getByRole('button', { name: K('invite.showQr') }))
		expect(await screen.findByTestId('access-qr')).toBeInTheDocument()
		await user.click(within(made).getByRole('button', { name: K('invite.done') }))

		expect(onClose).toHaveBeenCalledOnce()
		expect(screen.queryByDisplayValue(URL)).toBeNull()
	})

	it('says when the link could not be copied, and leaves it selected to copy by hand', async () => {
		vi.mocked(copyText).mockResolvedValueOnce(false)
		renderInvite()
		const user = userEvent.setup()

		await user.type(await screen.findByLabelText(K('invite.who')), 'Rosa')
		await create(user)
		await user.click(
			within(await screen.findByTestId('access-made-link')).getByRole('button', {
				name: K('invite.copy'),
			}),
		)

		expect(toast.error).toHaveBeenCalledWith(K('invite.copyFailed'))
	})

	it('makes an open link with a budget, and warns about an open link that lets people add books', async () => {
		const { app } = renderInvite()
		const user = userEvent.setup()

		await user.type(await screen.findByLabelText(K('invite.who')), 'Club de lectura')
		await user.click(screen.getByRole('button', { name: K('invite.several') }))
		expect(screen.getByLabelText(K('invite.howMany'))).toHaveValue('5')
		expect(screen.getByTestId('access-open-addbooks')).toBeInTheDocument()
		await user.selectOptions(screen.getByLabelText(K('invite.howMany')), '10')
		await user.selectOptions(screen.getByLabelText(K('invite.worksFor')), '168')
		await create(user)

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessCreateSignupLink')).toEqual([
				{
					input: {
						label: 'Club de lectura',
						kind: 'OPEN',
						validForHours: 168,
						maxUses: 10,
						preset: PRESET,
					},
				},
			]),
		)
	})

	it('lets the owner change what the account gets, and offers only what a friend may have', async () => {
		const { app } = renderInvite()
		const user = userEvent.setup()

		await user.type(await screen.findByLabelText(K('invite.who')), 'Rosa')
		await user.click(screen.getByRole('button', { name: K('signups.form.change') }))
		const editor = await screen.findByTestId('access-preset-editor')
		// the six a friend may have, and none that scans, uploads files or manages
		expect(within(editor).getAllByRole('checkbox', { name: /signups\.permission\./ })).toHaveLength(
			6,
		)
		expect(editor.textContent).not.toMatch(/SCAN_LIBRARY|UPLOAD_FILE|MANAGE/i)
		await user.click(
			within(editor).getByRole('checkbox', { name: K('signups.permission.addBooks') }),
		)
		await user.selectOptions(within(editor).getByLabelText(K('signups.editor.devices')), '5')
		await create(user)

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessCreateSignupLink')[0]).toMatchObject({
				input: {
					preset: {
						permissions: ['CHANGE_PASSWORD', 'CHANGE_AVATAR', 'DOWNLOAD_FILE'],
						maxDevices: 5,
					},
				},
			}),
		)
	})

	it('cannot make a link for no library, or with an age that is not possible', async () => {
		renderInvite()
		const user = userEvent.setup()

		await user.type(await screen.findByLabelText(K('invite.who')), 'Rosa')
		await user.click(screen.getByRole('button', { name: K('signups.form.change') }))
		await user.click(await screen.findByRole('checkbox', { name: 'Biblioteca' }))
		expect(screen.getByText(K('signups.editor.noLibraries'))).toBeInTheDocument()
		expect(screen.getByRole('button', { name: K('invite.create') })).toBeDisabled()

		await user.click(screen.getByRole('checkbox', { name: 'Biblioteca' }))
		await user.type(screen.getByLabelText(K('signups.editor.age')), '0')
		expect(screen.getByText(K('signups.editor.ageInvalid'))).toBeInTheDocument()
		expect(screen.getByRole('button', { name: K('invite.create') })).toBeDisabled()
	})

	it('starts from the values of a link that is nearly used up', async () => {
		const { app } = renderInvite({}, () => undefined, {
			label: 'club',
			kind: 'OPEN' as never,
			maxUses: 10,
			permissions: ['CHANGE_PASSWORD'],
			libraryIds: ['bib', 'manga'],
			maxDevices: 2,
		})
		const user = userEvent.setup()

		expect(await screen.findByLabelText(K('invite.who'))).toHaveValue('club')
		expect(screen.getByLabelText(K('invite.howMany'))).toHaveValue('10')
		await create(user)

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessCreateSignupLink')[0]).toMatchObject({
				input: {
					label: 'club',
					kind: 'OPEN',
					maxUses: 10,
					preset: { permissions: ['CHANGE_PASSWORD'], libraryIds: ['bib', 'manga'], maxDevices: 2 },
				},
			}),
		)
	})

	it('says why it cannot make a link while public sign-in is off on the server, and shows no form', async () => {
		renderInvite({ overview: overview({ publicSignin: false }) })

		expect(await screen.findByTestId('access-signup-off')).toHaveTextContent(K('invite.signinOff'))
		expect(screen.queryByTestId('access-invite-form')).toBeNull()
	})

	it('cannot make a link while public access is not set up', async () => {
		renderInvite({ overview: overview({ configured: false }) })

		await screen.findByTestId('access-invite-form')
		expect(screen.getByRole('button', { name: K('invite.create') })).toBeDisabled()
	})

	it('says when it could not make the link, and shows none', async () => {
		const { app } = renderInvite({}, (query) => {
			if (query.includes('mutation PublicAccessCreateSignupLink')) throw new Error('no room')
			return undefined
		})
		// the helper above answers the mutation first: make the server refuse it
		app.execute.mockImplementation(async (document: { toString(): string }, variables: unknown) => {
			if (document.toString().includes('mutation PublicAccessCreateSignupLink'))
				throw new Error('no room')
			return serverOf({})(document.toString(), variables as Record<string, unknown>) as never
		})
		const user = userEvent.setup()

		await user.type(await screen.findByLabelText(K('invite.who')), 'Rosa')
		await create(user)

		await waitFor(() =>
			expect(toast.error).toHaveBeenCalledWith(K('invite.error'), { description: 'no room' }),
		)
		expect(screen.queryByTestId('access-made-link')).toBeNull()
	})
})
