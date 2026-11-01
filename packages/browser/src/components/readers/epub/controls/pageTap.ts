/**
 * Fired (bubbling, on the reader's page container) when the reader is tapped or clicked
 * inside the book's frame. Readium only reports a tap that is not on a link, an annotation
 * or a text selection, so the page itself stays selectable: no overlay is needed on top of it.
 */
export const READER_PAGE_TAP_EVENT = 'stump:reader-page-tap'

export type ReaderPageTapDetail = {
	/** Where the tap landed across the page, from 0 (left edge) to 1 (right edge) */
	xFraction: number
}

/** The share of the page width, at each side, that turns the page when tapped on a phone */
export const SIDE_TAP_ZONE = 0.15

export type PageTapZone = 'left' | 'middle' | 'right'

export function pageTapZone(xFraction: number): PageTapZone {
	if (xFraction < SIDE_TAP_ZONE) return 'left'
	if (xFraction > 1 - SIDE_TAP_ZONE) return 'right'
	return 'middle'
}
