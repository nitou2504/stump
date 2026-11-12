import { PREFETCH_STALE_TIME, useSDK, useSuspenseGraphQL } from '@stump/client'
import { Breadcrumbs, ButtonOrLink } from '@stump/components'
import { graphql } from '@stump/graphql'
import { Api } from '@stump/sdk'
import { QueryClient, QueryKey, useQueryClient } from '@tanstack/react-query'
import { useCallback, useState } from 'react'
import { Helmet } from 'react-helmet'

import { SceneContainer } from '@/components/container'
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
import DiscoverBooks, { usePrefetchDiscoverBooks } from './DiscoverBooks'
import { HomeEmptySectionsProvider, WatchedSection } from './emptySections'
import NothingYet from './NothingYet'
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
	const prefetchDiscover = usePrefetchDiscoverBooks()

	const { sdk } = useSDK()
	const client = useQueryClient()
	const queryKey = useHomeArrangementKey()
	return () =>
		prefetchHomeSections(client, sdk, queryKey, {
			continueReading: prefetchContinueReading,
			onDeck: prefetchOnDeck,
			discover: prefetchDiscover,
			recentlyAddedBooks: prefetchRecentMedia,
			recentlyAddedSeries: prefetchRecentSeries,
		})
}

export default function HomeScene() {
	const { translate } = useTranslate()
	const paths = usePaths()
	const { data: arrangement } = useHomeArrangement()
	// which of the sections that hide themselves when empty have said they are
	const [emptySections, setEmptySections] = useState<Partial<Record<WatchedSection, boolean>>>({})
	const reportEmpty = useCallback(
		(section: WatchedSection, empty: boolean) =>
			setEmptySections((previous) =>
				previous[section] === empty ? previous : { ...previous, [section]: empty },
			),
		[],
	)
	const { sdk } = useSDK()
	const { data } = useSuspenseGraphQL(query, sdk.cacheKey('numberOfLibraries'))

	const helmet = (
		<>
			<Helmet>
				{/* Doing this so Helmet splits the title into an array, I'm not just insane lol */}
				<title>Stump | {translate('shared.common.home')}</title>
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
		discover: DiscoverBooks,
		recentlyAddedBooks: RecentlyAddedMedia,
		recentlyAddedSeries: RecentlyAddedSeries,
	}

	// a reader with nothing in progress and nothing next is told so once, not by a box per section
	const watched = sections
		.filter((section) => section.visible)
		.map((section) => getHomeSectionId(section))
		.filter((id): id is WatchedSection => id === 'continueReading' || id === 'onDeck')
	const nothingYet = watched.length > 0 && watched.every((id) => emptySections[id])

	return (
		<HomeEmptySectionsProvider value={reportEmpty}>
			{helmet}

			<SceneContainer className="gap-6 flex flex-1 flex-col">
				{nothingYet && <NothingYet />}

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
		</HomeEmptySectionsProvider>
	)
}
