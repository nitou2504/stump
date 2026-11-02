import { atScrollEdge, scrollKeyOf, scrollKeyTarget } from '../scrollKeys'

const key = (key: string, extra: Partial<KeyboardEvent> = {}) =>
	({
		key,
		ctrlKey: false,
		metaKey: false,
		altKey: false,
		shiftKey: false,
		...extra,
	}) as KeyboardEvent

/** A scroller 1000 x 800 showing 500 x 400 */
const scroller = { scrollWidth: 1000, scrollHeight: 800, clientWidth: 500, clientHeight: 400 }
const box = (x: number, y: number) => ({ ...scroller, scrollLeft: x, scrollTop: y })

describe('scrollKeyOf', () => {
	it('maps the arrows to a line, Space and Page Up / Page Down to a screen, Home / End to the edges', () => {
		expect(scrollKeyOf(key('ArrowDown'))).toEqual({ type: 'line', dx: 0, dy: 1 })
		expect(scrollKeyOf(key('ArrowUp'))).toEqual({ type: 'line', dx: 0, dy: -1 })
		expect(scrollKeyOf(key('ArrowRight'))).toEqual({ type: 'line', dx: 1, dy: 0 })
		expect(scrollKeyOf(key('ArrowLeft'))).toEqual({ type: 'line', dx: -1, dy: 0 })
		expect(scrollKeyOf(key(' '))).toEqual({ type: 'screen', dy: 1 })
		expect(scrollKeyOf(key(' ', { shiftKey: true }))).toEqual({ type: 'screen', dy: -1 })
		expect(scrollKeyOf(key('PageDown'))).toEqual({ type: 'screen', dy: 1 })
		expect(scrollKeyOf(key('PageUp'))).toEqual({ type: 'screen', dy: -1 })
		expect(scrollKeyOf(key('Home'))).toEqual({ type: 'edge', to: 'start' })
		expect(scrollKeyOf(key('End'))).toEqual({ type: 'edge', to: 'end' })
	})

	it('leaves modifier combinations (and Shift+arrows, a text selection) alone', () => {
		expect(scrollKeyOf(key('ArrowDown', { shiftKey: true }))).toBeNull()
		expect(scrollKeyOf(key('ArrowDown', { ctrlKey: true }))).toBeNull()
		expect(scrollKeyOf(key(' ', { metaKey: true }))).toBeNull()
		expect(scrollKeyOf(key('End', { altKey: true }))).toBeNull()
		expect(scrollKeyOf(key('a'))).toBeNull()
	})
})

describe('scrollKeyTarget', () => {
	it('moves a line (an eighth of the screen) and a screen (90% of it)', () => {
		expect(scrollKeyTarget(box(0, 0), { type: 'line', dx: 0, dy: 1 }, { x: 0, y: 0 })).toEqual({
			x: 0,
			y: 48,
		})
		expect(scrollKeyTarget(box(0, 0), { type: 'line', dx: 1, dy: 0 }, { x: 0, y: 0 })).toEqual({
			x: 60,
			y: 0,
		})
		expect(scrollKeyTarget(box(0, 0), { type: 'screen', dy: 1 }, { x: 0, y: 0 })).toEqual({
			x: 0,
			y: 360,
		})
	})

	it('counts from where a move under way is going, not where the scroller is', () => {
		expect(scrollKeyTarget(box(0, 0), { type: 'line', dx: 0, dy: 1 }, { x: 0, y: 96 })).toEqual({
			x: 0,
			y: 144,
		})
	})

	it('stops at the edges, and goes to the start or the end', () => {
		expect(
			scrollKeyTarget(box(500, 390), { type: 'line', dx: 1, dy: 0 }, { x: 500, y: 390 }),
		).toEqual({ x: 500, y: 390 })
		expect(scrollKeyTarget(box(0, 300), { type: 'screen', dy: 1 }, { x: 0, y: 300 })).toEqual({
			x: 0,
			y: 400,
		})
		expect(scrollKeyTarget(box(0, 300), { type: 'edge', to: 'start' }, { x: 0, y: 300 })).toEqual({
			x: 0,
			y: 0,
		})
		expect(scrollKeyTarget(box(0, 0), { type: 'edge', to: 'end' }, { x: 0, y: 0 })).toEqual({
			x: 0,
			y: 400,
		})
	})

	it('knows when an action can move no further', () => {
		expect(atScrollEdge(box(0, 400), { type: 'screen', dy: 1 }, { x: 0, y: 400 })).toBe(true)
		expect(atScrollEdge(box(0, 399), { type: 'screen', dy: 1 }, { x: 0, y: 399 })).toBe(false)
		expect(atScrollEdge(box(0, 0), { type: 'screen', dy: -1 }, { x: 0, y: 0 })).toBe(true)
		expect(atScrollEdge(box(500, 0), { type: 'line', dx: 1, dy: 0 }, { x: 500, y: 0 })).toBe(true)
	})
})
