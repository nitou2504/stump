import {
	isZoomedIn,
	scrollerTouchStyle,
	SWIPE_MAX_DURATION_MS,
	SWIPE_MIN_DISTANCE,
	swipeTurn,
} from '../gestures'

describe('PDF finger gestures', () => {
	it('locks the pages sideways at the fit and lets them pan when zoomed in', () => {
		expect(isZoomedIn(1)).toBe(false)
		expect(isZoomedIn(0.75)).toBe(false)
		expect(isZoomedIn(1.25)).toBe(true)
		expect(scrollerTouchStyle(false)).toEqual({ touchAction: 'pan-y', overflowX: 'hidden' })
		expect(scrollerTouchStyle(true)).toEqual({ touchAction: 'pan-x pan-y', overflowX: 'auto' })
	})

	it('turns to the next page on a quick swipe to the left, the previous on one to the right', () => {
		expect(swipeTurn({ dx: -120, dy: 10, durationMs: 200 })).toBe(1)
		expect(swipeTurn({ dx: 120, dy: -10, durationMs: 200 })).toBe(-1)
	})

	it("doesn't turn on short, slow or mostly vertical moves", () => {
		expect(swipeTurn({ dx: -(SWIPE_MIN_DISTANCE - 1), dy: 0, durationMs: 100 })).toBe(0)
		expect(swipeTurn({ dx: -200, dy: 0, durationMs: SWIPE_MAX_DURATION_MS + 1 })).toBe(0)
		// scrolling down a tall page with a slight sideways drift
		expect(swipeTurn({ dx: -70, dy: -300, durationMs: 250 })).toBe(0)
		// a diagonal that is not clearly sideways
		expect(swipeTurn({ dx: -90, dy: 80, durationMs: 250 })).toBe(0)
	})
})
