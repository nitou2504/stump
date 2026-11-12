import { UserPermission } from '@stump/graphql'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import { SeriesHeader } from '../SeriesHeader'

vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({ t: (key: string) => key, translate: (key: string) => key }),
}))

const permissions = vi.hoisted(() => ({ held: new Set<string>(), isServerOwner: false }))
vi.mock('@/context', () => ({
	useAppContext: () => ({
		isServerOwner: permissions.isServerOwner,
		checkPermission: (permission: string) =>
			permissions.isServerOwner || permissions.held.has(permission),
	}),
}))

vi.mock('@/paths', () => ({
	usePaths: () =>
		new Proxy({}, { get: (_, name) => (id?: string) => `/${String(name)}/${id ?? ''}` }),
}))

vi.mock('@stump/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/client')>()),
	usePrefetchFiles: () => vi.fn(),
	useGraphQLMutation: () => ({ mutate: vi.fn(), isPending: false }),
}))
vi.mock('@tanstack/react-query', async (importOriginal) => ({
	...(await importOriginal<typeof import('@tanstack/react-query')>()),
	useQueryClient: () => ({ invalidateQueries: vi.fn() }),
}))

vi.mock('../queries', () => ({ usePrefetchSeriesBooks: () => vi.fn() }))
vi.mock('../useSeriesThumbnailActions', () => ({
	useSeriesThumbnailActions: () => ({ canUpload: false }),
}))
vi.mock('../SeriesOverviewSheet', () => ({ SeriesOverviewSheet: () => null }))
vi.mock('../CompleteSeriesConfirmation', () => ({ CompleteSeriesConfirmation: () => null }))
vi.mock('../context', () => ({
	useSeriesContext: () => ({
		series: {
			id: 'series',
			resolvedName: 'Dune',
			path: '/biblioteca/dune',
			stats: { totalBytes: 0, totalReadingTimeSeconds: 0, bookCount: 0, completedBooks: 0 },
			library: { id: 'lib', name: 'Biblioteca' },
		},
	}),
}))

// the header is shown as its name and the link to the series' settings: what the page decides
vi.mock('@/components/sharedLayout', () => ({
	EntityHeader: ({ name, settingsLink }: { name: string; settingsLink?: string }) => (
		<header>
			<h1>{name}</h1>
			{settingsLink && <a href={settingsLink}>settings</a>}
		</header>
	),
}))

function setup(held: UserPermission[], isServerOwner = false) {
	permissions.held = new Set(held)
	permissions.isServerOwner = isServerOwner
	return render(
		<MemoryRouter>
			<SeriesHeader />
		</MemoryRouter>,
	)
}

describe('SeriesHeader settings gear', () => {
	it('is not shown to somebody who is not the server owner (the page is the owner only)', () => {
		setup([UserPermission.DownloadFile, UserPermission.AddBooks])
		expect(screen.getByRole('heading', { name: 'Dune' })).toBeInTheDocument()
		expect(screen.queryByRole('link', { name: 'settings' })).toBeNull()
	})

	it('is not shown to somebody who can edit metadata but is not the owner (the route sends them to /series)', () => {
		setup([UserPermission.EditMetadata])
		expect(screen.getByRole('heading', { name: 'Dune' })).toBeInTheDocument()
		expect(screen.queryByRole('link', { name: 'settings' })).toBeNull()
	})

	it('is shown to the server owner', () => {
		setup([], true)
		expect(screen.getByRole('link', { name: 'settings' })).toHaveAttribute(
			'href',
			'/seriesSettings/series',
		)
	})
})
