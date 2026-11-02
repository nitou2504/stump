import { useCallback, useMemo, useState } from 'react'

/**
 * The reader's own "back" and "forward" for big moves, the same in every reader: going to a
 * page or a location, a contents or outline entry, a search hit, a bookmark or an
 * annotation, or a link in the book remembers where the reader was, so the contents (or a
 * comic's pages) can offer "Back to page 23 · Chapter 2". Turning or scrolling pages never
 * adds a place.
 *
 * The places are kept per book for the browser tab (sessionStorage): a reload keeps them,
 * another tab or a later visit starts afresh.
 */

/** A place in the book and how it's shown, e.g. `Page 23 · Chapter 2` */
export type NavEntry<P> = {
	place: P
	label: string
}

export type NavHistoryState<P> = {
	/** The places to go back to, the latest last */
	back: NavEntry<P>[]
	/** The places left by going back, the latest last */
	forward: NavEntry<P>[]
}

/** How many places each way are kept */
export const NAV_HISTORY_LIMIT = 5

/** Places closer than this many pages (or locations) don't count as a jump */
export const FAR_JUMP_PAGES = 3

const EMPTY: NavHistoryState<never> = { back: [], forward: [] }

const capped = <T>(list: T[], limit: number) => (list.length > limit ? list.slice(-limit) : list)

/**
 * The reader is about to leave `from` with a jump: it becomes the place to go back to (unless
 * it already is), and what was ahead is dropped, as a browser does after a new page
 */
export function recordJump<P>(
	state: NavHistoryState<P>,
	from: NavEntry<P>,
	same: (a: P, b: P) => boolean,
	limit = NAV_HISTORY_LIMIT,
): NavHistoryState<P> {
	const last = state.back[state.back.length - 1]
	const back = last && same(last.place, from.place) ? state.back : [...state.back, from]
	return { back: capped(back, limit), forward: [] }
}

/**
 * Going back: the latest place to go back to, which leaves the history; `current` (where the
 * reader is) becomes the place to go forward to. `null` with nothing to go back to.
 */
export function takeBack<P>(
	state: NavHistoryState<P>,
	current: NavEntry<P> | null,
	same: (a: P, b: P) => boolean,
	limit = NAV_HISTORY_LIMIT,
): { state: NavHistoryState<P>; entry: NavEntry<P> } | null {
	return take(state, 'back', current, same, limit)
}

/** Going forward again, after going back: the mirror of `takeBack` */
export function takeForward<P>(
	state: NavHistoryState<P>,
	current: NavEntry<P> | null,
	same: (a: P, b: P) => boolean,
	limit = NAV_HISTORY_LIMIT,
): { state: NavHistoryState<P>; entry: NavEntry<P> } | null {
	return take(state, 'forward', current, same, limit)
}

function take<P>(
	state: NavHistoryState<P>,
	from: 'back' | 'forward',
	current: NavEntry<P> | null,
	same: (a: P, b: P) => boolean,
	limit: number,
): { state: NavHistoryState<P>; entry: NavEntry<P> } | null {
	const to = from === 'back' ? 'forward' : 'back'
	const list = state[from]
	// a place the reader is already at (e.g. it paged back there) isn't worth a step
	let end = list.length
	while (end > 0 && current && same(list[end - 1]!.place, current.place)) end -= 1
	const entry = list[end - 1]
	if (!entry) return null
	const other = state[to]
	const last = other[other.length - 1]
	const kept =
		!current || (last && same(last.place, current.place))
			? other
			: capped([...other, current], limit)
	return {
		entry,
		state: { ...state, [from]: list.slice(0, end - 1), [to]: kept } as NavHistoryState<P>,
	}
}

/**
 * The place at the top of `list` that the reader isn't already at (the row to show), or
 * `null`
 */
export function nextUseful<P>(
	list: NavEntry<P>[],
	current: P | null,
	same: (a: P, b: P) => boolean,
): NavEntry<P> | null {
	for (let index = list.length - 1; index >= 0; index -= 1) {
		const entry = list[index]!
		if (!current || !same(entry.place, current)) return entry
	}
	return null
}

/**
 * Whether going from one page (or location, from 1) to another is a jump worth remembering:
 * more than a few pages, or into another chapter (when the chapters are known)
 */
export function isFarJump(
	from: { page: number; chapter?: string | number | null },
	to: { page: number; chapter?: string | number | null },
	pages = FAR_JUMP_PAGES,
): boolean {
	if (from.chapter != null && to.chapter != null && from.chapter !== to.chapter) return true
	return Math.abs(to.page - from.page) > pages
}

// Storage -----------------------------------------------------------------------------------

/** The session storage key of a book's places in one reader */
export const navHistoryKey = (reader: 'epub' | 'pdf' | 'comic', bookId: string) =>
	`stump-reader-nav:${reader}:${bookId}`

function load<P>(key: string, isPlace: (value: unknown) => value is P): NavHistoryState<P> {
	try {
		const raw = window.sessionStorage.getItem(key)
		if (!raw) return EMPTY
		const parsed = JSON.parse(raw) as Partial<NavHistoryState<unknown>>
		const entries = (list: unknown): NavEntry<P>[] =>
			Array.isArray(list)
				? list.filter(
						(entry): entry is NavEntry<P> =>
							!!entry &&
							typeof entry === 'object' &&
							typeof (entry as NavEntry<unknown>).label === 'string' &&
							isPlace((entry as NavEntry<unknown>).place),
					)
				: []
		return { back: entries(parsed.back), forward: entries(parsed.forward) }
	} catch {
		return EMPTY
	}
}

function save<P>(key: string, state: NavHistoryState<P>) {
	try {
		if (!state.back.length && !state.forward.length) window.sessionStorage.removeItem(key)
		else window.sessionStorage.setItem(key, JSON.stringify(state))
	} catch {
		// a full or blocked storage only loses the history after a reload
	}
}

export type NavHistory<P> = {
	/** The place "Back to" goes to, if any (one the reader isn't already at) */
	back: NavEntry<P> | null
	/** The place "Forward to" goes to, after going back */
	forward: NavEntry<P> | null
	/** The reader is about to jump away from `from` */
	record: (from: NavEntry<P>) => void
	/** Take the place to go back to, leaving `current` to go forward to (`null`: none) */
	goBack: (current: NavEntry<P> | null) => NavEntry<P> | null
	/** Take the place to go forward to, leaving `current` to go back to (`null`: none) */
	goForward: (current: NavEntry<P> | null) => NavEntry<P> | null
	/**
	 * The reader went back to `place` another way (a link's Return, the browser's Back): when
	 * it's the place to go back to, it's taken as `goBack` would, without moving the reader
	 */
	wentBack: (place: P, current: NavEntry<P> | null) => void
}

type Options<P> = {
	/** `navHistoryKey(...)`: the book's places in this reader */
	storageKey: string
	/** Two places are the same spot (no step between them) */
	same: (a: P, b: P) => boolean
	/** A stored value is a place of this reader (what's read back from storage is checked) */
	isPlace: (value: unknown) => value is P
	/** Where the reader is, so a place it's already at isn't offered */
	current?: P | null
}

/** A book's places to go back and forward to in one reader (see the module's comment) */
export function useNavHistory<P>({
	storageKey,
	same,
	isPlace,
	current = null,
}: Options<P>): NavHistory<P> {
	const [state, setState] = useState<NavHistoryState<P>>(() => load(storageKey, isPlace))
	const [loadedKey, setLoadedKey] = useState(storageKey)
	if (loadedKey !== storageKey) {
		// another book in the same reader
		setLoadedKey(storageKey)
		setState(load(storageKey, isPlace))
	}

	// saved as it changes, so a reload in the middle of reading keeps the places
	const update = useCallback(
		(change: (state: NavHistoryState<P>) => NavHistoryState<P>) =>
			setState((previous) => {
				const next = change(previous)
				if (next !== previous) save(storageKey, next)
				return next
			}),
		[storageKey],
	)

	const record = useCallback(
		(from: NavEntry<P>) => update((previous) => recordJump(previous, from, same)),
		[update, same],
	)

	const goBack = useCallback(
		(here: NavEntry<P> | null) => {
			const taken = takeBack(state, here, same)
			if (!taken) return null
			save(storageKey, taken.state)
			setState(taken.state)
			return taken.entry
		},
		[state, same, storageKey],
	)

	const goForward = useCallback(
		(here: NavEntry<P> | null) => {
			const taken = takeForward(state, here, same)
			if (!taken) return null
			save(storageKey, taken.state)
			setState(taken.state)
			return taken.entry
		},
		[state, same, storageKey],
	)

	const wentBack = useCallback(
		(place: P, here: NavEntry<P> | null) =>
			update((previous) => {
				const taken = takeBack(previous, here, same)
				return taken && same(taken.entry.place, place) ? taken.state : previous
			}),
		[update, same],
	)

	const back = useMemo(() => nextUseful(state.back, current, same), [state.back, current, same])
	const forward = useMemo(
		() => nextUseful(state.forward, current, same),
		[state.forward, current, same],
	)

	return { back, forward, record, goBack, goForward, wentBack }
}

// Readers of pages (PDF, comics) ------------------------------------------------------------

/** A page of a PDF or a comic, from 1 */
export type PagePlace = { page: number }

const samePage = (a: PagePlace, b: PagePlace) => a.page === b.page
const isPagePlace = (value: unknown): value is PagePlace =>
	!!value && typeof value === 'object' && Number.isInteger((value as PagePlace).page)

type PageOptions = {
	reader: 'pdf' | 'comic'
	bookId: string
	/** The page on screen, from 1 */
	currentPage: number
	/** How a page reads in "Back to …", e.g. `Page 23 · Chapter 2` */
	labelOf: (page: number) => string
	/** A page's chapter, when the book has an outline: a jump into another one counts */
	chapterOf?: (page: number) => string | undefined
	/** Moves the reader to a page (the reader's own "go to") */
	goTo: (page: number) => void
}

export type PageNavHistory = {
	back: NavEntry<PagePlace> | null
	forward: NavEntry<PagePlace> | null
	/**
	 * Goes to a page from a jump (go to, the contents, a bookmark, a search hit, Home/End),
	 * with `move` when the jump goes somewhere in the page (a search hit's spot)
	 */
	jumpTo: (page: number, move?: () => void) => void
	goBack: () => void
	goForward: () => void
}

/**
 * The back and forward of a reader of pages: a jump of more than a few pages (or into
 * another chapter) remembers the page it left
 */
export function usePageNavHistory({
	reader,
	bookId,
	currentPage,
	labelOf,
	chapterOf,
	goTo,
}: PageOptions): PageNavHistory {
	const current = useMemo(() => ({ page: currentPage }), [currentPage])
	const history = useNavHistory<PagePlace>({
		storageKey: navHistoryKey(reader, bookId),
		same: samePage,
		isPlace: isPagePlace,
		current,
	})
	const { record, goBack: takeBack, goForward: takeForward } = history
	const here = useCallback(
		(): NavEntry<PagePlace> => ({ place: { page: currentPage }, label: labelOf(currentPage) }),
		[currentPage, labelOf],
	)

	const jumpTo = useCallback(
		(page: number, move?: () => void) => {
			const far = isFarJump(
				{ page: currentPage, chapter: chapterOf?.(currentPage) },
				{ page, chapter: chapterOf?.(page) },
			)
			if (far) record(here())
			if (move) move()
			else goTo(page)
		},
		[currentPage, chapterOf, record, here, goTo],
	)
	const goBack = useCallback(() => {
		const entry = takeBack(here())
		if (entry) goTo(entry.place.page)
	}, [takeBack, here, goTo])
	const goForward = useCallback(() => {
		const entry = takeForward(here())
		if (entry) goTo(entry.place.page)
	}, [takeForward, here, goTo])

	return { back: history.back, forward: history.forward, jumpTo, goBack, goForward }
}
