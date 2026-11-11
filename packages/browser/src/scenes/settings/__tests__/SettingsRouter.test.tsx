import { UserPermission } from '@stump/graphql'
import { render, screen } from '@testing-library/react'
import { Suspense } from 'react'
import { MemoryRouter, Outlet, Route, Routes } from 'react-router'

import { type IAppContext, useAppContext } from '@/context'
import { useAppStore } from '@/stores/app.ts'

import SettingsRouter from '../SettingsRouter'

vi.mock('@/context', () => ({ useAppContext: vi.fn() }))
vi.mock('@/stores/app.ts', () => ({ useAppStore: vi.fn() }))

// this layout pulls in many heavy deps that i dont want to bother
// implementing mocks for, nor do i think it would even be useful
vi.mock('../SettingsLayout.tsx', () => ({
	default: () => <Outlet />,
}))

vi.mock('../server/users/UsersRouter.tsx', () => ({
	default: () => <div data-testid="users-router" />,
}))
vi.mock('../server/email/EmailSettingsRouter.tsx', () => ({
	default: () => <div data-testid="email-router" />,
}))
vi.mock('../server/access/index.tsx', () => ({
	default: () => <div data-testid="access-scene" />,
}))

vi.mock('../app/general/GeneralSettingsScene.tsx', () => ({
	default: () => <div data-testid="account-scene" />,
}))
vi.mock('../server/general/GeneralServerSettingsScene.tsx', () => ({
	default: () => <div data-testid="server-scene" />,
}))
vi.mock('../server/bookApprovals', () => ({
	default: () => <div data-testid="book-approvals-scene" />,
}))

const buildContext = (permissions: UserPermission[], isServerOwner = false): IAppContext => ({
	checkPermission: (p: UserPermission) => permissions.includes(p),
	isServerOwner,
	user: {} as any,
	logout: vi.fn(),
	enforcePermission: vi.fn(),
})

/**
 * render at specific path with permissions
 *
 * @param subPath relative to settings route, e.g. 'users' or 'server'
 * @param permissions self explanatory stinky
 * @param isServerOwner whether the user is the server owner, whatever they hold
 */
const renderAt = (subPath: string, permissions: UserPermission[] = [], isServerOwner = false) => {
	vi.mocked(useAppContext).mockReturnValue(buildContext(permissions, isServerOwner))
	vi.mocked(useAppStore).mockImplementation((selector: any) => selector({ platform: 'browser' }))

	return render(
		<MemoryRouter initialEntries={[`/settings/${subPath}`]}>
			<Suspense fallback={null}>
				<Routes>
					<Route path="/" element={<div data-testid="home" />} />
					<Route path="/settings/*" element={<SettingsRouter />} />
				</Routes>
			</Suspense>
		</MemoryRouter>,
	)
}

describe('SettingsRouter', () => {
	beforeEach(() => {
		vi.clearAllMocks()
	})

	describe('users route', () => {
		it('renders when the user has ReadUsers', async () => {
			renderAt('users', [UserPermission.ReadUsers])
			expect(await screen.findByTestId('users-router')).toBeInTheDocument()
		})

		it('redirects to account when the user lacks ReadUsers', async () => {
			renderAt('users', [])
			expect(await screen.findByTestId('account-scene')).toBeInTheDocument()
			expect(screen.queryByTestId('users-router')).not.toBeInTheDocument()
		})
	})

	// the books other people add are approved by the owner and library managers (turning the page
	// off sends the route home: that is tested with the page)
	describe('book approvals route', () => {
		it('renders the page for those who may manage libraries', async () => {
			renderAt('book-approvals', [UserPermission.ManageLibrary])
			expect(await screen.findByTestId('book-approvals-scene')).toBeInTheDocument()
		})

		it('is not there for anyone else', async () => {
			renderAt('book-approvals', [UserPermission.AddBooks])
			expect(await screen.findByTestId('account-scene')).toBeInTheDocument()
			expect(screen.queryByTestId('book-approvals-scene')).not.toBeInTheDocument()
		})
	})

	// how people come in from the internet: the owner's alone, whatever anyone else holds
	describe('public access route', () => {
		it('renders for the server owner', async () => {
			renderAt('access', [], true)
			expect(await screen.findByTestId('access-scene')).toBeInTheDocument()
		})

		it('sends everybody else to their account, even with every server permission', async () => {
			renderAt(
				'access',
				[
					UserPermission.ManageServer,
					UserPermission.ManageUsers,
					UserPermission.ReadUsers,
					UserPermission.ManageLibrary,
				],
				false,
			)
			expect(await screen.findByTestId('account-scene')).toBeInTheDocument()
			expect(screen.queryByTestId('access-scene')).not.toBeInTheDocument()
		})
	})

	describe('server settings route', () => {
		it('renders when the user has ManageServer', async () => {
			renderAt('server', [UserPermission.ManageServer])
			expect(await screen.findByTestId('server-scene')).toBeInTheDocument()
		})

		it('redirects to account when the user lacks ManageServer', async () => {
			renderAt('server', [])
			expect(await screen.findByTestId('account-scene')).toBeInTheDocument()
			expect(screen.queryByTestId('server-scene')).not.toBeInTheDocument()
		})
	})
})
