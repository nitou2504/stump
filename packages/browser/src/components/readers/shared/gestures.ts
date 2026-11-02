/**
 * How a finger or a mouse moves through a book, the same in every reader (EPUB, PDF and
 * comics): where a tap lands and what it does, and when a drag is a deliberate swipe that
 * turns the page. The values follow the readers studied in the manga research (Mihon,
 * KOReader, Komga, Kavita; see FORK.md):
 *
 * - Tap zones, in the layouts where pages turn: a third of the width at each side on a touch
 *   screen (Mihon's right-and-left zones), a quarter with a mouse (Komga, Kavita, KOReader).
 *   The middle shows or hides the controls. The zones are screen sides: a reader maps them to
 *   forward or back for its reading direction (right to left: the left side goes forward).
 *   In a scrolled layout every tap is a middle tap: scrolling is the way through the book.
 * - Swipes: the page doesn't follow the finger (no drag, no wobble); a drag is judged when
 *   the finger lifts. It must move past the 16 px slop, mostly sideways (the cross-axis
 *   travel under half the main one), within 900 ms, and be either a quick flick or a drag
 *   across a quarter of the screen.
 */

// Tap zones -------------------------------------------------------------------------------------

/** The share of the width at each side that turns the page on a touch screen */
export const TOUCH_SIDE_ZONE = 1 / 3
/** The same with a mouse */
export const MOUSE_SIDE_ZONE = 0.25

export type PageTapZone = 'left' | 'middle' | 'right'

/** The width of the side zones for an input */
export function sideZoneWidth(coarse: boolean): number {
	return coarse ? TOUCH_SIDE_ZONE : MOUSE_SIDE_ZONE
}

/** The zone of a tap, from its position across the page (0 = left edge, 1 = right edge) */
export function pageTapZone(xFraction: number, coarse: boolean): PageTapZone {
	const side = sideZoneWidth(coarse)
	if (xFraction < side) return 'left'
	if (xFraction > 1 - side) return 'right'
	return 'middle'
}

type NavigationTapOptions = {
	/** `paged`: pages turn; `continuous`: the book scrolls, so there is no page to turn */
	flow: 'paged' | 'continuous'
	/** A touch screen (a third at each side) or a mouse (a quarter) */
	coarse: boolean
	/** The reader's 'tap the sides to turn' preference (on unless turned off) */
	sidesNavigate?: boolean
}

/**
 * What a tap does: the side zones turn the page only when pages turn. In a scrolled
 * (continuous) layout every tap is a middle tap, because the sides have nothing to turn: an
 * EPUB's 'next page' there is the next chapter, which a stray tap must not jump to.
 */
export function navigationTapZone(
	xFraction: number,
	{ flow, coarse, sidesNavigate = true }: NavigationTapOptions,
): PageTapZone {
	if (flow === 'continuous' || !sidesNavigate) return 'middle'
	return pageTapZone(xFraction, coarse)
}

/**
 * The screen direction of a turn: the right side (or a swipe to the left) shows what's to
 * the right, `1`; the left side `-1`. A reader turns that into forward or back for its
 * reading direction: in a right-to-left book `1` is back.
 */
export function zoneTurn(zone: PageTapZone): -1 | 0 | 1 {
	return zone === 'right' ? 1 : zone === 'left' ? -1 : 0
}

/** Forward (`1`) or back (`-1`) in the book for a screen direction */
export function bookTurn(screenTurn: -1 | 0 | 1, rtl: boolean): -1 | 0 | 1 {
	if (!rtl || screenTurn === 0) return screenTurn
	return screenTurn === 1 ? -1 : 1
}

// Swipes ----------------------------------------------------------------------------------------

/** A touch moving less than this is still a tap (Android's paging slop, Vuetify's 16 px) */
export const SWIPE_SLOP = 16
/** Lifted later than this, a drag isn't a swipe (KOReader) */
export const SWIPE_MAX_MS = 900
/** A fling: this far at this speed (Android's ViewPager: 25 dp at 400 dp/s) */
export const FLING_DISTANCE = 25
export const FLING_SPEED = 0.4
/** Or a slower drag across this share of the screen */
export const SWIPE_FRACTION = 0.25

/** Which way the finger moved: `left` shows what's to the right */
export type SwipeDirection = 'left' | 'right' | null

/**
 * A finished one-finger drag that the page didn't follow: a deliberate sideways swipe, or
 * nothing. Mostly sideways (the vertical travel under half the horizontal one), lifted within
 * 900 ms, and a quick flick or a drag across a quarter of the screen.
 */
export function swipeOf(dx: number, dy: number, ms: number, width: number): SwipeDirection {
	const distance = Math.abs(dx)
	if (distance < SWIPE_SLOP || Math.abs(dy) >= 0.5 * distance || ms > SWIPE_MAX_MS) return null
	const fling = distance > FLING_DISTANCE && distance / Math.max(1, ms) > FLING_SPEED
	if (!fling && distance < width * SWIPE_FRACTION) return null
	return dx < 0 ? 'left' : 'right'
}

/** The screen direction of a swipe's turn: a finger moving left turns to the right (`1`) */
export function swipeTurn(swipe: SwipeDirection): -1 | 0 | 1 {
	return swipe === 'left' ? 1 : swipe === 'right' ? -1 : 0
}

/** Whether the main input is a finger (a touch screen, at any width) */
export function isCoarsePointer(): boolean {
	return typeof window !== 'undefined' && !!window.matchMedia?.('(pointer: coarse)').matches
}
