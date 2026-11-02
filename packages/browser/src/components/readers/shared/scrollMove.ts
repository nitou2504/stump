/**
 * The moves the keys make in every reader (a zoomed PDF page, a comic's steps through a big
 * page, the scrolled layouts): short and snappy, easing out, like a native smooth scroll but
 * quicker. A key pressed again (or held) during a move doesn't queue another one: the move is
 * retargeted, from where it is, toward the new target, which the caller counts from the
 * previous target (`scrollTarget`), so a held key moves on steadily instead of stuttering.
 * With reduced motion the moves are instant.
 */

/** How long a key's move takes */
export const KEY_SCROLL_MS = 110

/** Something that scrolls: an element, or a book frame's `document.scrollingElement` */
export type Scroller = Pick<
	Element,
	'scrollLeft' | 'scrollTop' | 'scrollWidth' | 'scrollHeight' | 'clientWidth' | 'clientHeight'
>

export type ScrollPoint = { x: number; y: number }

type Move = { frame: number; target: ScrollPoint }

const moves = new WeakMap<Scroller, Move>()

/** The furthest a scroller can scroll each way */
export function maxScroll(scroller: Scroller): ScrollPoint {
	return {
		x: Math.max(0, scroller.scrollWidth - scroller.clientWidth),
		y: Math.max(0, scroller.scrollHeight - scroller.clientHeight),
	}
}

/** A point kept inside what the scroller can show */
export function clampScroll(scroller: Scroller, point: ScrollPoint): ScrollPoint {
	const max = maxScroll(scroller)
	return {
		x: Math.min(max.x, Math.max(0, point.x)),
		y: Math.min(max.y, Math.max(0, point.y)),
	}
}

/** Where the scroller is going: the target of a move under way, or where it is */
export function scrollTarget(scroller: Scroller): ScrollPoint {
	return moves.get(scroller)?.target ?? { x: scroller.scrollLeft, y: scroller.scrollTop }
}

/** The ease of a move: fast at first, settling at the end */
export function easeOut(t: number): number {
	const clamped = Math.min(1, Math.max(0, t))
	return 1 - Math.pow(1 - clamped, 3)
}

export function prefersReducedMotion(): boolean {
	return (
		typeof window !== 'undefined' &&
		!!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
	)
}

/** Stops a move under way (where it is) */
export function cancelScrollMove(scroller: Scroller) {
	const move = moves.get(scroller)
	if (move) window.cancelAnimationFrame(move.frame)
	moves.delete(scroller)
}

/**
 * Moves a scroller to a point (kept inside what it can show), quickly and smoothly. A move
 * already under way is retargeted from where it is; with reduced motion (or `instant`) the
 * scroller jumps there.
 */
export function scrollMoveTo(
	scroller: Scroller,
	point: ScrollPoint,
	{ duration = KEY_SCROLL_MS, instant = false }: { duration?: number; instant?: boolean } = {},
) {
	const target = clampScroll(scroller, point)
	const running = moves.get(scroller)
	if (running) window.cancelAnimationFrame(running.frame)
	if (instant || duration <= 0 || prefersReducedMotion()) {
		moves.delete(scroller)
		scroller.scrollLeft = target.x
		scroller.scrollTop = target.y
		return
	}
	const from = { x: scroller.scrollLeft, y: scroller.scrollTop }
	if (Math.abs(from.x - target.x) < 0.5 && Math.abs(from.y - target.y) < 0.5) {
		moves.delete(scroller)
		return
	}
	const start = performance.now()
	const move: Move = { frame: 0, target }
	const step = (now: number) => {
		const eased = easeOut((now - start) / duration)
		scroller.scrollLeft = from.x + (target.x - from.x) * eased
		scroller.scrollTop = from.y + (target.y - from.y) * eased
		if (eased < 1) {
			move.frame = window.requestAnimationFrame(step)
		} else {
			moves.delete(scroller)
		}
	}
	moves.set(scroller, move)
	move.frame = window.requestAnimationFrame(step)
}

/** Moves a scroller by an offset, counted from where it's going (see `scrollTarget`) */
export function scrollMoveBy(scroller: Scroller, dx: number, dy: number) {
	const from = scrollTarget(scroller)
	scrollMoveTo(scroller, { x: from.x + dx, y: from.y + dy })
}
