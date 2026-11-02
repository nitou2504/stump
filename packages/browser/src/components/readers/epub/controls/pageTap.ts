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

// The zones themselves are shared by every reader (see `shared/tapZones`)
export { type PageTapZone, pageTapZone, SIDE_TAP_ZONE } from '../../shared/tapZones'
