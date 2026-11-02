import { BookPreferences, ReaderSettings, ReaderStore } from '@stump/client'
import { PickSelect } from '@stump/components'
import { BookReaderSceneQuery, ReadingDirection, ReadingImageScaleFit } from '@stump/graphql'
import { useCallback, useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'

import { ImageReaderBookRef } from '@/components/readers/imageBased/context'
import { useReaderStore } from '@/stores'

type Params = {
	book: ImageReaderBookRef
}

type Return = Omit<
	ReaderStore,
	'bookPreferences' | 'setBookPreferences' | 'clearStore' | 'bookTimers' | 'setBookTimer'
> & {
	bookPreferences: BookPreferences
	setBookPreferences: (preferences: Partial<BookPreferences>) => void
}

export function useBookPreferences({ book }: Params): Return {
	const {
		bookPreferences: allPreferences,
		setBookPreferences: storedSetBookPreferences,
		settings,
		setSettings,
	} = useReaderStore(
		useShallow((state) => ({
			bookPreferences: state.bookPreferences,
			setBookPreferences: state.setBookPreferences,
			setSettings: state.setSettings,
			settings: state.settings,
		})),
	)

	const storedBookPreferences = useMemo(() => allPreferences[book.id], [allPreferences, book.id])

	/**
	 * The library configuration, used for picking default reader settings. This realistically
	 * should never be null once the query resolves
	 */
	const libraryConfig = useMemo(() => book.libraryConfig, [book])
	const isManga = useMemo(() => isMangaBook(book), [book])
	const libraryDefaults = useMemo(
		() => defaultsFromLibraryConfig(libraryConfig, isManga),
		[libraryConfig, isManga],
	)

	const bookPreferences = useMemo(
		() => buildPreferences(storedBookPreferences ?? {}, settings, libraryDefaults),
		[storedBookPreferences, libraryDefaults, settings],
	)

	// Only what was changed is kept for the book: the rest stays a default, which may depend on
	// the screen (a phone's fit-width, crop, ...), so it isn't frozen into the book on one device
	const setBookPreferences = useCallback(
		(preferences: Partial<typeof bookPreferences>) => {
			storedSetBookPreferences(book.id, preferences as BookPreferences)
		},
		[book.id, storedSetBookPreferences],
	)

	return {
		bookPreferences,
		setBookPreferences,
		setSettings,
		settings,
	}
}

/**
 * Whether a book is manga, from its genres: ComicInfo's `Genre` (and the curator's tags) name
 * it `Manga` or `…: Manga`, e.g. `Manga & Comics: Manga`. Stump doesn't keep ComicInfo's
 * `Manga=YesAndRightToLeft` itself, so this stands in for it.
 */
export function isMangaBook(book: { metadata?: { genres?: string[] | null } | null }): boolean {
	return (book.metadata?.genres ?? []).some((genre) => /(^|:\s*)manga$/i.test(genre.trim()))
}

/**
 * A library's default fit is chosen for a wide screen, where fit-to-height (or fit-page) shows
 * a whole page (or a two-page spread). On a phone, in portrait or landscape, it shrinks the page (and in
 * landscape a page fitted to a 390px height is unreadable): there the page fits the screen's
 * width instead, scrolling down a tall page, like Mihon's or Kavita's fit-width (in portrait a
 * manga page at full width nearly fits anyway). A fit the reader chose for the book itself is
 * never changed.
 */
export function fitForScreen(
	fit: ReadingImageScaleFit,
	screen: { width: number; height: number } | undefined = typeof window === 'undefined'
		? undefined
		: { width: window.innerWidth, height: window.innerHeight },
): ReadingImageScaleFit {
	const wholePage = fit === ReadingImageScaleFit.Height || fit === ReadingImageScaleFit.Auto
	if (!screen || !wholePage) return fit
	return isPhoneScreen(screen) ? ReadingImageScaleFit.Width : fit
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
): Partial<BookPreferences> => ({
	brightness: 1,
	// imageScaling: {
	// 	scaleToFit: libraryConfig?.defaultReadingImageScaleFit || ReadingImageScaleFit.Height,
	// },
	imageScaling: libraryConfig?.defaultReadingImageScaleFit
		? {
				scaleToFit: fitForScreen(libraryConfig.defaultReadingImageScaleFit as ReadingImageScaleFit),
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

const buildPreferences = (
	preferences: Partial<BookPreferences>,
	settings: ReaderSettings,
	libraryDefaults: Partial<BookPreferences>,
): BookPreferences => ({
	...settingsAsBookPreferences(settings),
	...libraryDefaults,
	...preferences,
})
