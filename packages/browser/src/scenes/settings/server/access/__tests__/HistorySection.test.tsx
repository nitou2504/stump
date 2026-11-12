import '@/__mocks__/resizeObserver'

import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import HistorySection from '../HistorySection'
import { K, serverOf } from './fixtures'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))

const signIn = (id: number, extra: Record<string, unknown> = {}) => ({
	id,
	at: '2026-11-11T10:00:00Z',
	ip: '198.51.100.7',
	userAgent: 'Mozilla/5.0 (X11; Linux x86_64) Chrome/120.0 Safari/537.36',
	agentLabel: 'Chrome on Linux',
	successful: true,
	deviceId: null,
	deviceLabel: null,
	reason: null,
	cleared: false,
	person: { id: 'ana', username: 'ana', removed: false },
	...extra,
})

const change = (id: number, extra: Record<string, unknown> = {}) => ({
	id,
	at: '2026-11-11T10:00:00Z',
	actor: 'nitou',
	action: 'device.revoke',
	target: 'abc',
	targetLabel: 'Chrome on Linux',
	detail: 'by nitou',
	...extra,
})

function renderHistory(
	data: Parameters<typeof serverOf>[0],
	focus?: { failuresOnly: boolean; nonce: number },
) {
	const app = setup({ answers: serverOf(data) })
	const view = render(<HistorySection focus={focus} />, { wrapper: app.Wrapper })
	return { app, ...view }
}

describe('History', () => {
	it('is closed, with one button, until the owner asks', async () => {
		const { app } = renderHistory({ signIns: [signIn(1)] })

		expect(await screen.findByRole('button', { name: K('history.show') })).toBeInTheDocument()
		expect(sent(app.execute, 'PublicAccessSignIns')).toEqual([])
		expect(screen.queryByTestId('access-signins-list')).toBeNull()
	})

	it('shows the other people’s sign-ins, and asks the server to leave the owner’s own out', async () => {
		const user = userEvent.setup()
		const { app } = renderHistory({
			signIns: [signIn(1), signIn(2, { successful: false, reason: 'WRONG_PASSWORD' })],
		})

		await user.click(await screen.findByRole('button', { name: K('history.show') }))

		const list = await screen.findByTestId('access-signins-list')
		expect(within(list).getByText(K('history.result.ok'))).toBeInTheDocument()
		expect(within(list).getByText(K('history.result.wrongPassword'))).toBeInTheDocument()
		expect(sent(app.execute, 'PublicAccessSignIns')[0]).toMatchObject({
			othersOnly: true,
			failuresOnly: false,
		})
		// the footer says where every sign-in is
		expect(
			screen.getByText(`${K('history.allSignIns')} users=settingsScene.server/users.title`),
		).toBeInTheDocument()
	})

	it('keeps the address and the browser for the row that is opened', async () => {
		const user = userEvent.setup()
		renderHistory({ signIns: [signIn(1, { deviceLabel: 'Ana’s phone' })] })
		await user.click(await screen.findByRole('button', { name: K('history.show') }))

		const row = await screen.findByTestId('access-signin-1')
		expect(row).toHaveTextContent('Ana’s phone')
		expect(row).not.toHaveTextContent('198.51.100.7')
		expect(row).not.toHaveTextContent('Mozilla')

		await user.click(within(row).getByRole('button'))
		expect(screen.getByTestId('access-signin-1-detail')).toHaveTextContent('198.51.100.7')
	})

	it('can show the failures only', async () => {
		const user = userEvent.setup()
		const { app } = renderHistory({ signIns: [signIn(1)] })
		await user.click(await screen.findByRole('button', { name: K('history.show') }))
		await screen.findByTestId('access-signins-list')

		await user.click(screen.getByRole('switch', { name: K('history.failuresOnly') }))

		await vi.waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSignIns').at(-1)).toMatchObject({ failuresOnly: true }),
		)
	})

	it('shows the changes as sentences with the name of what they were about, and no id', async () => {
		const user = userEvent.setup()
		renderHistory({
			audit: [
				change(1),
				change(2, {
					action: 'warning.dismiss',
					target: 'all',
					targetLabel: null,
					detail: '3 items',
				}),
				change(3, { actor: 'public', action: 'signup.created', targetLabel: 'lucia' }),
			],
		})
		await user.click(await screen.findByRole('button', { name: K('history.show') }))
		await user.click(await screen.findByRole('tab', { name: K('history.changes') }))

		const list = await screen.findByTestId('access-activity-list')
		expect(
			within(list).getByText(`${K('activity.actions.deviceRevoke')}: Chrome on Linux`),
		).toBeInTheDocument()
		expect(within(list).getByText(K('activity.actions.warningDismiss'))).toBeInTheDocument()
		expect(within(list).getByText(new RegExp(K('history.fromInternet')))).toBeInTheDocument()
		expect(list).not.toHaveTextContent('abc')
		// what the server wrote is behind the row
		expect(list).not.toHaveTextContent('3 items')
		await user.click(within(screen.getByTestId('access-activity-2')).getByRole('button'))
		expect(screen.getByTestId('access-activity-2-detail')).toHaveTextContent('3 items')
	})

	it('opens on the failed sign-ins when a warning points at them', async () => {
		const { app } = renderHistory({ signIns: [] }, { failuresOnly: true, nonce: 1 })

		expect(await screen.findByTestId('access-signins-list')).toBeInTheDocument()
		await vi.waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSignIns').at(-1)).toMatchObject({ failuresOnly: true }),
		)
	})
})
