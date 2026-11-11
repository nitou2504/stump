import { render, screen } from '@testing-library/react'

import PasswordGate from '../PasswordGate'

const store = vi.hoisted(() => ({ user: null as null | { mustChangePassword?: boolean } }))

vi.mock('@/stores', () => ({
	useUserStore: (selector: (state: { user: typeof store.user }) => unknown) =>
		selector({ user: store.user }),
}))
vi.mock('@/AppLayout', () => ({ AppLayout: () => <div data-testid="app" /> }))
vi.mock('../MustChangePasswordScene.tsx', () => ({
	default: () => <div data-testid="must-change" />,
}))

describe('PasswordGate', () => {
	it('is the app for somebody who chose their own password', async () => {
		store.user = { mustChangePassword: false }
		render(<PasswordGate />)

		expect(await screen.findByTestId('app')).toBeInTheDocument()
		expect(screen.queryByTestId('must-change')).not.toBeInTheDocument()
	})

	it('is the app for a user the server never said anything about', async () => {
		store.user = {}
		render(<PasswordGate />)

		expect(await screen.findByTestId('app')).toBeInTheDocument()
	})

	it('is the app when the user is not known yet, which signs them in (or sends them to do it)', async () => {
		store.user = null
		render(<PasswordGate />)

		expect(await screen.findByTestId('app')).toBeInTheDocument()
	})

	it("is the screen that asks for a new password, and not the app at all, while the owner's password is the one in use", async () => {
		store.user = { mustChangePassword: true }
		render(<PasswordGate />)

		expect(await screen.findByTestId('must-change')).toBeInTheDocument()
		expect(screen.queryByTestId('app')).not.toBeInTheDocument()
	})
})
