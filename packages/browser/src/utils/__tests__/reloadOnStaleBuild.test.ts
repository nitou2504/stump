import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { RELOAD_GUARD_MS, reloadOnStaleBuild, STALE_BUILD_RELOAD_KEY } from '../reloadOnStaleBuild'

type Options = { storage?: 'working' | 'broken'; online?: boolean }

function fakeWindow({ storage = 'working', online = true }: Options = {}) {
	const target = new EventTarget()
	const store = new Map<string, string>()
	const reload = vi.fn()
	const sessionStorage =
		storage === 'working'
			? {
					getItem: (key: string) => store.get(key) ?? null,
					setItem: (key: string, value: string) => void store.set(key, value),
				}
			: {
					getItem: () => {
						throw new Error('storage is blocked')
					},
					setItem: () => {
						throw new Error('storage is blocked')
					},
				}
	const win = {
		addEventListener: target.addEventListener.bind(target),
		removeEventListener: target.removeEventListener.bind(target),
		sessionStorage,
		location: { reload },
		navigator: { onLine: online },
	} as unknown as Window

	/** A failed dynamic import, as Vite reports it: a cancelable event. Returns whether it was left alone */
	const fail = () => {
		const event = new Event('vite:preloadError', { cancelable: true })
		target.dispatchEvent(event)
		return event.defaultPrevented
	}
	return { win, reload, store, fail }
}

describe('reloading a tab that runs an old build', () => {
	beforeEach(() => {
		vi.useFakeTimers()
		vi.setSystemTime(new Date('2026-11-14T16:00:00Z'))
	})
	afterEach(() => {
		vi.useRealTimers()
	})

	it('reloads once, and keeps the failure from reaching the error screen', () => {
		const { win, reload, fail, store } = fakeWindow()
		reloadOnStaleBuild(win)

		expect(fail()).toBe(true)

		expect(reload).toHaveBeenCalledTimes(1)
		expect(store.get(STALE_BUILD_RELOAD_KEY)).toBe(String(Date.now()))
	})

	it('does not loop: a failure right after the reload shows the error instead', () => {
		const { win, reload, fail } = fakeWindow()
		reloadOnStaleBuild(win)
		fail()

		vi.advanceTimersByTime(RELOAD_GUARD_MS - 1)
		expect(fail()).toBe(false)

		expect(reload).toHaveBeenCalledTimes(1)
	})

	it('reloads again for a deploy that happens much later in the same tab', () => {
		const { win, reload, fail } = fakeWindow()
		reloadOnStaleBuild(win)
		fail()

		vi.advanceTimersByTime(RELOAD_GUARD_MS)
		expect(fail()).toBe(true)

		expect(reload).toHaveBeenCalledTimes(2)
	})

	it('does not take a clock that went backwards as a reload a moment ago', () => {
		const { win, reload, fail, store } = fakeWindow()
		store.set(STALE_BUILD_RELOAD_KEY, String(Date.now() + 60 * 60 * 1000))
		reloadOnStaleBuild(win)

		expect(fail()).toBe(true)
		expect(reload).toHaveBeenCalledTimes(1)
	})

	it('reloads nothing when the guard cannot be kept (no working sessionStorage)', () => {
		const { win, reload, fail } = fakeWindow({ storage: 'broken' })
		reloadOnStaleBuild(win)

		expect(fail()).toBe(false)
		expect(reload).not.toHaveBeenCalled()
	})

	it('reloads nothing offline: the browser would show its own offline page', () => {
		const { win, reload, fail } = fakeWindow({ online: false })
		reloadOnStaleBuild(win)

		expect(fail()).toBe(false)
		expect(reload).not.toHaveBeenCalled()
	})

	it('stops listening when told to', () => {
		const { win, reload, fail } = fakeWindow()
		const stop = reloadOnStaleBuild(win)
		stop()

		expect(fail()).toBe(false)
		expect(reload).not.toHaveBeenCalled()
	})
})
