import { useCallback, useEffect } from 'react'
import { useLocation, useNavigate, useNavigationType } from 'react-router'

/**
 * The app page each of this tab's browser history entries shows, by the entry's index
 * (React Router keeps it in `history.state.idx`). It lives in session storage, like the
 * browser history itself, so it survives reloads.
 */
export type HistoryTrail = Record<number, string>

const STORAGE_KEY = 'stump-history-trail'
/** Entries further back than this are forgotten, so the trail stays small in long sessions */
const MAX_ENTRIES = 200

const currentHistoryIndex = (): number | undefined => {
	const index = window.history.state?.idx
	return typeof index === 'number' ? index : undefined
}

const readTrail = (): HistoryTrail => {
	try {
		return JSON.parse(sessionStorage.getItem(STORAGE_KEY) ?? '{}') as HistoryTrail
	} catch {
		return {}
	}
}

const writeTrail = (trail: HistoryTrail) => {
	try {
		sessionStorage.setItem(STORAGE_KEY, JSON.stringify(trail))
	} catch {
		// unavailable or full: leaving pages then falls back to replacing them
	}
}

/** Records the page shown by each history entry. Use it once, in the app layout */
export function useRecordHistoryTrail() {
	const { key, pathname, search } = useLocation()
	const navigationType = useNavigationType()

	useEffect(() => {
		const index = currentHistoryIndex()
		if (index === undefined) return

		const trail = readTrail()
		if (navigationType === 'PUSH') {
			// a new entry drops the entries that were ahead of it
			for (const entry of Object.keys(trail).map(Number)) {
				if (entry > index || entry <= index - MAX_ENTRIES) delete trail[entry]
			}
		}
		trail[index] = pathname + search
		writeTrail(trail)
	}, [key, pathname, search, navigationType])
}

/**
 * How to leave the pages matching `isLeaving`, the last of which is the history entry
 * at `index`: go `back` entries, to the page shown before them. When the trail doesn't
 * reach such a page (e.g. a book page opened from a bookmark), go back to the first of
 * them instead and `replace` it, so that going back never returns to those pages.
 */
export function planLeave(
	trail: HistoryTrail,
	index: number,
	isLeaving: (path: string) => boolean,
): { back: number; replace: boolean } {
	let first = index
	for (let entry = index - 1; entry >= 0; entry--) {
		const path = trail[entry]
		if (path === undefined) break
		if (!isLeaving(path)) return { back: index - entry, replace: false }
		first = entry
	}
	return { back: index - first, replace: true }
}

/**
 * Leaves pages that no longer exist (e.g. those of a book that was just removed) by going
 * back to the page the user was on before them, with its search and filters, as if they
 * had pressed back. Without such a page in the history, they are replaced with `fallback`.
 * Nothing happens when the current page is not one of them.
 */
export function useLeavePages() {
	const navigate = useNavigate()

	return useCallback(
		(isLeaving: (path: string) => boolean, fallback: string) => {
			if (!isLeaving(window.location.pathname + window.location.search)) return

			const index = currentHistoryIndex()
			const { back, replace } =
				index === undefined ? { back: 0, replace: true } : planLeave(readTrail(), index, isLeaving)

			if (!replace) {
				navigate(-back)
			} else if (back === 0) {
				navigate(fallback, { replace: true })
			} else {
				// replace the first of the pages once the browser is back on it
				window.addEventListener('popstate', () => navigate(fallback, { replace: true }), {
					once: true,
				})
				navigate(-back)
			}
		},
		[navigate],
	)
}
