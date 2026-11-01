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
	visible: boolean
	fullscreen: boolean
	setFullscreen: (fullscreen: boolean) => void
	/** Whether the keyboard shortcuts help is open (`?`) */
	shortcutsHelpOpen: boolean
	setShortcutsHelpOpen: (open: boolean) => void
	setVisible: (visible: boolean) => void
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
		setShortcutsHelpOpen: noop,
		setVisible: noop,
		shortcutsHelpOpen: false,
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
