import { useEffect } from 'react'

import { isEditableTarget } from '@/components/readers/annotations/shortcuts'

/** The list searches on screen (the toolbar search of Home, Explore, Authors, Genres, lists) */
const openers: Array<() => void> = []

/** A list search registers how to open it; returns the function that unregisters it */
export function registerListSearch(open: () => void): () => void {
	openers.push(open)
	return () => {
		const index = openers.lastIndexOf(open)
		if (index >= 0) openers.splice(index, 1)
	}
}

/** Opens or focuses the list search on screen; `false` if there's none */
export function openOrFocusListSearch(): boolean {
	const open = openers[openers.length - 1]
	if (!open) return false
	open()
	return true
}

/**
 * App-wide `/`: opens the page's list search, so Firefox's quick find bar doesn't pop up.
 * Mounted once (in the app layout). Typing in a field, an open dialog or a page without a
 * list search (e.g. a reader, which handles `/` itself) leave the key alone.
 */
export function useSlashOpensListSearch() {
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.key !== '/' || event.defaultPrevented) return
			if (event.ctrlKey || event.metaKey || event.altKey) return
			if (isEditableTarget(event.target)) return
			if (document.querySelector('[role="dialog"][data-state="open"]')) return
			if (openOrFocusListSearch()) event.preventDefault()
		}
		window.addEventListener('keydown', onKeyDown)
		return () => window.removeEventListener('keydown', onKeyDown)
	}, [])
}
