import { ReadingDirection, ReadingImageScaleFit, ReadingMode } from '@stump/graphql'
import { create } from 'zustand'
import { createJSONStorage, devtools, persist, StateStorage } from 'zustand/middleware'

export type BookImageScaling = {
	scaleToFit: ReadingImageScaleFit
}

export type DoublePageBehavior = 'auto' | 'always' | 'off'
export const isDoublePageBehavior = (value: string): value is DoublePageBehavior =>
	['auto', 'always', 'off'].includes(value)

/**
 * The number of columns to render for a paginated EPUB. `'auto'` lets the reader pick
 * 1 or 2 columns based on the available stage width; `1`/`2` pin an explicit column count.
 * Has no effect in continuous scroll mode.
 */
export type EpubColumnCount = 'auto' | 1 | 2

export const EPUB_READER_THEMES = ['auto', 'light', 'sepia', 'dark', 'black'] as const
export type EpubReaderTheme = (typeof EPUB_READER_THEMES)[number]
export type EpubTextAlign = 'auto' | 'justify' | 'start'

/**
 * The preferences for a book, which represents an override of a user's default preferences for a
 * specific book
 */
export type BookPreferences = {
	animatedReader?: boolean
	/**
	 * A brightness value for the book, which will apply a filter to dim / brighten the page.
	 * This must be a value between 0 and 1.
	 */
	brightness: number
	/**
	 * The current reader mode
	 */
	readingMode: ReadingMode
	/**
	 * The reading direction of the book. There are two possible values: `ltr` and `rtl`.
	 */
	readingDirection: ReadingDirection
	/**
	 * The behavior for double-page spreads
	 */
	doublePageBehavior?: DoublePageBehavior
	/**
	 * The font size to use for the book. This will have no effect if the book is image-based
	 */
	fontSize?: number
	/**
	 * The font family to use for the book. This will have no effect if the book is image-based
	 */
	fontFamily?: string
	/**
	 * The line height to use for the book. This will have no effect if the book is image-based
	 */
	lineHeight?: number
	/**
	 * The EPUB reader colour theme (`auto` follows the app's light/dark theme). Read from the
	 * global reader settings. This will have no effect if the book is image-based
	 */
	theme?: EpubReaderTheme
	/**
	 * Text alignment for EPUBs: `auto` keeps the book's own, `justify` or `start` force it
	 */
	textAlign?: EpubTextAlign
	/**
	 * The image scaling preferences for the book. This will have no effect if the book is not an image-based book
	 */
	imageScaling: BookImageScaling
	/**
	 * The behavior for tapping the sides of the screen to navigate
	 */
	tapSidesToNavigate: boolean
	/**
	 * Whether or not to track elapsed time for the book
	 */
	trackElapsedTime: boolean
	/**
	 * Whether or not the page 2 should be displayed separately. This will have no effect if the book is not image-based
	 */
	secondPageSeparate: boolean
	/**
	 * Whether the Ctrl key (or also Cmd key on Mac) is required to use panning and zooming.
	 */
	panzoomWithoutCtrl?: boolean
	/**
	 * The number of columns to render for a paginated EPUB. This will have no effect if the
	 * book is not an EPUB, or if the reading mode is continuous scroll.
	 */
	columnCount?: EpubColumnCount
	/**
	 * The margin (in rem, roughly) applied around the page content for an EPUB. This will have
	 * no effect if the book is not an EPUB.
	 */
	pageMargins?: number
}

/**
 * A type alias for a book ID
 */
type BookID = string

/**
 * The store for the reader itself, less specific to a single book and more about the reader
 */
export type ReaderSettings = {
	/**
	 * Whether the toolbar should be shown
	 */
	showToolBar: boolean
	/**
	 * Whether the readers show the small page number in a corner (unset: shown)
	 */
	showPageIndicator?: boolean
	/**
	 * The preferences for preloading pages
	 */
	preload: {
		/**
		 * The number of pages to preload ahead of the current page. This will have no effect if the book
		 * is not an image-based book
		 */
		ahead: number
		/**
		 * The number of pages to preload behind the current page. This will have no effect if the book
		 * is not an image-based book
		 */
		behind: number
	}
} & BookPreferences

type ElapsedSeconds = number

export type ReaderStore = {
	/**
	 * The preferences for the reader
	 */
	settings: ReaderSettings
	/**
	 * A setter for the layout preferences
	 */
	setSettings: (settings: Partial<ReaderSettings>) => void
	/**
	 * The preferences for each book, if they have been overridden from the default preferences
	 */
	bookPreferences: Record<BookID, BookPreferences>

	bookTimers: Record<BookID, ElapsedSeconds>
	setBookTimer: (id: string, timer: ElapsedSeconds) => void

	/**
	 * A function to clear the store
	 */
	clearStore: () => void
	/**
	 * A setter for a *specific* book's preferences
	 */
	setBookPreferences: (id: BookID, preferences: BookPreferences) => void
}

/**
 * The default EPUB font size, in CSS px. Was 13 (persisted store version 5 and older),
 * which read too small
 */
export const DEFAULT_EPUB_FONT_SIZE = 22
const PREVIOUS_DEFAULT_EPUB_FONT_SIZE = 13

export const DEFAULT_BOOK_PREFERENCES = {
	animatedReader: false,
	fontSize: DEFAULT_EPUB_FONT_SIZE,
	lineHeight: 1.5,
	brightness: 1,
	readingMode: ReadingMode.Paged,
	readingDirection: ReadingDirection.Ltr,
	imageScaling: {
		scaleToFit: ReadingImageScaleFit.Height,
	},
	doublePageBehavior: 'off',
	secondPageSeparate: false,
	panWithoutCtrl: false,
	trackElapsedTime: true,
	tapSidesToNavigate: true,
	columnCount: 'auto',
	pageMargins: 1.0,
} as const

export const createReaderStore = (storage?: StateStorage) =>
	create<ReaderStore>()(
		devtools(
			persist(
				(set, get) =>
					({
						bookPreferences: {},
						setBookPreferences: (id, preferences) => {
							const existingPreferences = get().bookPreferences[id]
							set({
								bookPreferences: {
									...get().bookPreferences,
									[id]: { ...existingPreferences, ...preferences },
								},
							})
						},

						bookTimers: {},
						setBookTimer: (id, elapsedSeconds) =>
							set({ bookTimers: { ...get().bookTimers, [id]: elapsedSeconds } }),

						clearStore: () =>
							set({
								bookPreferences: {},
							}),
						setSettings: (settings) => set({ settings: { ...get().settings, ...settings } }),
						settings: {
							preload: {
								ahead: 5,
								behind: 3,
							},
							showToolBar: false,
							...DEFAULT_BOOK_PREFERENCES,
						},
					}) as ReaderStore,
				{
					name: 'stump-new-reader-store',
					storage: storage ? createJSONStorage(() => storage) : undefined,
					version: 6,
					migrate: migrateReaderStore,
				},
			),
		),
	)

/**
 * Upgrades a persisted reader store to the current version. Version 6 raised the default
 * EPUB font size from 13 to 22px: the old default, kept in the settings and copied into
 * every book whose preferences were changed, becomes the new one (a size someone picked
 * stays). Stores older than version 5 are dropped, as before migrations existed.
 */
export function migrateReaderStore(persisted: unknown, version: number): ReaderStore {
	const state = persisted as Partial<ReaderStore> | undefined
	if (!state || version < 5) {
		// Merged over the initial state, so this keeps the defaults
		return undefined as unknown as ReaderStore
	}

	const upgradeFontSize = <T extends { fontSize?: number }>(preferences: T): T =>
		preferences.fontSize === PREVIOUS_DEFAULT_EPUB_FONT_SIZE
			? { ...preferences, fontSize: DEFAULT_EPUB_FONT_SIZE }
			: preferences

	return {
		...state,
		...(state.settings ? { settings: upgradeFontSize(state.settings) } : {}),
		...(state.bookPreferences
			? {
					bookPreferences: Object.fromEntries(
						Object.entries(state.bookPreferences).map(([id, preferences]) => [
							id,
							upgradeFontSize(preferences),
						]),
					),
				}
			: {}),
	} as ReaderStore
}
