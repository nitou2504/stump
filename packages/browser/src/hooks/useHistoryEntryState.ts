import { Dispatch, SetStateAction, useEffect, useState } from 'react'
import { useLocation, useNavigationType } from 'react-router'

import { currentHistoryIndex, readHistoryEntry, updateHistoryEntry } from './historyEntries'

/**
 * Like `useState`, but the value is kept with the current browser history entry: going
 * back (or forward, or reloading) to it starts from the value it had there, while a new
 * visit starts from `initial`. For page state the page's scroll position depends on, such
 * as which groups of a list are open. The value must survive JSON.
 */
export function useHistoryEntryState<T>(
	id: string,
	initial: T | (() => T),
): [T, Dispatch<SetStateAction<T>>] {
	const { pathname, search } = useLocation()
	const navigationType = useNavigationType()
	const path = pathname + search

	const [value, setValue] = useState<T>(() => {
		const index = currentHistoryIndex()
		if (navigationType === 'POP' && index !== undefined) {
			const saved = readHistoryEntry(index, path)?.state?.[id]
			if (saved !== undefined) return saved as T
		}
		return typeof initial === 'function' ? (initial as () => T)() : initial
	})

	useEffect(() => {
		const index = currentHistoryIndex()
		if (index === undefined) return
		updateHistoryEntry(index, path, { state: { [id]: value } })
	}, [id, path, value])

	return [value, setValue]
}
