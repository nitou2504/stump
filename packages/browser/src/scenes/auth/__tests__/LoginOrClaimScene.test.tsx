import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Route } from 'react-router'

import LoginOrClaimScene from '../LoginOrClaimScene'
import { type Answer, appAround, fakeServer, stubLocation, USER, Where } from './support'

const stores = vi.hoisted(() => ({ setUser: vi.fn(), platform: 'browser' }))

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/stores', () => ({
	useUserStore: (selector: (state: { setUser: unknown }) => unknown) =>
		selector({ setUser: stores.setUser }),
	useAppStore: (selector: (state: { platform: string }) => unknown) =>
		selector({ platform: stores.platform }),
}))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

import { toast } from 'sonner'

const OIDC_OFF = { enabled: false, allowRegistration: false, disableLocalAuth: false }

/** What the public edge's stubs say, and what the app says at home */
const PUBLIC = { isClaimed: true, isPublic: true }
const HOME = { isClaimed: true }

function renderScene({
	claim = PUBLIC,
	oidc = OIDC_OFF,
	login = { status: 200, data: USER } as Answer,
	entry = '/auth',
}: { claim?: unknown; oidc?: unknown; login?: Answer; entry?: string } = {}) {
	const server = fakeServer({
		'GET /claim': { data: claim },
		'GET /auth/oidc/config': { data: oidc },
		'POST /auth/login': login,
	})
	const App = appAround(
		server.sdk,
		entry,
		<>
			<Route path="/auth" element={<LoginOrClaimScene />} />
			<Route path="*" element={<Where />} />
		</>,
	)
	render(<App />)
	return server
}

const fill = async (username = 'ana', password = 'tulip-Harbor-47') => {
	const user = userEvent.setup()
	await user.type(await screen.findByLabelText('authScene.form.labels.username'), username)
	await user.type(screen.getByLabelText('authScene.form.labels.password'), password)
	await user.click(screen.getByTestId('loginOrRegisterButton'))
}

describe('the sign-in screen on the public address', () => {
	let location: ReturnType<typeof stubLocation>

	beforeEach(() => {
		vi.clearAllMocks()
		stores.platform = 'browser'
		location = stubLocation()
	})
	afterEach(() => location.restore())

	it('is the app’s own screen with the short version: two fields, a button and one line', async () => {
		renderScene()

		await screen.findByLabelText('authScene.form.labels.username')

		expect(document.querySelectorAll('input')).toHaveLength(2)
		expect(screen.getByLabelText('authScene.form.labels.password')).toHaveAttribute(
			'type',
			'password',
		)
		expect(screen.getByTestId('loginOrRegisterButton')).toBeInTheDocument()
		expect(screen.getByText('authScene.public.forgot')).toBeInTheDocument()
		// nothing about invite links (people who hold one just open it)
		expect(document.body.textContent?.toLowerCase()).not.toContain('invite')
		expect(document.querySelector('input[name="t"]')).toBeNull()
		expect(document.querySelector('a[href*="_i"]')).toBeNull()
	})

	it('hides what a visitor must never see: the claim form, the identity provider, the server list', async () => {
		stores.platform = 'desktop'
		renderScene({
			claim: { isClaimed: false, isPublic: true },
			oidc: { enabled: true, allowRegistration: true, disableLocalAuth: false },
		})

		await screen.findByLabelText('authScene.form.labels.username')

		expect(screen.queryByText('authScene.claimHeading')).toBeNull()
		expect(screen.queryByText('authScene.form.buttons.loginWithOidc')).toBeNull()
		expect(screen.queryByText('authScene.form.buttons.createAccountWithOidc')).toBeNull()
		expect(screen.queryByText('shared.common.goToServers')).toBeNull()
	})

	it.each([
		[401, { status: 401, message: 'Unauthorized' }, 'authScene.public.wrongCredentials'],
		[
			429,
			{ status: 429, message: 'Too many attempts. Wait a few minutes and try again.' },
			'authScene.public.tooMany',
		],
		[
			403,
			{
				status: 403,
				message:
					'You already use 3 devices. Remove one in Settings → My devices, or ask the owner.',
			},
			'You already use 3 devices. Remove one in Settings → My devices, or ask the owner.',
		],
		[403, undefined, 'authScene.public.deviceLimit'],
		[500, { status: 500, message: 'boom' }, 'authScene.public.generic'],
		[404, '', 'authScene.public.generic'],
	])('says why a %i answer failed, in the form', async (status, data, text) => {
		renderScene({ login: { status, data } })

		await fill()

		expect(await screen.findByRole('alert')).toHaveTextContent(text)
		// no toast, no in-app navigation, no page load
		expect(toast.error).not.toHaveBeenCalled()
		expect(stores.setUser).not.toHaveBeenCalled()
		expect(location.assign).not.toHaveBeenCalled()
	})

	it('signs in with a full page load that goes where the person was going', async () => {
		const server = renderScene({ entry: '/auth?redirect=%2Fbooks%2Fabc' })

		await fill()

		await waitFor(() => expect(location.assign).toHaveBeenCalledExactlyOnceWith('/books/abc'))
		// what was sent: the two fields, no query
		const login = server.requests.find((request) => request.key === 'POST /auth/login')
		expect(login?.body).toEqual({ password: 'tulip-Harbor-47', username: 'ana' })
		expect(login?.query).toBe('')
		// nothing signed-in ran under the page the visitor was given
		expect(stores.setUser).not.toHaveBeenCalled()
		expect(screen.queryByTestId('where')).toBeNull()
	})

	it.each([
		'//evil.example',
		'/\\evil.example',
		'/\t/evil.example',
		' //evil.example',
		'https://evil.example/api',
		'javascript:alert(1)',
		'/auth?redirect=%2Fx',
		'/join#abc',
	])('never goes anywhere but home for the redirect %j', async (redirect) => {
		renderScene({ entry: `/auth?redirect=${encodeURIComponent(redirect)}` })

		await fill()

		await waitFor(() => expect(location.assign).toHaveBeenCalledExactlyOnceWith('/'))
	})

	it('goes home when nothing says where', async () => {
		renderScene()

		await fill()

		await waitFor(() => expect(location.assign).toHaveBeenCalledExactlyOnceWith('/'))
	})
})

describe('the sign-in screen at home', () => {
	let location: ReturnType<typeof stubLocation>

	beforeEach(() => {
		vi.clearAllMocks()
		// the screen logs a failed sign-in, as it always did
		vi.spyOn(console, 'error').mockImplementation(() => {})
		stores.platform = 'browser'
		location = stubLocation()
	})
	afterEach(() => {
		location.restore()
		vi.restoreAllMocks()
	})

	it('is the same screen as before: the toast, no public line', async () => {
		renderScene({
			claim: HOME,
			login: { status: 401, data: { status: 401, message: 'Unauthorized' } },
		})

		await fill()

		await waitFor(() => expect(toast.error).toHaveBeenCalledWith('authScene.toasts.loginFailed'))
		expect(screen.queryByText('authScene.public.forgot')).toBeNull()
		expect(screen.queryByRole('alert')).toBeNull()
	})

	it('shows the server’s message for a locked account, as the API words it (an object, not a string)', async () => {
		renderScene({
			claim: HOME,
			login: { status: 403, data: { status: 403, message: 'This account is locked' } },
		})

		await fill()

		expect(await screen.findByText('This account is locked')).toBeInTheDocument()
	})

	it('also reads a bare string from an older server', async () => {
		renderScene({ claim: HOME, login: { status: 403, data: 'Account is locked' } })

		await fill()

		expect(await screen.findByText('Account is locked')).toBeInTheDocument()
	})

	it('signs in within the app: the user is kept and the router goes on (no page load)', async () => {
		renderScene({ claim: HOME, entry: '/auth?redirect=%2Fbooks%2Fabc' })

		await fill()

		await waitFor(() => expect(screen.getByTestId('where')).toHaveTextContent('/books/abc'))
		expect(stores.setUser).toHaveBeenCalledOnce()
		expect(location.assign).not.toHaveBeenCalled()
	})

	it('still only follows a path of this site', async () => {
		renderScene({ claim: HOME, entry: '/auth?redirect=%2F%2Fevil.example' })

		await fill()

		await waitFor(() => expect(screen.getByTestId('where')).toHaveTextContent('/'))
		expect(screen.getByTestId('where')).not.toHaveTextContent('evil')
	})
})
