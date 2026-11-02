/**
 * How a finger moves the PDF pages on a touch screen. At the fit (not zoomed in) a page must
 * stay put under a stray finger: only the reading direction scrolls, sideways never does (no
 * wobble, no stretch), and in the paged layout a deliberate sideways swipe turns the page,
 * without dragging the page along (the swipe rule is every reader's, `shared/gestures`).
 * Zoomed in, the page pans freely, as a zoomed page should.
 */

/** Zoomed in past the fit (anything at or under the fit fits the screen's width) */
export function isZoomedIn(zoom: number): boolean {
	return zoom > 1.001
}

/** The scroller's touch handling and sideways overflow for a zoom */
export function scrollerTouchStyle(zoomedIn: boolean): {
	touchAction: 'pan-x pan-y' | 'pan-y'
	overflowX: 'auto' | 'hidden'
} {
	return zoomedIn
		? { touchAction: 'pan-x pan-y', overflowX: 'auto' }
		: // vertical scrolling only: the browser doesn't pan sideways, and a pixel of rounding
			// in the pages' width can't make the content wobble from side to side
			{ touchAction: 'pan-y', overflowX: 'hidden' }
}
