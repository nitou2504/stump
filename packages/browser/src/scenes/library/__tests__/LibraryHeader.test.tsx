import { UserPermission } from '@stump/graphql'
import { render, screen } from '@testing-library/react'
import { ReactNode } from 'react'
import { MemoryRouter } from 'react-router'

import { LibraryContext } from '../context'
import LibraryHeader from '../LibraryHeader'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))

const permissions = vi.hoisted(() => ({ held: new Set<string>() }))
vi.mock('@/context', () => ({
	useAppContext: () => ({
		checkPermission: (permission: string) => permissions.held.has(permission),
	}),
}))

vi.mock('@/paths', () => ({
	usePaths: () =>
		new Proxy({}, { get: (_, name) => (id?: string) => `/${String(name)}/${id ?? ''}` }),
}))

vi.mock('@stump/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/client')>()),
	usePrefetchFiles: () => vi.fn(),
}))

vi.mock('../tabs/books/queries', () => ({ usePrefetchLibraryBooks: () => vi.fn() }))
vi.mock('../tabs/series/queries', () => ({ usePrefetchLibrarySeries: () => vi.fn() }))
vi.mock('../LibraryOverviewSheet', () => ({ LibraryOverviewSheet: () => null }))

// the header is shown as its name and the link to the library's settings: what the page decides
vi.mock('@/components/sharedLayout', () => ({
	EntityHeader: ({
		name,
		settingsLink,
		endActions,
	}: {
		name: string
		settingsLink?: string
		endActions?: ReactNode
	}) => (
		<header>
			<h1>{name}</h1>
			{settingsLink && <a href={settingsLink}>settings</a>}
			{endActions}
		</header>
	),
	HeaderIconButton: ({ label }: { label: string }) => <a href="/add">{label}</a>,
}))

const library = {
	id: 'lib',
	name: 'Biblioteca',
	path: '/biblioteca',
	stats: null,
	config: { hideSeriesView: false, oneshotsDirectory: null },
}

function setup(held: UserPermission[]) {
	permissions.held = new Set(held)
	return render(
		<MemoryRouter>
			<LibraryContext.Provider value={{ library } as never}>
				<LibraryHeader />
			</LibraryContext.Provider>
		</MemoryRouter>,
	)
}

describe('LibraryHeader settings gear', () => {
	it('is not shown to somebody who cannot manage the library (it would lead nowhere)', () => {
		setup([UserPermission.DownloadFile, UserPermission.AddBooks])
		expect(screen.getByRole('heading', { name: 'Biblioteca' })).toBeInTheDocument()
		expect(screen.queryByRole('link', { name: 'settings' })).toBeNull()
	})

	it('is shown to somebody who can manage the library', () => {
		setup([UserPermission.ManageLibrary])
		expect(screen.getByRole('link', { name: 'settings' })).toHaveAttribute(
			'href',
			'/libraryManage/lib',
		)
	})

	it('names the Add books button through the translations (not in English)', () => {
		setup([UserPermission.AddBooks])
		expect(screen.getByRole('link', { name: 'sidebar.buttons.addBooks' })).toBeInTheDocument()
	})
})
