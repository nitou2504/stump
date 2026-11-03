import { PDF_EXTENSION, type WidePagesBehavior } from '@stump/client'
import { cn, Dialog, Heading, Slider } from '@stump/components'
import { ReadingDirection, ReadingImageScaleFit, ReadingMode } from '@stump/graphql'
import { Columns2, Scan, Sun, ZoomIn } from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'

import { useTheme } from '@/hooks/useTheme'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'
import { useBookPreferences } from '@/scenes/book/reader/useBookPreferences'
import { useReaderStore } from '@/stores'

import { isEditableTarget, resolveReaderShortcut, type ShortcutRow } from '../../annotations'
import { useReaderChromeTheme } from '../../epub/readium/useReaderChromeTheme'
import { usePdfReaderStore } from '../../pdf/store'
import GoToField, { resolvePlaceInput } from '../../shared/GoToField'
import { canShowTwoPages, pageIndicatorText, type ReaderLayout } from '../../shared/layout'
import { type PageNavHistory, usePageNavHistory } from '../../shared/navHistory'
import NavHistoryRow from '../../shared/NavHistoryRow'
import { nextDayNightTheme } from '../../shared/readerKeys'
import ReaderPageIndicator from '../../shared/ReaderPageIndicator'
import ReaderSettingsDialog from '../../shared/ReaderSettingsDialog'
import ReaderTopBar from '../../shared/ReaderTopBar'
import ReadingBar, { useClockText } from '../../shared/ReadingBar'
import { type ReadingBarItem, useReadingProgress } from '../../shared/readingProgress'
import {
	DirectionRow,
	FitGlyph,
	type FitGlyphMode,
	Segmented,
	SettingRow,
	SwitchRow,
} from '../../shared/settingsControls'
import { useReaderChrome } from '../../shared/useReaderChrome'
import { useImageBaseReaderContext } from '../context'
import NextInSeries from './NextInSeries'

/**
 * The image (comic, manga) reader's frame, the same as the other readers': the top bar over
 * the pages, the page number in a corner, and the shared settings with the comic's own
 * (reading direction, fit, brightness, the cover alone in spreads).
 */
export default function ControlsOverlay() {
	const { translate } = useTranslate()
	const paths = usePaths()
	const { isDarkVariant } = useTheme()
	const { book, currentPage, pageSets, setCurrentPage } = useImageBaseReaderContext()
	const {
		settings: { showToolBar },
		bookPreferences: { readingMode, readingDirection, doublePageBehavior, coverAlone },
		setBookPreferences,
		setSettings,
		differsFromDefaults,
		makeDefault,
		resetToDefaults,
	} = useBookPreferences({ book })
	const theme = useReaderStore((state) => state.settings.theme ?? 'auto')
	useReaderChromeTheme(theme)
	const readingProgress = useReadingProgress('comic')

	const [settingsOpen, setSettingsOpen] = useState(false)
	const [settingsSection, setSettingsSection] = useState<'shortcuts' | null>(null)
	const [pagesOpen, setPagesOpen] = useState(false)
	/** The pages were opened to go to one (`g`, the position in the top bar): its field has the focus */
	const [goToFocus, setGoToFocus] = useState(false)
	const openPages = useCallback((focusGoTo = false) => {
		setGoToFocus(focusGoTo)
		setPagesOpen(true)
	}, [])
	const setToolbar = useCallback(
		(visible: boolean) => setSettings({ showToolBar: visible }),
		[setSettings],
	)
	const chrome = useReaderChrome({
		pinned: settingsOpen || pagesOpen,
		visible: showToolBar,
		onVisibleChange: setToolbar,
	})

	const openSettings = useCallback((section: 'shortcuts' | null = null) => {
		setSettingsSection(section)
		setSettingsOpen(true)
	}, [])

	// Back and forward after big moves (going to a page): "Back to Page 12" in the pages
	const pageLabelOf = useCallback(
		(page: number) => translate('shared.readerAnnotations.page', { page }),
		[translate],
	)
	const pageHistory = usePageNavHistory({
		reader: 'comic',
		bookId: book.id,
		currentPage,
		labelOf: pageLabelOf,
		goTo: setCurrentPage,
	})

	// The shared reader keys: d day/night, ? the shortcuts; Ctrl/Cmd+F and / are taken so the
	// browser's find bar or quick find don't open over the pages (a comic has no text to search)
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || isEditableTarget(event.target)) return
			const action = resolveReaderShortcut(event, { hasTarget: false, targetIsAnnotation: false })
			if (!action) return
			if (action.type === 'dayNight') {
				event.preventDefault()
				setSettings({ theme: nextDayNightTheme(theme, isDarkVariant) })
			} else if (action.type === 'help') {
				event.preventDefault()
				openSettings('shortcuts')
			} else if (action.type === 'search') {
				event.preventDefault()
				openPages()
			}
		}
		window.addEventListener('keydown', onKeyDown)
		return () => window.removeEventListener('keydown', onKeyDown)
	}, [setSettings, theme, isDarkVariant, openSettings, openPages])

	// g: go to a page (the pages, with their go-to field focused), as in the other readers
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || isEditableTarget(event.target)) return
			if (event.ctrlKey || event.metaKey || event.altKey || event.key.toLowerCase() !== 'g') return
			if (document.querySelector('[role="dialog"][data-state="open"]')) return
			event.preventDefault()
			openPages(true)
		}
		window.addEventListener('keydown', onKeyDown)
		return () => window.removeEventListener('keydown', onKeyDown)
	}, [openPages])

	const isPdf = !!book.extension?.match(PDF_EXTENSION)
	const setPdfPreferences = usePdfReaderStore((state) => state.setBookPreferences)

	// Two pages show when picked ('always'), or for manga by default ('auto') on a wide
	// screen in landscape: the picker shows what's on screen
	const landscape = window.innerWidth > window.innerHeight
	const wideEnough = canShowTwoPages(window.innerWidth)
	const layout = useMemo<ReaderLayout>(
		() => ({
			flow: readingMode === ReadingMode.Paged ? 'paged' : 'continuous',
			spread:
				readingMode === ReadingMode.Paged &&
				wideEnough &&
				(doublePageBehavior === 'always' || (doublePageBehavior === 'auto' && landscape))
					? 'double'
					: 'single',
		}),
		[readingMode, doublePageBehavior, landscape, wideEnough],
	)

	const currentSet = pageSets.find((set) => set.includes(currentPage - 1)) ?? [currentPage - 1]
	const indicator = pageIndicatorText(
		currentSet.map((index) => index + 1),
		book.pages,
	)

	const layoutDisabledReason = useCallback(
		(option: ReaderLayout) => {
			if (option.spread === 'double' && option.flow === 'continuous') {
				return translate('shared.reader.layout.spreadsTurnOnly')
			}
			if (option.spread === 'double' && !canShowTwoPages(window.innerWidth)) {
				return translate('shared.reader.layout.needsWiderScreen')
			}
			return null
		},
		[translate],
	)

	// the keys of the layout on screen: pages that turn (stepping through a big page), or the
	// strip that scrolls
	const shortcuts = useMemo<ShortcutRow[]>(
		() =>
			layout.flow === 'continuous'
				? [
						{ keys: ['↑', '↓'], label: translate('shared.reader.shortcuts.scrollLine') },
						{
							keys: ['Space', 'PgUp', 'PgDn'],
							label: translate('shared.reader.shortcuts.scrollScreen'),
						},
						{ keys: ['Home', 'End'], label: translate('shared.reader.shortcuts.firstLastPage') },
						{ keys: ['G'], label: translate('shared.reader.goToPage') },
					]
				: [
						{ keys: ['←', '→'], label: translate('shared.reader.shortcuts.stepPage') },
						{ keys: ['Space', '⇧ Space'], label: translate('shared.reader.shortcuts.stepPage') },
						{ keys: ['↑', '↓'], label: translate('shared.reader.shortcuts.scrollPage') },
						{ keys: ['PgUp', 'PgDn'], label: translate('shared.reader.shortcuts.turnPageAlways') },
						{ keys: ['G'], label: translate('shared.reader.goToPage') },
					],
		[translate, layout.flow],
	)

	return (
		<>
			<ReaderTopBar
				chrome={chrome}
				title={book.resolvedName}
				backTo={paths.bookOverview(book.id)}
				onContents={() => openPages()}
				onSettings={() => openSettings()}
				settingsOpen={settingsOpen}
				position={{
					text: indicator,
					label: `${translate('shared.reader.goToPage')} (G)`,
					onClick: () => openPages(true),
				}}
				className="fixed"
			/>
			{readingProgress.display === 'bar' ? (
				<ComicReadingBar
					pages={currentSet.map((index) => index + 1)}
					pageCount={book.pages}
					items={readingProgress.items}
					rtl={readingDirection === ReadingDirection.Rtl}
					// with the top bar: the pages use the whole screen, so the bar would lie on them
					visible={chrome.visible && !(settingsOpen && window.innerWidth < 640)}
					emphasized={chrome.visible}
					onClick={() => openPages(true)}
				/>
			) : (
				<ReaderPageIndicator
					text={indicator}
					shown={readingProgress.display === 'number'}
					className="fixed"
					onClick={() => openPages(true)}
					title={`${translate('shared.reader.goToPage')} (G)`}
				/>
			)}
			<NextInSeries />

			<ReaderSettingsDialog
				open={settingsOpen}
				onOpenChange={setSettingsOpen}
				section={settingsSection}
				reader="comic"
				layout={layout}
				onLayoutChange={(next) =>
					setBookPreferences({
						readingMode: next.flow === 'paged' ? ReadingMode.Paged : ReadingMode.ContinuousVertical,
						doublePageBehavior: next.spread === 'double' ? 'always' : 'off',
					})
				}
				layoutDisabledReason={layoutDisabledReason}
				more={
					<>
						{layout.spread === 'double' ? (
							<SwitchRow
								label={translate('shared.reader.layout.coverAlone')}
								hint={translate('shared.reader.layout.coverAloneHint')}
								checked={coverAlone ?? true}
								onCheckedChange={(checked) => setBookPreferences({ coverAlone: checked })}
							/>
						) : null}
						<OnePageSettings onePage={layout.flow === 'paged' && layout.spread === 'single'} />
					</>
				}
				shortcuts={shortcuts}
				bookDefaults={{
					kind: 'comic',
					differs: differsFromDefaults,
					onMakeDefault: makeDefault,
					onReset: resetToDefaults,
				}}
				footer={
					isPdf ? (
						<button
							type="button"
							className="h-8 px-2 text-xs self-start rounded-md text-foreground/85 hover:bg-foreground/6"
							onClick={() => setPdfPreferences(book.id, { reader: 'pdf' })}
						>
							{translate('shared.pdfReader.openAsText')}
						</button>
					) : null
				}
			>
				<DirectionRow
					value={readingDirection}
					onChange={(direction) => setBookPreferences({ readingDirection: direction })}
				/>
				<ImagePageSettings />
			</ReaderSettingsDialog>

			<PagesDialog
				open={pagesOpen}
				onOpenChange={setPagesOpen}
				focusGoTo={goToFocus}
				currentPage={currentPage}
				onGo={(page) => {
					setPagesOpen(false)
					pageHistory.jumpTo(page)
				}}
				history={pageHistory}
				rtl={readingDirection === ReadingDirection.Rtl}
			/>
		</>
	)
}

const FIT_GLYPHS: Record<ReadingImageScaleFit, FitGlyphMode> = {
	[ReadingImageScaleFit.Auto]: 'page',
	[ReadingImageScaleFit.Height]: 'height',
	[ReadingImageScaleFit.Width]: 'width',
	[ReadingImageScaleFit.None]: 'original',
}
const FIT_LABELS: Record<ReadingImageScaleFit, string> = {
	[ReadingImageScaleFit.Auto]: 'shared.reader.fit.page',
	[ReadingImageScaleFit.Height]: 'shared.reader.fit.height',
	[ReadingImageScaleFit.Width]: 'shared.reader.fit.width',
	[ReadingImageScaleFit.None]: 'shared.reader.fit.original',
}

/** The comic's own rows: how the page fits and how bright it is */
function ImagePageSettings() {
	const { book } = useImageBaseReaderContext()
	const {
		bookPreferences: { imageScaling, brightness },
		setBookPreferences,
	} = useBookPreferences({ book })
	const { translate } = useTranslate()
	const fit = imageScaling?.scaleToFit ?? ReadingImageScaleFit.Height
	const level = Number.isFinite(brightness) ? brightness : 1
	return (
		<>
			<SettingRow label={translate('shared.reader.fit.label')}>
				<Segmented<ReadingImageScaleFit>
					label={translate('shared.reader.fit.label')}
					testId="reader-fit"
					className="w-full"
					value={fit}
					onChange={(scaleToFit) => setBookPreferences({ imageScaling: { scaleToFit } })}
					options={[
						ReadingImageScaleFit.Auto,
						ReadingImageScaleFit.Height,
						ReadingImageScaleFit.Width,
						ReadingImageScaleFit.None,
					].map((value) => ({
						value,
						label: translate(FIT_LABELS[value]),
						icon: <FitGlyph mode={FIT_GLYPHS[value]} />,
					}))}
				/>
			</SettingRow>
			<SettingRow label={translate('shared.reader.settings.brightness')}>
				<div className="gap-2.5 pointer-coarse:h-10 flex h-[30px] w-full items-center text-foreground/80">
					<Sun aria-hidden className="size-3 shrink-0" />
					<Slider
						value={[level]}
						min={0.1}
						max={1}
						step={0.01}
						aria-label={translate('shared.reader.settings.brightness')}
						onValueChange={([value]) =>
							value !== undefined && setBookPreferences({ brightness: Math.max(0.1, value) })
						}
						className={cn(
							// the panel's ink on paper: a track of ink at 7%, filled with ink, a paper knob
							'[&>span:first-child]:h-1 [&>span:first-child]:bg-foreground/7 [&>span:first-child>span]:bg-foreground',
							'[&_[role=slider]]:border-0 [&_[role=slider]]:bg-popover [&_[role=slider]]:shadow-[0_0_0_1.5px_var(--foreground),0_1px_3px_rgb(0_0_0/0.2)]',
						)}
					/>
					<Sun aria-hidden className="size-[17px] shrink-0" />
				</div>
			</SettingRow>
		</>
	)
}

/**
 * One page at a time, under More: whether its blank margins are cropped and how a wide (two
 * page) image shows on a narrow screen
 */
function OnePageSettings({ onePage }: { onePage: boolean }) {
	const { book } = useImageBaseReaderContext()
	const {
		bookPreferences: { cropBorders, widePages = 'zoom' },
		setBookPreferences,
	} = useBookPreferences({ book })
	const { translate } = useTranslate()
	if (!onePage) return null
	return (
		<>
			<SwitchRow
				label={translate('shared.reader.comic.crop')}
				hint={translate('shared.reader.comic.cropHint')}
				checked={!!cropBorders}
				onCheckedChange={(checked) => setBookPreferences({ cropBorders: checked })}
			/>
			<SettingRow
				label={translate('shared.reader.comic.widePages')}
				hint={translate('shared.reader.comic.widePagesHint')}
			>
				<Segmented<WidePagesBehavior>
					label={translate('shared.reader.comic.widePages')}
					testId="reader-wide-pages"
					className="w-full"
					value={widePages}
					onChange={(value) => setBookPreferences({ widePages: value })}
					options={[
						{
							value: 'zoom',
							label: translate('shared.reader.comic.wideZoom'),
							icon: <ZoomIn aria-hidden />,
						},
						{
							value: 'split',
							label: translate('shared.reader.comic.wideSplit'),
							icon: <Columns2 aria-hidden />,
						},
						{
							value: 'fit',
							label: translate('shared.reader.comic.wideFit'),
							icon: <Scan aria-hidden />,
						},
					]}
				/>
			</SettingRow>
		</>
	)
}

type PagesDialogProps = {
	open: boolean
	onOpenChange: (open: boolean) => void
	/** Opened to go to a page: the go-to field has the focus */
	focusGoTo?: boolean
	currentPage: number
	onGo: (page: number) => void
	rtl: boolean
	/** Back to the page left by the last jump (and forward again), above the field */
	history?: Pick<PageNavHistory, 'back' | 'forward' | 'goBack' | 'goForward'>
}

/**
 * The book's pages as a grid, to jump to one (the comic reader's 'contents'), with a field
 * to type the page (or a share of the book, `45%`) at the top
 */
function PagesDialog({
	open,
	onOpenChange,
	focusGoTo = false,
	currentPage,
	onGo,
	rtl,
	history,
}: PagesDialogProps) {
	const { translate } = useTranslate()
	const { book, getPageUrl } = useImageBaseReaderContext()
	const pages = Array.from({ length: book.pages }, (_, index) => index + 1)

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<Dialog.Content
				size="lg"
				className="z-101 flex max-h-[calc(100dvh-2rem)] flex-col"
				data-testid="reader-pages"
			>
				<Dialog.Header>
					<Heading size="md">{translate('shared.reader.pages')}</Heading>
					<Dialog.Close />
				</Dialog.Header>
				{history && (
					<NavHistoryRow
						back={history.back}
						forward={history.forward}
						onBack={() => {
							onOpenChange(false)
							history.goBack()
						}}
						onForward={() => {
							onOpenChange(false)
							history.goForward()
						}}
						className="-mx-2 -mb-1"
					/>
				)}
				<GoToField
					label={translate('shared.reader.goToPage')}
					placeholder={`1 – ${book.pages}`}
					resolve={(input) => resolvePlaceInput(input, book.pages)}
					invalidMessage={translate('shared.pdfReader.noSuchPage')}
					onGo={onGo}
					autoFocus={open && focusGoTo}
					resetKey={open}
				/>
				<div
					className="gap-2 pb-2 sm:grid-cols-5 md:grid-cols-6 grid grid-cols-3 overflow-y-auto overscroll-contain"
					dir={rtl ? 'rtl' : 'ltr'}
				>
					{pages.map((page) => (
						<button
							key={page}
							type="button"
							onClick={() => onGo(page)}
							className={cn(
								'gap-1 p-1 flex flex-col items-center rounded-md border-2',
								page === currentPage ? 'border-primary' : 'border-transparent hover:border-border',
							)}
							aria-current={page === currentPage ? 'page' : undefined}
						>
							<img
								src={getPageUrl(page)}
								alt=""
								loading="lazy"
								decoding="async"
								className="aspect-[2/3] w-full rounded-sm bg-muted object-cover"
							/>
							<span className="text-xs text-muted-foreground tabular-nums">{page}</span>
						</button>
					))}
				</div>
			</Dialog.Content>
		</Dialog>
	)
}

type ComicReadingBarProps = {
	/** The pages on screen, from 1 (two in a spread) */
	pages: number[]
	pageCount: number
	items: ReadingBarItem[]
	rtl: boolean
	visible: boolean
	emphasized: boolean
	onClick: () => void
}

/**
 * The comic's reading bar: the page and the percentage on the right, the rule filling from
 * the right in a right-to-left book. (A comic's chapters aren't known here: no ticks.)
 */
function ComicReadingBar({
	pages,
	pageCount,
	items,
	rtl,
	visible,
	emphasized,
	onClick,
}: ComicReadingBarProps) {
	const { translate } = useTranslate()
	const clock = useClockText(items.includes('clock'))
	const total = Math.max(1, pageCount)
	const last = Math.max(1, ...pages)
	const right: string[] = []
	if (items.includes('page')) {
		right.push(
			translate('shared.reader.progress.pageOf', {
				page: pages.length > 1 ? `${Math.min(...pages)}–${last}` : last,
				total,
			}),
		)
	}
	if (items.includes('percent')) right.push(`${Math.round((last / total) * 100)}%`)
	if (items.includes('clock')) right.push(clock)
	return (
		<ReadingBar
			chapters={[]}
			position={total > 1 ? (last - 1) / (total - 1) : 1}
			right={right}
			rtl={rtl}
			onClick={onClick}
			label={[...right, translate('shared.reader.pages')].join(', ')}
			visible={visible}
			emphasized={emphasized}
			band
			fixed
			className="sm:inset-x-auto sm:left-1/2 sm:w-[min(680px,calc(100%-96px))] sm:-translate-x-1/2 inset-x-[22px]"
		/>
	)
}
