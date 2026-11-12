import { UserPermission } from '@stump/graphql'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import { SeriesHeader } from '../SeriesHeader'

// the words are their keys; the "read" word also shows the count it was asked for, because in Spanish
// it agrees with it ("1 leído", "2 leídos")
vi.mock('@/hooks/useTranslate', () => ({
	useTranslate: () => ({
		t: (key: string, options?: { count?: number }) =>
			key === 'statChips.read' ? `${key}(${options?.count})` : key,
		translate: (key: string) => key,
	}),
}))

const permissions = vi.hoisted(() => ({
	held: new Set<string>(),
	isServerOwner: false,
	stats: { totalBytes: 0, totalReadingTimeSeconds: 0, bookCount: 0, completedBooks: 0 },
}))
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
			stats: permissions.stats,
			library: { id: 'lib', name: 'Biblioteca' },
		},
	}),
}))

// the header is shown as its name and the link to the series' settings: what the page decides
vi.mock('@/components/sharedLayout', () => ({
	EntityHeader: ({
		name,
		settingsLink,
		stats,
	}: {
		name: string
		settingsLink?: string
		stats?: { key: string; value: string | number; suffix?: string }[]
	}) => (
		<header>
			<h1>{name}</h1>
			{settingsLink && <a href={settingsLink}>settings</a>}
			<ul>
				{stats?.map((stat) => (
					<li key={stat.key}>{[stat.value, stat.suffix].filter(Boolean).join(' ')}</li>
				))}
			</ul>
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

describe('SeriesHeader number chips', () => {
	it('says what each number counts: the books, how many are read, the size', () => {
		permissions.stats = {
			totalBytes: 2_100_000_000,
			totalReadingTimeSeconds: 3600,
			bookCount: 6,
			completedBooks: 2,
		}
		setup([])

		const chips = screen.getAllByRole('listitem').map((item) => item.textContent ?? '')
		expect(chips.slice(0, 2)).toEqual(['6 statChips.books', '2 statChips.read(2)'])
		expect(chips[2]).toMatch(/\bGB$|\bGiB$/)
		// the in-progress count and the reading time are in the info sheet
		expect(chips).toHaveLength(3)
	})

	it('gives the word "read" the number of books read, so Spanish says 1 leído, not 1 leídos', () => {
		permissions.stats = {
			totalBytes: 0,
			totalReadingTimeSeconds: 0,
			bookCount: 6,
			completedBooks: 1,
		}
		setup([])

		const chips = screen.getAllByRole('listitem').map((item) => item.textContent ?? '')
		expect(chips.slice(0, 2)).toEqual(['6 statChips.books', '1 statChips.read(1)'])
	})
})
