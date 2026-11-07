import { FXLFrameManager } from '@readium/navigator'

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

type PageSize = { width: number; height: number }

/** The last size read from each fixed-layout page (see `patchFixedLayoutPageSize`) */
const lastPageSizes = new WeakMap<object, PageSize>()

/**
 * Readium's fixed-layout reader sizes every page it holds on each resize from the page's
 * viewport meta (or, without one, its body). A page being loaded again at that moment (pages
 * far from the reader are unloaded and come back) has no body yet: the resize threw
 * ("Cannot read properties of null (reading 'scrollWidth')") and left the remaining pages
 * unsized. Such a page keeps the size it had (or its frame's) until it has loaded.
 *
 * Patches `FXLFrameManager.prototype.loadPageSize` (`@readium/navigator` 2.8.2, its private
 * `frame` read directly); re-check on a Readium upgrade.
 */
export function patchFixedLayoutPageSize(): void {
	const proto = FXLFrameManager.prototype as unknown as {
		loadPageSize: (this: FixedLayoutFrame) => PageSize
		__stumpPageSize?: boolean
	}
	if (proto.__stumpPageSize) return
	proto.__stumpPageSize = true

	const original = proto.loadPageSize
	proto.loadPageSize = function loadPageSizeWhenParsed(this: FixedLayoutFrame) {
		const doc = this.frame?.contentWindow?.document
		if (doc?.head && doc.body) {
			const size = original.call(this)
			lastPageSizes.set(this, size)
			return size
		}
		return (
			lastPageSizes.get(this) ?? {
				width: Number.parseFloat(this.frame?.style.width ?? '') || this.frame?.clientWidth || 1,
				height: Number.parseFloat(this.frame?.style.height ?? '') || this.frame?.clientHeight || 1,
			}
		)
	}
}

type FixedLayoutFrame = { frame?: HTMLIFrameElement }
