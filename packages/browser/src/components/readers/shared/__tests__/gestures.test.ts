import {
	bookTurn,
	edgeSwipeTurn,
	MOUSE_SIDE_ZONE,
	navigationTapZone,
	pageTapZone,
	scrollEdges,
	sideZoneWidth,
	swipeOf,
	swipeTurn,
	TOUCH_SIDE_ZONE,
	zoneTurn,
} from '../gestures'

describe('reader tap zones (every reader)', () => {
	it('uses a third of the width at each side on touch screens and a quarter with a mouse', () => {
		expect(TOUCH_SIDE_ZONE).toBeCloseTo(1 / 3)
		expect(MOUSE_SIDE_ZONE).toBe(0.25)
		expect(sideZoneWidth(true)).toBe(TOUCH_SIDE_ZONE)
		expect(sideZoneWidth(false)).toBe(MOUSE_SIDE_ZONE)
		expect(pageTapZone(0.3, true)).toBe('left')
		expect(pageTapZone(0.3, false)).toBe('middle')
		expect(pageTapZone(0.2, false)).toBe('left')
		expect(pageTapZone(0.7, true)).toBe('right')
		expect(pageTapZone(0.7, false)).toBe('middle')
		expect(pageTapZone(0.8, false)).toBe('right')
		expect(pageTapZone(0.5, true)).toBe('middle')
		expect(pageTapZone(0.5, false)).toBe('middle')
	})

	it('turns pages from the sides only when pages turn', () => {
		expect(navigationTapZone(0.1, { flow: 'paged', coarse: true })).toBe('left')
		expect(navigationTapZone(0.9, { flow: 'paged', coarse: false })).toBe('right')
		expect(navigationTapZone(0.5, { flow: 'paged', coarse: true })).toBe('middle')
	})

	it('never navigates from a tap in a scrolled layout: every tap is a middle tap', () => {
		for (const coarse of [true, false]) {
			for (const x of [0, 0.05, 0.3, 0.5, 0.7, 0.95, 1]) {
				expect(navigationTapZone(x, { flow: 'continuous', coarse })).toBe('middle')
			}
		}
	})

	it('never turns a zoomed page from a tap at its side', () => {
		for (const coarse of [true, false]) {
			for (const x of [0.02, 0.1, 0.9, 0.98]) {
				expect(navigationTapZone(x, { flow: 'paged', coarse, zoomed: true })).toBe('middle')
			}
		}
		expect(navigationTapZone(0.95, { flow: 'paged', coarse: true, zoomed: false })).toBe('right')
	})

	it("honours the 'tap the sides to turn' preference", () => {
		expect(navigationTapZone(0.95, { flow: 'paged', coarse: true, sidesNavigate: false })).toBe(
			'middle',
		)
		expect(navigationTapZone(0.95, { flow: 'paged', coarse: true, sidesNavigate: true })).toBe(
			'right',
		)
	})

	it('maps the screen sides to forward and back, mirrored for right-to-left books', () => {
		expect(zoneTurn('right')).toBe(1)
		expect(zoneTurn('left')).toBe(-1)
		expect(zoneTurn('middle')).toBe(0)
		expect(bookTurn(1, false)).toBe(1)
		expect(bookTurn(-1, false)).toBe(-1)
		// a manga: the left side goes forward
		expect(bookTurn(-1, true)).toBe(1)
		expect(bookTurn(1, true)).toBe(-1)
		expect(bookTurn(0, true)).toBe(0)
	})
})

describe('swipes (every reader): a drag the page did not follow', () => {
	it('turns on a quick sideways flick', () => {
		expect(swipeOf(-60, 5, 120, 390)).toBe('left')
		expect(swipeOf(70, -10, 150, 390)).toBe('right')
	})

	it('turns on a slow drag across a quarter of the screen', () => {
		expect(swipeOf(-120, 0, 800, 390)).toBe('left')
		expect(swipeOf(-200, 0, 800, 1440)).toBeNull() // a quarter of a desktop screen is 360px
	})

	it('ignores small, slow, diagonal or late drags', () => {
		expect(swipeOf(-10, 0, 50, 390)).toBeNull() // under the 16px slop
		expect(swipeOf(-40, 0, 800, 390)).toBeNull() // slow and short
		expect(swipeOf(-80, 60, 120, 390)).toBeNull() // too diagonal
		expect(swipeOf(-70, -300, 250, 390)).toBeNull() // scrolling down a tall page
		expect(swipeOf(-300, 0, 1200, 390)).toBeNull() // held too long: not a swipe
	})

	it('turns to what is on the side the finger moved away from', () => {
		expect(swipeTurn('left')).toBe(1)
		expect(swipeTurn('right')).toBe(-1)
		expect(swipeTurn(null)).toBe(0)
	})
})

describe('edge swipes on a page bigger than the screen (comic and PDF)', () => {
	const box = (scrollLeft: number) => ({ scrollLeft, scrollWidth: 1170, clientWidth: 390 })

	it('tells the edges a scroller is at, both for a page that fits', () => {
		expect(scrollEdges(box(0))).toEqual({ left: true, right: false })
		expect(scrollEdges(box(780))).toEqual({ left: false, right: true })
		expect(scrollEdges(box(400))).toEqual({ left: false, right: false })
		expect(scrollEdges({ scrollLeft: 0, scrollWidth: 390, clientWidth: 390 })).toEqual({
			left: true,
			right: true,
		})
	})

	it('turns only from the edge the swipe goes toward, when it was there from the start', () => {
		const right = { left: false, right: true }
		const left = { left: true, right: false }
		const middle = { left: false, right: false }
		// a finger moving left shows what's to the right: from the right edge it turns (1)
		expect(edgeSwipeTurn('left', right, right)).toBe(1)
		expect(edgeSwipeTurn('right', left, left)).toBe(-1)
		// a swipe that starts away from the edge only pans the page to it
		expect(edgeSwipeTurn('left', middle, right)).toBe(0)
		expect(edgeSwipeTurn('right', middle, left)).toBe(0)
		// toward the other side the page pans back
		expect(edgeSwipeTurn('right', right, middle)).toBe(0)
		expect(edgeSwipeTurn(null, right, right)).toBe(0)
	})

	it('turns a page that fits on any deliberate swipe', () => {
		const both = { left: true, right: true }
		expect(edgeSwipeTurn('left', both, both)).toBe(1)
		expect(edgeSwipeTurn('right', both, both)).toBe(-1)
	})
})
