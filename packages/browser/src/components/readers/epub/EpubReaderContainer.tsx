import type { EpubTextAlign } from '@stump/client'
import { Label, NativeSelect } from '@stump/components'
import { ReadingMode } from '@stump/graphql'
import { useCallback, useMemo, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'
import { useBookPreferences } from '@/scenes/book/reader/useBookPreferences'
import { useReaderStore } from '@/stores'

import type { ShortcutRow } from '../annotations'
import { canShowTwoPages, type ReaderLayout } from '../shared/layout'
import ReaderPageIndicator from '../shared/ReaderPageIndicator'
import ReaderSettingsDialog from '../shared/ReaderSettingsDialog'
import ReaderTopBar from '../shared/ReaderTopBar'
import { useDocumentFullscreen, useReaderChrome } from '../shared/useReaderChrome'
import {
	EpubReaderContext,
	EpubReaderControls,
	EpubReaderMeta,
	useEpubReaderContext,
} from './context'
import { EpubNavigationControls, PageMargins, ReadingDirection, SearchCommand } from './controls'
import FontFamily from './controls/FontFamily'
import FontSizeControl from './controls/FontSizeControl'
import LineHeightControl from './controls/LineHeightControl'
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
		| 'onGoToLocator'
		| 'getLocatorPreviewText'
		| 'searchBook'
		| 'canGoForward'
		| 'canGoBackward'
	>
	/** The book sets its own page layout (a fixed-layout EPUB): the layout picker is off */
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
					onMouseEnterControls: chrome.barProps.onMouseEnter,
					onMouseLeaveControls: chrome.barProps.onMouseLeave,
					setVisible,
					toggleVisible: toggle,
					visible: chrome.visible,
				},
				readerMeta,
			}}
		>
			<div className="min-h-0 relative h-full w-full overflow-hidden">
				<EpubNavigationControls>{children}</EpubNavigationControls>
				<EpubTopBar
					chrome={chrome}
					onContents={() => setContentsOpen(true)}
					onSearch={controls.searchBook ? () => setSearchOpen(true) : undefined}
					onSettings={() => openSettings()}
				/>
				<EpubPageIndicator />
				<EpubSettings
					open={settingsOpen}
					onOpenChange={setSettingsOpen}
					section={settingsSection}
					isFixedLayout={isFixedLayout}
				/>
				<LocationManager open={contentsOpen} onOpenChange={setContentsOpen} />
				<SearchCommand open={searchOpen} onOpenChange={setSearchOpen} />
			</div>
		</EpubReaderContext.Provider>
	)
}

function EpubTopBar({
	chrome,
	onContents,
	onSearch,
	onSettings,
}: {
	chrome: ReturnType<typeof useReaderChrome>
	onContents: () => void
	onSearch?: () => void
	onSettings: () => void
}) {
	const paths = usePaths()
	const {
		readerMeta: { bookEntity },
	} = useEpubReaderContext()
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
		/>
	)
}

/** The position in the book (Readium's positions, about a page of text each) */
function EpubPageIndicator() {
	const { bookMeta, progress } = useEpubReaderContext().readerMeta
	if (!bookMeta) return null
	const { chapter } = bookMeta
	const totalProgression = chapter.totalProgression ?? progress ?? 0
	const text =
		chapter.locatorPosition != null && chapter.totalPositions != null
			? `${chapter.locatorPosition} / ${chapter.totalPositions}`
			: `${Math.round(Math.min(1, Math.max(0, totalProgression)) * 100)}%`
	return <ReaderPageIndicator text={text} />
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
	} = useBookPreferences({ book })
	const textAlign = useReaderStore((state) => state.settings.textAlign ?? 'auto')
	const setSettings = useReaderStore((state) => state.setSettings)

	const layout = useMemo(() => epubLayout(readingMode, columnCount), [readingMode, columnCount])
	const shortcuts = useMemo<ShortcutRow[]>(
		() => [{ keys: ['←', '→'], label: translate('shared.readerAnnotations.shortcuts.turnPage') }],
		[translate],
	)

	const disabledReason = useCallback(
		(option: ReaderLayout) => {
			if (isFixedLayout) return translate('shared.reader.layout.fixedLayout')
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
			layout={layout}
			onLayoutChange={(next) =>
				setBookPreferences({
					readingMode:
						next.flow === 'continuous' ? ReadingMode.ContinuousVertical : ReadingMode.Paged,
					columnCount: next.spread === 'double' ? 2 : 1,
				})
			}
			layoutDisabledReason={disabledReason}
			layoutExtra={<ReadingDirection />}
			readerSection={{
				title: translate('shared.reader.settings.text'),
				content: (
					<div className="gap-1 flex flex-col">
						<FontFamily />
						<FontSizeControl />
						<LineHeightControl />
						<div className="py-1.5">
							<Label htmlFor="text-align">{translate('shared.reader.text.alignment')}</Label>
							<NativeSelect
								id="text-align"
								size="sm"
								className="mt-1.5"
								value={textAlign}
								onChange={(e) => setSettings({ textAlign: e.target.value as EpubTextAlign })}
								options={[
									{ label: translate('shared.reader.text.alignBook'), value: 'auto' },
									{ label: translate('shared.reader.text.alignJustify'), value: 'justify' },
									{ label: translate('shared.reader.text.alignStart'), value: 'start' },
								]}
							/>
						</div>
						<PageMargins />
					</div>
				),
			}}
			shortcuts={shortcuts}
		/>
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
