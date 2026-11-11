import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import SignInSection from '../SignInSection'
import { page, sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

import { toast } from 'sonner'

const K = (key: string) => `settingsScene.server/access.${key}`

const signIn = (id: number, extra: Record<string, unknown> = {}) => ({
	id,
	at: '2026-11-10T12:00:00Z',
	ip: '203.0.113.9',
	userAgent: 'Mozilla/5.0',
	successful: false,
	deviceId: 'ana-phone-k3f9x',
	deviceLabel: "Ana's phone",
	reason: 'WRONG_PASSWORD',
	cleared: false,
	person: { id: 'ana', username: 'ana', removed: false },
	...extra,
})

const BLOCKED = [
	{
		failuresLast24H: 9,
		lastFailureAt: '2026-11-10T12:00:00Z',
		lockedBy: 'FAILURES',
		person: { id: 'ana', username: 'ana' },
	},
	{
		failuresLast24H: 0,
		lastFailureAt: null,
		lockedBy: 'OWNER',
		person: { id: 'ben', username: 'ben' },
	},
]

function renderSection({ blocked = BLOCKED, signIns = [signIn(1)] } = {}) {
	const app = setup({
		answers: (query, variables) => {
			if (query.includes('query PublicAccessBlocked')) return { accessBlockedUsers: blocked }
			if (query.includes('query PublicAccessSignIns')) {
				const onlyFailures = (variables as { failuresOnly: boolean }).failuresOnly
				return {
					accessSignIns: page(onlyFailures ? signIns.filter((s) => !s.successful) : signIns),
				}
			}
			if (query.includes('mutation PublicAccessUnblock')) return { unblockUser: true }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<SignInSection />, { wrapper: app.Wrapper })
	return app
}

describe('Sign-in protection', () => {
	beforeEach(() => {
		vi.clearAllMocks()
	})

	it('lists who is locked, why, and how many failed', async () => {
		renderSection()

		const ana = await screen.findByTestId('access-blocked-ana')
		expect(within(ana).getByText(K('signIns.lockedBy.failures'))).toBeInTheDocument()
		expect(within(ana).getByText(/signIns\.blockedDetail count=9/)).toBeInTheDocument()
		expect(
			within(screen.getByTestId('access-blocked-ben')).getByText(K('signIns.lockedBy.owner')),
		).toBeInTheDocument()
	})

	it('lets somebody back in with one button, which asks for nothing more', async () => {
		const { execute } = renderSection()
		const user = userEvent.setup()

		const ana = await screen.findByTestId('access-blocked-ana')
		await user.click(within(ana).getByRole('button', { name: K('signIns.unblock') }))

		await waitFor(() => expect(sent(execute, 'PublicAccessUnblock')).toEqual([{ id: 'ana' }]))
		expect(toast.success).toHaveBeenCalledWith(`${K('signIns.unblocked')} name=ana`)
	})

	it('says it could not unblock', async () => {
		const app = setup({
			answers: (query) => {
				if (query.includes('query PublicAccessBlocked')) return { accessBlockedUsers: BLOCKED }
				if (query.includes('query PublicAccessSignIns')) return { accessSignIns: page([]) }
				throw new Error('Not allowed from here')
			},
		})
		render(<SignInSection />, { wrapper: app.Wrapper })
		const user = userEvent.setup()

		await user.click(
			within(await screen.findByTestId('access-blocked-ana')).getByRole('button', {
				name: K('signIns.unblock'),
			}),
		)

		await waitFor(() =>
			expect(toast.error).toHaveBeenCalledWith(K('signIns.errors.unblock'), {
				description: 'Not allowed from here',
			}),
		)
	})

	it("says when nobody is locked out, and that the proxy's own limit clears itself", async () => {
		renderSection({ blocked: [] })

		expect(await screen.findByText(K('signIns.nobodyBlocked'))).toBeInTheDocument()
		expect(screen.getByText(K('signIns.edgeLimitNote'))).toBeInTheDocument()
	})

	it('lists the attempts with their result, reason and device, and dims a failure that was forgiven', async () => {
		renderSection({
			signIns: [
				signIn(1),
				signIn(2, {
					successful: true,
					reason: null,
					person: { id: 'ben', username: 'ben', removed: false },
				}),
				signIn(3, { cleared: true }),
				signIn(4, { reason: 'OWNER_VIA_EDGE', deviceId: null, deviceLabel: null }),
				signIn(5, { reason: 'WRONG_PERSON' }),
				signIn(6, { reason: 'LOCKED' }),
			],
		})

		const first = await screen.findByTestId('access-signin-1')
		expect(within(first).getByText(K('signIns.failed'))).toBeInTheDocument()
		expect(within(first).getByText(K('signIns.reason.wrongPassword'))).toBeInTheDocument()
		expect(within(first).getByText(/device=Ana's phone/)).toBeInTheDocument()

		expect(
			within(screen.getByTestId('access-signin-2')).getByText(K('signIns.ok')),
		).toBeInTheDocument()
		expect(
			within(screen.getByTestId('access-signin-3')).getByText(K('signIns.cleared')),
		).toBeInTheDocument()
		expect(
			within(screen.getByTestId('access-signin-4')).getByText(K('signIns.reason.ownerViaEdge')),
		).toBeInTheDocument()
		expect(
			within(screen.getByTestId('access-signin-5')).getByText(K('signIns.reason.wrongPerson')),
		).toBeInTheDocument()
		expect(
			within(screen.getByTestId('access-signin-6')).getByText(K('signIns.reason.locked')),
		).toBeInTheDocument()
		// a success is not forgiven, and says no reason
		expect(
			within(screen.getByTestId('access-signin-2')).queryByText(K('signIns.cleared')),
		).toBeNull()
	})

	it('shows only the failures when asked, and asks the server for exactly that', async () => {
		const { execute } = renderSection({
			signIns: [signIn(1), signIn(2, { successful: true, reason: null })],
		})
		const user = userEvent.setup()

		await screen.findByTestId('access-signin-2')
		expect(sent(execute, 'PublicAccessSignIns')).toEqual([
			{ failuresOnly: false, page: 1, pageSize: 50 },
		])

		await user.click(screen.getByRole('switch', { name: K('signIns.failuresOnly') }))

		// the list that was shown stays until the filtered one is ready, then only the failures
		await waitFor(() => expect(screen.queryByTestId('access-signin-2')).not.toBeInTheDocument())
		expect(await screen.findByTestId('access-signin-1')).toBeInTheDocument()
		expect(sent(execute, 'PublicAccessSignIns').at(-1)).toEqual({
			failuresOnly: true,
			page: 1,
			pageSize: 50,
		})
	})

	it('goes through the pages, from the first again when the filter changes', async () => {
		const app = setup({
			answers: (query, variables) => {
				if (query.includes('query PublicAccessBlocked')) return { accessBlockedUsers: [] }
				if (query.includes('query PublicAccessSignIns')) {
					const { page: number } = variables as { page: number }
					return { accessSignIns: page([signIn(number)], 3) }
				}
				throw new Error(`Unexpected query ${query}`)
			},
		})
		render(<SignInSection />, { wrapper: app.Wrapper })
		const user = userEvent.setup()

		await screen.findByTestId('access-signin-1')
		await user.click(screen.getByRole('button', { name: K('pager.next') }))
		await screen.findByTestId('access-signin-2')
		expect(screen.getByText(`${K('pager.page')} page=2 total=3`)).toBeInTheDocument()

		await user.click(screen.getByRole('switch', { name: K('signIns.failuresOnly') }))
		await screen.findByTestId('access-signin-1')
		expect(screen.getByText(`${K('pager.page')} page=1 total=3`)).toBeInTheDocument()
	})
})
