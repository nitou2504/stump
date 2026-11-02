import {
	cancelScrollMove,
	easeOut,
	KEY_SCROLL_MS,
	scrollMoveBy,
	scrollMoveTo,
	scrollTarget,
} from '../scrollMove'

/** A scroller 2000 tall showing 400, and a hand-driven clock for its animation frames */
function setup() {
	const scroller = {
		scrollLeft: 0,
		scrollTop: 0,
		scrollWidth: 500,
		scrollHeight: 2000,
		clientWidth: 500,
		clientHeight: 400,
	}
	let now = 0
	let queue: FrameRequestCallback[] = []
	vi.spyOn(performance, 'now').mockImplementation(() => now)
	vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
		queue.push(callback)
		return queue.length
	})
	vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => {
		queue = []
	})
	/** Runs the frames for `ms` of time, one every 16ms */
	const advance = (ms: number) => {
		const end = now + ms
		while (now < end) {
			now = Math.min(end, now + 16)
			const frames = queue
			queue = []
			frames.forEach((frame) => frame(now))
		}
	}
	return { scroller, advance }
}

afterEach(() => {
	vi.restoreAllMocks()
})

describe('easeOut', () => {
	it('starts fast and settles at the end', () => {
		expect(easeOut(0)).toBe(0)
		expect(easeOut(1)).toBe(1)
		expect(easeOut(0.5)).toBeGreaterThan(0.8)
	})
})

describe('scrollMoveTo', () => {
	it('arrives in about 110ms, easing out', () => {
		const { scroller, advance } = setup()
		scrollMoveTo(scroller, { x: 0, y: 300 })
		expect(scrollTarget(scroller)).toEqual({ x: 0, y: 300 })
		advance(48)
		expect(scroller.scrollTop).toBeGreaterThan(200)
		expect(scroller.scrollTop).toBeLessThan(300)
		advance(KEY_SCROLL_MS)
		expect(scroller.scrollTop).toBe(300)
		// done: the target is where it is
		expect(scrollTarget(scroller)).toEqual({ x: 0, y: 300 })
	})

	it('is retargeted by a press during a move: held keys add up instead of stuttering', () => {
		const { scroller, advance } = setup()
		// a held key: a press every ~33ms, each a step of 100 from where the last one was going
		for (let press = 0; press < 5; press++) {
			scrollMoveBy(scroller, 0, 100)
			advance(33)
		}
		advance(KEY_SCROLL_MS + 20)
		expect(scroller.scrollTop).toBe(500)
	})

	it('stays inside what the scroller can show', () => {
		const { scroller, advance } = setup()
		scrollMoveTo(scroller, { x: 50, y: 5000 })
		advance(KEY_SCROLL_MS + 20)
		expect(scroller.scrollTop).toBe(1600)
		expect(scroller.scrollLeft).toBe(0)
	})

	it('jumps when asked to, or when motion is reduced', () => {
		const { scroller } = setup()
		scrollMoveTo(scroller, { x: 0, y: 700 }, { instant: true })
		expect(scroller.scrollTop).toBe(700)
		vi.spyOn(window, 'matchMedia').mockImplementation(
			(query: string) => ({ matches: query.includes('reduce') }) as MediaQueryList,
		)
		scrollMoveTo(scroller, { x: 0, y: 100 })
		expect(scroller.scrollTop).toBe(100)
	})

	it('can be stopped where it is', () => {
		const { scroller, advance } = setup()
		scrollMoveTo(scroller, { x: 0, y: 400 })
		advance(32)
		cancelScrollMove(scroller)
		const stopped = scroller.scrollTop
		advance(200)
		expect(scroller.scrollTop).toBe(stopped)
		expect(scrollTarget(scroller)).toEqual({ x: 0, y: stopped })
	})
})
