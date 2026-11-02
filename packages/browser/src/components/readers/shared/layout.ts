/**
 * The page layout shared by every reader (EPUB, PDF, image/comic): one page or two side by
 * side (a spread), turned page by page or scrolled continuously. Four combinations, picked
 * from the same 2×2 picker in each reader's settings.
 */

export type PageSpread = 'single' | 'double'
export type PageFlow = 'paged' | 'continuous'

export type ReaderLayout = { spread: PageSpread; flow: PageFlow }

/** The picker's options, row by row: paged first, then continuous; single, then double */
export const LAYOUT_OPTIONS: ReaderLayout[] = [
	{ spread: 'single', flow: 'paged' },
	{ spread: 'double', flow: 'paged' },
	{ spread: 'single', flow: 'continuous' },
	{ spread: 'double', flow: 'continuous' },
]

/**
 * Below this width two pages side by side are too small to read, so the double layouts are
 * disabled (and a stored double layout shows one page)
 */
export const DOUBLE_PAGE_MIN_WIDTH = 900

export function canShowTwoPages(viewportWidth: number): boolean {
	return viewportWidth >= DOUBLE_PAGE_MIN_WIDTH
}

export function sameLayout(a: ReaderLayout, b: ReaderLayout): boolean {
	return a.spread === b.spread && a.flow === b.flow
}

/**
 * The layout actually shown: a double layout falls back to one page on a narrow screen
 * (the preference is kept, so turning the phone or widening the window brings it back)
 */
export function effectiveLayout(layout: ReaderLayout, viewportWidth: number): ReaderLayout {
	if (layout.spread === 'double' && viewportWidth > 0 && !canShowTwoPages(viewportWidth)) {
		return { ...layout, spread: 'single' }
	}
	return layout
}

/**
 * Groups pages (0-based) into the rows a layout shows: one page per row, or pairs. With
 * `coverAlone` the first page (the cover) gets a row of its own and the pairs start at the
 * second page, like an open book; otherwise pairs start at the first page.
 */
export function spreadRows(
	pageCount: number,
	spread: PageSpread,
	{ coverAlone = true }: { coverAlone?: boolean } = {},
): number[][] {
	const rows: number[][] = []
	if (pageCount <= 0) return rows
	if (spread === 'single') {
		for (let index = 0; index < pageCount; index++) rows.push([index])
		return rows
	}
	let index = 0
	if (coverAlone) {
		rows.push([0])
		index = 1
	}
	for (; index < pageCount; index += 2) {
		rows.push(index + 1 < pageCount ? [index, index + 1] : [index])
	}
	return rows
}

/** The row (in `rows`) that shows a page, or -1 */
export function rowOfPage(rows: number[][], pageIndex: number): number {
	// rows are in page order: binary search on the first page of each row
	let low = 0
	let high = rows.length - 1
	while (low <= high) {
		const mid = (low + high) >> 1
		const row = rows[mid]!
		const first = row[0]!
		const last = row[row.length - 1]!
		if (pageIndex < first) high = mid - 1
		else if (pageIndex > last) low = mid + 1
		else return mid
	}
	return -1
}

/**
 * The page indicator's text for the pages in view (1-based), e.g. `23 / 327` or, for a
 * spread, `22–23 / 327`
 */
export function pageIndicatorText(pages: number[], pageCount: number): string {
	if (!pages.length) return ''
	const first = Math.min(...pages)
	const last = Math.max(...pages)
	const shown = first === last ? `${first}` : `${first}–${last}`
	return `${shown} / ${pageCount}`
}
