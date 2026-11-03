import { create } from 'zustand'
import { persist } from 'zustand/middleware'

import type { PageSpread } from '../shared/layout'
import type { FitMode } from './geometry'

export type PdfScrollMode = 'continuous' | 'paged'

/**
 * Which reader opens a PDF: `pdf` (pages with selectable text, annotations) or `pages`
 * (the image reader, for comic/manga-style PDFs: double spreads, right-to-left)
 */
export type PdfReaderKind = 'pdf' | 'pages'

export type PdfBookPreferences = {
	/** Unset: fit width on phones, fit page on larger screens */
	fit?: FitMode
	zoom?: number
	scrollMode?: PdfScrollMode
	/** One page or two side by side; unset: one */
	spread?: PageSpread
	/** In two-page layouts the first page stays alone (like an open book); unset: on */
	coverAlone?: boolean
	/** Dark themes invert the page picture; off keeps illustrations as they are */
	invertPagesInDark?: boolean
	reader?: PdfReaderKind
}

/**
 * The settings "Make default" carries from a PDF to every PDF: how its pages fit, turn and
 * look. Not the zoom (a moment's need) nor how the book opens (as text or as pages).
 */
export const PDF_DEFAULT_KEYS = [
	'fit',
	'scrollMode',
	'spread',
	'coverAlone',
	'invertPagesInDark',
] as const satisfies readonly (keyof PdfBookPreferences)[]

/** What "Reset this book" clears: those settings, and the zoom */
export const PDF_RESET_KEYS = [
	...PDF_DEFAULT_KEYS,
	'zoom',
] as const satisfies readonly (keyof PdfBookPreferences)[]

type PdfReaderStore = {
	books: Record<string, PdfBookPreferences>
	/** The defaults picked for every PDF ("Make default"), under a book's own settings */
	defaults: Partial<PdfBookPreferences>
	setBookPreferences: (bookId: string, preferences: Partial<PdfBookPreferences>) => void
	/** "Make default": the book's settings become every PDF's */
	makeDefault: (bookId: string) => void
	/** "Reset this book": its settings go back to the defaults */
	resetBook: (bookId: string) => void
}

/** A copy of `preferences` without `keys` */
function withoutKeys<T extends object>(preferences: T, keys: readonly PropertyKey[]): T {
	return Object.fromEntries(Object.entries(preferences).filter(([key]) => !keys.includes(key))) as T
}

/** The books' settings with one book's replaced (dropped when nothing is left) */
function withBook(
	books: Record<string, PdfBookPreferences>,
	bookId: string,
	book: PdfBookPreferences,
): Record<string, PdfBookPreferences> {
	const rest = withoutKeys(books, [bookId])
	return Object.keys(book).length ? { ...rest, [bookId]: book } : rest
}

/**
 * "Make default": the book's own fit, layout and look become every PDF's defaults and leave
 * the book; other PDFs keep their own settings
 */
export function promotePdfDefaults(
	state: Pick<PdfReaderStore, 'books' | 'defaults'>,
	bookId: string,
): Pick<PdfReaderStore, 'books' | 'defaults'> {
	const book = state.books[bookId]
	if (!book) return state
	const promoted = Object.fromEntries(
		Object.entries(book).filter(([key]) => (PDF_DEFAULT_KEYS as readonly string[]).includes(key)),
	)
	return {
		defaults: { ...state.defaults, ...promoted },
		books: withBook(state.books, bookId, withoutKeys(book, PDF_DEFAULT_KEYS)),
	}
}

/** Per-book PDF reader preferences, kept in this browser like the other reader settings */
export const usePdfReaderStore = create<PdfReaderStore>()(
	persist(
		(set) => ({
			books: {},
			defaults: {},
			setBookPreferences: (bookId, preferences) =>
				set((state) => ({
					books: { ...state.books, [bookId]: { ...state.books[bookId], ...preferences } },
				})),
			makeDefault: (bookId) => set((state) => promotePdfDefaults(state, bookId)),
			resetBook: (bookId) =>
				set((state) => {
					const book = state.books[bookId]
					if (!book) return state
					return { books: withBook(state.books, bookId, withoutKeys(book, PDF_RESET_KEYS)) }
				}),
		}),
		{
			name: 'stump-pdf-reader-store',
			// version 2: every PDF's defaults (`defaults`, empty until "Make default")
			version: 2,
			migrate: (persisted) => {
				const state = (persisted ?? {}) as Partial<PdfReaderStore>
				return { ...state, defaults: state.defaults ?? {} } as PdfReaderStore
			},
		},
	),
)

/** A book's settings: every PDF's defaults, then its own */
export function usePdfBookPreferences(bookId: string): PdfBookPreferences {
	const book = usePdfReaderStore((state) => state.books[bookId] ?? EMPTY)
	const defaults = usePdfReaderStore((state) => state.defaults ?? EMPTY)
	return mergedPreferences(defaults, book)
}

/** Every PDF's defaults under a book's own settings (cached, so the result is stable) */
const merged = new WeakMap<PdfBookPreferences, WeakMap<PdfBookPreferences, PdfBookPreferences>>()
function mergedPreferences(defaults: PdfBookPreferences, book: PdfBookPreferences) {
	if (defaults === EMPTY) return book
	let byBook = merged.get(defaults)
	if (!byBook) {
		byBook = new WeakMap()
		merged.set(defaults, byBook)
	}
	let result = byBook.get(book)
	if (!result) {
		result = { ...defaults, ...book }
		byBook.set(book, result)
	}
	return result
}

/**
 * What a PDF shows without settings of its own: every PDF's defaults, else the built-in ones
 * (fit width on a phone, fit page on a larger screen; one page scrolling; the cover alone;
 * pages inverted in dark themes)
 */
export function pdfEffectiveDefaults(
	defaults: Partial<PdfBookPreferences>,
	isPhone: boolean,
): Required<Omit<PdfBookPreferences, 'reader'>> {
	return {
		fit: defaults.fit ?? (isPhone ? 'width' : 'page'),
		zoom: 1,
		scrollMode: defaults.scrollMode ?? 'continuous',
		spread: defaults.spread ?? 'single',
		coverAlone: defaults.coverAlone ?? true,
		invertPagesInDark: defaults.invertPagesInDark ?? true,
	}
}

/** Whether a PDF has settings of its own that differ from what it would show without them */
export function pdfHasOwnSettings(
	book: PdfBookPreferences | undefined,
	defaults: Partial<PdfBookPreferences>,
	isPhone: boolean,
): boolean {
	if (!book) return false
	const effective = pdfEffectiveDefaults(defaults, isPhone)
	return PDF_RESET_KEYS.some((key) => key in book && book[key] !== effective[key])
}

const EMPTY: PdfBookPreferences = {}
