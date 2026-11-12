import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import { PropsWithChildren } from 'react'
import { MemoryRouter } from 'react-router'

import { localeWrapper } from '@/__mocks__/locale'

import SeriesSettingsScene from '../SeriesSettingsScene'

vi.mock('@stump/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('@stump/client')>()),
	useSDK: () => ({ sdk: { cacheKey: () => ['series'] } }),
	useSuspenseGraphQL: () => ({ data: { seriesById: { id: 's', tags: [], metadata: {} } } }),
	useGraphQLMutation: () => ({ mutate: vi.fn() }),
}))
vi.mock('@/components/series/context', () => ({
	useSeriesContext: () => ({ series: { id: 's', kind: 'DEFAULT', library: { id: 'l' } } }),
}))
vi.mock('@/components/series/metadata', () => ({ SeriesMetadataEditor: () => null }))
vi.mock('@/components/series/SeriesTagEditor', () => ({ default: () => null }))
vi.mock('@/components/metadata/metadataEditor', () => ({ ResetMetadata: () => null }))
vi.mock('@/components/container', () => ({
	SceneContainer: ({ children }: PropsWithChildren) => <div>{children}</div>,
}))
vi.mock('@/components/sharedLayout', () => ({
	SettingsSection: ({
		title,
		description,
		children,
	}: PropsWithChildren<{ title: string; description: string }>) => (
		<section>
			<h2>{title}</h2>
			<p>{description}</p>
			{children}
		</section>
	),
}))
vi.mock('@/context', () => ({ useAppContext: () => ({ checkPermission: () => true }) }))
vi.mock('@/paths', () => ({ usePaths: () => new Proxy({}, { get: () => () => '/somewhere' }) }))

function render_(locale: 'en-US' | 'es-ES') {
	const Locale = localeWrapper(locale)
	const client = new QueryClient()
	return render(
		<Locale>
			<QueryClientProvider client={client}>
				<MemoryRouter>
					<SeriesSettingsScene />
				</MemoryRouter>
			</QueryClientProvider>
		</Locale>,
	)
}

describe('SeriesSettingsScene', () => {
	it('names its section in Spanish', () => {
		render_('es-ES')
		expect(screen.getByRole('heading', { name: 'Metadatos' })).toBeInTheDocument()
		expect(screen.getByText('Información adicional sobre tu serie')).toBeInTheDocument()
		expect(screen.queryByText('Extra information about your series')).toBeNull()
	})

	it('reads the same in English as before', () => {
		render_('en-US')
		expect(screen.getByRole('heading', { name: 'Metadata' })).toBeInTheDocument()
		expect(screen.getByText('Extra information about your series')).toBeInTheDocument()
	})
})
