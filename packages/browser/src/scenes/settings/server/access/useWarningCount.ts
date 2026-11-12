import { useGraphQL } from '@stump/client'

import { PUBLIC_ACCESS_KEY, warningCountQuery } from './graphql'

/**
 * How many things *Public access* has for the owner to look at: the badge of the sidebar and of
 * the settings navigation. It asks only for a number, once a minute, and only for the server
 * owner (the server refuses anybody else, and a request through the public address)
 */
export function useAccessWarningCount(enabled: boolean) {
	const result = useGraphQL(warningCountQuery, [PUBLIC_ACCESS_KEY, 'warningCount'], undefined, {
		enabled,
		refetchInterval: 60_000,
	})
	return enabled ? (result.data?.accessWarningCount ?? 0) : 0
}
