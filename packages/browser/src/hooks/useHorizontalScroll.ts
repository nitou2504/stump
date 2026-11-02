import { useCallback, useEffect, useRef, useState } from 'react'

import { useRestoredScrollLeft } from './useRestoredScrollLeft'

type UseHorizontalScrollOptions = {
	/**
	 * As a percentage of the container width
	 **/
	scrollAmount?: number
	/**
	 * Keeps the row's position with the history entry under this key (unique on the page),
	 * so going back to the page returns the row to where it was
	 */
	restoreKey?: string
}

type Return = {
	scrollerRef: (ref: HTMLElement | Window | null) => void
	canSkipBackward: boolean
	canSkipForward: boolean
	handleSkipBackward: () => void
	handleSkipAhead: () => void
}

export function useHorizontalScroll(options: UseHorizontalScrollOptions = {}): Return {
	const { scrollAmount = 0.6, restoreKey } = options
	const attachRestore = useRestoredScrollLeft(restoreKey)

	const elementRef = useRef<HTMLDivElement | null>(null)
	const resizeObserverRef = useRef<ResizeObserver | null>(null)
	const mutationObserverRef = useRef<MutationObserver | null>(null)
	const [canSkipBackward, setCanSkipBackward] = useState(false)
	const [canSkipForward, setCanSkipForward] = useState(false)

	const updateScrollState = useCallback(() => {
		const el = elementRef.current
		if (!el) return
		const { scrollLeft, scrollWidth, clientWidth } = el
		setCanSkipBackward(scrollLeft > 0)
		setCanSkipForward(scrollLeft < scrollWidth - clientWidth - 1)
	}, [])

	const scrollerRef = useCallback(
		(ref: HTMLElement | Window | null) => {
			if (elementRef.current) {
				elementRef.current.removeEventListener('scroll', updateScrollState)
			}

			if (resizeObserverRef.current) {
				resizeObserverRef.current.disconnect()
				resizeObserverRef.current = null
			}
			mutationObserverRef.current?.disconnect()
			mutationObserverRef.current = null

			// Note: This hook is really only intended for Virtuoso scrollers, and this
			// type check is because the `scrollerRef` can be a Window. We don't care
			// about that case here.
			if (ref instanceof HTMLDivElement) {
				elementRef.current = ref
				ref.addEventListener('scroll', updateScrollState)

				// Watch the scroller and its content: the scroller keeps its size while the
				// cards render in, so observing only it left "Seek ahead" disabled until the
				// list was scrolled by hand
				const resizeObserver = new ResizeObserver(() => {
					updateScrollState()
				})
				// the scroller, Virtuoso's viewport and its item list (whose width grows with the cards)
				const observeTree = () => {
					resizeObserver.observe(ref)
					for (const child of Array.from(ref.children)) {
						resizeObserver.observe(child)
						for (const grandchild of Array.from(child.children)) {
							resizeObserver.observe(grandchild)
						}
					}
				}
				observeTree()
				resizeObserverRef.current = resizeObserver

				const mutationObserver = new MutationObserver(() => {
					observeTree()
					updateScrollState()
				})
				mutationObserver.observe(ref, { childList: true, subtree: true })
				mutationObserverRef.current = mutationObserver

				updateScrollState()
				attachRestore(ref)
			} else {
				elementRef.current = null
				attachRestore(null)
			}
		},
		[updateScrollState, attachRestore],
	)

	useEffect(() => {
		return () => {
			if (elementRef.current) {
				elementRef.current.removeEventListener('scroll', updateScrollState)
			}

			if (resizeObserverRef.current) {
				resizeObserverRef.current.disconnect()
			}
			mutationObserverRef.current?.disconnect()
		}
	}, [updateScrollState])

	const handleSkipAhead = useCallback(() => {
		if (!elementRef.current) return
		const amount = elementRef.current.clientWidth * scrollAmount
		elementRef.current.scrollBy({ left: amount, behavior: 'smooth' })
	}, [scrollAmount])

	const handleSkipBackward = useCallback(() => {
		if (!elementRef.current) return
		const amount = elementRef.current.clientWidth * scrollAmount
		elementRef.current.scrollBy({ left: -amount, behavior: 'smooth' })
	}, [scrollAmount])

	return {
		scrollerRef,
		canSkipBackward,
		canSkipForward,
		handleSkipBackward,
		handleSkipAhead,
	}
}
