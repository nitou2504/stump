import { useEffect, useRef } from 'react'

import { swipeOf, swipeTurn } from './gestures'

type Options = {
	/** Off in the scrolled layouts (scrolling moves through the book there) and when zoomed in */
	enabled: boolean
	/** A deliberate swipe: `1` shows what's to the right, `-1` what's to the left */
	onSwipe: (screenTurn: 1 | -1) => void
}

/**
 * A finger's deliberate sideways swipe on an element, judged when it lifts with the rule
 * every reader shares (`swipeOf`): the element is never dragged along. Two fingers (a pinch),
 * a cancelled touch (the browser scrolled) or a text selection made by the drag are not
 * swipes. Mouse drags never are: with a mouse, a drag selects.
 */
export function useSwipeTurn(
	elementRef: React.RefObject<HTMLElement | null>,
	{ enabled, onSwipe }: Options,
) {
	const stateRef = useRef({ enabled, onSwipe })
	useEffect(() => {
		stateRef.current = { enabled, onSwipe }
	}, [enabled, onSwipe])

	useEffect(() => {
		const element = elementRef.current
		if (!element) return undefined
		let start: { x: number; y: number; time: number } | null = null
		const onTouchStart = (event: TouchEvent) => {
			const touch = event.touches[0]
			start =
				event.touches.length === 1 && touch
					? { x: touch.clientX, y: touch.clientY, time: performance.now() }
					: null
		}
		const onTouchEnd = (event: TouchEvent) => {
			const touch = event.changedTouches[0]
			const from = start
			start = null
			if (!from || !touch || event.touches.length > 0 || !stateRef.current.enabled) return
			const selection = window.getSelection()
			if (selection && !selection.isCollapsed) return
			const turn = swipeTurn(
				swipeOf(
					touch.clientX - from.x,
					touch.clientY - from.y,
					performance.now() - from.time,
					element.clientWidth,
				),
			)
			if (turn) stateRef.current.onSwipe(turn)
		}
		const onTouchCancel = () => {
			start = null
		}
		element.addEventListener('touchstart', onTouchStart, { passive: true })
		element.addEventListener('touchend', onTouchEnd)
		element.addEventListener('touchcancel', onTouchCancel)
		return () => {
			element.removeEventListener('touchstart', onTouchStart)
			element.removeEventListener('touchend', onTouchEnd)
			element.removeEventListener('touchcancel', onTouchCancel)
		}
	}, [elementRef])
}
