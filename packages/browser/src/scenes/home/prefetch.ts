import { Api } from '@stump/sdk'
import { QueryClient, QueryKey } from '@tanstack/react-query'

import { prefetchContinueReading } from './ContinueReading'
import { prefetchDiscoverBooks } from './DiscoverBooks'
import { prefetchHomeSections } from './HomeScene'
import { prefetchOnDeck } from './OnDeck'
import { prefetchRecentlyAddedMedia } from './RecentlyAddedMedia'
import { prefetchRecentlyAddedSeries } from './RecentlyAddedSeries'

/**
 * The home page's prefetch without React hooks, so the always-loaded sidebar can import it on
 * demand instead of bundling the whole home page into the first download
 */
export function prefetchHomeScene(client: QueryClient, sdk: Api, queryKey: QueryKey) {
	return prefetchHomeSections(client, sdk, queryKey, {
		continueReading: () => prefetchContinueReading(client, sdk),
		onDeck: () => prefetchOnDeck(client, sdk),
		discover: () => prefetchDiscoverBooks(client, sdk),
		recentlyAddedBooks: () => prefetchRecentlyAddedMedia(client, sdk),
		recentlyAddedSeries: () => prefetchRecentlyAddedSeries(client, sdk),
	})
}
