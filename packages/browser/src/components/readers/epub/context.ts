import { Bookmark } from '@stump/graphql'
import type { EpubSearchResponse } from '@stump/sdk'
import { createContext, useContext } from 'react'

import { ImageReaderBookRef } from '@/components/readers/imageBased/context'

import { noop } from '../../../utils/misc'
import type { ReaderLocator } from '../annotations/types'
import type { EpubAnnotation } from './annotations/types'

export type { ReaderLocator } from '../annotations/types'
export type { EpubSearchResponse, EpubSearchResult } from '@stump/sdk'

export type EpubReaderChapterMeta = {
	name?: string
	/** The chapter's place in the contents, from its part down to its own title */
	path?: string[]
	/** The page (screen) of the chapter on screen, from 1, and the chapter's pages */
	pageInChapter?: number
	pagesInChapter?: number
	/** The pages after this one in the chapter */
	pagesLeftInChapter?: number
	position?: number
	sectionSpineIndex?: number
	totalProgression?: number
	locatorPosition?: number
	totalPositions?: number
	currentLocator?: ReaderLocator | null
}

export interface EpubContent {
	label: string
	content: string
	children: EpubContent[]
	play_order: number
}

export type EpubReaderBookMeta = {
	chapter: EpubReaderChapterMeta
	/** Where the top-level chapters start in the book, from 0 to 1 (the reading bar's ticks) */
	chapterStarts?: number[]
	toc: EpubContent[]
	bookmarks: Record<string, Bookmark>
	annotations: EpubAnnotation[]
}

export type EpubReaderMeta = {
	bookEntity: ImageReaderBookRef
	bookMeta: EpubReaderBookMeta | null
	progress: number | null
}

export type EpubReaderControls = {
	/** Whether the top bar is shown */
	visible: boolean
	fullscreen: boolean
	setFullscreen: (fullscreen: boolean) => void
	/** Opens the reader settings, at the keyboard shortcuts with `'shortcuts'` (`?`) */
	openSettings: (section?: 'shortcuts' | null) => void
	/** Opens the search (Ctrl/Cmd+F, `/`) */
	openSearch: () => void
	/** Opens the contents with its "go to location" field focused (`g`, the top bar's position) */
	openGoTo: () => void
	/** Goes to one of the book's locations (Readium's positions, about a page each), from 1 */
	goToPosition?: (position: number) => void
	setVisible: (visible: boolean) => void
	/** A tap in the middle of the page: show or hide the top bar */
	toggleVisible: () => void
	onMouseEnterControls: () => void
	onMouseLeaveControls: () => void
	onLinkClick: (href: string) => void
	onPaginateForward: () => void
	onPaginateBackward: () => void
	jumpToSection: (section: number) => void
	canGoForward?: boolean
	canGoBackward?: boolean
	onGoToLocator: (locator: ReaderLocator) => Promise<boolean>
	getLocatorPreviewText: (locator: ReaderLocator) => string | null
	searchBook?: (
		query: string,
		opts?: { cursor?: string; signal?: AbortSignal },
	) => Promise<EpubSearchResponse>
}

export type EpubReaderContextProps = {
	readerMeta: EpubReaderMeta
	controls: EpubReaderControls
}

export const EpubReaderContext = createContext<EpubReaderContextProps>({
	controls: {
		fullscreen: false,
		getLocatorPreviewText: () => null,
		onGoToLocator: async () => false,
		onLinkClick: noop,
		onMouseEnterControls: noop,
		onMouseLeaveControls: noop,
		onPaginateBackward: noop,
		onPaginateForward: noop,
		jumpToSection: noop,
		setFullscreen: noop,
		openSettings: noop,
		openSearch: noop,
		openGoTo: noop,
		setVisible: noop,
		toggleVisible: noop,
		visible: false,
	},
	readerMeta: {
		bookEntity: {} as ImageReaderBookRef,
		bookMeta: null,
		progress: null,
	},
})
export const useEpubReaderContext = () => useContext<EpubReaderContextProps>(EpubReaderContext)
export const useEpubReaderControls = () => useEpubReaderContext().controls
