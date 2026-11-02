/**
 * How a finger moves the PDF pages on a touch screen. At the fit (not zoomed in) a page must
 * stay put under a stray finger: only the reading direction scrolls, sideways never does (no
 * wobble, no stretch), and in the paged layout a deliberate sideways swipe turns the page,
 * without dragging the page along. Zoomed in, the page pans freely, as a zoomed page should.
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

/** How far a finger must travel sideways for a swipe to turn the page, in CSS px */
export const SWIPE_MIN_DISTANCE = 60
/** A swipe is quick; a slower drag is reading, selecting or panning */
export const SWIPE_MAX_DURATION_MS = 600
/** Mostly sideways: the horizontal travel is at least this many times the vertical */
export const SWIPE_AXIS_RATIO = 1.5

export type Swipe = {
	/** The finger's travel, end minus start, in CSS px */
	dx: number
	dy: number
	durationMs: number
}

/**
 * The page a one-finger swipe turns to: `1` the next (the finger moved to the left), `-1`
 * the previous, `0` when it isn't a deliberate sideways swipe (too short, too slow or too
 * vertical, e.g. a scroll down a tall page)
 */
export function swipeTurn({ dx, dy, durationMs }: Swipe): -1 | 0 | 1 {
	if (durationMs > SWIPE_MAX_DURATION_MS) return 0
	if (Math.abs(dx) < SWIPE_MIN_DISTANCE) return 0
	if (Math.abs(dx) < Math.abs(dy) * SWIPE_AXIS_RATIO) return 0
	return dx < 0 ? 1 : -1
}
