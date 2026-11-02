import { ReadingImageScaleFit } from '@stump/graphql'

/**
 * The geometry of the paged comic reader, kept free of React so it can be tested: which
 * views (a page, half a wide page, or a two-page spread) follow each other, how big a view
 * is drawn for a fit, and how the next/previous inputs move through a view that's bigger
 * than the screen before turning the page.
 *
 * Mostly after Mihon, Kavita and KOReader (see FORK.md for the references): a page that
 * doesn't fit is stepped through in screen-sized parts (KOReader), wide pages on a narrow
 * screen are zoomed so half of the spread fills the width and panned (Mihon) or cut in two
 * (Mihon, Kavita, KCC).
 */

/** A rectangle in an image's own pixels */
export type Box = { x: number; y: number; w: number; h: number }

/** The size of something on screen or in an image */
export type Size = { w: number; h: number }

/** What part of a page a view shows; halves are named by their side on screen */
export type PagePart = 'whole' | 'left' | 'right'

/** One page (0-based) or one half of it */
export type ViewItem = { page: number; part: PagePart }

/**
 * Wider than this (width / height) a page is a spread drawn on one image: Mihon's and
 * Komga's 'wide' test is width > height
 */
export const WIDE_RATIO = 1
/**
 * Halves are only made of spreads between these ratios (KCC's rule: narrower is a single
 * landscape page, wider is a panorama or a three-page fold-out, which stays whole)
 */
export const SPLIT_MIN_RATIO = 1.16
export const SPLIT_MAX_RATIO = 1.8

/** How a wide page is shown when one page is shown at a time on a narrow screen */
export type WidePages = 'zoom' | 'split' | 'fit'

export function isWide(ratio: number | undefined): boolean {
	return ratio !== undefined && ratio > WIDE_RATIO
}

export function canSplit(ratio: number | undefined): boolean {
	return ratio !== undefined && ratio > SPLIT_MIN_RATIO && ratio < SPLIT_MAX_RATIO
}

/**
 * The views in screen order (left to right; the page sets of a right-to-left book are
 * already reversed). With `split`, a lone wide page becomes its left and right halves: in
 * screen order the left half comes first whatever the direction, so going forward in a
 * right-to-left book (leftward) shows its right half first, as printed.
 */
export function buildViews(
	pageSets: number[][],
	ratioOf: (page: number) => number | undefined,
	split: boolean,
): ViewItem[][] {
	const views: ViewItem[][] = []
	for (const set of pageSets) {
		const only = set.length === 1 ? set[0] : undefined
		if (split && only !== undefined && canSplit(ratioOf(only))) {
			views.push([{ page: only, part: 'left' }], [{ page: only, part: 'right' }])
		} else {
			views.push(set.map((page) => ({ page, part: 'whole' })))
		}
	}
	return views
}

/** The view that shows a page (0-based), preferring its left half when it's split */
export function viewOfPage(views: ViewItem[][], page: number, prefer: PagePart = 'left'): number {
	let found = -1
	for (let index = 0; index < views.length; index++) {
		const view = views[index]!
		const match = view.find((item) => item.page === page)
		if (!match) continue
		if (match.part === 'whole' || match.part === prefer) return index
		if (found < 0) found = index
	}
	return found
}

/** The box of an item inside its image: the whole page, one half, both after cropping */
export function itemBox(
	part: PagePart,
	natural: Size,
	crop?: { whole?: Box | null; left?: Box | null; right?: Box | null } | null,
): Box {
	const half = Math.floor(natural.w / 2)
	if (part === 'left') return crop?.left ?? { x: 0, y: 0, w: half, h: natural.h }
	if (part === 'right') return crop?.right ?? { x: natural.w - half, y: 0, w: half, h: natural.h }
	return crop?.whole ?? { x: 0, y: 0, w: natural.w, h: natural.h }
}

/**
 * The size a view is drawn at for a fit, at zoom 1. The items of a spread sit side by side
 * at the same height. `screen` fits wholly inside (contain), `height` fills the height,
 * `width` fills the width, `original` is 1:1 (Mihon's `minScale()`, Komga's CSS sizes).
 */
export function fitSize(boxes: Size[], fit: ReadingImageScaleFit, viewport: Size): Size {
	if (!boxes.length || viewport.w <= 0 || viewport.h <= 0) return { w: 0, h: 0 }
	// at a common height of 1, each box is `ratio` wide
	const ratio = boxes.reduce((sum, box) => sum + box.w / Math.max(1, box.h), 0)
	const naturalHeight = Math.max(...boxes.map((box) => box.h))
	let height: number
	switch (fit) {
		case ReadingImageScaleFit.Height:
			height = viewport.h
			break
		case ReadingImageScaleFit.Width:
			height = viewport.w / ratio
			break
		case ReadingImageScaleFit.None:
			height = naturalHeight
			break
		case ReadingImageScaleFit.Auto:
		default:
			height = Math.min(viewport.h, viewport.w / ratio)
			break
	}
	return { w: height * ratio, h: height }
}

/**
 * A lone wide page on a narrow screen with the `zoom` setting: drawn so half of it fills the
 * screen's width (Mihon's wide-page zoom), never taller than the page at fit-height
 */
export function wideZoomSize(ratio: number, viewport: Size): Size {
	const height = Math.min(viewport.h, (2 * viewport.w) / ratio)
	return { w: height * ratio, h: height }
}

// Stepping through a view bigger than the screen ----------------------------------------------

/** Each step shows this share of the screen again (KOReader overlaps 40%, Komga 5%) */
export const STEP_OVERLAP = 0.15

/** Where the screen is over the view: the view's scroll offsets */
export type ViewPos = { x: number; y: number }

export type StepInput = {
	pos: ViewPos
	/** The view's drawn size */
	content: Size
	/** The screen's size */
	viewport: Size
	/** Which way the input moves on screen: right (+1) or left (-1) */
	toward: 1 | -1
	/** Whether that is forward in the book (rightward in a left-to-right book) */
	forward: boolean
	overlap?: number
}

const END_SLACK = 1

/**
 * The next part of the view in the direction of an input, or `'turn'` when the view has been
 * seen to its end that way (KOReader's `onGotoViewRel`): along the row first (right to left
 * for a right-to-left book, as manga panels read), then down to the next row (up when going
 * back), and only then the page turns. A view that fits the screen turns at once.
 */
export function stepView({
	pos,
	content,
	viewport,
	toward,
	forward,
	overlap = STEP_OVERLAP,
}: StepInput): ViewPos | 'turn' {
	const maxX = Math.max(0, content.w - viewport.w)
	const maxY = Math.max(0, content.h - viewport.h)
	const stepX = viewport.w * (1 - overlap)
	const stepY = viewport.h * (1 - overlap)

	// along the row, the way the input points
	const rowEnd = toward > 0 ? maxX : 0
	if (Math.abs(pos.x - rowEnd) > END_SLACK) {
		const x = toward > 0 ? Math.min(maxX, pos.x + stepX) : Math.max(0, pos.x - stepX)
		return { x, y: pos.y }
	}
	// then the next row: down going forward, up going back
	const columnEnd = forward ? maxY : 0
	if (Math.abs(pos.y - columnEnd) > END_SLACK) {
		const y = forward ? Math.min(maxY, pos.y + stepY) : Math.max(0, pos.y - stepY)
		// a new row starts at the side the input comes from
		return { x: toward > 0 ? 0 : maxX, y }
	}
	return 'turn'
}

/**
 * Where a view is entered after a turn: its first part going forward, its last part coming
 * back (KOReader's `goto_end`), the row starting at the side the input comes from
 */
export function entryPos(content: Size, viewport: Size, toward: 1 | -1, forward: boolean): ViewPos {
	const maxX = Math.max(0, content.w - viewport.w)
	const maxY = Math.max(0, content.h - viewport.h)
	return { x: toward > 0 ? 0 : maxX, y: forward ? 0 : maxY }
}
