/**
 * Where a tap on a reader's page lands, shared by every reader so their tap zones behave
 * the same: a strip at each side turns the page, the middle shows or hides the controls.
 */

/** The share of the page width, at each side, that turns the page when tapped */
export const SIDE_TAP_ZONE = 0.15

export type PageTapZone = 'left' | 'middle' | 'right'

/** The zone of a tap, from its position across the page (0 = left edge, 1 = right edge) */
export function pageTapZone(xFraction: number): PageTapZone {
	if (xFraction < SIDE_TAP_ZONE) return 'left'
	if (xFraction > 1 - SIDE_TAP_ZONE) return 'right'
	return 'middle'
}

type NavigationTapOptions = {
	/** `paged`: pages turn; `continuous`: the book scrolls, so there is no page to turn */
	flow: 'paged' | 'continuous'
	/** The reader's 'tap the sides to turn' preference (on unless turned off) */
	sidesNavigate?: boolean
}

/**
 * What a tap does: the side strips turn the page only when pages turn. In a scrolled
 * (continuous) layout every tap is a middle tap, because the sides have nothing to turn: an
 * EPUB's 'next page' there is the next chapter, which a stray tap must not jump to; scrolling
 * is the way through the book.
 */
export function navigationTapZone(
	xFraction: number,
	{ flow, sidesNavigate = true }: NavigationTapOptions,
): PageTapZone {
	if (flow === 'continuous' || !sidesNavigate) return 'middle'
	return pageTapZone(xFraction)
}
