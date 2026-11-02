import {
	bookTurn,
	MOUSE_SIDE_ZONE,
	navigationTapZone,
	pageTapZone,
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
