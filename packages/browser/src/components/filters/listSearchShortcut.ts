import { useEffect } from 'react'

import { isEditableTarget } from '@/components/readers/annotations/shortcuts'

import { openListSearch } from './searchRegistry'

/** Opens or focuses the list search on screen; `false` if there's none (see `searchRegistry`) */
export function openOrFocusListSearch(): boolean {
	return openListSearch()
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
