import { isZoomedIn, scrollerTouchStyle } from '../gestures'

describe('PDF finger gestures', () => {
	it('locks the pages sideways at the fit and lets them pan when zoomed in', () => {
		expect(isZoomedIn(1)).toBe(false)
		expect(isZoomedIn(0.75)).toBe(false)
		expect(isZoomedIn(1.25)).toBe(true)
		expect(scrollerTouchStyle(false)).toEqual({ touchAction: 'pan-y', overflowX: 'hidden' })
		expect(scrollerTouchStyle(true)).toEqual({ touchAction: 'pan-x pan-y', overflowX: 'auto' })
	})
})
