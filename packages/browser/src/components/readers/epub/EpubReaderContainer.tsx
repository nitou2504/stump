import { ReadingDirection, ReadingMode } from '@stump/graphql'
import { useCallback, useLayoutEffect, useMemo, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'
import { useBookPreferences } from '@/scenes/book/reader/useBookPreferences'

import type { ShortcutRow } from '../annotations'
import { canShowTwoPages, type ReaderLayout } from '../shared/layout'
import ReaderPageIndicator from '../shared/ReaderPageIndicator'
import ReaderSettingsDialog from '../shared/ReaderSettingsDialog'
import ReaderTopBar from '../shared/ReaderTopBar'
import ReadingBar, { useClockText } from '../shared/ReadingBar'
import { chapterLabel, useReadingProgress } from '../shared/readingProgress'
import { useDocumentFullscreen, useReaderChrome } from '../shared/useReaderChrome'
import {
	type EpubReaderChapterMeta,
	EpubReaderContext,
	EpubReaderControls,
	EpubReaderMeta,
	useEpubReaderContext,
} from './context'
import { EpubNavigationControls, SearchCommand } from './controls'
import TextSettings, { EpubReadingDirection } from './controls/TextSettings'
import { useEpubBookmark } from './controls/useEpubBookmark'
import { LocationManager } from './locations'

type Props = {
	children: React.ReactNode
	readerMeta: EpubReaderMeta
	controls: Pick<
		EpubReaderControls,
		| 'onLinkClick'
		| 'onPaginateBackward'
		| 'onPaginateForward'
		| 'jumpToSection'
		| 'goToPosition'
		| 'onGoToLocator'
		| 'getLocatorPreviewText'
		| 'searchBook'
		| 'canGoForward'
		| 'canGoBackward'
		| 'navHistory'
	>
	/**
	 * The book sets its own page layout (a fixed-layout EPUB): its pages turn one or two at a
	 * time (never scroll) and keep the publisher's text, so there are no text settings
	 */
	isFixedLayout?: boolean
}

/**
 * The frame every EPUB reader shares: the pages take the whole screen, with the shared top
 * bar over them, the page number in a corner and the shared settings, contents and search.
 */
export default function EpubReaderContainer({
	children,
	readerMeta,
	controls,
	isFixedLayout = false,
}: Props) {
	const [settingsOpen, setSettingsOpen] = useState(false)
	const [settingsSection, setSettingsSection] = useState<'shortcuts' | null>(null)
	const [searchOpen, setSearchOpen] = useState(false)
	const [contentsOpen, setContentsOpen] = useState(false)
	/** The contents were opened to go to a location: its field has the focus */
	const [goToFocus, setGoToFocus] = useState(false)
	const openContents = useCallback((focusGoTo = false) => {
		setGoToFocus(focusGoTo)
		setContentsOpen(true)
	}, [])
	const chrome = useReaderChrome({ pinned: settingsOpen || searchOpen || contentsOpen })
	const fullscreen = useDocumentFullscreen()

	const openSettings = useCallback((section: 'shortcuts' | null = null) => {
		setSettingsSection(section)
		setSettingsOpen(true)
	}, [])

	const { show, hide, toggle } = chrome
	const setVisible = useCallback(
		(visible: boolean) => (visible ? show(4000) : hide()),
		[show, hide],
	)

	return (
		<EpubReaderContext.Provider
			value={{
				controls: {
					...controls,
					fullscreen: fullscreen.isFullscreen,
					setFullscreen: (enabled) => enabled !== fullscreen.isFullscreen && fullscreen.toggle(),
					openSettings,
					openSearch: () => controls.searchBook && setSearchOpen(true),
					openGoTo: () => openContents(true),
					onMouseEnterControls: chrome.barProps.onMouseEnter,
					onMouseLeaveControls: chrome.barProps.onMouseLeave,
					setVisible,
					toggleVisible: toggle,
					visible: chrome.visible,
				},
				readerMeta,
			}}
		>
			<div className="min-h-0 relative h-full w-full overflow-hidden" data-epub-stage>
				<EpubNavigationControls isFixedLayout={isFixedLayout}>{children}</EpubNavigationControls>
				<EpubTopBar
					chrome={chrome}
					onContents={() => openContents()}
					onGoTo={() => openContents(true)}
					onSearch={controls.searchBook ? () => setSearchOpen(true) : undefined}
					onSettings={() => openSettings()}
					settingsOpen={settingsOpen}
				/>
				<EpubProgress
					onClick={() => openContents(true)}
					chromeVisible={chrome.visible}
					settingsOpen={settingsOpen}
				/>
				<EpubSettings
					open={settingsOpen}
					onOpenChange={setSettingsOpen}
					section={settingsSection}
					isFixedLayout={isFixedLayout}
				/>
				<LocationManager open={contentsOpen} onOpenChange={setContentsOpen} focusGoTo={goToFocus} />
				<SearchCommand open={searchOpen} onOpenChange={setSearchOpen} />
			</div>
		</EpubReaderContext.Provider>
	)
}

function EpubTopBar({
	chrome,
	onContents,
	onGoTo,
	onSearch,
	onSettings,
	settingsOpen,
}: {
	chrome: ReturnType<typeof useReaderChrome>
	onContents: () => void
	/** Opens the contents with the "go to location" field focused */
	onGoTo: () => void
	onSearch?: () => void
	onSettings: () => void
	settingsOpen: boolean
}) {
	const paths = usePaths()
	const { translate } = useTranslate()
	const {
		readerMeta: { bookEntity, bookMeta, progress },
	} = useEpubReaderContext()
	const position = epubPositionText(bookMeta?.chapter, progress)
	const {
		createBookmark,
		deleteBookmark,
		isUnknownCfiRange,
		canBookmarkCurrent,
		currentIsBookmarked,
	} = useEpubBookmark()

	return (
		<ReaderTopBar
			chrome={chrome}
			title={bookEntity.resolvedName}
			backTo={paths.bookOverview(bookEntity?.id || '')}
			onContents={onContents}
			onSearch={onSearch}
			bookmark={{
				active: currentIsBookmarked,
				disabled: isUnknownCfiRange,
				onToggle: () => {
					if (currentIsBookmarked) deleteBookmark()
					else if (canBookmarkCurrent) createBookmark()
				},
			}}
			onSettings={onSettings}
			settingsOpen={settingsOpen}
			position={
				position
					? {
							text: position,
							label: `${translate('shared.reader.goToLocation')} (G)`,
							onClick: onGoTo,
						}
					: undefined
			}
		/>
	)
}

/**
 * The reading progress the reader picked: the reading bar at the foot of the page (the
 * default: the chapter, the pages left in it and the book's percentage), the small number
 * in a corner, or nothing. A tap on either opens the contents.
 */
function EpubProgress({
	onClick,
	chromeVisible,
	settingsOpen,
}: {
	onClick: () => void
	chromeVisible: boolean
	settingsOpen: boolean
}) {
	const { display } = useReadingProgress('epub')
	if (display === 'bar') {
		return (
			<EpubReadingBar onClick={onClick} chromeVisible={chromeVisible} settingsOpen={settingsOpen} />
		)
	}
	return <EpubPageIndicator onClick={onClick} shown={display === 'number'} />
}

/** Phones: below Tailwind's `sm`, where the settings are a sheet over the page's foot */
const PHONE_QUERY = '(max-width: 639.98px)'
/** The bar's side margins on a phone, and its widest on a larger screen */
const PHONE_BAR_INSET = 22
const MAX_BAR_WIDTH = 680

function EpubReadingBar({
	onClick,
	chromeVisible,
	settingsOpen,
}: {
	onClick: () => void
	chromeVisible: boolean
	settingsOpen: boolean
}) {
	const { translate } = useTranslate()
	const {
		readerMeta: { bookMeta, progress, bookEntity: book },
	} = useEpubReaderContext()
	const {
		bookPreferences: { readingDirection },
	} = useBookPreferences({ book })
	const { items } = useReadingProgress('epub')
	const clock = useClockText(items.includes('clock'))
	const placement = useTextColumnPlacement(bookMeta?.chapter.totalProgression)
	if (!bookMeta) return null

	const { chapter } = bookMeta
	const position = Math.min(1, Math.max(0, chapter.totalProgression ?? progress ?? 0))
	const isPhone = !!window.matchMedia?.(PHONE_QUERY).matches
	const t = (key: string, options?: Record<string, unknown>) =>
		translate(`shared.reader.progress.${key}`, options)

	const left = items.includes('chapter') ? chapterLabel(chapter.path ?? []) : undefined
	const right: string[] = []
	if (items.includes('page') && chapter.pageInChapter && chapter.pagesInChapter) {
		right.push(t('pageInChapter', { page: chapter.pageInChapter, total: chapter.pagesInChapter }))
	}
	if (items.includes('pagesLeft') && chapter.pagesLeftInChapter != null) {
		right.push(
			t(isPhone ? 'pagesLeftShort' : 'pagesLeftInChapter', { count: chapter.pagesLeftInChapter }),
		)
	}
	if (items.includes('percent')) right.push(`${Math.round(position * 100)}%`)
	if (items.includes('clock')) right.push(clock)

	return (
		<ReadingBar
			chapters={bookMeta.chapterStarts ?? []}
			position={position}
			left={left}
			right={right}
			rtl={readingDirection === ReadingDirection.Rtl}
			onClick={onClick}
			label={[left, ...right, translate('shared.epubReader.locationManager')]
				.filter(Boolean)
				.join(', ')}
			// it stays (the pages leave it room), except under the phone's settings sheet
			visible={!(settingsOpen && isPhone)}
			emphasized={chromeVisible}
			style={placement}
		/>
	)
}

/**
 * Where the bar sits: across the book's text column (Readium narrows its container to the
 * column), at most 680px wide; on a phone, across the screen less 22px at each side
 */
function useTextColumnPlacement(at: number | undefined): React.CSSProperties {
	const [placement, setPlacement] = useState<React.CSSProperties>({
		left: PHONE_BAR_INSET,
		right: PHONE_BAR_INSET,
	})
	// measured again as the book moves on: Readium sizes its column once a chapter is laid out
	useLayoutEffect(() => {
		const measure = () => {
			const column = document.querySelector<HTMLElement>('[data-epub-column]')
			const stage = column?.closest<HTMLElement>('[data-epub-stage]')
			if (!column || !stage || window.matchMedia?.(PHONE_QUERY).matches) {
				setPlacement({
					left: `calc(${PHONE_BAR_INSET}px + env(safe-area-inset-left))`,
					right: `calc(${PHONE_BAR_INSET}px + env(safe-area-inset-right))`,
				})
				return
			}
			const box = column.getBoundingClientRect()
			const frame = stage.getBoundingClientRect()
			const width = Math.min(MAX_BAR_WIDTH, Math.max(0, box.width - 2 * PHONE_BAR_INSET))
			const left = box.left - frame.left + (box.width - width) / 2
			setPlacement({ left, width })
		}
		measure()
		const column = document.querySelector<HTMLElement>('[data-epub-column]')
		const observer =
			typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(() => measure())
		if (column) observer?.observe(column)
		window.addEventListener('resize', measure)
		return () => {
			observer?.disconnect()
			window.removeEventListener('resize', measure)
		}
	}, [at])
	return placement
}

/**
 * Where the reader is in the book, for the top bar: the location (Readium's positions, about
 * a page each, which "go to" takes) and the book's percentage, e.g. `123 / 2310 · 5%`
 */
function epubPositionText(
	chapter: EpubReaderChapterMeta | undefined,
	progress: number | null,
): string | null {
	const total = Math.min(1, Math.max(0, chapter?.totalProgression ?? progress ?? 0))
	const percent = `${Math.round(total * 100)}%`
	if (chapter?.locatorPosition != null && chapter.totalPositions != null) {
		return `${chapter.locatorPosition} / ${chapter.totalPositions} · ${percent}`
	}
	return chapter || progress != null ? percent : null
}

/**
 * The position in the book (Readium's positions, about a page of text each); a tap on it
 * opens the contents, to go to another chapter
 */
function EpubPageIndicator({ onClick, shown }: { onClick: () => void; shown: boolean }) {
	const { translate } = useTranslate()
	const { bookMeta, progress } = useEpubReaderContext().readerMeta
	if (!bookMeta) return null
	const { chapter } = bookMeta
	const totalProgression = chapter.totalProgression ?? progress ?? 0
	const text =
		chapter.locatorPosition != null && chapter.totalPositions != null
			? `${chapter.locatorPosition} / ${chapter.totalPositions}`
			: `${Math.round(Math.min(1, Math.max(0, totalProgression)) * 100)}%`
	return (
		<ReaderPageIndicator
			text={text}
			onClick={onClick}
			shown={shown}
			title={translate('shared.epubReader.locationManager')}
		/>
	)
}

function EpubSettings({
	open,
	onOpenChange,
	section,
	isFixedLayout,
}: {
	open: boolean
	onOpenChange: (open: boolean) => void
	section: 'shortcuts' | null
	isFixedLayout: boolean
}) {
	const { translate } = useTranslate()
	const {
		readerMeta: { bookEntity: book },
	} = useEpubReaderContext()
	const {
		bookPreferences: { readingMode, columnCount },
		setBookPreferences,
		differsFromDefaults,
		makeDefault,
		resetToDefaults,
	} = useBookPreferences({ book })

	const layout = useMemo(
		() => epubLayout(isFixedLayout ? ReadingMode.Paged : readingMode, columnCount),
		[isFixedLayout, readingMode, columnCount],
	)
	// the keys of the layout on screen: pages that turn, or a chapter that scrolls
	const shortcuts = useMemo<ShortcutRow[]>(
		() =>
			layout.flow === 'continuous'
				? [
						{ keys: ['↑', '↓'], label: translate('shared.reader.shortcuts.scrollLine') },
						{
							keys: ['Space', 'PgUp', 'PgDn'],
							label: translate('shared.reader.shortcuts.scrollScreenChapter'),
						},
						{ keys: ['Home', 'End'], label: translate('shared.reader.shortcuts.chapterStartEnd') },
						{ keys: ['G'], label: translate('shared.reader.goToLocation') },
					]
				: [
						{
							keys: ['←', '→', 'PgUp', 'PgDn'],
							label: translate('shared.readerAnnotations.shortcuts.turnPage'),
						},
						{
							keys: ['Space', '⇧ Space'],
							label: translate('shared.readerAnnotations.shortcuts.turnPage'),
						},
						{ keys: ['G'], label: translate('shared.reader.goToLocation') },
					],
		[translate, layout.flow],
	)

	const disabledReason = useCallback(
		(option: ReaderLayout) => {
			// a fixed-layout book's pages turn (one or two side by side), they don't scroll
			if (isFixedLayout && option.flow === 'continuous') {
				return translate('shared.reader.layout.fixedLayout')
			}
			if (option.spread === 'double' && option.flow === 'continuous') {
				return translate('shared.reader.layout.flowingTextScrollsInOneColumn')
			}
			if (option.spread === 'double' && !canShowTwoPages(window.innerWidth)) {
				return translate('shared.reader.layout.needsWiderScreen')
			}
			return null
		},
		[isFixedLayout, translate],
	)

	return (
		<ReaderSettingsDialog
			open={open}
			onOpenChange={onOpenChange}
			section={section}
			reader="epub"
			layout={layout}
			onLayoutChange={(next) =>
				setBookPreferences({
					readingMode:
						next.flow === 'continuous' ? ReadingMode.ContinuousVertical : ReadingMode.Paged,
					columnCount: next.spread === 'double' ? 2 : 1,
				})
			}
			layoutDisabledReason={disabledReason}
			more={<EpubReadingDirection />}
			shortcuts={shortcuts}
			bookDefaults={{
				kind: 'epub',
				differs: differsFromDefaults,
				onMakeDefault: makeDefault,
				onReset: resetToDefaults,
			}}
		>
			{isFixedLayout ? (
				<p
					className="leading-snug text-[12.5px] text-muted-foreground"
					data-testid="fixed-layout-text"
				>
					{translate('shared.reader.settings.fixedLayoutText')}
				</p>
			) : (
				<TextSettings />
			)}
		</ReaderSettingsDialog>
	)
}

/**
 * The EPUB layout from the stored reading mode and column count. Columns `auto` (the old
 * default, two columns on wide screens) now reads as one page unless two were picked.
 */
export function epubLayout(
	readingMode: ReadingMode | undefined,
	columnCount: 'auto' | 1 | 2 | undefined,
): ReaderLayout {
	const continuous = readingMode === ReadingMode.ContinuousVertical
	return {
		flow: continuous ? 'continuous' : 'paged',
		spread: !continuous && columnCount === 2 ? 'double' : 'single',
	}
}
