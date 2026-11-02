import { act, renderHook, waitFor } from '@testing-library/react'

import { findScrollElement, getMainScrollElement, useScrollElement } from '../useScrollElement'

/** The app's main area, like AppLayout renders it (`#main`, scrolling vertically) */
function renderMain() {
	const main = document.createElement('main')
	main.id = 'main'
	main.style.overflowY = 'auto'
	const page = document.createElement('div')
	const list = document.createElement('div')
	page.appendChild(list)
	main.appendChild(page)
	document.body.appendChild(main)
	return { main, page, list }
}

/** What OverlayScrollbars does when it sets up: moves the content into its viewport */
function addOverlayViewport(main: HTMLElement, { scrolling = true } = {}) {
	const viewport = document.createElement('div')
	viewport.setAttribute('data-overlayscrollbars-viewport', 'scrollbarHidden')
	viewport.style.overflowY = scrolling ? 'scroll' : 'visible'
	while (main.firstChild) viewport.appendChild(main.firstChild)
	main.style.overflowY = 'hidden'
	main.appendChild(viewport)
	return viewport
}

afterEach(() => {
	document.body.innerHTML = ''
})

describe('getMainScrollElement', () => {
	it('is the main area without custom scrollbars, their viewport with them', () => {
		const { main } = renderMain()
		expect(getMainScrollElement()).toBe(main)
		const viewport = addOverlayViewport(main)
		expect(getMainScrollElement()).toBe(viewport)
	})
})

describe('findScrollElement', () => {
	it('finds the custom scrollbars viewport even while it measures the page', () => {
		const { main, list } = renderMain()
		// mid-measure: neither the viewport nor the main area reports a scrolling overflow
		const viewport = addOverlayViewport(main, { scrolling: false })
		expect(findScrollElement(list)).toBe(viewport)
	})

	it('finds a scroller of its own between the element and the main area', () => {
		const { page, list } = renderMain()
		page.style.overflowY = 'auto'
		expect(findScrollElement(list)).toBe(page)
	})

	it('falls back to the main scroll element for an element outside the main area', () => {
		const { main } = renderMain()
		const viewport = addOverlayViewport(main)
		const detached = document.createElement('div')
		expect(findScrollElement(detached)).toBe(viewport)
	})
})

describe('useScrollElement', () => {
	it('follows the custom scrollbars when they are set up after the list', async () => {
		const { main, list } = renderMain()
		const ref = { current: list }
		const { result } = renderHook(() => useScrollElement(ref))
		expect(result.current).toBe(main)

		let viewport: HTMLElement | undefined
		act(() => {
			viewport = addOverlayViewport(main)
		})
		await waitFor(() => expect(result.current).toBe(viewport))
	})
})
