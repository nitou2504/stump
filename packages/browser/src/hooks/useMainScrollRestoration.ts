import { useCallback, useEffect, useLayoutEffect, useRef } from 'react'
import { NavigationType, useLocation, useNavigationType } from 'react-router'

import {
	currentHistoryIndex,
	dropHistoryEntriesAfter,
	flushHistoryEntries,
	HistoryEntryRecord,
	readHistoryEntry,
	updateHistoryEntry,
} from './historyEntries'

export type MainScrollPlan = { kind: 'restore'; top: number } | { kind: 'top' } | { kind: 'keep' }

type PlanParams = {
	navigationType: NavigationType | `${NavigationType}`
	/** The record of the entry being shown, if it was recorded for this page */
	record?: HistoryEntryRecord
	pathname: string
	/** The pathname shown before this navigation (undefined on the first render) */
	previousPathname?: string
	/** The full page (`pathname + search`) shown before this navigation */
	previousPath?: string
	path: string
	hash: string
}

/**
 * What to do with the main scroll position after a navigation, like a website would:
 * - back/forward (and a reload) return to where the page was left
 * - a new page starts at the top, except a link to an anchor of the page being shown
 * - replacing the entry with the same page (e.g. typing a search, which lives in the URL)
 *   keeps the position; a redirect to another page starts at the top
 */
export function planMainScroll({
	navigationType,
	record,
	pathname,
	previousPathname,
	previousPath,
	path,
	hash,
}: PlanParams): MainScrollPlan {
	if (navigationType === 'POP') {
		return record?.top ? { kind: 'restore', top: record.top } : { kind: 'top' }
	}
	if (navigationType === 'PUSH') {
		return hash && previousPath === path ? { kind: 'keep' } : { kind: 'top' }
	}
	return previousPathname === undefined || previousPathname === pathname
		? { kind: 'keep' }
		: { kind: 'top' }
}

type Scrollable = Pick<HTMLElement, 'scrollTop' | 'scrollHeight' | 'clientHeight'>

type RestoreOptions = {
	/** How long to keep trying once the scroll element exists, in ms */
	timeout?: number
	/** For how many frames the position must hold (with an unchanged height) to be done */
	stableFrames?: number
	onDone?: (element: Scrollable | null) => void
	now?: () => number
	requestFrame?: (callback: () => void) => number
	cancelFrame?: (id: number) => void
}

/** Gives up waiting for the scroll element itself after this long, in ms */
const MAX_WAIT_FOR_ELEMENT = 10_000

/**
 * Scrolls the element returned by `getElement` to `target`. The page under it is often not
 * tall enough yet (its data or a virtualised list is still arriving), so it scrolls as far
 * as it can and tries again every frame, until the target holds for a few frames or the
 * timeout passes. Returns a function that stops it.
 */
export function restoreScrollTop(
	getElement: () => Scrollable | null,
	target: number,
	{
		timeout = 4000,
		stableFrames = 8,
		onDone,
		now = () => performance.now(),
		requestFrame = (callback) => requestAnimationFrame(callback),
		cancelFrame = (id) => cancelAnimationFrame(id),
	}: RestoreOptions = {},
): () => void {
	const startedAt = now()
	let elementSince: number | undefined
	let stable = 0
	let lastHeight = -1
	let frame: number | undefined
	let stopped = false

	const finish = (element: Scrollable | null) => {
		stopped = true
		onDone?.(element)
	}

	const step = () => {
		frame = undefined
		if (stopped) return
		const element = getElement()
		const time = now()
		if (!element) {
			if (time - startedAt > MAX_WAIT_FOR_ELEMENT) return finish(null)
			frame = requestFrame(step)
			return
		}
		elementSince ??= time

		const maxScroll = Math.max(0, element.scrollHeight - element.clientHeight)
		const desired = Math.min(target, maxScroll)
		if (Math.abs(element.scrollTop - desired) >= 1) {
			element.scrollTop = desired
		}

		const reached = maxScroll >= target - 1 && Math.abs(element.scrollTop - target) < 1
		stable = reached && element.scrollHeight === lastHeight ? stable + 1 : reached ? 1 : 0
		lastHeight = element.scrollHeight

		if (stable >= stableFrames || time - elementSince > timeout) return finish(element)
		frame = requestFrame(step)
	}

	step()

	return () => {
		stopped = true
		if (frame !== undefined) cancelFrame(frame)
	}
}

/** Input that means the user is scrolling (or going somewhere) themselves */
const USER_INPUT_EVENTS = ['wheel', 'touchstart', 'pointerdown', 'keydown'] as const

/**
 * Keeps the main scroll position of every history entry and restores it on back/forward
 * (and reloads), like a website does with the window. The app scrolls in its own element
 * (the main area, or its custom scrollbars' viewport), which browsers don't restore.
 * `getScrollElement` returns that element; `host` is the main area, which contains it.
 * Use it once, in the app layout. Nothing happens while `skip` is set (the readers, which
 * scroll on their own).
 */
export function useMainScrollRestoration(
	getScrollElement: () => HTMLElement | null,
	host: HTMLElement | null,
	{ skip = false }: { skip?: boolean } = {},
) {
	const location = useLocation()
	const navigationType = useNavigationType()
	const { key, pathname, hash } = location
	const path = pathname + location.search

	// The latest getter, for the listeners and the restore loop (the element can change after
	// a navigation was handled, e.g. when the custom scrollbars are set up)
	const getElementRef = useRef(getScrollElement)
	useLayoutEffect(() => {
		getElementRef.current = getScrollElement
	}, [getScrollElement])
	const scrollElement = useCallback(() => getElementRef.current(), [])

	/** The entry and page whose position scrolling records, i.e. the one shown */
	const shownRef = useRef<{ index?: number; path: string; pathname: string } | undefined>(undefined)
	/** Set while a position is being restored: its own scrolling isn't recorded */
	const restoringRef = useRef(false)

	const record = useCallback(() => {
		const shown = shownRef.current
		const element = scrollElement()
		if (!shown || shown.index === undefined || !element || restoringRef.current) return
		updateHistoryEntry(shown.index, shown.path, { top: element.scrollTop })
	}, [scrollElement])

	// Record the position as the user scrolls (only the main element: the capture listener on
	// the host also hears nested scrollers, e.g. horizontal rows)
	useEffect(() => {
		if (!host) return
		const onScroll = (event: Event) => {
			if (event.target === scrollElement()) record()
		}
		host.addEventListener('scroll', onScroll, { capture: true, passive: true })
		return () => host.removeEventListener('scroll', onScroll, { capture: true })
	}, [host, scrollElement, record])

	useEffect(() => {
		const onHide = () => {
			record()
			flushHistoryEntries()
		}
		const onVisibility = () => document.visibilityState === 'hidden' && onHide()
		window.addEventListener('pagehide', onHide)
		document.addEventListener('visibilitychange', onVisibility)
		return () => {
			window.removeEventListener('pagehide', onHide)
			document.removeEventListener('visibilitychange', onVisibility)
		}
	}, [record])

	// On each navigation (the other dependencies don't change between navigations)
	useLayoutEffect(() => {
		const previous = shownRef.current
		const index = currentHistoryIndex()
		shownRef.current = { index, path, pathname }
		if (index === undefined || skip) return

		const plan = planMainScroll({
			navigationType,
			record: readHistoryEntry(index, path),
			pathname,
			previousPathname: previous?.pathname,
			previousPath: previous?.path,
			path,
			hash,
		})

		if (navigationType === 'PUSH') {
			// a new entry: whatever was recorded at its index or after belongs to entries it dropped
			dropHistoryEntriesAfter(index - 1)
		} else if (navigationType === 'REPLACE' && previous && previous.index === index) {
			// the same entry, now showing `path`: it keeps its page state
			const replaced = readHistoryEntry(index, previous.path)
			if (replaced?.state) updateHistoryEntry(index, path, { state: replaced.state })
		}

		const element = scrollElement()
		if (plan.kind === 'top') {
			if (element) element.scrollTop = 0
			updateHistoryEntry(index, path, { top: 0 })
			return
		}
		if (plan.kind === 'keep') {
			updateHistoryEntry(index, path, { top: element?.scrollTop ?? 0 })
			return
		}

		// restore, until the position holds, the user scrolls themselves, or they leave
		restoringRef.current = true
		const restore: { stop?: () => void } = {}
		const stop = (recordPosition: boolean) => {
			if (!restoringRef.current) return
			restoringRef.current = false
			restore.stop?.()
			for (const type of USER_INPUT_EVENTS) {
				window.removeEventListener(type, onInput, { capture: true })
			}
			if (recordPosition) record()
		}
		const onInput = () => stop(true)
		for (const type of USER_INPUT_EVENTS) {
			window.addEventListener(type, onInput, { capture: true, passive: true })
		}
		restore.stop = restoreScrollTop(scrollElement, plan.top, { onDone: () => stop(true) })
		// leaving before it's done: the next page is already in the DOM, so the position read
		// now would be that page's, not this one's
		return () => stop(false)
	}, [key, pathname, hash, path, navigationType, skip, scrollElement, record])
}
