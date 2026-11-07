import type { ChapterPages } from '../../shared/readingProgress'

/**
 * Fixed-layout (pre-paginated) EPUBs: every page of the book is its own document, drawn as
 * the publisher placed it and scaled as a whole by Readium's fixed-layout reader.
 */

/**
 * Pages side by side, from the Readium `columnCount` the reader asked for (see
 * `resolveEffectiveColumnCount`): two only when two were picked and fit, otherwise one.
 * Readium's fixed-layout reader starts in two-page spreads on a landscape screen whatever the
 * preferences say, so this is set on it after it loads.
 */
export function fixedLayoutPerPage(columnCount: number | null | undefined): 1 | 2 {
	return columnCount === 2 ? 2 : 1
}

/**
 * The page of its chapter a fixed-layout page is, counted in pages (one position each): the
 * chapter runs from the last contents start at or before the page to the next one (or the
 * book's end). `chapterStarts` are the chapters' total progressions, sorted; `position` is the
 * page's 1-based position of `total`. Null before the first chapter or without positions.
 */
export function fixedLayoutChapterPages(
	chapterStarts: number[],
	position: number | undefined,
	total: number,
): ChapterPages | null {
	if (!position || total < 1 || position > total) return null
	const toIndex = (progression: number) => Math.round(progression * total)
	const index = position - 1
	const starts = [...new Set(chapterStarts.map(toIndex))].sort((a, b) => a - b)
	const start = starts.filter((s) => s <= index).pop()
	if (start === undefined) return null
	const end = starts.find((s) => s > index) ?? total
	const pages = end - start
	const page = index - start + 1
	return { page, pages, left: pages - page }
}
