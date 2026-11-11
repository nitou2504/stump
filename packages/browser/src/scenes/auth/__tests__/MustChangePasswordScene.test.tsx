import { SDKContext, StumpClientContext } from '@stump/client'
import { Api } from '@stump/sdk'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PropsWithChildren } from 'react'
import { MemoryRouter, Route, Routes } from 'react-router'

import MustChangePasswordScene from '../MustChangePasswordScene'

const store = vi.hoisted(() => ({
	user: null as unknown,
	setUser: vi.fn(),
}))

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/stores', () => ({
	useUserStore: (selector: (state: { user: unknown; setUser: unknown }) => unknown) =>
		selector({ user: store.user, setUser: store.setUser }),
}))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

import { toast } from 'sonner'

const K = (key: string) => `authScene.mustChangePassword.${key}`

const USER = {
	id: 'ana',
	username: 'ana',
	permissions: ['CHANGE_PASSWORD'],
	ageRestriction: null,
	maxSessionsAllowed: 3,
	mustChangePassword: true,
}

function renderScene(
	answer: () => unknown = () => ({ updateViewer: { id: 'ana', username: 'ana' } }),
) {
	const sdk = new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' })
	const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } })
	const execute = vi.spyOn(sdk, 'execute').mockImplementation(async () => answer() as never)
	// `sdk.auth` makes a new object each time it is read, so what the scene reads is replaced
	const logout = vi.fn().mockResolvedValue(undefined)
	vi.spyOn(sdk, 'auth', 'get').mockReturnValue({ logout } as never)
	const clear = vi.spyOn(client, 'clear')

	function Wrapper({ children }: PropsWithChildren) {
		return (
			<SDKContext.Provider value={{ sdk, setSDK: vi.fn() }}>
				<StumpClientContext.Provider value={{}}>
					<QueryClientProvider client={client}>
						<MemoryRouter initialEntries={['/']}>
							<Routes>
								<Route path="/" element={children} />
								<Route path="/auth" element={<div data-testid="login" />} />
							</Routes>
						</MemoryRouter>
					</QueryClientProvider>
				</StumpClientContext.Provider>
			</SDKContext.Provider>
		)
	}

	render(<MustChangePasswordScene />, { wrapper: Wrapper })
	return { execute, logout, clear }
}

const field = (key: string) => screen.getByLabelText(K(key))
const submit = () => screen.getByRole('button', { name: K('submit') })

describe('MustChangePasswordScene', () => {
	beforeEach(() => {
		vi.clearAllMocks()
		store.user = USER
	})

	it('asks for the new password twice and takes nothing under 8 characters', async () => {
		renderScene()
		const user = userEvent.setup()

		expect(submit()).toBeDisabled()

		await user.type(field('newPassword'), 'short')
		expect(screen.getByText(`${K('tooShort')} min=8`)).toBeInTheDocument()
		expect(submit()).toBeDisabled()

		// long enough, but not repeated yet
		await user.type(field('newPassword'), 'er-than-eight')
		expect(screen.queryByText(`${K('tooShort')} min=8`)).toBeNull()
		expect(submit()).toBeDisabled()

		await user.type(field('confirmPassword'), 'different')
		expect(screen.getByText(K('mismatch'))).toBeInTheDocument()
		expect(submit()).toBeDisabled()

		await user.clear(field('confirmPassword'))
		await user.type(field('confirmPassword'), 'shorter-than-eight')
		expect(screen.queryByText(K('mismatch'))).toBeNull()
		expect(submit()).toBeEnabled()
	})

	it("saves it with the profile form's input, so nothing else about the account changes, and goes on", async () => {
		const { execute, clear } = renderScene()
		const user = userEvent.setup()

		await user.type(field('newPassword'), 'a-new-password')
		await user.type(field('confirmPassword'), 'a-new-password')
		await user.click(submit())

		await waitFor(() => expect(execute).toHaveBeenCalledTimes(1))
		expect(execute.mock.calls[0]![1]).toEqual({
			input: {
				username: 'ana',
				password: 'a-new-password',
				permissions: ['CHANGE_PASSWORD'],
				ageRestriction: null,
				maxSessionsAllowed: 3,
			},
		})
		await waitFor(() =>
			expect(store.setUser).toHaveBeenCalledWith({ ...USER, mustChangePassword: false }),
		)
		expect(toast.success).toHaveBeenCalledWith(K('success'))
		// what was asked while the password was the given one was refused: start afresh
		expect(clear).toHaveBeenCalled()
	})

	it("says what the server says when it won't take it, and stays where it is", async () => {
		renderScene(() => {
			throw new Error('Choose a password that is different from the one you were given')
		})
		const user = userEvent.setup()

		await user.type(field('newPassword'), 'a-new-password')
		await user.type(field('confirmPassword'), 'a-new-password')
		await user.click(submit())

		expect(
			await screen.findByText('Choose a password that is different from the one you were given'),
		).toBeInTheDocument()
		expect(store.setUser).not.toHaveBeenCalled()
	})

	it('can sign out instead', async () => {
		const { logout, clear } = renderScene()
		const user = userEvent.setup()

		await user.click(screen.getByRole('button', { name: K('signOut') }))

		await waitFor(() => expect(screen.getByTestId('login')).toBeInTheDocument())
		expect(logout).toHaveBeenCalled()
		expect(clear).toHaveBeenCalled()
		expect(store.setUser).toHaveBeenCalledWith(null)
	})

	it('is nothing when there is no user to ask', () => {
		store.user = null
		renderScene()

		expect(screen.queryByRole('button', { name: K('submit') })).toBeNull()
	})
})
