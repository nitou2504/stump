import '@/__mocks__/resizeObserver'

import { AccessRole } from '@stump/graphql'
import { render, screen } from '@testing-library/react'
import { PropsWithChildren } from 'react'

import { SmartListContext } from '../context'
import TableHeaderActions from '../items/table/TableHeaderActions'
import { createSmartListViewStore, SmartListViewStoreContext } from '../store'
import UserSmartListHeader from '../UserSmartListHeader'
import { setup } from './support'

vi.mock('@/hooks/useTranslate', () => import('./translations'))
vi.mock('@/paths', () => ({
	usePaths: () => ({
		smartLists: () => '/smart-lists',
		smartListItems: (id: string) => `/smart-lists/${id}/items`,
		smartListSettings: (id: string) => `/smart-lists/${id}/settings`,
	}),
}))
// the header is shown as its name and the link to the list's settings, which is what the page decides
vi.mock('@/components/sharedLayout', () => ({
	EntityHeader: ({ name, settingsLink }: { name: string; settingsLink?: string }) => (
		<header>
			<h1>{name}</h1>
			{settingsLink && <a href={settingsLink}>settings</a>}
		</header>
	),
}))
// the toolbar's own controls are tested where they are; here only the save menu, which is the page's call
vi.mock('@/components/filters', () => ({
	ListToolbar: ({ children }: PropsWithChildren) => <div>{children}</div>,
	ListToolbarControls: ({ children }: PropsWithChildren) => <div>{children}</div>,
	Search: () => <input aria-label="search" />,
	URLFilterDrawer: () => null,
}))
vi.mock('@/components/filters/useFilterScene', () => ({
	readSearchParam: () => '',
	writeSearchParam: () => undefined,
}))
vi.mock('../items/table/ViewManagerDropdown', () => ({
	default: () => <button>save view</button>,
}))
vi.mock('../items/table/SmartListMoreMenu', () => ({ default: () => null }))

/** A smart list's page as a person who can change the list, or only read it, has it */
function renderAs(
	{ canEdit }: { canEdit: boolean },
	ui: React.ReactNode,
	{ changedView = false }: { changedView?: boolean } = {},
) {
	const { Wrapper } = setup({ answers: () => ({}) })
	const store = createSmartListViewStore()
	if (changedView) store.getState().updateWorkingView({ search: 'dune' })

	const context = {
		list: { id: 'list', name: 'Shelf', canEdit, views: [] },
		meta: undefined,
		viewerRole: canEdit ? AccessRole.CoCreator : AccessRole.Reader,
		patchSmartList: vi.fn(),
		grouping: 'BY_BOOKS',
		setGrouping: vi.fn(),
		fieldGrouping: null,
		setFieldGrouping: vi.fn(),
	} as never

	return render(
		<SmartListViewStoreContext.Provider value={store}>
			<SmartListContext.Provider value={context}>{ui}</SmartListContext.Provider>
		</SmartListViewStoreContext.Provider>,
		{ wrapper: Wrapper },
	)
}

describe("a smart list's settings link", () => {
	it('is there for who may change the list', () => {
		renderAs({ canEdit: true }, <UserSmartListHeader />)

		expect(screen.getByRole('link', { name: 'settings' })).toHaveAttribute(
			'href',
			'/smart-lists/list/settings',
		)
	})

	it('is not there for someone who only reads the list', () => {
		renderAs({ canEdit: false }, <UserSmartListHeader />)

		expect(screen.getByRole('heading', { name: 'Shelf' })).toBeInTheDocument()
		expect(screen.queryByRole('link', { name: 'settings' })).not.toBeInTheDocument()
	})
})

describe("a smart list's save menu", () => {
	it('shows once a view was changed, for who may change the list', () => {
		renderAs({ canEdit: true }, <TableHeaderActions />, { changedView: true })

		expect(screen.getByRole('button', { name: 'save view' })).toBeInTheDocument()
	})

	it('is not there for someone who only reads the list, however they change the view', () => {
		renderAs({ canEdit: false }, <TableHeaderActions />, { changedView: true })

		expect(screen.getByLabelText('search')).toBeInTheDocument()
		expect(screen.queryByRole('button', { name: 'save view' })).not.toBeInTheDocument()
	})

	it('stays hidden until a view was changed, for anyone', () => {
		renderAs({ canEdit: true }, <TableHeaderActions />)

		expect(screen.queryByRole('button', { name: 'save view' })).not.toBeInTheDocument()
	})
})
