import { act, renderHook } from '@testing-library/react'
import { useLocation, useNavigationType } from 'react-router'

import {
	dropHistoryEntriesAfter,
	flushHistoryEntries,
	readHistoryEntry,
	resetHistoryEntriesForTests,
	setShownHistoryEntry,
	updateHistoryEntry,
} from '../historyEntries'
import { useHistoryEntryState } from '../useHistoryEntryState'
import {
	planMainScroll,
	restoreScroll,
	useMainScrollRestoration,
} from '../useMainScrollRestoration'
import { useRestoredScrollLeft } from '../useRestoredScrollLeft'

vi.mock('react-router', () => ({
	useLocation: vi.fn(),
	useNavigationType: vi.fn(),
}))

/** A scroll element with layout, which jsdom doesn't have */
const makeScrollable = (scrollHeight: number, clientHeight = 800) => {
	const element = document.createElement('div')
	let scrollTop = 0
	Object.defineProperty(element, 'scrollTop', {
		configurable: true,
		get: () => scrollTop,
		// like a browser: clamped to what the content allows
		set: (value: number) => {
			scrollTop = Math.max(0, Math.min(value, element.scrollHeight - element.clientHeight))
		},
	})
	setScrollHeight(element, scrollHeight)
	Object.defineProperty(element, 'clientHeight', { configurable: true, value: clientHeight })
	return element
}

/** The content's height changing, e.g. as a list's data arrives */
const setScrollHeight = (element: HTMLElement, scrollHeight: number) =>
	Object.defineProperty(element, 'scrollHeight', { configurable: true, value: scrollHeight })

/** Runs frames by hand: `restoreScroll` with a fake clock and frame queue */
const frames = () => {
	let time = 0
	const queue = new Map<number, () => void>()
	let nextId = 1
	return {
		now: () => time,
		requestFrame: (callback: () => void) => {
			const id = nextId++
			queue.set(id, callback)
			return id
		},
		cancelFrame: (id: number) => queue.delete(id),
		pending: () => queue.size,
		/** Runs one frame, 16 ms later */
		tick() {
			time += 16
			const callbacks = [...queue.values()]
			queue.clear()
			callbacks.forEach((callback) => callback())
		},
	}
}

beforeEach(() => {
	sessionStorage.clear()
	resetHistoryEntriesForTests()
	window.history.replaceState({ idx: 0 }, '')
})

describe('historyEntries', () => {
	it('keeps a record per entry, only for the page it was made for', () => {
		updateHistoryEntry(3, '/libraries/l1/books?page=2', { top: 1200 })
		expect(readHistoryEntry(3, '/libraries/l1/books?page=2')?.top).toBe(1200)
		// the same index showing another page (e.g. after a new search) has no record
		expect(readHistoryEntry(3, '/libraries/l1/books?page=3')).toBeUndefined()
		expect(readHistoryEntry(4, '/libraries/l1/books?page=2')).toBeUndefined()
	})

	it('merges page state and replaces a record made for another page', () => {
		updateHistoryEntry(1, '/smart-lists/s1', { top: 50, state: { groups: { a: true } } })
		updateHistoryEntry(1, '/smart-lists/s1', { state: { other: 1 } })
		expect(readHistoryEntry(1, '/smart-lists/s1')).toEqual({
			path: '/smart-lists/s1',
			top: 50,
			state: { groups: { a: true }, other: 1 },
		})

		updateHistoryEntry(1, '/books/b1', { top: 0 })
		expect(readHistoryEntry(1, '/books/b1')).toEqual({ path: '/books/b1', top: 0 })
	})

	it('forgets the entries a new entry drops', () => {
		updateHistoryEntry(1, '/a', { top: 1 })
		updateHistoryEntry(2, '/b', { top: 2 })
		updateHistoryEntry(3, '/c', { top: 3 })
		dropHistoryEntriesAfter(1)
		expect(readHistoryEntry(1, '/a')?.top).toBe(1)
		expect(readHistoryEntry(2, '/b')).toBeUndefined()
		expect(readHistoryEntry(3, '/c')).toBeUndefined()
	})

	it('survives a reload through session storage', () => {
		updateHistoryEntry(2, '/browse/authors?q=fr', { top: 640 })
		flushHistoryEntries()
		resetHistoryEntriesForTests()
		expect(readHistoryEntry(2, '/browse/authors?q=fr')?.top).toBe(640)
	})
})

describe('planMainScroll', () => {
	const base = { pathname: '/libraries/l1/books', path: '/libraries/l1/books', hash: '' }

	it('restores the recorded position on back/forward and reloads', () => {
		expect(
			planMainScroll({ ...base, navigationType: 'POP', record: { path: base.path, top: 900 } }),
		).toEqual({ kind: 'restore', top: 900 })
		// nothing recorded for the entry (or recorded at the top)
		expect(planMainScroll({ ...base, navigationType: 'POP' })).toEqual({ kind: 'top' })
		expect(
			planMainScroll({ ...base, navigationType: 'POP', record: { path: base.path, top: 0 } }),
		).toEqual({ kind: 'top' })
	})

	it('starts new pages at the top, except a link to an anchor of the same page', () => {
		expect(
			planMainScroll({
				...base,
				navigationType: 'PUSH',
				previousPathname: '/books/b1',
				previousPath: '/books/b1',
			}),
		).toEqual({ kind: 'top' })
		// the next page of results
		expect(
			planMainScroll({
				...base,
				path: '/libraries/l1/books?page=2',
				navigationType: 'PUSH',
				previousPathname: base.pathname,
				previousPath: base.path,
			}),
		).toEqual({ kind: 'top' })
		expect(
			planMainScroll({
				...base,
				hash: '#notes',
				navigationType: 'PUSH',
				previousPathname: base.pathname,
				previousPath: base.path,
			}),
		).toEqual({ kind: 'keep' })
	})

	it('keeps the position when the entry is replaced with the same page, e.g. typing a search', () => {
		expect(
			planMainScroll({
				...base,
				path: '/libraries/l1/books?search=dune',
				navigationType: 'REPLACE',
				previousPathname: base.pathname,
				previousPath: base.path,
			}),
		).toEqual({ kind: 'keep' })
		// a redirect to another page
		expect(
			planMainScroll({
				...base,
				navigationType: 'REPLACE',
				previousPathname: '/libraries/l1',
				previousPath: '/libraries/l1',
			}),
		).toEqual({ kind: 'top' })
	})
})

describe('restoreScroll', () => {
	it('scrolls there at once when the page is already tall enough', () => {
		const clock = frames()
		const element = makeScrollable(5000)
		const onDone = vi.fn()
		restoreScroll(() => element, 2400, { ...clock, onDone, stableFrames: 3, minDuration: 0 })
		expect(element.scrollTop).toBe(2400)

		for (let i = 0; i < 3; i++) clock.tick()
		expect(onDone).toHaveBeenCalledWith(element)
		expect(clock.pending()).toBe(0)
	})

	it('follows the page as its content arrives', () => {
		const clock = frames()
		const element = makeScrollable(1200)
		const onDone = vi.fn()
		restoreScroll(() => element, 2400, { ...clock, onDone, stableFrames: 3, minDuration: 0 })
		// as far as it goes for now
		expect(element.scrollTop).toBe(400)

		clock.tick()
		setScrollHeight(element, 2000)
		clock.tick()
		expect(element.scrollTop).toBe(1200)

		setScrollHeight(element, 6000)
		clock.tick()
		expect(element.scrollTop).toBe(2400)
		expect(onDone).not.toHaveBeenCalled()

		// a virtualised list re-measuring its rows moves it: it goes back to the target
		element.scrollTop = 2300
		clock.tick()
		expect(element.scrollTop).toBe(2400)
		for (let i = 0; i < 3; i++) clock.tick()
		expect(onDone).toHaveBeenCalled()
	})

	it('holds the position for a while: a list attaching later resets it to its start', () => {
		const clock = frames()
		const element = makeScrollable(5000)
		const onDone = vi.fn()
		restoreScroll(() => element, 2400, { ...clock, onDone, stableFrames: 3, minDuration: 200 })
		for (let i = 0; i < 5; i++) clock.tick()
		expect(onDone).not.toHaveBeenCalled()

		// e.g. TanStack Virtual scrolling its new scroll element to its initial offset
		element.scrollTop = 0
		clock.tick()
		expect(element.scrollTop).toBe(2400)
		for (let i = 0; i < 12; i++) clock.tick()
		expect(onDone).toHaveBeenCalled()
		expect(element.scrollTop).toBe(2400)
	})

	it('waits for the scroll element and gives up after the timeout', () => {
		const clock = frames()
		let element: HTMLElement | null = null
		const onDone = vi.fn()
		restoreScroll(() => element, 3000, { ...clock, onDone, timeout: 100 })
		clock.tick()
		element = makeScrollable(1000)
		// never tall enough: stays as far as it can go until the timeout
		for (let i = 0; i < 6; i++) clock.tick()
		expect(onDone).not.toHaveBeenCalled()
		expect(element.scrollTop).toBe(200)
		for (let i = 0; i < 2; i++) clock.tick()
		expect(onDone).toHaveBeenCalledWith(element)
	})

	it('stops when told to', () => {
		const clock = frames()
		const element = makeScrollable(1000)
		const stop = restoreScroll(() => element, 3000, { ...clock })
		stop()
		expect(clock.pending()).toBe(0)
		setScrollHeight(element, 5000)
		clock.tick()
		expect(element.scrollTop).toBe(200)
	})
})

describe('useMainScrollRestoration', () => {
	// the restore's first step is synchronous; later frames aren't needed here
	beforeEach(() => {
		vi.stubGlobal(
			'requestAnimationFrame',
			vi.fn(() => 1),
		)
		vi.stubGlobal('cancelAnimationFrame', vi.fn())
	})
	afterEach(() => vi.unstubAllGlobals())

	const at = (index: number, pathname: string, type: 'POP' | 'PUSH' | 'REPLACE', search = '') => {
		window.history.replaceState({ idx: index }, '', pathname + search)
		vi.mocked(useLocation).mockReturnValue({
			hash: '',
			key: `${index}${pathname}${search}${type}`,
			pathname,
			search,
			state: null,
		})
		vi.mocked(useNavigationType).mockReturnValue(type as never)
	}

	it('records the position as the user scrolls and restores it on back', () => {
		const element = makeScrollable(6000)
		const getElement = () => element

		at(1, '/libraries/l1/books', 'PUSH')
		const { rerender } = renderHook(() => useMainScrollRestoration(getElement, element))
		expect(element.scrollTop).toBe(0)

		element.scrollTop = 3100
		element.dispatchEvent(new Event('scroll'))
		expect(readHistoryEntry(1, '/libraries/l1/books')?.top).toBe(3100)

		// a book: a new page, at the top
		at(2, '/books/b1', 'PUSH')
		rerender()
		expect(element.scrollTop).toBe(0)
		expect(readHistoryEntry(1, '/libraries/l1/books')?.top).toBe(3100)

		// back to the list
		at(1, '/libraries/l1/books', 'POP')
		rerender()
		expect(element.scrollTop).toBe(3100)
	})

	it("doesn't record the page's scrolling while the next page is loading", () => {
		const element = makeScrollable(6000)
		at(1, '/books/b1', 'PUSH')
		renderHook(() => useMainScrollRestoration(() => element, element))
		element.scrollTop = 400
		element.dispatchEvent(new Event('scroll'))
		expect(readHistoryEntry(1, '/books/b1')?.top).toBe(400)

		// the reader's URL is current, the book page still on screen: its content is hidden
		window.history.pushState({ idx: 2 }, '', '/books/b1/epub-reader')
		element.scrollTop = 0
		element.dispatchEvent(new Event('scroll'))
		expect(readHistoryEntry(1, '/books/b1')?.top).toBe(400)
	})

	it('keeps the position when a search replaces the entry, and records it for the new URL', () => {
		const element = makeScrollable(6000)
		const getElement = () => element

		at(4, '/browse/authors', 'PUSH')
		const { rerender } = renderHook(() => useMainScrollRestoration(getElement, element))
		element.scrollTop = 800
		element.dispatchEvent(new Event('scroll'))

		at(4, '/browse/authors', 'REPLACE', '?q=fr')
		rerender()
		expect(element.scrollTop).toBe(800)
		expect(readHistoryEntry(4, '/browse/authors?q=fr')?.top).toBe(800)
	})

	it('leaves the readers alone', () => {
		const element = makeScrollable(6000)
		element.scrollTop = 500
		at(5, '/books/b1/epub-reader', 'PUSH')
		renderHook(() => useMainScrollRestoration(() => element, element, { skip: true }))
		expect(element.scrollTop).toBe(500)
	})
})

describe('useHistoryEntryState', () => {
	const at = (index: number, pathname: string, type: 'POP' | 'PUSH') => {
		window.history.replaceState({ idx: index }, '', pathname)
		vi.mocked(useLocation).mockReturnValue({
			hash: '',
			key: `${index}${type}`,
			pathname,
			search: '',
			state: null,
		})
		vi.mocked(useNavigationType).mockReturnValue(type as never)
	}

	it('starts a new visit from the initial value and a return from the kept one', () => {
		at(2, '/smart-lists/s1', 'PUSH')
		const first = renderHook(() => useHistoryEntryState<Record<string, boolean>>('groups', {}))
		expect(first.result.current[0]).toEqual({})
		act(() => first.result.current[1]({ dune: true }))
		first.unmount()

		at(2, '/smart-lists/s1', 'POP')
		const back = renderHook(() => useHistoryEntryState<Record<string, boolean>>('groups', {}))
		expect(back.result.current[0]).toEqual({ dune: true })
		back.unmount()

		// a new visit of the same list
		at(3, '/smart-lists/s1', 'PUSH')
		const again = renderHook(() => useHistoryEntryState<Record<string, boolean>>('groups', {}))
		expect(again.result.current[0]).toEqual({})
	})
})

describe('useRestoredScrollLeft', () => {
	beforeEach(() => {
		vi.stubGlobal(
			'requestAnimationFrame',
			vi.fn(() => 1),
		)
		vi.stubGlobal('cancelAnimationFrame', vi.fn())
	})
	afterEach(() => vi.unstubAllGlobals())

	/** A horizontal row with layout */
	const makeRow = () => {
		const element = document.createElement('div')
		let scrollLeft = 0
		Object.defineProperty(element, 'scrollLeft', {
			configurable: true,
			get: () => scrollLeft,
			set: (value: number) => {
				scrollLeft = Math.max(0, Math.min(value, 4000 - 1000))
			},
		})
		Object.defineProperty(element, 'scrollWidth', { configurable: true, value: 4000 })
		Object.defineProperty(element, 'clientWidth', { configurable: true, value: 1000 })
		return element
	}

	const at = (index: number, type: 'POP' | 'PUSH') => {
		window.history.replaceState({ idx: index }, '', '/')
		vi.mocked(useLocation).mockReturnValue({
			hash: '',
			key: `${index}${type}`,
			pathname: '/',
			search: '',
			state: null,
		})
		vi.mocked(useNavigationType).mockReturnValue(type as never)
		setShownHistoryEntry({ index, path: '/' })
	}

	it("records a row's position and returns to it on back", () => {
		at(0, 'PUSH')
		const first = renderHook(() => useRestoredScrollLeft('home-on-deck'))
		const row = makeRow()
		first.result.current(row)
		row.scrollLeft = 1700
		row.dispatchEvent(new Event('scroll'))
		expect(readHistoryEntry(0, '/')?.state).toEqual({ 'scrollLeft:home-on-deck': 1700 })
		first.unmount()

		at(0, 'POP')
		const back = renderHook(() => useRestoredScrollLeft('home-on-deck'))
		const again = makeRow()
		back.result.current(again)
		expect(again.scrollLeft).toBe(1700)
	})

	it('starts a new visit at the start, and does nothing without a key', () => {
		updateHistoryEntry(0, '/', { state: { 'scrollLeft:home-on-deck': 900 } })
		at(0, 'PUSH')
		const fresh = renderHook(() => useRestoredScrollLeft('home-on-deck'))
		const row = makeRow()
		fresh.result.current(row)
		expect(row.scrollLeft).toBe(0)

		at(0, 'POP')
		const keyless = renderHook(() => useRestoredScrollLeft(undefined))
		const other = makeRow()
		keyless.result.current(other)
		expect(other.scrollLeft).toBe(0)
		other.scrollLeft = 300
		other.dispatchEvent(new Event('scroll'))
		expect(readHistoryEntry(0, '/')?.state).toEqual({ 'scrollLeft:home-on-deck': 900 })
	})
})
