import { navigationTapZone, pageTapZone, SIDE_TAP_ZONE } from '../tapZones'

describe('reader tap zones', () => {
	it('splits the page into a strip at each side and the middle', () => {
		expect(SIDE_TAP_ZONE).toBe(0.15)
		expect(pageTapZone(0.05)).toBe('left')
		expect(pageTapZone(0.5)).toBe('middle')
		expect(pageTapZone(0.9)).toBe('right')
	})

	it('turns pages from the sides only when pages turn', () => {
		expect(navigationTapZone(0.05, { flow: 'paged' })).toBe('left')
		expect(navigationTapZone(0.95, { flow: 'paged' })).toBe('right')
		expect(navigationTapZone(0.5, { flow: 'paged' })).toBe('middle')
	})

	it('never navigates from a tap in a scrolled layout: every tap is a middle tap', () => {
		for (const x of [0, 0.05, 0.5, 0.95, 1]) {
			expect(navigationTapZone(x, { flow: 'continuous' })).toBe('middle')
		}
	})

	it("honours the 'tap the sides to turn' preference", () => {
		expect(navigationTapZone(0.95, { flow: 'paged', sidesNavigate: false })).toBe('middle')
		expect(navigationTapZone(0.95, { flow: 'paged', sidesNavigate: true })).toBe('right')
	})
})
