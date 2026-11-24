import { useSDK } from '@stump/client'
import { useQuery } from '@tanstack/react-query'

/**
 * Passages of the books' text that match `q`. A server that has no index, or a failure, is an
 * empty answer: the search of titles does not depend on it.
 */
export function useContentSearch(q: string, enabled: boolean, pageSize: number, page = 1) {
	const { sdk } = useSDK()
	return useQuery({
		queryKey: [sdk.content.keys.search, q, pageSize, page],
		queryFn: ({ signal }) => sdk.content.search({ q, page, page_size: pageSize, signal }),
		enabled: enabled && q.length > 0,
		placeholderData: (previous) => previous,
		staleTime: 60_000,
		retry: false,
	})
}
