import { clampScroll, type Scroller, type ScrollPoint } from './scrollMove'

/**
 * The keys that move through a page or a scrolled book, the same in every reader:
 *
 * - `↑` / `↓` (and `←` / `→` sideways, where the page is wider than the screen) move a line,
 *   about an eighth of the screen
 * - `Space` / `Shift+Space` and `Page Down` / `Page Up` move a screen, less a line kept for
 *   the eye to follow
 * - `Home` / `End` go to the start / the end
 *
 * A reader decides what an action does where the scrolling ends (an EPUB's next chapter, a
 * turned page) and which keys it takes this way (a zoomed PDF page keeps Page Up / Page Down
 * for turning pages).
 */

/** How far a line moves: about an eighth of the screen */
export const LINE_STEP = 0.12
/** How far a screen moves: most of it, so the last line is still there to read on from */
export const SCREEN_STEP = 0.9

export type ScrollKeyAction =
	| { type: 'line'; dx: -1 | 0 | 1; dy: -1 | 0 | 1 }
	| { type: 'screen'; dy: -1 | 1 }
	| { type: 'edge'; to: 'start' | 'end' }

type KeyLike = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>

/**
 * The scroll a key asks for, or `null`. Modifier combinations belong to the browser (and
 * Shift+arrows extend a text selection), except Shift+Space, which goes back a screen.
 */
export function scrollKeyOf(event: KeyLike): ScrollKeyAction | null {
	if (typeof event.key !== 'string' || event.ctrlKey || event.metaKey || event.altKey) return null
	if (event.key === ' ' || event.key === 'Spacebar') {
		return { type: 'screen', dy: event.shiftKey ? -1 : 1 }
	}
	if (event.shiftKey) return null
	switch (event.key) {
		case 'ArrowDown':
			return { type: 'line', dx: 0, dy: 1 }
		case 'ArrowUp':
			return { type: 'line', dx: 0, dy: -1 }
		case 'ArrowRight':
			return { type: 'line', dx: 1, dy: 0 }
		case 'ArrowLeft':
			return { type: 'line', dx: -1, dy: 0 }
		case 'PageDown':
			return { type: 'screen', dy: 1 }
		case 'PageUp':
			return { type: 'screen', dy: -1 }
		case 'Home':
			return { type: 'edge', to: 'start' }
		case 'End':
			return { type: 'edge', to: 'end' }
		default:
			return null
	}
}

/**
 * Where a scroll action takes a scroller from `from` (where it is, or where a move under way
 * is going), kept inside what it can show: at an edge it stays where it is
 */
export function scrollKeyTarget(
	scroller: Scroller,
	action: ScrollKeyAction,
	from: ScrollPoint,
): ScrollPoint {
	switch (action.type) {
		case 'line':
			return clampScroll(scroller, {
				x: from.x + action.dx * scroller.clientWidth * LINE_STEP,
				y: from.y + action.dy * scroller.clientHeight * LINE_STEP,
			})
		case 'screen':
			return clampScroll(scroller, {
				x: from.x,
				y: from.y + action.dy * scroller.clientHeight * SCREEN_STEP,
			})
		case 'edge':
			return clampScroll(scroller, {
				x: from.x,
				y: action.to === 'start' ? 0 : scroller.scrollHeight,
			})
	}
}

/** Whether a scroll action can't move any further (the scroller is at that edge) */
export function atScrollEdge(scroller: Scroller, action: ScrollKeyAction, from: ScrollPoint) {
	const to = scrollKeyTarget(scroller, action, from)
	return Math.abs(to.x - from.x) < 1 && Math.abs(to.y - from.y) < 1
}
