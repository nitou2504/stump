/**
 * The continuous (scrolling) layout of the PDF reader: every page has a placeholder of its
 * own size, so the scroll height is right from the start and never jumps. Pages sit in rows:
 * one page per row, or two side by side (a spread), stacked with a gap before, between and
 * after the rows. Only the pages near the viewport are mounted.
 */

export type PageSlot = {
	/** 0-based page index */
	index: number
	/** The row's top */
	top: number
	height: number
	width: number
	/** From the row's left edge */
	left: number
	/** 0-based row index */
	row: number
}

export type RowSlot = {
	/** The 0-based page indexes shown side by side */
	pages: number[]
	top: number
	/** The tallest page's height */
	height: number
	/** The pages' widths and the gap between them */
	width: number
}

export type ScrollLayout = {
	slots: PageSlot[]
	rows: RowSlot[]
	totalHeight: number
	gap: number
	/** The widest row */
	maxRowWidth: number
}

/** Most pages kept mounted at once (the visible ones plus a margin) */
export const KEEP_PAGES = 7

/**
 * Stacks rows of pages of the given CSS sizes top to bottom with a gap before, between and
 * after them. `rows` lists the pages of each row (see `spreadRows`); by default every page
 * is a row of its own. In a row the pages are side by side with `gap` between them, and
 * vertically centred on the tallest.
 */
export function buildScrollLayout(
	sizes: Array<{ width: number; height: number }>,
	gap: number,
	rows: number[][] = sizes.map((_, index) => [index]),
): ScrollLayout {
	const slots: PageSlot[] = new Array(sizes.length)
	const rowSlots: RowSlot[] = []
	let top = gap
	let maxRowWidth = 0
	rows.forEach((pages, row) => {
		const height = Math.max(0, ...pages.map((index) => sizes[index]?.height ?? 0))
		let left = 0
		pages.forEach((index, position) => {
			const size = sizes[index] ?? { width: 0, height: 0 }
			if (position > 0) left += gap
			slots[index] = { index, top, height: size.height, width: size.width, left, row }
			left += size.width
		})
		rowSlots.push({ pages, top, height, width: left })
		maxRowWidth = Math.max(maxRowWidth, left)
		top += height + gap
	})
	return { slots, rows: rowSlots, totalHeight: top, gap, maxRowWidth }
}

/** The index of the last row whose top is at or above `offset` (binary search) */
export function rowAt(layout: ScrollLayout, offset: number): number {
	const { rows } = layout
	if (!rows.length) return -1
	let low = 0
	let high = rows.length - 1
	while (low < high) {
		const mid = Math.ceil((low + high) / 2)
		if (rows[mid]!.top <= offset) {
			low = mid
		} else {
			high = mid - 1
		}
	}
	return low
}

/** The index of the first page of the row at `offset` (see `rowAt`) */
export function slotAt(layout: ScrollLayout, offset: number): number {
	const row = rowAt(layout, offset)
	return row < 0 ? -1 : layout.rows[row]!.pages[0]!
}

/** The rows intersecting `[scrollTop, scrollTop + viewportHeight]` */
export function visibleRows(
	layout: ScrollLayout,
	scrollTop: number,
	viewportHeight: number,
): [number, number] | null {
	if (!layout.rows.length) return null
	const first = Math.max(0, rowAt(layout, scrollTop))
	const firstRow = layout.rows[first]!
	// the gap below a row belongs to no page; start at the next one if we're in it
	const start =
		firstRow.top + firstRow.height < scrollTop ? Math.min(first + 1, layout.rows.length - 1) : first
	const end = Math.max(start, rowAt(layout, scrollTop + viewportHeight))
	return [start, end]
}

function pageSpan(layout: ScrollLayout, rows: [number, number]): [number, number] {
	const first = layout.rows[rows[0]]!.pages
	const last = layout.rows[rows[1]]!.pages
	return [first[0]!, last[last.length - 1]!]
}

/** The 0-based indexes of the pages intersecting `[scrollTop, scrollTop + viewportHeight]` */
export function visibleRange(
	layout: ScrollLayout,
	scrollTop: number,
	viewportHeight: number,
): [number, number] | null {
	const rows = visibleRows(layout, scrollTop, viewportHeight)
	return rows ? pageSpan(layout, rows) : null
}

/** The row that fills most of the viewport. On ties the upper row wins */
export function dominantRow(
	layout: ScrollLayout,
	scrollTop: number,
	viewportHeight: number,
): number {
	const range = visibleRows(layout, scrollTop, viewportHeight)
	if (!range) return 0
	let best = range[0]
	let bestArea = -1
	for (let index = range[0]; index <= range[1]; index++) {
		const row = layout.rows[index]!
		const visible =
			Math.min(row.top + row.height, scrollTop + viewportHeight) - Math.max(row.top, scrollTop)
		if (visible > bestArea) {
			best = index
			bestArea = visible
		}
	}
	return best
}

/**
 * The page that fills most of the viewport (the "current" page for progress, bookmarks and
 * the page counter): the first page of the dominant row.
 */
export function dominantPage(
	layout: ScrollLayout,
	scrollTop: number,
	viewportHeight: number,
): number {
	if (!layout.rows.length) return 0
	return layout.rows[dominantRow(layout, scrollTop, viewportHeight)]!.pages[0]!
}

/**
 * Which pages to mount: the visible ones and an overscan of one viewport above and below,
 * at most about `keep` pages (whole rows), centred on the dominant row.
 */
export function mountRange(
	layout: ScrollLayout,
	scrollTop: number,
	viewportHeight: number,
	keep = KEEP_PAGES,
): [number, number] | null {
	const range = visibleRows(layout, Math.max(0, scrollTop - viewportHeight), viewportHeight * 3)
	if (!range) return null
	const perRow = Math.max(1, ...layout.rows.map((row) => row.pages.length))
	const keepRows = Math.max(1, Math.floor(keep / perRow))
	let [start, end] = range
	if (end - start + 1 > keepRows) {
		const center = dominantRow(layout, scrollTop, viewportHeight)
		start = Math.max(range[0], center - Math.floor((keepRows - 1) / 2))
		end = Math.min(range[1], start + keepRows - 1)
		start = Math.max(range[0], end - keepRows + 1)
	}
	return pageSpan(layout, [start, end])
}

/**
 * Where to scroll to show a page (0-based) with `progression` (0-1) of its row at the top of
 * the viewport, e.g. to keep the reading position when the zoom changes.
 */
export function scrollTopFor(layout: ScrollLayout, index: number, progression = 0): number {
	const slot = layout.slots[Math.max(0, Math.min(index, layout.slots.length - 1))]
	if (!slot) return 0
	const row = layout.rows[slot.row]!
	if (progression <= 0) return Math.max(0, row.top - layout.gap)
	return Math.max(0, row.top + row.height * Math.min(1, progression))
}

/** How far down the row of a page (0-based) the top of the viewport is (0-1) */
export function progressionWithin(layout: ScrollLayout, index: number, scrollTop: number): number {
	const slot = layout.slots[index]
	const row = slot ? layout.rows[slot.row] : undefined
	if (!row || row.height <= 0) return 0
	return Math.min(1, Math.max(0, (scrollTop - row.top) / row.height))
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
