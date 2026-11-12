import type { Ordering } from '@/components/filters/context'

/**
 * What the books of a search are ranked by: the search itself, so the best title comes first. When
 * a sort was chosen it decides the order alone, and with nothing searched there is nothing to rank.
 */
export function relevanceFor(search: string | undefined, ordering: Ordering): string | undefined {
	const text = search?.trim()
	if (!text) return undefined
	return ordering?.orderBy && ordering?.direction ? undefined : text
}
