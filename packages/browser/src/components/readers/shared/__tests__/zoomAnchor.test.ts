import {
	anchoredScroll,
	anchorOnPage,
	clampPinchTranslate,
	clampRatio,
	isDoubleTap,
	midpoint,
	pinchTransform,
	scrollForAnchor,
} from '../zoomAnchor'

describe('zoom anchor maths', () => {
	it('keeps the content point under the anchor when scaling', () => {
		// a point 100px into the viewport, scrolled 300px: content point 400
		const scroll = anchoredScroll(300, 100, 2)
		// after doubling, content point 800 should sit 100px into the viewport
		expect(scroll + 100).toBe(800)
		expect(anchoredScroll(0, 0, 3)).toBe(0)
	})

	it('limits a gesture to the zoom range', () => {
		expect(clampRatio(1, 10, 0.5, 5)).toBe(5)
		expect(clampRatio(2, 0.1, 0.5, 5)).toBe(0.25)
		expect(clampRatio(1, 1.5, 0.5, 5)).toBe(1.5)
		expect(clampRatio(1, Number.NaN, 0.5, 5)).toBe(1)
	})

	it('scales around the pinch origin and follows the midpoint', () => {
		expect(pinchTransform({ x: 400, y: 900 }, { x: 100, y: 200 }, { x: 110, y: 190 }, 2)).toEqual({
			transformOrigin: '400px 900px',
			transform: 'translate3d(10px, -10px, 0) scale(2)',
		})
		expect(midpoint({ x: 0, y: 0 }, { x: 10, y: 20 })).toEqual({ x: 5, y: 10 })
	})

	it('puts the same spot of a page back under the fingers after the zoom is laid out', () => {
		// a page at (20, 50) of 300x400 on screen; the container's corner at (0, 40)
		const anchor = anchorOnPage(
			7,
			{ left: 20, top: 50, width: 300, height: 400 },
			{ x: 170, y: 250 },
			{
				x: 0,
				y: 40,
			},
		)
		expect(anchor).toEqual({ page: 7, fx: 0.5, fy: 0.5, x: 170, y: 210 })
		// zoomed to twice the size, the page is now at content (40, 3000), 600x800
		const scroll = scrollForAnchor(anchor, { left: 40, top: 3000, width: 600, height: 800 })
		// the page's middle (340, 3400) sits at the anchor point (170, 210) of the viewport
		expect(scroll).toEqual({ left: 340 - 170, top: 3400 - 210 })
	})

	it('tells a double tap from two taps', () => {
		const first = { time: 1000, point: { x: 100, y: 100 } }
		expect(isDoubleTap(first, 1200, { x: 110, y: 105 })).toBe(true)
		expect(isDoubleTap(first, 1400, { x: 100, y: 100 })).toBe(false)
		expect(isDoubleTap(first, 1100, { x: 200, y: 100 })).toBe(false)
		expect(isDoubleTap(null, 1100, { x: 100, y: 100 })).toBe(false)
	})
})

describe('a pinch held to what the zoom can show', () => {
	const view = { left: 0, top: 0, width: 390, height: 844 }
	// a page at its fit on a phone: as wide as the screen, centred vertically
	const page = { left: 0, top: 127.5, width: 390, height: 589 }

	it('holds the page still at the fit, whatever the fingers do', () => {
		const origin = { x: 100, y: 200 }
		expect(clampPinchTranslate(page, view, origin, 1, { x: 60, y: -40 })).toEqual({ x: 0, y: 0 })
		const look = pinchTransform(origin, { x: 100, y: 327.5 }, { x: 160, y: 288 }, 1, {
			content: page,
			view,
		})
		expect(look.transform).toBe('translate3d(0px, 0px, 0) scale(1)')
	})

	it('keeps a page bigger than the screen covering it, without empty space past its edges', () => {
		// zooming 2x around the page's left edge: the page may not move right of the screen's edge
		const origin = { x: 0, y: 300 }
		const move = clampPinchTranslate(page, view, origin, 2, { x: 50, y: 0 })
		// the scaled page starts at the screen's left edge
		expect(page.left + origin.x - origin.x * 2 + move.x).toBe(0)
		// within its range the fingers' move is kept
		expect(clampPinchTranslate(page, view, { x: 195, y: 300 }, 2, { x: 30, y: 0 }).x).toBe(30)
	})

	it('centres a page smaller than the screen along that side', () => {
		const origin = { x: 195, y: 0 }
		const move = clampPinchTranslate(page, view, origin, 1.2, { x: 0, y: 80 })
		const top = page.top + origin.y - origin.y * 1.2 + move.y
		const height = page.height * 1.2
		expect(top).toBeCloseTo((844 - height) / 2)
	})
})
