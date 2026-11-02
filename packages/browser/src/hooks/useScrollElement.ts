import { useEffect, useState } from 'react'

/** The attribute OverlayScrollbars (v2) puts on the element it scrolls */
const OVERLAY_VIEWPORT = 'data-overlayscrollbars-viewport'

/**
 * The element the pages scroll in: the main area's custom scrollbars viewport when they're
 * set up, otherwise the main area itself. Found by structure, not by computed `overflow`:
 * the viewport briefly isn't `overflow: scroll` while the scrollbars measure the page (e.g.
 * right after a navigation renders a list), and the main area is `overflow: hidden` while
 * they're on, so a style check at that moment finds neither.
 */
export function getMainScrollElement(
	main: HTMLElement | null = document.getElementById('main'),
): HTMLElement | null {
	return main?.querySelector<HTMLElement>(`:scope > [${OVERLAY_VIEWPORT}]`) ?? main
}

const isScrollable = (element: HTMLElement) => {
	const { overflowY } = window.getComputedStyle(element)
	return overflowY === 'auto' || overflowY === 'scroll' || overflowY === 'overlay'
}

/**
 * The element that scrolls `element` vertically. Inside the main area that's the main scroll
 * element (see `getMainScrollElement`), unless something between them scrolls on its own.
 */
export function findScrollElement(element: HTMLElement | null): HTMLElement | null {
	const main = document.getElementById('main')
	let parent = element?.parentElement ?? null
	while (parent) {
		if (
			parent === main ||
			(parent.parentElement === main && parent.hasAttribute(OVERLAY_VIEWPORT))
		) {
			return getMainScrollElement(main)
		}
		if (isScrollable(parent)) return parent
		parent = parent.parentElement
	}
	return getMainScrollElement(main)
}

/**
 * The element that scrolls the one in `ref`, kept up to date when the main area's custom
 * scrollbars are set up or removed (they move the page into their own viewport element).
 * Virtualised lists listen to its scroll events: a wrong element freezes what they render,
 * e.g. a list left showing the rows of where it was restored while it's scrolled elsewhere.
 */
export function useScrollElement(ref: React.RefObject<HTMLDivElement | null>) {
	const [scrollElement, setScrollElement] = useState<HTMLElement | null>(null)

	useEffect(() => {
		const update = () => {
			if (!ref.current) return
			const found = findScrollElement(ref.current)
			setScrollElement((current) => (current === found ? current : found))
		}

		update()
		// the element may only be attached a moment later (e.g. after a suspended render)
		const timeout = setTimeout(update, 100)

		// the custom scrollbars add (or remove) their viewport as a child of the main area
		const main = document.getElementById('main')
		const observer = main ? new MutationObserver(update) : null
		if (main) observer?.observe(main, { childList: true })

		return () => {
			clearTimeout(timeout)
			observer?.disconnect()
		}
	}, [ref])

	return scrollElement
}
