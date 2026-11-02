import { panDirectionOf, panOrTurn, panOrTurnTransformed, SCREEN_STEP } from '../panKeys'

/** A page zoomed to twice the screen each way, scrolled to (left, top) */
const zoomed = (left: number, top: number) => ({
	scrollLeft: left,
	scrollTop: top,
	scrollWidth: 2000,
	scrollHeight: 1600,
	clientWidth: 1000,
	clientHeight: 800,
})

describe('panOrTurn (a zoomed page in a scroller)', () => {
	it('moves around the page by an eighth of the screen per arrow press', () => {
		expect(panOrTurn(zoomed(0, 0), { dx: 1, dy: 0 })).toEqual({ type: 'pan', left: 120, top: 0 })
		expect(panOrTurn(zoomed(500, 400), { dx: 0, dy: -1 })).toEqual({
			type: 'pan',
			left: 500,
			top: 304,
		})
	})

	it('stops at the edge, then turns the page on the next press', () => {
		expect(panOrTurn(zoomed(950, 0), { dx: 1, dy: 0 })).toEqual({ type: 'pan', left: 1000, top: 0 })
		expect(panOrTurn(zoomed(1000, 0), { dx: 1, dy: 0 })).toEqual({ type: 'turn', delta: 1 })
		expect(panOrTurn(zoomed(0, 300), { dx: -1, dy: 0 })).toEqual({ type: 'turn', delta: -1 })
		expect(panOrTurn(zoomed(0, 800), { dx: 0, dy: 1 })).toEqual({ type: 'turn', delta: 1 })
		expect(panOrTurn(zoomed(0, 0), { dx: 0, dy: -1 })).toEqual({ type: 'turn', delta: -1 })
	})

	it('moves by most of a screen for Space', () => {
		expect(panOrTurn(zoomed(0, 0), { dx: 0, dy: 1 }, SCREEN_STEP)).toEqual({
			type: 'pan',
			left: 0,
			top: 720,
		})
	})
})

describe('panOrTurnTransformed (a page zoomed with a transform)', () => {
	const view = { left: 0, top: 0, right: 1000, bottom: 800 }

	it('moves the page the other way, never past its edge', () => {
		// the page sticks out 300px on the right: it moves left by 120 (an eighth of the screen)
		expect(
			panOrTurnTransformed({ left: -200, top: -100, right: 1300, bottom: 900 }, view, {
				dx: 1,
				dy: 0,
			}),
		).toEqual({ type: 'pan', x: -120, y: 0 })
		// only 50px left above: it moves down by 50
		expect(
			panOrTurnTransformed({ left: -200, top: -50, right: 1300, bottom: 900 }, view, {
				dx: 0,
				dy: -1,
			}),
		).toEqual({ type: 'pan', x: 0, y: 50 })
	})

	it('turns the page at its edge', () => {
		expect(
			panOrTurnTransformed({ left: -500, top: -100, right: 1000, bottom: 900 }, view, {
				dx: 1,
				dy: 0,
			}),
		).toEqual({ type: 'turn', delta: 1 })
		expect(
			panOrTurnTransformed({ left: -500, top: 0, right: 1000, bottom: 900 }, view, {
				dx: 0,
				dy: -1,
			}),
		).toEqual({ type: 'turn', delta: -1 })
	})
})

describe('panDirectionOf', () => {
	it('maps the arrows, and nothing else', () => {
		expect(panDirectionOf('ArrowDown')).toEqual({ dx: 0, dy: 1 })
		expect(panDirectionOf('ArrowLeft')).toEqual({ dx: -1, dy: 0 })
		expect(panDirectionOf('PageDown')).toBeNull()
	})
})
