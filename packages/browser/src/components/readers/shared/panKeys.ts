/**
 * Arrow keys on a zoomed page (PDF in page-by-page mode, comics): they move around the page
 * instead of turning it, like KOReader or a PDF viewer. At the page's edge in that direction
 * the next press turns the page. Page Up / Page Down always turn pages.
 */

/** How far one arrow press moves: about an eighth of the screen */
export const PAN_STEP = 0.12
/** Space moves by most of a screen */
export const SCREEN_STEP = 0.9

/** A page counts as zoomed when it's bigger than the fit by more than this */
export const ZOOMED_EPSILON = 0.01

export type ScrollBox = {
	scrollLeft: number
	scrollTop: number
	scrollWidth: number
	scrollHeight: number
	clientWidth: number
	clientHeight: number
}

export type PanResult = { type: 'pan'; left: number; top: number } | { type: 'turn'; delta: -1 | 1 }

export type PanDirection = { dx: -1 | 0 | 1; dy: -1 | 0 | 1 }

/** The pan an arrow key asks for, or `null` for other keys */
export function panDirectionOf(key: string): PanDirection | null {
	switch (key) {
		case 'ArrowLeft':
			return { dx: -1, dy: 0 }
		case 'ArrowRight':
			return { dx: 1, dy: 0 }
		case 'ArrowUp':
			return { dx: 0, dy: -1 }
		case 'ArrowDown':
			return { dx: 0, dy: 1 }
		default:
			return null
	}
}

/**
 * Moves a scrolled page by a step in a direction, or turns the page when it's already at
 * its edge in that direction (right or down: the next page; left or up: the previous one)
 */
export function panOrTurn(
	box: ScrollBox,
	{ dx, dy }: PanDirection,
	fraction = PAN_STEP,
): PanResult {
	const maxLeft = Math.max(0, box.scrollWidth - box.clientWidth)
	const maxTop = Math.max(0, box.scrollHeight - box.clientHeight)
	const EDGE = 1
	if (dx !== 0) {
		const atEdge = dx > 0 ? box.scrollLeft >= maxLeft - EDGE : box.scrollLeft <= EDGE
		if (atEdge) return { type: 'turn', delta: dx }
		const left = Math.min(maxLeft, Math.max(0, box.scrollLeft + dx * box.clientWidth * fraction))
		return { type: 'pan', left, top: box.scrollTop }
	}
	if (dy !== 0) {
		const atEdge = dy > 0 ? box.scrollTop >= maxTop - EDGE : box.scrollTop <= EDGE
		if (atEdge) return { type: 'turn', delta: dy }
		const top = Math.min(maxTop, Math.max(0, box.scrollTop + dy * box.clientHeight * fraction))
		return { type: 'pan', left: box.scrollLeft, top }
	}
	return { type: 'pan', left: box.scrollLeft, top: box.scrollTop }
}

export type Box = { left: number; top: number; right: number; bottom: number }

/**
 * The same for a page zoomed with a transform (the comic reader's pinch zoom): the page's box
 * against the screen's. Returns how far to move the page (in screen pixels), or a page turn.
 */
export function panOrTurnTransformed(
	page: Box,
	view: Box,
	{ dx, dy }: PanDirection,
	fraction = PAN_STEP,
): { type: 'pan'; x: number; y: number } | { type: 'turn'; delta: -1 | 1 } {
	const EDGE = 1
	const width = view.right - view.left
	const height = view.bottom - view.top
	if (dx !== 0) {
		// moving right shows what's to the right: the page moves left, until its right edge
		const room = dx > 0 ? page.right - view.right : view.left - page.left
		if (room <= EDGE) return { type: 'turn', delta: dx }
		return { type: 'pan', x: -dx * Math.min(room, width * fraction), y: 0 }
	}
	if (dy !== 0) {
		const room = dy > 0 ? page.bottom - view.bottom : view.top - page.top
		if (room <= EDGE) return { type: 'turn', delta: dy }
		return { type: 'pan', x: 0, y: -dy * Math.min(room, height * fraction) }
	}
	return { type: 'pan', x: 0, y: 0 }
}
