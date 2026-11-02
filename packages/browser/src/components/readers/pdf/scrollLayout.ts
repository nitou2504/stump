/**
 * The continuous (scrolling) layout of the PDF reader: every page has a placeholder of its
 * own size, stacked with a gap, so the scroll height is right from the start and never
 * jumps. Only the pages near the viewport are mounted.
 */

export type PageSlot = {
	/** 0-based page index */
	index: number
	top: number
	height: number
	width: number
}

export type ScrollLayout = {
	slots: PageSlot[]
	totalHeight: number
	gap: number
}

/** Most pages kept mounted at once (the visible ones plus a margin) */
export const KEEP_PAGES = 7

/** Stacks pages of the given CSS sizes top to bottom with a gap before, between and after */
export function buildScrollLayout(
	sizes: Array<{ width: number; height: number }>,
	gap: number,
): ScrollLayout {
	const slots: PageSlot[] = []
	let top = gap
	sizes.forEach((size, index) => {
		slots.push({ index, top, height: size.height, width: size.width })
		top += size.height + gap
	})
	return { slots, totalHeight: top, gap }
}

/** The index of the last slot whose top is at or above `offset` (binary search) */
export function slotAt(layout: ScrollLayout, offset: number): number {
	const { slots } = layout
	if (!slots.length) return -1
	let low = 0
	let high = slots.length - 1
	while (low < high) {
		const mid = Math.ceil((low + high) / 2)
		if (slots[mid]!.top <= offset) {
			low = mid
		} else {
			high = mid - 1
		}
	}
	return low
}

/** The 0-based indexes of the pages intersecting `[scrollTop, scrollTop + viewportHeight]` */
export function visibleRange(
	layout: ScrollLayout,
	scrollTop: number,
	viewportHeight: number,
): [number, number] | null {
	if (!layout.slots.length) return null
	const first = Math.max(0, slotAt(layout, scrollTop))
	const firstSlot = layout.slots[first]!
	// the gap below a page belongs to no page; start at the next one if we're in it
	const start =
		firstSlot.top + firstSlot.height < scrollTop
			? Math.min(first + 1, layout.slots.length - 1)
			: first
	const end = Math.max(start, slotAt(layout, scrollTop + viewportHeight))
	return [start, end]
}

/**
 * The page that fills most of the viewport (the "current" page for progress, bookmarks and
 * the page counter). On ties the upper page wins.
 */
export function dominantPage(
	layout: ScrollLayout,
	scrollTop: number,
	viewportHeight: number,
): number {
	const range = visibleRange(layout, scrollTop, viewportHeight)
	if (!range) return 0
	let best = range[0]
	let bestArea = -1
	for (let index = range[0]; index <= range[1]; index++) {
		const slot = layout.slots[index]!
		const visible =
			Math.min(slot.top + slot.height, scrollTop + viewportHeight) - Math.max(slot.top, scrollTop)
		if (visible > bestArea) {
			best = index
			bestArea = visible
		}
	}
	return best
}

/**
 * Which pages to mount: the visible ones and an overscan of one viewport above and below,
 * at most `keep` pages, centred on the dominant page.
 */
export function mountRange(
	layout: ScrollLayout,
	scrollTop: number,
	viewportHeight: number,
	keep = KEEP_PAGES,
): [number, number] | null {
	const range = visibleRange(layout, Math.max(0, scrollTop - viewportHeight), viewportHeight * 3)
	if (!range) return null
	let [start, end] = range
	if (end - start + 1 > keep) {
		const center = dominantPage(layout, scrollTop, viewportHeight)
		start = Math.max(range[0], center - Math.floor((keep - 1) / 2))
		end = Math.min(range[1], start + keep - 1)
		start = Math.max(range[0], end - keep + 1)
	}
	return [start, end]
}

/**
 * Where to scroll to show a page (0-based) with `progression` (0-1) of it at the top of the
 * viewport, e.g. to keep the reading position when the zoom changes.
 */
export function scrollTopFor(layout: ScrollLayout, index: number, progression = 0): number {
	const slot = layout.slots[Math.max(0, Math.min(index, layout.slots.length - 1))]
	if (!slot) return 0
	if (progression <= 0) return Math.max(0, slot.top - layout.gap)
	return Math.max(0, slot.top + slot.height * Math.min(1, progression))
}

/** How far down the page at the top of the viewport is scrolled (0-1) */
export function progressionWithin(layout: ScrollLayout, index: number, scrollTop: number): number {
	const slot = layout.slots[index]
	if (!slot || slot.height <= 0) return 0
	return Math.min(1, Math.max(0, (scrollTop - slot.top) / slot.height))
}

/**
 * The pages to fetch ahead of time around the current one (0-based): the next `ahead` and
 * the previous `behind`, nearest first, skipping out-of-range pages.
 */
export function prefetchIndexes(
	current: number,
	pageCount: number,
	ahead = 2,
	behind = 1,
): number[] {
	const result: number[] = []
	for (let step = 1; step <= Math.max(ahead, behind); step++) {
		if (step <= ahead && current + step < pageCount) result.push(current + step)
		if (step <= behind && current - step >= 0) result.push(current - step)
	}
	return result
}
