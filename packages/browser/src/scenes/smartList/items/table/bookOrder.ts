import { Media } from '@stump/graphql'

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' })

const position = (book: Media) => book.metadata?.number ?? book.metadata?.volume ?? null

/**
 * The natural order of a smart list's books when the view has no explicit sorting:
 * by position in the series (number, then volume) when grouping by series, then by
 * title with numbers compared numerically (`v2` before `v10`). Smart list queries
 * themselves return books in no particular order.
 */
export const naturalBookOrder =
	(bySeriesPosition: boolean) =>
	(a: Media, b: Media): number => {
		if (bySeriesPosition) {
			const pa = position(a)
			const pb = position(b)
			if (pa != null && pb != null && pa !== pb) return pa - pb
			if (pa != null && pb == null) return -1
			if (pa == null && pb != null) return 1
		}
		return collator.compare(a.resolvedName, b.resolvedName)
	}

/** Groups by name, with the standalone (entity-less) group last */
export const naturalGroupOrder = (
	a: { entity?: { name: string } | null },
	b: { entity?: { name: string } | null },
): number => {
	if (!a.entity) return b.entity ? 1 : 0
	if (!b.entity) return -1
	return collator.compare(a.entity.name, b.entity.name)
}
