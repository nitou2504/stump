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

type PdfReaderStore = {
	books: Record<string, PdfBookPreferences>
	setBookPreferences: (bookId: string, preferences: Partial<PdfBookPreferences>) => void
}

/** Per-book PDF reader preferences, kept in this browser like the other reader settings */
export const usePdfReaderStore = create<PdfReaderStore>()(
	persist(
		(set) => ({
			books: {},
			setBookPreferences: (bookId, preferences) =>
				set((state) => ({
					books: { ...state.books, [bookId]: { ...state.books[bookId], ...preferences } },
				})),
		}),
		{ name: 'stump-pdf-reader-store', version: 1 },
	),
)

export function usePdfBookPreferences(bookId: string): PdfBookPreferences {
	return usePdfReaderStore((state) => state.books[bookId] ?? EMPTY)
}

const EMPTY: PdfBookPreferences = {}
