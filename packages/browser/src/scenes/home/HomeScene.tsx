import { PREFETCH_STALE_TIME, useSDK, useSuspenseGraphQL } from '@stump/client'
import { Breadcrumbs, ButtonOrLink } from '@stump/components'
import { graphql } from '@stump/graphql'
import { Api } from '@stump/sdk'
import { QueryClient, QueryKey, useQueryClient } from '@tanstack/react-query'
import { Helmet } from 'react-helmet'

import { SceneContainer } from '@/components/container'
import { FilterHeader } from '@/components/filters'
import { NoLibraries } from '@/components/library'
import { OwlEmptyState } from '@/components/Owl'
import { PageHeader } from '@/components/sharedLayout'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import {
	getHomeSectionId,
	homeArrangementQuery,
	HomeSectionId,
	useHomeArrangement,
	useHomeArrangementKey,
} from './arrangement'
import ContinueReadingMedia, { usePrefetchContinueReading } from './ContinueReading'
import HomeSearch from './HomeSearch'
import OnDeck, { usePrefetchOnDeck } from './OnDeck'
import RecentlyAddedMedia, { usePrefetchRecentlyAddedMedia } from './RecentlyAddedMedia'
import RecentlyAddedSeries, { usePrefetchRecentlyAddedSeries } from './RecentlyAddedSeries'

const query = graphql(`
	query HomeSceneQuery {
		numberOfLibraries
	}
`)

type HomeSectionPrefetchers = Record<HomeSectionId, () => Promise<unknown>>

/**
 * Prefetches the home sections the user has turned on. Shared by the hook below and by the
 * sidebar, which loads it (with `./prefetch`) only when the Home link is hovered
 */
export async function prefetchHomeSections(
	client: QueryClient,
	sdk: Api,
	queryKey: QueryKey,
	prefetch: HomeSectionPrefetchers,
) {
	const data = await client
		.fetchQuery({
			queryKey,
			queryFn: () => sdk.execute(homeArrangementQuery),
			staleTime: PREFETCH_STALE_TIME,
		})
		.catch(() => undefined)
	if (!data) return
	await Promise.all(
		data.me.preferences.homeArrangement.sections.map((section) => {
			const id = getHomeSectionId(section)
			return section.visible && id ? prefetch[id]() : undefined
		}),
	)
}

export const usePrefetchHomeScene = () => {
	const prefetchRecentMedia = usePrefetchRecentlyAddedMedia()
	const prefetchContinueReading = usePrefetchContinueReading()
	const prefetchRecentSeries = usePrefetchRecentlyAddedSeries()
	const prefetchOnDeck = usePrefetchOnDeck()

	const { sdk } = useSDK()
	const client = useQueryClient()
	const queryKey = useHomeArrangementKey()
	return () =>
		prefetchHomeSections(client, sdk, queryKey, {
			continueReading: prefetchContinueReading,
			onDeck: prefetchOnDeck,
			recentlyAddedBooks: prefetchRecentMedia,
			recentlyAddedSeries: prefetchRecentSeries,
		})
}

export default function HomeScene() {
	const { translate } = useTranslate()
	const paths = usePaths()
	const { data: arrangement } = useHomeArrangement()
	const { sdk } = useSDK()
	const { data } = useSuspenseGraphQL(query, sdk.cacheKey('numberOfLibraries'))

	const helmet = (
		<>
			<Helmet>
				{/* Doing this so Helmet splits the title into an array, I'm not just insane lol */}
				<title>Stump | {'Home'}</title>
			</Helmet>

			<PageHeader>
				<Breadcrumbs segments={[{ label: translate('shared.common.home') }]} />
			</PageHeader>
		</>
	)

	if (!data) {
		return null
	}

	const { numberOfLibraries } = data

	if (numberOfLibraries === 0) {
		return (
			<>
				{helmet}
				<NoLibraries />
			</>
		)
	}

	const sections = arrangement.me.preferences.homeArrangement.sections
	const components = {
		continueReading: ContinueReadingMedia,
		onDeck: OnDeck,
		recentlyAddedBooks: RecentlyAddedMedia,
		recentlyAddedSeries: RecentlyAddedSeries,
	}

	return (
		<>
			{helmet}

			{/* The search sits in the same toolbar as on the list pages (library books, explore) */}
			<FilterHeader search={<HomeSearch />} navOffset />

			<SceneContainer className="gap-6 flex flex-1 flex-col">
				{sections.map((section) => {
					const id = getHomeSectionId(section)
					if (!id || !section.visible) return null
					const Component = components[id]
					return <Component key={id} />
				})}

				{!sections.some((section) => section.visible && getHomeSectionId(section)) && (
					<OwlEmptyState
						title={translate('shared.homeSections.allSectionsHidden.label')}
						description={translate('shared.homeSections.allSectionsHidden.description')}
						actions={
							<ButtonOrLink
								href={paths.settings('preferences')}
								data-testid="customize-home-button"
							>
								{translate('shared.homeSections.allSectionsHidden.customizeHome')}
							</ButtonOrLink>
						}
					/>
				)}
				<div className="pb-5 sm:pb-0" />
			</SceneContainer>
		</>
	)
}
