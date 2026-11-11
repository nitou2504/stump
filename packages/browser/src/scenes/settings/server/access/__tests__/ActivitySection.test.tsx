import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import ActivitySection from '../ActivitySection'
import { page, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))

const K = (key: string) => `settingsScene.server/access.${key}`

const entry = (id: number, action: string, extra: Record<string, unknown> = {}) => ({
	id,
	at: '2026-11-10T12:00:00Z',
	actor: 'g3',
	action,
	target: 'x',
	detail: '',
	...extra,
})

function renderSection(nodes: ReturnType<typeof entry>[], totalPages = 1) {
	const app = setup({
		answers: (query, variables) => {
			if (query.includes('query PublicAccessAudit'))
				return { accessAudit: page(nodes, totalPages), requested: variables }
			throw new Error(`Unexpected query ${query}`)
		},
	})
	render(<ActivitySection />, { wrapper: app.Wrapper })
	return app
}

describe('Activity', () => {
	it('names what was done in the words of the page, with who did it and when', async () => {
		renderSection([entry(1, 'invite.create', { detail: "Ana's phone" }), entry(2, 'device.revoke')])

		const first = await screen.findByTestId('access-activity-1')
		expect(within(first).getByText(K('activity.actions.inviteCreate'))).toBeInTheDocument()
		expect(within(first).getByText("Ana's phone")).toBeInTheDocument()
		expect(within(first).getByText(/activity\.when actor=g3 at=/)).toBeInTheDocument()
		expect(
			within(screen.getByTestId('access-activity-2')).getByText(K('activity.actions.deviceRevoke')),
		).toBeInTheDocument()
	})

	it("shows an action it doesn't know as the server wrote it", async () => {
		renderSection([entry(1, 'something.new')])

		expect(await screen.findByText('something.new')).toBeInTheDocument()
	})

	it('says when there is nothing yet', async () => {
		renderSection([])

		expect(await screen.findByText(K('activity.empty'))).toBeInTheDocument()
	})

	it('goes through the pages when there are several', async () => {
		renderSection([entry(1, 'invite.create')], 2)
		const user = userEvent.setup()

		await screen.findByTestId('access-activity-1')
		expect(screen.getByRole('button', { name: K('pager.previous') })).toBeDisabled()
		await user.click(screen.getByRole('button', { name: K('pager.next') }))
		expect(await screen.findByText(`${K('pager.page')} page=2 total=2`)).toBeInTheDocument()
	})
})
