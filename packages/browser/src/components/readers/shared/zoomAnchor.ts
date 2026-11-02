/**
 * Zooming that keeps the point under the fingers (or the mouse) where it is: a pinch, a
 * Ctrl/⌘ + wheel (trackpad pinch) or a double tap. While the gesture runs only a CSS
 * transform is applied (no re-layout); when it ends the zoom is committed once and the
 * scroll position is set so the same spot of the same page lands under the same point.
 */

export type Point = { x: number; y: number }
export type Box = { left: number; top: number; width: number; height: number }

/**
 * A spot on a page to keep in place: the page (1-based), the spot as fractions of the page
 * box, and where it should be in the scroll container's viewport (CSS px from its corner)
 */
export type ZoomAnchor = { page: number; fx: number; fy: number; x: number; y: number }

export function midpoint(a: Point, b: Point): Point {
	return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 }
}

export function distance(a: Point, b: Point): number {
	return Math.hypot(a.x - b.x, a.y - b.y)
}

/**
 * The scroll offset that keeps a content point under a viewport point when everything
 * scales by `ratio`: `(anchor + scroll) * ratio - anchor`
 */
export function anchoredScroll(scroll: number, anchor: number, ratio: number): number {
	return (anchor + scroll) * ratio - anchor
}

/** The ratio a gesture may apply from `zoom` without leaving `[min, max]` */
export function clampRatio(zoom: number, ratio: number, min: number, max: number): number {
	if (!Number.isFinite(ratio) || ratio <= 0 || zoom <= 0) return 1
	const next = Math.min(max, Math.max(min, zoom * ratio))
	return next / zoom
}

/**
 * The CSS transform shown during a pinch, on the content element (whose own origin is its
 * top-left corner): scaled by `ratio` around the content point that was under the fingers
 * when the pinch started (`origin`, content px), and moved with the fingers' midpoint
 */
export function pinchTransform(origin: Point, startMid: Point, mid: Point, ratio: number) {
	const dx = mid.x - startMid.x
	const dy = mid.y - startMid.y
	return {
		transformOrigin: `${origin.x}px ${origin.y}px`,
		transform: `translate3d(${dx}px, ${dy}px, 0) scale(${ratio})`,
	}
}

/** The anchor for a point in the viewport over a page's box (both in viewport px) */
export function anchorOnPage(
	page: number,
	pageBox: Box,
	point: Point,
	container: Point,
): ZoomAnchor {
	const fx = pageBox.width > 0 ? (point.x - pageBox.left) / pageBox.width : 0
	const fy = pageBox.height > 0 ? (point.y - pageBox.top) / pageBox.height : 0
	return {
		page,
		fx: Math.min(1, Math.max(0, fx)),
		fy: Math.min(1, Math.max(0, fy)),
		x: point.x - container.x,
		y: point.y - container.y,
	}
}

/**
 * Where to scroll so the anchor's spot (on a page now laid out at `pageBox`, in content px)
 * is back under its viewport point
 */
export function scrollForAnchor(anchor: ZoomAnchor, pageBox: Box): { left: number; top: number } {
	return {
		left: Math.max(0, pageBox.left + anchor.fx * pageBox.width - anchor.x),
		top: Math.max(0, pageBox.top + anchor.fy * pageBox.height - anchor.y),
	}
}

/** Two taps this close in time and space are a double tap */
export const DOUBLE_TAP_MS = 280
export const DOUBLE_TAP_PX = 30

export function isDoubleTap(
	previous: { time: number; point: Point } | null,
	time: number,
	point: Point,
): boolean {
	return (
		!!previous &&
		time - previous.time <= DOUBLE_TAP_MS &&
		distance(previous.point, point) <= DOUBLE_TAP_PX
	)
}
