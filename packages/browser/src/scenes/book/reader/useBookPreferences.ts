import {
	BOOK_RESET_KEYS,
	BookPreferences,
	type FormatKind,
	formatKindOf,
	ReaderSettings,
	ReaderStore,
} from '@stump/client'
import { PickSelect } from '@stump/components'
import { BookReaderSceneQuery, ReadingDirection, ReadingImageScaleFit } from '@stump/graphql'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'

import { ImageReaderBookRef } from '@/components/readers/imageBased/context'
import { useReaderStore } from '@/stores'

type Params = {
	book: ImageReaderBookRef
}

type Return = Pick<ReaderStore, 'settings' | 'setSettings'> & {
	bookPreferences: BookPreferences
	setBookPreferences: (preferences: Partial<BookPreferences>) => void
	/** The reader the book opens in, whose defaults it follows: an EPUB's or the image reader's */
	kind: FormatKind
	/** What the book would use without its own settings */
	defaults: BookPreferences
	/** Whether the book has settings of its own (that "Reset this book" would undo) */
	differsFromDefaults: boolean
	/** "Make default": the book's settings become the defaults of every book of its kind */
	makeDefault: () => void
	/** "Reset this book": its settings go back to the defaults */
	resetToDefaults: () => void
}

export function useBookPreferences({ book }: Params): Return {
	const kind = formatKindOf(book)
	const {
		bookPreferences: allPreferences,
		setBookPreferences: storedSetBookPreferences,
		settings,
		setSettings,
		formatDefaults,
		makeFormatDefault,
		resetBookPreferences,
	} = useReaderStore(
		useShallow((state) => ({
			bookPreferences: state.bookPreferences,
			setBookPreferences: state.setBookPreferences,
			setSettings: state.setSettings,
			settings: state.settings,
			formatDefaults: state.formatDefaults?.[kind],
			makeFormatDefault: state.makeFormatDefault,
			resetBookPreferences: state.resetBookPreferences,
		})),
	)

	const storedBookPreferences = useMemo(() => allPreferences[book.id], [allPreferences, book.id])

	/**
	 * The library configuration, used for picking default reader settings. This realistically
	 * should never be null once the query resolves
	 */
	const libraryConfig = useMemo(() => book.libraryConfig, [book])
	const isManga = useMemo(() => isMangaBook(book), [book])
	// recomputed when a phone turns: its default fit differs in portrait and landscape
	const screen = useScreenShape()
	const libraryDefaults = useMemo(
		() => defaultsFromLibraryConfig(libraryConfig, isManga, screen),
		[libraryConfig, isManga, screen],
	)

	const defaults = useMemo(
		() => buildPreferences({}, settings, libraryDefaults, formatDefaults),
		[libraryDefaults, settings, formatDefaults],
	)
	const bookPreferences = useMemo(
		() => buildPreferences(storedBookPreferences ?? {}, settings, libraryDefaults, formatDefaults),
		[storedBookPreferences, libraryDefaults, settings, formatDefaults],
	)
	const differsFromDefaults = useMemo(
		() => hasOwnSettings(kind, storedBookPreferences, defaults),
		[kind, storedBookPreferences, defaults],
	)

	// Only what was changed is kept for the book: the rest stays a default, which may depend on
	// the screen (a phone's fit-width, crop, ...), so it isn't frozen into the book on one device
	const setBookPreferences = useCallback(
		(preferences: Partial<typeof bookPreferences>) => {
			storedSetBookPreferences(book.id, preferences as BookPreferences)
		},
		[book.id, storedSetBookPreferences],
	)

	const makeDefault = useCallback(
		() => makeFormatDefault(kind, book.id),
		[makeFormatDefault, kind, book.id],
	)
	const resetToDefaults = useCallback(
		() => resetBookPreferences(kind, book.id),
		[resetBookPreferences, kind, book.id],
	)

	return {
		bookPreferences,
		setBookPreferences,
		setSettings,
		settings,
		kind,
		defaults,
		differsFromDefaults,
		makeDefault,
		resetToDefaults,
	}
}

/**
 * Whether a book has settings of its own: one of its kind's settings stored for it, with a
 * value its defaults don't have (a book whose size was turned up and back down doesn't).
 * The EPUB's columns `auto` (the old default) shows one page, like `1`.
 */
export function hasOwnSettings(
	kind: FormatKind,
	stored: Partial<BookPreferences> | undefined,
	defaults: Partial<BookPreferences>,
): boolean {
	if (!stored) return false
	const normalize = (key: string, value: unknown) =>
		key === 'columnCount' && (value === 'auto' || value === undefined) ? 1 : value
	return BOOK_RESET_KEYS[kind].some(
		(key) =>
			key in stored &&
			JSON.stringify(normalize(key, stored[key])) !== JSON.stringify(normalize(key, defaults[key])),
	)
}

/**
 * Whether a book is manga, from its genres: ComicInfo's `Genre` (and the library's tags) name
 * it `Manga` or `…: Manga`, e.g. `Manga & Comics: Manga`. Stump doesn't keep ComicInfo's
 * `Manga=YesAndRightToLeft` itself, so this stands in for it.
 */
export function isMangaBook(book: { metadata?: { genres?: string[] | null } | null }): boolean {
	return (book.metadata?.genres ?? []).some((genre) => /(^|:\s*)manga$/i.test(genre.trim()))
}

/**
 * A library's default fit is chosen for a wide screen, where fit-to-height (or fit-page) shows
 * a whole page (or a two-page spread). On a phone it would shrink the page:
 * - in portrait the page fits the screen (Fit Page, Mihon's default 'fit screen'): a manga
 *   page then fills the width, as fit-width would, and a wide (two-page) image gets the wide
 *   page handling (zoomed in and panned, or split), which belongs to Fit Page;
 * - in landscape a page fitted to a 390px height is unreadable: the page fits the screen's
 *   width instead and is read down in steps (Mihon's, Kavita's fit-width).
 * A fit the reader chose for the book itself is never changed.
 */
export function fitForScreen(
	fit: ReadingImageScaleFit,
	screen: { width: number; height: number } | undefined = typeof window === 'undefined'
		? undefined
		: { width: window.innerWidth, height: window.innerHeight },
): ReadingImageScaleFit {
	const wholePage = fit === ReadingImageScaleFit.Height || fit === ReadingImageScaleFit.Auto
	if (!screen || !wholePage || !isPhoneScreen(screen)) return fit
	return screen.height > screen.width ? ReadingImageScaleFit.Auto : ReadingImageScaleFit.Width
}

/** The screen as far as the default fit cares: a phone or not, upright or on its side */
type ScreenShape = { width: number; height: number }

/**
 * The screen's shape, kept current through rotations (a phone's default fit depends on it).
 * Only the orientation and the phone/larger-screen line matter, so a phone's address bar
 * showing or hiding (a resize too) doesn't re-render the readers.
 */
function useScreenShape(): ScreenShape | undefined {
	const [key, setKey] = useState(screenShapeKey)
	useEffect(() => {
		const update = () => setKey(screenShapeKey())
		window.addEventListener('resize', update)
		return () => window.removeEventListener('resize', update)
	}, [])
	// the size at the time the shape last changed
	return useMemo(
		() =>
			key && typeof window !== 'undefined'
				? { width: window.innerWidth, height: window.innerHeight }
				: undefined,
		[key],
	)
}

/** What the default fit depends on: a phone or a larger screen, in portrait or landscape */
function screenShapeKey(): string {
	if (typeof window === 'undefined') return ''
	const screen = { width: window.innerWidth, height: window.innerHeight }
	const kind = isPhoneScreen(screen) ? 'phone' : 'wide'
	return `${kind}-${screen.height > screen.width ? 'portrait' : 'landscape'}`
}

/** Whether this screen is a phone's (see {@link isPhoneScreen}) */
export function isPhoneNow(): boolean {
	return typeof window !== 'undefined'
		? isPhoneScreen({ width: window.innerWidth, height: window.innerHeight })
		: false
}

/** A phone (or a small tablet in portrait): narrower than two pages need, or short in landscape */
export function isPhoneScreen(screen: { width: number; height: number }): boolean {
	return screen.width < 900 || Math.min(screen.width, screen.height) < 500
}

const defaultsFromLibraryConfig = (
	libraryConfig?: PickSelect<NonNullable<BookReaderSceneQuery['mediaById']>, 'libraryConfig'>,
	isManga = false,
	screen?: ScreenShape,
): Partial<BookPreferences> => ({
	brightness: 1,
	imageScaling: libraryConfig?.defaultReadingImageScaleFit
		? {
				scaleToFit: fitForScreen(
					libraryConfig.defaultReadingImageScaleFit as ReadingImageScaleFit,
					screen,
				),
			}
		: undefined,
	// manga reads right to left unless the book's own preference says otherwise
	readingDirection: isManga ? ReadingDirection.Rtl : libraryConfig?.defaultReadingDir,
	// phones trim the pages' blank margins: every pixel of a small screen counts
	cropBorders: isPhoneNow(),
	// and shows two pages side by side when there's room: a wide screen in landscape
	...(isManga ? { doublePageBehavior: 'auto' as const } : {}),
	readingMode: libraryConfig?.defaultReadingMode,
})

const settingsAsBookPreferences = (settings: ReaderSettings): BookPreferences => ({
	animatedReader: settings.animatedReader,
	brightness: settings.brightness,
	imageScaling: settings.imageScaling,
	readingDirection: settings.readingDirection,
	readingMode: settings.readingMode,
	tapSidesToNavigate: settings.tapSidesToNavigate,
	fontSize: settings.fontSize,
	lineHeight: settings.lineHeight,
	trackElapsedTime: settings.trackElapsedTime,
	doublePageBehavior: settings.doublePageBehavior,
	fontFamily: settings.fontFamily,
	secondPageSeparate: settings.secondPageSeparate,
	panzoomWithoutCtrl: settings.panzoomWithoutCtrl,
	columnCount: settings.columnCount,
	pageMargins: settings.pageMargins,
})

/**
 * A book's settings, from the least to the most specific: the reader's settings, the
 * library's defaults (its fit, reading mode and direction; manga read right to left), the
 * defaults picked for every book of its kind ("Make default"), then the book's own.
 */
const buildPreferences = (
	preferences: Partial<BookPreferences>,
	settings: ReaderSettings,
	libraryDefaults: Partial<BookPreferences>,
	formatDefaults: Partial<BookPreferences> | undefined,
): BookPreferences => ({
	...settingsAsBookPreferences(settings),
	...libraryDefaults,
	...formatDefaults,
	...preferences,
})
