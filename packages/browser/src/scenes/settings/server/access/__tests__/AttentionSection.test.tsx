import '@/__mocks__/resizeObserver'

import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import AttentionSection, { type InvitePrefill } from '../AttentionSection'
import type { AccessPersonRow } from '../graphql'
import { device, K, row, serverOf, signupLink, warning } from './fixtures'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/paths', () => ({
	usePaths: () => ({
		bookApprovals: () => '/settings/book-approvals',
		updateUser: (id: string) => `/settings/users/${id}/manage`,
	}),
}))

const toast = vi.hoisted(() => Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn() }))
vi.mock('sonner', () => ({ toast }))

type Handlers = { onOpenPerson?: (id: string) => void; onInvite?: (prefill: InvitePrefill) => void }

function renderList(
	warnings: ReturnType<typeof warning>[],
	extra: Parameters<typeof serverOf>[0] = {},
	answers: (query: string, variables: Record<string, unknown> | undefined) => unknown = () =>
		undefined,
	handlers: Handlers = {},
) {
	const server = serverOf({
		warnings,
		people: [row('ana', 'ana', { publicSignin: true, activeDevices: 3 })],
		...extra,
	})
	const app = setup({
		answers: (query, variables) => {
			const own = answers(query, variables)
			return own === undefined ? server(query, variables) : own
		},
	})
	const props = {
		onOpenPerson: handlers.onOpenPerson ?? vi.fn<(id: string) => void>(),
		onInvite: handlers.onInvite ?? vi.fn<(prefill: InvitePrefill) => void>(),
		onSeeAttempts: vi.fn<() => void>(),
		onLetIn: vi.fn<(row: AccessPersonRow) => void>(),
		onSetPassword: vi.fn<(person: { id: string; username: string }) => void>(),
	}
	render(<AttentionSection {...props} />, { wrapper: app.Wrapper })
	return { app, props }
}

describe('Needs your attention', () => {
	beforeEach(() => vi.clearAllMocks())

	it('says one calm thing when there is nothing to look at, and has no button', async () => {
		renderList([])

		expect(await screen.findByTestId('access-all-good')).toHaveTextContent(K('attention.allGood'))
		expect(screen.queryByRole('button')).toBeNull()
	})

	it('shows each thing in one line: what it says, its one button and a way to dismiss it', async () => {
		renderList([
			warning('AtDeviceLimit:ana', 'AT_DEVICE_LIMIT', { count: 3 }),
			warning('BooksWaiting:-', 'BOOKS_WAITING', { person: null, count: 2 }),
		])

		const line = await screen.findByTestId('access-warning-AtDeviceLimit:ana')
		expect(line).toHaveTextContent(`${K('attention.kinds.atDeviceLimit')} name=ana count=3`)
		expect(within(line).getAllByRole('button')).toHaveLength(2)
		expect(
			within(line).getByRole('button', { name: K('attention.actions.raiseLimit') }),
		).toBeInTheDocument()
		expect(within(line).getByRole('button', { name: K('attention.dismiss') })).toBeInTheDocument()
		// the title counts them
		expect(screen.getByText(`${K('attention.title')} count=2`)).toBeInTheDocument()
		// books waiting go to the page that approves them
		const books = screen.getByTestId('access-warning-BooksWaiting:-')
		expect(
			within(books).getByRole('link', { name: K('attention.actions.review') }),
		).toHaveAttribute('href', '/settings/book-approvals')
	})

	it('shows the alerts first', async () => {
		renderList([
			warning('ManySessions:ana', 'MANY_SESSIONS', { lastAt: '2026-11-11T12:00:00Z' }),
			warning('GlobalBudget:-', 'GLOBAL_BUDGET', {
				severity: 'ALERT',
				person: null,
				lastAt: '2026-10-01T12:00:00Z',
			}),
		])

		await screen.findByTestId('access-warning-GlobalBudget:-')
		const items = screen.getAllByTestId(/^access-warning-/)
		expect(items[0]).toHaveAttribute('data-severity', 'ALERT')
	})

	it('dismisses one, and offers to take it back', async () => {
		const user = userEvent.setup()
		const { app } = renderList([warning('ManySessions:ana', 'MANY_SESSIONS')], {}, (query) =>
			query.includes('mutation PublicAccessDismissWarnings')
				? { dismissAccessWarnings: 1 }
				: undefined,
		)

		const line = await screen.findByTestId('access-warning-ManySessions:ana')
		await user.click(within(line).getByRole('button', { name: K('attention.dismiss') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessDismissWarnings')).toEqual([
				{ ids: ['ManySessions:ana'] },
			]),
		)
		expect(toast).toHaveBeenCalledWith(
			K('attention.dismissed'),
			expect.objectContaining({ action: expect.objectContaining({ label: K('attention.undo') }) }),
		)
	})

	it('clears everything shown at once, says how many, and Undo shows them again', async () => {
		const user = userEvent.setup()
		const { app } = renderList(
			[
				warning('ManySessions:ana', 'MANY_SESSIONS'),
				warning('BooksWaiting:-', 'BOOKS_WAITING', { person: null }),
			],
			{},
			(query, variables) => {
				if (query.includes('mutation PublicAccessDismissWarnings'))
					return { dismissAccessWarnings: 2 }
				if (query.includes('mutation PublicAccessRestoreWarnings'))
					return { restoreAccessWarnings: (variables?.ids as string[]).length }
				return undefined
			},
		)

		await user.click(await screen.findByTestId('access-clear-all'))

		// the ids of what was on the screen: one that arrived after the last refresh is not cleared
		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessDismissWarnings')).toEqual([
				{ ids: ['ManySessions:ana', 'BooksWaiting:-'] },
			]),
		)
		const [message, options] = toast.mock.calls[0] as [string, { action: { onClick: () => void } }]
		expect(message).toBe(`${K('attention.cleared')} count=2`)

		options.action.onClick()
		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessRestoreWarnings')).toEqual([
				// the ids of what was shown, in the order it was shown
				{ ids: ['ManySessions:ana', 'BooksWaiting:-'] },
			]),
		)
	})

	it('shows five and the rest behind Show all', async () => {
		const user = userEvent.setup()
		renderList(
			Array.from({ length: 7 }, (_, index) =>
				warning(`ManySessions:p${index}`, 'MANY_SESSIONS', {
					person: { id: `p${index}`, username: `p${index}`, removed: false },
				}),
			),
		)

		await screen.findByText(`${K('attention.title')} count=7`)
		expect(screen.getAllByTestId(/^access-warning-/)).toHaveLength(5)
		await user.click(screen.getByRole('button', { name: `${K('attention.showAll')} count=7` }))
		expect(screen.getAllByTestId(/^access-warning-/)).toHaveLength(7)
	})

	it('raises the limit of somebody at it by one, and can undo that', async () => {
		const user = userEvent.setup()
		const { app } = renderList([warning('AtDeviceLimit:ana', 'AT_DEVICE_LIMIT')], {}, (query) =>
			query.includes('mutation PublicAccessSetPersonAccess')
				? {
						setPersonAccess: {
							publicSignin: true,
							maxDevices: 4,
							hiddenLibraries: 0,
							newLibrariesHidden: true,
						},
					}
				: undefined,
		)

		await user.click(await screen.findByRole('button', { name: K('attention.actions.raiseLimit') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSetPersonAccess')).toEqual([
				{ input: { personId: 'ana', maxDevices: 4 } },
			]),
		)
		const [message, options] = toast.mock.calls[0] as [string, { action: { onClick: () => void } }]
		expect(message).toBe(`${K('attention.raised')} name=ana count=4`)
		options.action.onClick()
		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSetPersonAccess')[1]).toEqual({
				input: { personId: 'ana', maxDevices: 3 },
			}),
		)
	})

	it('looks at the devices instead when the limit is already the most there can be', async () => {
		const onOpenPerson = vi.fn<(id: string) => void>()
		const user = userEvent.setup()
		renderList(
			[warning('AtDeviceLimit:ana', 'AT_DEVICE_LIMIT', { count: 10 })],
			{ people: [row('ana', 'ana', { publicSignin: true, maxDevices: 10 })] },
			() => undefined,
			{ onOpenPerson },
		)

		await user.click(await screen.findByRole('button', { name: K('attention.actions.seeDevices') }))
		expect(onOpenPerson).toHaveBeenCalledWith('ana')
	})

	it('lets a locked account back in', async () => {
		const user = userEvent.setup()
		const { app } = renderList(
			[warning('AccountLocked:ana', 'ACCOUNT_LOCKED', { count: 12 })],
			{},
			(query) =>
				query.includes('mutation PublicAccessUnblock') ? { unblockUser: true } : undefined,
		)

		await user.click(await screen.findByRole('button', { name: K('attention.actions.unblock') }))

		await waitFor(() => expect(sent(app.execute, 'PublicAccessUnblock')).toEqual([{ id: 'ana' }]))
	})

	it('asks before it signs somebody out everywhere', async () => {
		const user = userEvent.setup()
		const { app } = renderList(
			[warning('ManySessions:ana', 'MANY_SESSIONS', { count: 6 })],
			{},
			(query) =>
				query.includes('mutation PublicAccessSignOutEverywhere')
					? { deleteUserSessions: 6 }
					: undefined,
		)

		await user.click(await screen.findByRole('button', { name: K('attention.actions.signOutAll') }))
		expect(sent(app.execute, 'PublicAccessSignOutEverywhere')).toEqual([])
		const dialog = await screen.findByRole('alertdialog')
		await user.click(within(dialog).getByRole('button', { name: K('person.signOutAll') }))

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessSignOutEverywhere')).toEqual([{ id: 'ana' }]),
		)
	})

	it('makes another link like one that is nearly used up, with its note and what it gives', async () => {
		const onInvite = vi.fn<(prefill: InvitePrefill) => void>()
		const user = userEvent.setup()
		renderList(
			[
				warning('LinkNearBudget:l1', 'LINK_NEAR_BUDGET', {
					person: null,
					linkId: 'l1',
					detail: 'club',
					count: 1,
				}),
			],
			{ links: [signupLink('l1', 'club', { kind: 'OPEN', maxUses: 5, uses: 4 })] },
			() => undefined,
			{ onInvite },
		)

		await user.click(await screen.findByRole('button', { name: K('attention.actions.newLink') }))

		expect(onInvite).toHaveBeenCalledWith({
			label: 'club',
			kind: 'OPEN',
			maxUses: 5,
			permissions: ['CHANGE_PASSWORD', 'ADD_BOOKS'],
			libraryIds: ['bib'],
			maxDevices: 3,
		})
	})

	it('names the old devices that still work before it removes them, and removes only those', async () => {
		const user = userEvent.setup()
		const { app } = renderList(
			[warning('LeftoverDevices:-', 'LEFTOVER_DEVICES', { person: null, count: 2 })],
			{
				devices: [
					device('terminal-pass', { legacy: true, person: null }),
					device('gone', { person: { id: 'x', username: 'x', removed: true } }),
					device('fine'),
				],
			},
			(query) =>
				query.includes('mutation PublicAccessRevokeDevice')
					? { revokeAccessDevice: true }
					: undefined,
		)

		await user.click(
			await screen.findByRole('button', { name: K('attention.actions.removeLeftovers') }),
		)
		const dialog = await screen.findByRole('alertdialog')
		const listed = within(dialog).getByTestId('access-leftovers')
		expect(listed).toHaveTextContent('terminal-pass')
		expect(listed).toHaveTextContent('gone')
		expect(listed).not.toHaveTextContent('fine')
		await user.click(
			within(dialog).getByRole('button', { name: K('attention.actions.removeLeftovers') }),
		)

		await waitFor(() =>
			expect(sent(app.execute, 'PublicAccessRevokeDevice')).toEqual([
				{ id: 'terminal-pass' },
				{ id: 'gone' },
			]),
		)
	})

	it('says when it could not mark a warning as seen, and keeps it', async () => {
		const user = userEvent.setup()
		renderList([warning('ManySessions:ana', 'MANY_SESSIONS')], {}, (query) => {
			if (query.includes('mutation PublicAccessDismissWarnings')) throw new Error('down')
			return undefined
		})

		const line = await screen.findByTestId('access-warning-ManySessions:ana')
		await user.click(within(line).getByRole('button', { name: K('attention.dismiss') }))

		await waitFor(() =>
			expect(toast.error).toHaveBeenCalledWith(K('attention.error'), { description: 'down' }),
		)
		expect(screen.getByTestId('access-warning-ManySessions:ana')).toBeInTheDocument()
	})
})
