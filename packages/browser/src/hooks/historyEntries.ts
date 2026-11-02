/**
 * What the app remembers about each of this tab's browser history entries: the main
 * scroll position and small bits of page state (e.g. which smart list groups are open),
 * so that going back or forward shows a page as it was left. Entries are identified by
 * their index (React Router keeps it in `history.state.idx`, which survives reloads and
 * stays the same when an entry is replaced, e.g. while typing a search), and the page they
 * showed, so a record is never applied to a different page. It lives in session storage,
 * like the browser history itself.
 */
export type HistoryEntryRecord = {
	/** The page the entry showed (`pathname + search`) when this was recorded */
	path: string
	/** The main scroll element's `scrollTop` */
	top?: number
	/** Page state by id (see `useHistoryEntryState`) */
	state?: Record<string, unknown>
}

type Records = Record<number, HistoryEntryRecord>

const STORAGE_KEY = 'stump-history-entries'
/** Entries further back than this are forgotten, so the records stay small in long sessions */
const MAX_ENTRIES = 200
const PERSIST_DELAY = 300

/** The index of the current browser history entry, as React Router numbers them */
export const currentHistoryIndex = (): number | undefined => {
	const index = window.history.state?.idx
	return typeof index === 'number' ? index : undefined
}

let records: Records | null = null
/** The entry (and page) on screen, as the main scroll restoration last handled it */
let shown: { index: number; path: string } | undefined
let persistTimer: ReturnType<typeof setTimeout> | undefined

const load = (): Records => {
	if (records) return records
	try {
		records = JSON.parse(sessionStorage.getItem(STORAGE_KEY) ?? '{}') as Records
	} catch {
		records = {}
	}
	return records
}

/** Writes the records to session storage now (e.g. before the page is hidden or reloaded) */
export function flushHistoryEntries() {
	if (persistTimer !== undefined) {
		clearTimeout(persistTimer)
		persistTimer = undefined
	}
	if (!records) return
	try {
		sessionStorage.setItem(STORAGE_KEY, JSON.stringify(records))
	} catch {
		// unavailable or full: positions are then only kept until the tab is reloaded
	}
}

const schedulePersist = () => {
	if (persistTimer !== undefined) return
	persistTimer = setTimeout(flushHistoryEntries, PERSIST_DELAY)
}

/** The record of the entry at `index`, if it was recorded for the page `path` */
export function readHistoryEntry(index: number, path: string): HistoryEntryRecord | undefined {
	const record = load()[index]
	return record?.path === path ? record : undefined
}

/**
 * Records `patch` for the entry at `index`, which shows `path`. A record made for another
 * page is replaced (the entry now shows `path`, e.g. after a redirect or a new search)
 */
export function updateHistoryEntry(
	index: number,
	path: string,
	patch: Omit<HistoryEntryRecord, 'path'>,
) {
	const all = load()
	const existing = all[index]
	const base = existing && existing.path === path ? existing : { path }
	all[index] = {
		...base,
		...patch,
		path,
		state: patch.state ? { ...base.state, ...patch.state } : base.state,
	}
	schedulePersist()
}

/**
 * Forgets the entries after `index` (a new entry drops the ones that were ahead of it)
 * and those too far behind it
 */
export function dropHistoryEntriesAfter(index: number) {
	const all = load()
	for (const entry of Object.keys(all).map(Number)) {
		if (entry > index || entry <= index - MAX_ENTRIES) delete all[entry]
	}
	schedulePersist()
}

/** Sets the entry (and page) on screen, once a navigation to it is rendered */
export function setShownHistoryEntry(entry: { index: number; path: string } | undefined) {
	shown = entry
}

/**
 * The entry (and page) on screen. Unlike the URL, it doesn't change until the page a
 * navigation goes to is rendered, so it's what records about the page on screen belong to
 */
export function shownHistoryEntry() {
	return shown
}

/** Test helper: forget the in-memory copy so the next read comes from session storage */
export function resetHistoryEntriesForTests() {
	if (persistTimer !== undefined) clearTimeout(persistTimer)
	persistTimer = undefined
	records = null
	shown = undefined
}
