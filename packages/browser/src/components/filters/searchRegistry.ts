/**
 * The list searches currently on screen, so a page-wide shortcut (e.g. '/') can open the one
 * the user sees without knowing which page it is on. Each mounted `Search` registers itself;
 * the most recently mounted visible one wins.
 */
type ListSearchEntry = {
	/** Opens the search (expanding it on phones) and focuses its input */
	open: () => void
	/** Whether the search is currently rendered and visible */
	isVisible: () => boolean
}

const entries: ListSearchEntry[] = []

/** Registers a search; returns the function that unregisters it */
export function registerListSearch(entry: ListSearchEntry) {
	entries.push(entry)
	return () => {
		const index = entries.lastIndexOf(entry)
		if (index >= 0) entries.splice(index, 1)
	}
}

/**
 * Opens and focuses the list search on screen. Returns whether there was one, so a caller
 * can fall back to its own behaviour (and only then skip `preventDefault`).
 */
export function openListSearch() {
	for (let index = entries.length - 1; index >= 0; index--) {
		const entry = entries[index]
		if (entry?.isVisible()) {
			entry.open()
			return true
		}
	}
	return false
}
