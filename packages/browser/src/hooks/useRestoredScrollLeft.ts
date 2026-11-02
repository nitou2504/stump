import { useCallback, useEffect, useRef } from 'react'
import { useLocation, useNavigationType } from 'react-router'

import {
	currentHistoryIndex,
	readHistoryEntry,
	shownHistoryEntry,
	updateHistoryEntry,
} from './historyEntries'
import { restoreScroll } from './useMainScrollRestoration'

/** Input that means the user is scrolling the row themselves */
const USER_INPUT_EVENTS = ['wheel', 'touchstart', 'pointerdown'] as const

/**
 * Keeps a horizontal row's scroll position with the current history entry, under `key`
 * (unique on the page): going back to the page returns the row to where it was left, like
 * the page itself. Returns a callback to give the row's scroller to (or null when it goes
 * away). Without a key it does nothing.
 */
export function useRestoredScrollLeft(key?: string) {
	const { pathname, search } = useLocation()
	const navigationType = useNavigationType()
	const path = pathname + search

	// the page this row is on: records belong to it, never to a page being navigated to
	const pathRef = useRef(path)
	useEffect(() => {
		pathRef.current = path
	}, [path])
	/** Only a back/forward visit restores, once */
	const restorePendingRef = useRef(navigationType === 'POP')
	const detachRef = useRef<(() => void) | null>(null)

	const attach = useCallback(
		(element: HTMLElement | null) => {
			detachRef.current?.()
			detachRef.current = null
			if (!key || !element) return
			const stateKey = `scrollLeft:${key}`

			let stopRestore: (() => void) | undefined
			const index = currentHistoryIndex()
			if (restorePendingRef.current && index !== undefined) {
				restorePendingRef.current = false
				const saved = readHistoryEntry(index, pathRef.current)?.state?.[stateKey]
				if (typeof saved === 'number' && saved > 0) {
					stopRestore = restoreScroll(() => element, saved, {
						axis: 'left',
						minDuration: 800,
						onDone: () => {
							stopRestore = undefined
						},
					})
				}
			}
			const stop = () => {
				stopRestore?.()
				stopRestore = undefined
			}

			const onScroll = () => {
				if (stopRestore) return
				const shown = shownHistoryEntry()
				if (!shown || shown.path !== pathRef.current) return
				updateHistoryEntry(shown.index, shown.path, {
					state: { [stateKey]: Math.round(element.scrollLeft) },
				})
			}
			element.addEventListener('scroll', onScroll, { passive: true })
			for (const type of USER_INPUT_EVENTS) {
				element.addEventListener(type, stop, { passive: true })
			}
			detachRef.current = () => {
				stop()
				element.removeEventListener('scroll', onScroll)
				for (const type of USER_INPUT_EVENTS) {
					element.removeEventListener(type, stop)
				}
			}
		},
		[key],
	)

	useEffect(() => () => detachRef.current?.(), [])

	return attach
}
