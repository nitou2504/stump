import { render, screen, within } from '@testing-library/react'

import StatusStrip from '../StatusStrip'
import { setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))

const K = (key: string) => `settingsScene.server/access.${key}`

const overview = (extra: Record<string, unknown> = {}) => ({
	hasKey: true,
	hasPublicUrl: true,
	hasStateDir: true,
	configured: true,
	requirePass: true,
	publicUrl: 'https://g3.test:8443',
	stateWrittenAt: '2026-11-10T12:00:00Z',
	activeDevices: 4,
	openInvites: 2,
	publicSignin: true,
	activeSignupLinks: 1,
	signinStats: { failuresLast10Min: 0, namesWaiting: 0, globalBudgetTripped: false },
	...extra,
})

function renderStrip(answer: ReturnType<typeof overview>) {
	const app = setup({
		answers: (query) => {
			if (query.includes('query PublicAccessOverview')) return { accessOverview: answer }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<StatusStrip />, { wrapper: app.Wrapper })
}

describe('Public access status', () => {
	it('says it is ready, with the address, the counts and when the proxy was last told', async () => {
		renderStrip(overview())

		const status = await screen.findByTestId('access-status')
		expect(within(status).getByText(K('status.ready'))).toBeInTheDocument()
		expect(within(status).getByText('https://g3.test:8443')).toBeInTheDocument()
		expect(within(status).getByText('4')).toBeInTheDocument()
		expect(within(status).getByText('2')).toBeInTheDocument()
		expect(within(status).getByText(K('status.yes'))).toBeInTheDocument()
		expect(within(status).queryByText(K('status.notConfigured.title'))).toBeNull()
	})

	it("names what is missing, by the settings' own names, when it is not set up", async () => {
		renderStrip(
			overview({
				configured: false,
				hasKey: false,
				hasStateDir: false,
				stateWrittenAt: null,
				publicUrl: null,
			}),
		)

		const status = await screen.findByTestId('access-status')
		expect(within(status).getByText(K('status.notConfigured.title'))).toBeInTheDocument()
		expect(
			within(status).getByText(
				`${K('status.notConfigured.description')} missing=STUMP_EDGE_KEY, STUMP_EDGE_STATE_DIR`,
			),
		).toBeInTheDocument()
		expect(within(status).getByText(K('status.notReady'))).toBeInTheDocument()
		expect(within(status).getByText(K('status.never'))).toBeInTheDocument()
		expect(within(status).getByText(K('status.missing'))).toBeInTheDocument()
	})

	it('reminds that passes are not yet required, once everything is set up', async () => {
		renderStrip(overview({ requirePass: false }))

		expect(await screen.findByText(K('status.requirePassHint'))).toBeInTheDocument()
		expect(screen.getByText(K('status.notYet'))).toBeInTheDocument()
	})

	it('says whether public sign-in is on, with what the sign-in page has seen lately', async () => {
		renderStrip(
			overview({
				activeSignupLinks: 3,
				signinStats: { failuresLast10Min: 7, namesWaiting: 2, globalBudgetTripped: false },
			}),
		)

		const status = await screen.findByTestId('access-status')
		expect(within(status).getByText(K('status.on'))).toBeInTheDocument()
		expect(within(status).getByText('3')).toBeInTheDocument()
		expect(within(status).getByText('7')).toBeInTheDocument()
		expect(within(status).queryByText(K('status.publicSigninOffHint'))).toBeNull()
		expect(within(status).queryByTestId('access-budget-tripped')).toBeNull()
	})

	it('explains how to turn it on when it is off, and hides the counters of a page that is not there', async () => {
		renderStrip(overview({ publicSignin: false }))

		const status = await screen.findByTestId('access-status')
		expect(within(status).getByText(K('status.off'))).toBeInTheDocument()
		expect(within(status).getByText(K('status.publicSigninOffHint'))).toBeInTheDocument()
		expect(within(status).queryByText(K('status.failedLately'))).toBeNull()
		expect(within(status).queryByText(K('status.namesWaiting'))).toBeNull()
	})

	it('alerts when the sign-in page is refusing everybody for a few minutes', async () => {
		renderStrip(
			overview({
				signinStats: { failuresLast10Min: 40, namesWaiting: 5, globalBudgetTripped: true },
			}),
		)

		const alert = await screen.findByTestId('access-budget-tripped')
		expect(within(alert).getByText(K('status.budgetTripped.title'))).toBeInTheDocument()
	})
})
