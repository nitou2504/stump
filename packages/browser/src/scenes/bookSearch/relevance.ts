import type { OrderDirection } from '@stump/graphql'

import type { Ordering, OrderingField } from '@/components/filters/context'

/**
 * The sort a page's address carries (`orderBy` and `direction` together), or none. A sort that
 * was only saved in the browser, from the Explore page for instance, is not in the address, so it
 * was not chosen on this search.
 */
export function orderingInUrl(params: URLSearchParams): Ordering {
	const orderBy = params.get('orderBy') as OrderingField | null
	const direction = params.get('direction') as OrderDirection | null
	return orderBy && direction ? { orderBy, direction } : {}
}

/**
 * What the books of a search are ranked by: the search itself, so the best title comes first.
 * Pass the sort the reader chose on this search (`orderingInUrl`): it decides the order alone. A
 * sort that was only saved in the browser is not one, it orders the books inside each rank. With
 * nothing searched there is nothing to rank.
 */
export function relevanceFor(search: string | undefined, chosen: Ordering): string | undefined {
	const text = search?.trim()
	if (!text) return undefined
	return chosen?.orderBy && chosen?.direction ? undefined : text
}
