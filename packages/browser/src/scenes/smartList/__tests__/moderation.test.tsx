import '@/__mocks__/resizeObserver'

import { AccessRole } from '@stump/graphql'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Route, Routes } from 'react-router'

import { SmartListContext } from '../context'
import UserSmartListHeader from '../UserSmartListHeader'
import { sent, setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/paths', () => ({
	usePaths: () => ({
		smartLists: () => '/smart-lists',
		smartListItems: (id: string) => `/smart-lists/${id}/items`,
		smartListSettings: (id: string) => `/smart-lists/${id}/settings`,
	}),
}))
// the header is shown as its name, the link to the list's settings and its menu items, which is
// what the page decides
vi.mock('@/components/sharedLayout', () => ({
	EntityHeader: ({
		name,
		settingsLink,
		actions,
	}: {
		name: string
		settingsLink?: string
		actions?: { items: { label: string; onClick: () => void }[] }[]
	}) => (
		<header>
			<h1>{name}</h1>
			{settingsLink && <a href={settingsLink}>settings</a>}
			<nav aria-label="menu">
				{actions
					?.flatMap((group) => group.items)
					.map((item) => (
						<button key={item.label} onClick={item.onClick}>
							{item.label}
						</button>
					))}
			</nav>
		</header>
	),
}))

type Info = { canEdit?: boolean; visibility?: 'PUBLIC' | 'PRIVATE' }

/**
 * A smart list's header as its maker (`canEdit`), the server owner, or a friend who only reads it
 * has it. A list of someone else is Ana's
 */
function renderHeader({ canEdit = false, visibility = 'PUBLIC' }: Info, isServerOwner: boolean) {
	const { execute, Wrapper } = setup({
		user: { id: 'me', isServerOwner },
		path: '/smart-lists/list',
		answers: (query) => {
			if (query.includes('mutation UpdateSmartList'))
				return { updateSmartList: { __typename: 'SmartList' } }
			if (query.includes('mutation DeleteSmartList'))
				return { deleteSmartList: { __typename: 'SmartList' } }
			throw new Error(`Unexpected query ${query}`)
		},
	})

	const context = {
		list: {
			id: 'list',
			name: 'Shelf',
			description: null,
			defaultGrouping: 'BY_BOOKS',
			joiner: 'AND',
			// the rules are the maker's alone: everybody else gets none
			filters: [],
			visibility,
			canEdit,
			creator: { id: canEdit ? 'me' : 'ana', username: canEdit ? 'me' : 'ana' },
			views: [],
		},
		meta: undefined,
		viewerRole: canEdit ? AccessRole.CoCreator : AccessRole.Reader,
		patchSmartList: vi.fn(),
		grouping: 'BY_BOOKS',
		setGrouping: vi.fn(),
		fieldGrouping: null,
		setFieldGrouping: vi.fn(),
	} as never

	render(
		<Routes>
			<Route
				path="/smart-lists/:id"
				element={
					<SmartListContext.Provider value={context}>
						<UserSmartListHeader />
					</SmartListContext.Provider>
				}
			/>
			<Route path="*" element={<p>Somewhere else</p>} />
		</Routes>,
		{ wrapper: ({ children }) => <Wrapper>{children}</Wrapper> },
	)
	return { execute }
}

const menu = () => within(screen.getByRole('navigation', { name: 'menu' }))
const items = () =>
	menu()
		.queryAllByRole('button')
		.map((item) => item.textContent)

describe("a public smart list of someone else, as the server owner's", () => {
	it('has a menu to make it private or delete it, and no settings', () => {
		renderHeader({}, true)

		expect(screen.getByRole('heading', { name: 'Shelf' })).toBeInTheDocument()
		expect(items()).toEqual([
			'userSmartListScene.moderation.makePrivate',
			'userSmartListScene.moderation.delete',
		])
		// the list's rules, name and views are its maker's to change
		expect(screen.queryByRole('link', { name: 'settings' })).not.toBeInTheDocument()
	})

	it('is made private after asking, with nothing else of the list sent', async () => {
		const user = userEvent.setup()
		const { execute } = renderHeader({}, true)

		await user.click(
			menu().getByRole('button', { name: 'userSmartListScene.moderation.makePrivate' }),
		)
		expect(sent(execute, 'UpdateSmartList')).toEqual([])
		await user.click(
			await screen.findByRole('button', { name: 'userSmartListScene.moderation.unpublishConfirm' }),
		)

		// only the visibility matters: the rest is what the API needs to take the request, and the
		// rules are none (the server owner is not given them, and the server does not apply them)
		await waitFor(() =>
			expect(sent(execute, 'UpdateSmartList')).toEqual([
				{
					id: 'list',
					input: {
						name: 'Shelf',
						description: null,
						defaultGrouping: 'BY_BOOKS',
						joiner: 'AND',
						filters: [],
						visibility: 'PRIVATE',
					},
				},
			]),
		)
		// it is private now, and not the server owner's to see: the page is left
		expect(await screen.findByText('Somewhere else')).toBeInTheDocument()
	})

	it('is deleted after asking', async () => {
		const user = userEvent.setup()
		const { execute } = renderHeader({}, true)

		await user.click(menu().getByRole('button', { name: 'userSmartListScene.moderation.delete' }))
		expect(sent(execute, 'DeleteSmartList')).toEqual([])
		await user.click(
			await screen.findByRole('button', {
				name: 'smartListSettingsScene.danger-zone/delete.sections.deleteList.confirmation.confirm',
			}),
		)

		await waitFor(() => expect(sent(execute, 'DeleteSmartList')).toEqual([{ id: 'list' }]))
		expect(await screen.findByText('Somewhere else')).toBeInTheDocument()
	})
})

describe('a smart list nobody may moderate', () => {
	it('gives a friend who only reads a public list neither settings nor a menu', () => {
		renderHeader({}, false)

		expect(screen.getByRole('heading', { name: 'Shelf' })).toBeInTheDocument()
		expect(items()).toEqual([])
		expect(screen.queryByRole('link', { name: 'settings' })).not.toBeInTheDocument()
	})

	it('gives the server owner settings, and no moderation menu, on a list of their own', () => {
		renderHeader({ canEdit: true }, true)

		expect(screen.getByRole('link', { name: 'settings' })).toHaveAttribute(
			'href',
			'/smart-lists/list/settings',
		)
		expect(items()).toEqual([])
	})

	it('gives the server owner no menu on a list that is not public', () => {
		renderHeader({ visibility: 'PRIVATE' }, true)

		expect(items()).toEqual([])
		expect(screen.queryByRole('link', { name: 'settings' })).not.toBeInTheDocument()
	})
})
