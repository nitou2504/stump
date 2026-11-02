import { isAtVisibleEnd, spreadStartFor } from '../edgeExtend'

const box = (left: number, top: number, width = 10, height = 20) => ({
	left,
	top,
	right: left + width,
	bottom: top + height,
})

describe('isAtVisibleEnd', () => {
	const width = 800

	it('is at the end when the next line starts on a page that is not shown', () => {
		// the selection ends on the last line; the next line is the top of the next page
		expect(isAtVisibleEnd(box(600, 740), box(830, 40), width)).toBe(true)
	})

	it('is not at the end mid-page or when the next line is still shown', () => {
		expect(isAtVisibleEnd(box(200, 300), box(40, 330), width)).toBe(false)
		// two pages side by side: the left page's last line continues at the top of the right one
		expect(isAtVisibleEnd(box(350, 740), box(420, 40), width)).toBe(false)
	})

	it('is at the end when the selection was dragged past the edge', () => {
		expect(isAtVisibleEnd(box(805, 400), null, width)).toBe(true)
	})

	it('mirrors for right-to-left books', () => {
		expect(isAtVisibleEnd(box(150, 740), box(-40, 40), width, true)).toBe(true)
		expect(isAtVisibleEnd(box(150, 300), box(400, 330), width, true)).toBe(false)
	})

	it('is not at the end on the chapter’s last line (nothing comes after it)', () => {
		expect(isAtVisibleEnd(box(300, 500), null, width)).toBe(false)
	})
})

describe('spreadStartFor', () => {
	it('snaps to the page (one column) or the spread (two) where the selection ends', () => {
		expect(spreadStartFor(3, 1, 800)).toBe(2400)
		// spreads of two 400px pages: columns 2 and 3 are the second spread
		expect(spreadStartFor(3, 2, 800)).toBe(800)
		expect(spreadStartFor(4, 2, 800)).toBe(1600)
		expect(spreadStartFor(-1, 2, 800)).toBe(0)
	})
})
