import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Route } from 'react-router'

import JoinScene from '../JoinScene'
import { forgetJoinToken, joinToken, resetJoinTokenForTests } from '../joinToken'
import { type Answer, appAround, fakeServer, stubLocation, USER, Where } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))

const PUBLIC = { isClaimed: true, isPublic: true }
const HOME = { isClaimed: true }
const PASSWORD = 'tulip-Harbor-47'

function renderScene({
	claim = PUBLIC,
	join = { status: 200, data: USER } as Answer,
}: { claim?: unknown; join?: Answer } = {}) {
	const server = fakeServer({
		'GET /claim': { data: claim },
		'POST /auth/join': join,
	})
	const App = appAround(
		server.sdk,
		'/join',
		<>
			<Route path="/join" element={<JoinScene />} />
			<Route path="*" element={<Where />} />
		</>,
	)
	render(<App />)
	return server
}

const type = async (username: string, password: string, again = password) => {
	const user = userEvent.setup()
	await user.type(await screen.findByLabelText('authScene.join.username'), username)
	await user.type(screen.getByLabelText('authScene.join.password'), password)
	await user.type(screen.getByLabelText('authScene.join.passwordAgain'), again)
	await user.click(screen.getByTestId('joinButton'))
}

describe('the join screen', () => {
	let location: ReturnType<typeof stubLocation>
	let replaceState: ReturnType<typeof vi.spyOn>

	beforeEach(() => {
		vi.clearAllMocks()
		resetJoinTokenForTests()
		// the address as the invite link leaves it: `/join#<token>`
		location = stubLocation('/join', '#TOKEN-abc_123')
		replaceState = vi.spyOn(window.history, 'replaceState').mockImplementation(() => {})
	})
	afterEach(() => {
		location.restore()
		replaceState.mockRestore()
		forgetJoinToken()
	})

	it('takes the token out of the address before it asks the server anything', async () => {
		const server = renderScene()

		await screen.findByLabelText('authScene.join.username')

		expect(replaceState).toHaveBeenCalledOnce()
		expect(String(replaceState.mock.calls[0][2])).toBe('/join')
		expect(String(replaceState.mock.calls[0][2])).not.toContain('TOKEN')
		expect(server.requests.length).toBeGreaterThan(0)
		expect(server.requests.some((request) => JSON.stringify(request).includes('TOKEN'))).toBe(false)
	})

	it('is the app’s form: a name, a password twice, the rules, one button, a way back', async () => {
		renderScene()

		await screen.findByLabelText('authScene.join.username')

		expect(document.querySelectorAll('input')).toHaveLength(3)
		expect(screen.getByLabelText('authScene.join.password')).toHaveAttribute('type', 'password')
		expect(screen.getByLabelText('authScene.join.passwordAgain')).toHaveAttribute(
			'type',
			'password',
		)
		expect(screen.getByText('authScene.join.rules')).toBeInTheDocument()
		expect(screen.getByTestId('joinButton')).toHaveTextContent('authScene.join.button')
		expect(screen.getByText('authScene.join.haveAccount').closest('a')).toHaveAttribute(
			'href',
			'/auth',
		)
		// no box to paste a link into
		expect(document.body.textContent?.toLowerCase()).not.toContain('paste')
	})

	it('makes the account and loads the app again, as a signed-in one, with the token spent', async () => {
		const server = renderScene()

		await type('Ana', PASSWORD)

		await waitFor(() => expect(location.assign).toHaveBeenCalledExactlyOnceWith('/'))
		const join = server.requests.find((request) => request.key === 'POST /auth/join')
		// the three things the server needs, and nothing else; the token is the one from the address
		expect(join?.body).toEqual({ password: PASSWORD, token: 'TOKEN-abc_123', username: 'Ana' })
		expect(join?.query).toBe('')
		expect(joinToken()).toBeNull()
	})

	it('checks the two passwords against each other, and the lengths, before it asks the server', async () => {
		const server = renderScene()

		await type('ana', PASSWORD, 'tulip-Harbor-48')
		expect(await screen.findByText('authScene.join.mismatch')).toBeInTheDocument()

		await userEvent.setup().clear(screen.getByLabelText('authScene.join.username'))
		await userEvent.setup().type(screen.getByLabelText('authScene.join.username'), 'a')
		await userEvent.setup().click(screen.getByTestId('joinButton'))
		expect((await screen.findAllByText('authScene.join.rules')).length).toBeGreaterThan(1)

		expect(server.requests.filter((request) => request.key === 'POST /auth/join')).toHaveLength(0)
		expect(location.assign).not.toHaveBeenCalled()
	})

	it('says a link that does not work does not, and takes the form away', async () => {
		renderScene({
			join: { status: 404, data: { status: 404, message: 'This link doesn’t work.' } },
		})

		await type('ana', PASSWORD)

		expect(await screen.findByTestId('joinProblem')).toHaveTextContent('authScene.join.badLink')
		expect(screen.queryByTestId('joinButton')).toBeNull()
		expect(location.assign).not.toHaveBeenCalled()
	})

	it('tells a rule that was broken as the server words it, and lets the person try again', async () => {
		renderScene({
			join: { status: 422, data: { status: 422, message: 'That username is taken' } },
		})

		await type('ana', PASSWORD)

		expect(await screen.findByTestId('joinProblem')).toHaveTextContent('That username is taken')
		expect(screen.getByTestId('joinButton')).toBeEnabled()
		expect(location.assign).not.toHaveBeenCalled()
	})

	it('sends somebody whose account exists but whose session could not start to sign in', async () => {
		renderScene({ join: { status: 409, data: { status: 409, message: 'Your account is ready.' } } })

		await type('ana', PASSWORD)

		expect(await screen.findByTestId('joinProblem')).toHaveTextContent(
			'authScene.join.accountReady',
		)
		expect(screen.queryByTestId('joinButton')).toBeNull()
		expect(screen.getByText('authScene.join.haveAccount').closest('a')).toHaveAttribute(
			'href',
			'/auth',
		)
	})

	it.each([
		[429, 'authScene.public.tooMany'],
		[500, 'authScene.join.generic'],
	])('says briefly what a %i answer means', async (status, text) => {
		renderScene({ join: { status, data: { status, message: 'x' } } })

		await type('ana', PASSWORD)

		expect(await screen.findByTestId('joinProblem')).toHaveTextContent(text)
	})

	it('has no form when the address carried no token', async () => {
		resetJoinTokenForTests()
		location.restore()
		location = stubLocation('/join', '')
		renderScene()

		expect(await screen.findByTestId('joinProblem')).toHaveTextContent('authScene.join.badLink')
		expect(screen.queryByTestId('joinButton')).toBeNull()
	})

	it('is for the public address only: anywhere else it goes to the sign-in screen', async () => {
		renderScene({ claim: HOME })

		await waitFor(() => expect(screen.getByTestId('where')).toHaveTextContent('/auth'))
		expect(screen.queryByTestId('joinButton')).toBeNull()
	})
})
