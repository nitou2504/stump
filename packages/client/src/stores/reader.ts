import { ReadingDirection, ReadingImageScaleFit, ReadingMode } from '@stump/graphql'
import { create } from 'zustand'
import { createJSONStorage, devtools, persist, StateStorage } from 'zustand/middleware'

export type BookImageScaling = {
	scaleToFit: ReadingImageScaleFit
}

export type DoublePageBehavior = 'auto' | 'always' | 'off'

/**
 * How the comic reader shows a wide page (a two-page spread on one image) when it shows one
 * page at a time on a narrow screen: `zoom` so half of it fills the width and step across it,
 * `split` into its two halves, or `fit` it whole (small)
 */
export type WidePagesBehavior = 'zoom' | 'split' | 'fit'

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
	 * In two-page spreads, whether the first page (the cover) stays alone; unset: it does.
	 * This will have no effect if the book is not image-based
	 */
	coverAlone?: boolean
	/**
	 * Whether the Ctrl key (or also Cmd key on Mac) is required to use panning and zooming.
	 */
	panzoomWithoutCtrl?: boolean
	/**
	 * Trim the blank margins of a comic's pages (one page at a time; spreads are never
	 * cropped). Unset: on for phones. This will have no effect if the book is not image-based
	 */
	cropBorders?: boolean
	/**
	 * How a wide page shows on a narrow screen, one page at a time. Unset: `zoom`. This will
	 * have no effect if the book is not image-based
	 */
	widePages?: WidePagesBehavior
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

/** The three readers, which keep their own reading progress settings */
export type ReaderKind = 'epub' | 'pdf' | 'comic'

/** How a reader shows the reading progress: nothing, the small number, or the reading bar */
export type ReadingProgressDisplay = 'off' | 'number' | 'bar'

/**
 * What the reading bar can show, in its fixed order: the chapter on the left; the page
 * (in the chapter for an EPUB, `x of y` for a PDF or comic), the pages left in the chapter,
 * the book's percentage and the clock on the right
 */
export const READING_BAR_ITEMS = ['chapter', 'page', 'pagesLeft', 'percent', 'clock'] as const
export type ReadingBarItem = (typeof READING_BAR_ITEMS)[number]

/**
 * A reader's reading progress: off, the small number or the reading bar, and what the bar
 * shows. Where the bar goes is the reader's: an EPUB keeps it at the foot of the page (its
 * pages leave room for it), a PDF or comic shows it with the top bar, over the pages
 */
export type ReadingProgressPreferences = {
	display?: ReadingProgressDisplay
	items?: ReadingBarItem[]
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
	 * Whether the readers show the small page number in a corner (unset: shown). Replaced by
	 * `readingProgress` (store version 7), which this still seeds for older stores
	 */
	showPageIndicator?: boolean
	/**
	 * Each reader's reading progress: off, the small number in a corner or the reading bar,
	 * and what the bar shows. Unset values take the reader's defaults
	 */
	readingProgress?: Partial<Record<ReaderKind, ReadingProgressPreferences>>
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
					version: 9,
					migrate: migrateReaderStore,
				},
			),
		),
	)

/**
 * Upgrades a persisted reader store to the current version. Version 6 raised the default
 * EPUB font size from 13 to 22px: the old default, kept in the settings and copied into
 * every book whose preferences were changed, becomes the new one (a size someone picked
 * stays). Version 7 replaced the page number switch with each reader's reading progress:
 * a page number that was turned off stays off in every reader. Version 8 dropped two
 * settings: the reading bar's 'show' (always or with the controls: each reader now has its
 * own place for the bar) and the comic's page background (the reader theme colours the
 * pages' surround); the reading progress kept is the display (off, number or bar) and the
 * bar's items. Version 9 thawed books' frozen defaults (see `withoutFrozenDefaults`). Stores
 * older than version 5 are dropped, as before migrations existed.
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

	const upgradeProgress = (settings: ReaderSettings): ReaderSettings =>
		version < 7 && settings.showPageIndicator === false && !settings.readingProgress
			? {
					...settings,
					readingProgress: {
						epub: { display: 'off' },
						pdf: { display: 'off' },
						comic: { display: 'off' },
					},
				}
			: settings

	// Version 8: the comic's page background and the reading bar's 'show' are gone
	const withoutBackground = <T extends object>(preferences: T): T => {
		if (!('pageBackground' in preferences)) return preferences
		// eslint-disable-next-line @typescript-eslint/no-unused-vars
		const { pageBackground, ...rest } = preferences as T & { pageBackground?: unknown }
		return rest as T
	}
	const withoutBarShow = (settings: ReaderSettings): ReaderSettings => {
		if (!settings.readingProgress) return settings
		const readingProgress = Object.fromEntries(
			Object.entries(settings.readingProgress).map(([kind, progress]) => {
				if (!progress || !('show' in progress)) return [kind, progress]
				// eslint-disable-next-line @typescript-eslint/no-unused-vars
				const { show, ...rest } = progress as ReadingProgressPreferences & { show?: unknown }
				return [kind, rest]
			}),
		) as ReaderSettings['readingProgress']
		return { ...settings, readingProgress }
	}

	// Version 9: the defaults older builds froze into a book's preferences
	const thaw = <T extends object>(preferences: T): T =>
		version < 9 ? withoutFrozenDefaults(preferences) : preferences

	return {
		...state,
		...(state.settings
			? {
					settings: withoutBarShow(
						withoutBackground(upgradeProgress(upgradeFontSize(state.settings))),
					),
				}
			: {}),
		...(state.bookPreferences
			? {
					bookPreferences: Object.fromEntries(
						Object.entries(state.bookPreferences).map(([id, preferences]) => [
							id,
							thaw(withoutBackground(upgradeFontSize(preferences))),
						]),
					),
				}
			: {}),
	} as ReaderStore
}

/**
 * Keys only a whole copy of the reader preferences carries: until version 9, changing any
 * setting of a book (the reader's own `setBookPreferences`, the old reader-defaults screen)
 * stored every preference then in effect for it, not just the change. The readers now store
 * only what was changed, and never write these.
 */
const SNAPSHOT_KEYS = [
	'animatedReader',
	'trackElapsedTime',
	'secondPageSeparate',
	'panzoomWithoutCtrl',
	'tapSidesToNavigate',
] as const

/** The values those copies froze in when they were the defaults */
const FROZEN_DEFAULTS: Record<string, unknown> = {
	...DEFAULT_BOOK_PREFERENCES,
	panzoomWithoutCtrl: false,
}

/**
 * A book's preferences without the defaults a whole copy froze into it. Such a copy kept, for
 * example, left to right as the reading direction of a manga opened before manga were read
 * right to left by default, and fit-to-height from before phones fitted pages to the width:
 * those then won over every later default. A value equal to the default is dropped (a choice
 * equal to the default is the default anyway, except where the book's own default differs,
 * which is what got frozen); a value that differs from the default was a choice and stays.
 * Preferences that aren't such a copy are left alone.
 */
export function withoutFrozenDefaults<T extends object>(preferences: T): T {
	if (!SNAPSHOT_KEYS.some((key) => key in preferences)) return preferences
	return Object.fromEntries(
		Object.entries(preferences).filter(
			([key, value]) =>
				!(key in FROZEN_DEFAULTS) || JSON.stringify(value) !== JSON.stringify(FROZEN_DEFAULTS[key]),
		),
	) as T
}
