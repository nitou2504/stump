import '@/__mocks__/resizeObserver'

import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import AccessScene from '../AccessScene'
import { K, row, serverOf, warning } from './fixtures'
import { setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/paths', () => ({
	usePaths: () => ({
		bookApprovals: () => '/settings/book-approvals',
		updateUser: (id: string) => `/settings/users/${id}/manage`,
	}),
}))
vi.mock('@/components/container', () => ({
	SceneContainer: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}))
vi.mock('react-helmet', () => ({ Helmet: () => null }))
vi.mock('sonner', () => ({ toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn() }) }))

const ANA = row('ana', 'ana', {
	publicSignin: true,
	activeDevices: 3,
	lastSignInAt: '2026-11-11T09:00:00Z',
})

function renderScene(path = '/', data: Parameters<typeof serverOf>[0] = {}) {
	const app = setup({
		path,
		answers: serverOf({
			people: [ANA],
			warnings: [warning('AtDeviceLimit:ana', 'AT_DEVICE_LIMIT')],
			...data,
		}),
	})
	const view = render(<AccessScene />, { wrapper: app.Wrapper })
	return { app, ...view }
}

describe('Public access', () => {
	it('has four things, in the order the owner needs them: is it working, what needs attention, who, and the history', async () => {
		renderScene()

		await screen.findByTestId('access-people')
		const order = ['access-status', 'access-attention', 'access-people', 'access-history'].map(
			(id) => screen.getByTestId(id),
		)
		for (const [index, element] of order.entries()) {
			const next = order[index + 1]
			if (next)
				expect(
					element.compareDocumentPosition(next) & Node.DOCUMENT_POSITION_FOLLOWING,
				).toBeTruthy()
		}
	})

	it('has exactly one main button outside a dialog: Invite someone', async () => {
		const { container } = renderScene()

		await screen.findByTestId('access-people')
		const main = [...container.querySelectorAll('button')].filter((button) =>
			button.className.split(' ').includes('bg-primary'),
		)
		expect(main).toHaveLength(1)
		expect(main[0]).toHaveTextContent(K('people.invite'))
	})

	it('opens the invite dialog from that button', async () => {
		renderScene()
		const user = userEvent.setup()

		await user.click(await screen.findByRole('button', { name: K('people.invite') }))

		expect(await screen.findByTestId('access-invite')).toBeInTheDocument()
	})

	it('opens a person’s sheet when the address asks for it, and when a row is clicked', async () => {
		renderScene('/settings/access?person=ana')

		const sheet = await screen.findByTestId('access-sheet')
		expect(within(sheet).getByText('ana')).toBeInTheDocument()
	})

	it('opens a person’s sheet from the people list', async () => {
		renderScene()
		const user = userEvent.setup()

		await user.click(await screen.findByTestId('access-person-ana'))

		expect(await screen.findByTestId('access-sheet')).toBeInTheDocument()
	})

	it('says all is well when nothing needs attention, with no badge anywhere', async () => {
		renderScene('/', { warnings: [] })

		expect(await screen.findByTestId('access-all-good')).toBeInTheDocument()
		expect(screen.queryByTestId('access-clear-all')).toBeNull()
	})
})
