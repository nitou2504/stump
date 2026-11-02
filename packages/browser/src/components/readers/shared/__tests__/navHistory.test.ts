import { act, renderHook } from '@testing-library/react'

import {
	isFarJump,
	NAV_HISTORY_LIMIT,
	navHistoryKey,
	nextUseful,
	recordJump,
	takeBack,
	takeForward,
	usePageNavHistory,
} from '../navHistory'

type P = { page: number }
const same = (a: P, b: P) => a.page === b.page
const at = (page: number) => ({ place: { page }, label: `Page ${page}` })
const empty = { back: [], forward: [] }

describe('recordJump', () => {
	it('remembers the place a jump leaves and drops the way forward', () => {
		const state = recordJump({ back: [at(1)], forward: [at(9)] }, at(5), same)
		expect(state.back.map((entry) => entry.place.page)).toEqual([1, 5])
		expect(state.forward).toEqual([])
	})

	it("doesn't repeat the place it already remembers last", () => {
		const state = recordJump({ back: [at(5)], forward: [] }, at(5), same)
		expect(state.back.map((entry) => entry.place.page)).toEqual([5])
	})

	it(`keeps the last ${NAV_HISTORY_LIMIT} places`, () => {
		let state: { back: ReturnType<typeof at>[]; forward: ReturnType<typeof at>[] } = empty
		for (let page = 1; page <= NAV_HISTORY_LIMIT + 3; page += 1) {
			state = recordJump(state, at(page * 10), same)
		}
		expect(state.back).toHaveLength(NAV_HISTORY_LIMIT)
		expect(state.back[0]!.place.page).toBe(40)
		expect(state.back[NAV_HISTORY_LIMIT - 1]!.place.page).toBe(80)
	})
})

describe('takeBack and takeForward', () => {
	it('goes back to the last place, leaving the current one to go forward to', () => {
		const taken = takeBack({ back: [at(1), at(5)], forward: [] }, at(40), same)
		expect(taken?.entry.place.page).toBe(5)
		expect(taken?.state.back.map((entry) => entry.place.page)).toEqual([1])
		expect(taken?.state.forward.map((entry) => entry.place.page)).toEqual([40])

		const again = takeForward(taken!.state, at(5), same)
		expect(again?.entry.place.page).toBe(40)
		expect(again?.state.back.map((entry) => entry.place.page)).toEqual([1, 5])
		expect(again?.state.forward).toEqual([])
	})

	it('skips a place the reader is already at', () => {
		const taken = takeBack({ back: [at(1), at(5)], forward: [] }, at(5), same)
		expect(taken?.entry.place.page).toBe(1)
		expect(taken?.state.back).toEqual([])
	})

	it('has nothing to take from an empty list', () => {
		expect(takeBack(empty, at(3), same)).toBeNull()
		expect(takeForward(empty, at(3), same)).toBeNull()
	})
})

describe('nextUseful', () => {
	it('offers the latest place the reader is not at', () => {
		expect(nextUseful([at(1), at(5)], { page: 9 }, same)?.place.page).toBe(5)
		expect(nextUseful([at(1), at(5)], { page: 5 }, same)?.place.page).toBe(1)
		expect(nextUseful([at(5)], { page: 5 }, same)).toBeNull()
	})
})

describe('isFarJump', () => {
	it('counts more than a few pages, or another chapter', () => {
		expect(isFarJump({ page: 10 }, { page: 13 })).toBe(false)
		expect(isFarJump({ page: 10 }, { page: 14 })).toBe(true)
		expect(isFarJump({ page: 10 }, { page: 6 })).toBe(true)
		expect(isFarJump({ page: 10, chapter: 'One' }, { page: 11, chapter: 'Two' })).toBe(true)
		expect(isFarJump({ page: 10, chapter: 'One' }, { page: 12, chapter: 'One' })).toBe(false)
	})
})

describe('usePageNavHistory', () => {
	beforeEach(() => window.sessionStorage.clear())

	function setup(start = 10) {
		let page = start
		const goTo = vi.fn((to: number) => {
			page = to
		})
		const view = renderHook(
			({ current }) =>
				usePageNavHistory({
					reader: 'pdf',
					bookId: 'book-1',
					currentPage: current,
					labelOf: (at) => `Page ${at}`,
					goTo,
				}),
			{ initialProps: { current: page } },
		)
		const moved = () => view.rerender({ current: page })
		return { ...view, goTo, moved, page: () => page }
	}

	it('remembers far jumps only, and goes back and forward', () => {
		const { result, goTo, moved, page } = setup()

		act(() => result.current.jumpTo(12))
		moved()
		expect(goTo).toHaveBeenLastCalledWith(12)
		expect(result.current.back).toBeNull()

		act(() => result.current.jumpTo(200))
		moved()
		expect(result.current.back?.label).toBe('Page 12')

		act(() => result.current.goBack())
		moved()
		expect(page()).toBe(12)
		expect(result.current.back).toBeNull()
		expect(result.current.forward?.label).toBe('Page 200')

		act(() => result.current.goForward())
		moved()
		expect(page()).toBe(200)
		expect(result.current.back?.label).toBe('Page 12')
	})

	it('moves with the given move (e.g. to a search hit in the page)', () => {
		const { result, goTo, rerender } = setup()
		const move = vi.fn()
		act(() => result.current.jumpTo(90, move))
		expect(move).toHaveBeenCalled()
		expect(goTo).not.toHaveBeenCalled()
		rerender({ current: 90 })
		expect(result.current.back?.label).toBe('Page 10')
	})

	it('keeps the places for the tab, per book and reader', () => {
		const first = setup()
		act(() => first.result.current.jumpTo(90))
		const stored = JSON.parse(window.sessionStorage.getItem(navHistoryKey('pdf', 'book-1'))!)
		expect(stored.back).toEqual([{ place: { page: 10 }, label: 'Page 10' }])
		first.unmount()

		const second = setup(90)
		expect(second.result.current.back?.label).toBe('Page 10')
	})

	it('ignores what isn\u2019t a place in storage', () => {
		window.sessionStorage.setItem(
			navHistoryKey('pdf', 'book-1'),
			JSON.stringify({ back: [{ place: { page: 'x' }, label: 'bad' }, 'junk'], forward: 3 }),
		)
		const { result } = setup()
		expect(result.current.back).toBeNull()
		expect(result.current.forward).toBeNull()
	})
})
