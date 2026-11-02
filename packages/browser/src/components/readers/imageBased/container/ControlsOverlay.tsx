import { type PageBackground, PDF_EXTENSION, type WidePagesBehavior } from '@stump/client'
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
import { canShowTwoPages, pageIndicatorText, type ReaderLayout } from '../../shared/layout'
import { nextDayNightTheme } from '../../shared/readerKeys'
import ReaderPageIndicator from '../../shared/ReaderPageIndicator'
import ReaderSettingsDialog from '../../shared/ReaderSettingsDialog'
import ReaderTopBar from '../../shared/ReaderTopBar'
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
	} = useBookPreferences({ book })
	const theme = useReaderStore((state) => state.settings.theme ?? 'auto')
	useReaderChromeTheme(theme)

	const [settingsOpen, setSettingsOpen] = useState(false)
	const [settingsSection, setSettingsSection] = useState<'shortcuts' | null>(null)
	const [pagesOpen, setPagesOpen] = useState(false)
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
				setPagesOpen(true)
			}
		}
		window.addEventListener('keydown', onKeyDown)
		return () => window.removeEventListener('keydown', onKeyDown)
	}, [setSettings, theme, isDarkVariant, openSettings])

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

	const shortcuts = useMemo<ShortcutRow[]>(
		() => [
			{ keys: ['←', '→'], label: translate('shared.reader.shortcuts.stepPage') },
			{ keys: ['Space', '⇧ Space'], label: translate('shared.reader.shortcuts.stepPage') },
			{ keys: ['↑', '↓'], label: translate('shared.reader.shortcuts.scrollPage') },
			{ keys: ['PgUp', 'PgDn'], label: translate('shared.reader.shortcuts.turnPageAlways') },
		],
		[translate],
	)

	return (
		<>
			<ReaderTopBar
				chrome={chrome}
				title={book.resolvedName}
				backTo={paths.bookOverview(book.id)}
				onContents={() => setPagesOpen(true)}
				onSettings={() => openSettings()}
				className="fixed"
			/>
			<ReaderPageIndicator
				text={indicator}
				className="fixed"
				onClick={() => setPagesOpen(true)}
				title={translate('shared.reader.pages')}
			/>
			<NextInSeries />

			<ReaderSettingsDialog
				open={settingsOpen}
				onOpenChange={setSettingsOpen}
				section={settingsSection}
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
						<BackgroundRow />
					</>
				}
				shortcuts={shortcuts}
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
				<ImagePageSettings onePage={layout.flow === 'paged' && layout.spread === 'single'} />
			</ReaderSettingsDialog>

			<PagesDialog
				open={pagesOpen}
				onOpenChange={setPagesOpen}
				currentPage={currentPage}
				onGo={(page) => {
					setPagesOpen(false)
					setCurrentPage(page)
				}}
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

/**
 * The comic's own rows: how the page fits and how bright it is; one page at a time, also
 * whether its blank margins are cropped and how a wide (two-page) image shows
 */
function ImagePageSettings({ onePage }: { onePage: boolean }) {
	const { book } = useImageBaseReaderContext()
	const {
		bookPreferences: { imageScaling, brightness, cropBorders, widePages = 'zoom' },
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
			{onePage && (
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
									icon: <ZoomIn aria-hidden className="h-4 w-4" />,
								},
								{
									value: 'split',
									label: translate('shared.reader.comic.wideSplit'),
									icon: <Columns2 aria-hidden className="h-4 w-4" />,
								},
								{
									value: 'fit',
									label: translate('shared.reader.comic.wideFit'),
									icon: <Scan aria-hidden className="h-4 w-4" />,
								},
							]}
						/>
					</SettingRow>
				</>
			)}
			<SettingRow label={translate('shared.reader.settings.brightness')}>
				<div className="gap-2 px-1 flex w-full items-center">
					<Sun className="h-3 w-3 shrink-0 text-muted-foreground" />
					<Slider
						value={[level]}
						min={0.1}
						max={1}
						step={0.01}
						aria-label={translate('shared.reader.settings.brightness')}
						onValueChange={([value]) =>
							value !== undefined && setBookPreferences({ brightness: Math.max(0.1, value) })
						}
					/>
					<Sun className="h-4 w-4 shrink-0 text-muted-foreground" />
				</div>
			</SettingRow>
		</>
	)
}

const BACKGROUNDS: { value: PageBackground; key: string; swatch: string }[] = [
	{
		value: 'theme',
		key: 'shared.reader.comic.backgroundTheme',
		swatch: 'var(--color-background, var(--background))',
	},
	{
		value: 'auto',
		key: 'shared.reader.comic.backgroundAuto',
		swatch: 'linear-gradient(135deg, #ffffff 0 50%, #000000 50% 100%)',
	},
	{ value: 'black', key: 'shared.reader.comic.backgroundBlack', swatch: '#000000' },
	{ value: 'white', key: 'shared.reader.comic.backgroundWhite', swatch: '#ffffff' },
]

/** The colour around the pages: the theme's, the page's own margins', black or white */
function BackgroundRow() {
	const { book } = useImageBaseReaderContext()
	const {
		bookPreferences: { pageBackground = 'theme' },
		setBookPreferences,
	} = useBookPreferences({ book })
	const { translate } = useTranslate()
	const label = translate('shared.reader.comic.background')
	return (
		<SettingRow label={label}>
			<Segmented<PageBackground>
				label={label}
				testId="reader-page-background"
				className="w-full"
				value={pageBackground}
				onChange={(value) => setBookPreferences({ pageBackground: value })}
				options={BACKGROUNDS.map(({ value, key, swatch }) => ({
					value,
					label: translate(key),
					icon: (
						<span
							aria-hidden
							className="size-4 block rounded-full border border-foreground/25"
							style={{ background: swatch }}
						/>
					),
				}))}
			/>
		</SettingRow>
	)
}

type PagesDialogProps = {
	open: boolean
	onOpenChange: (open: boolean) => void
	currentPage: number
	onGo: (page: number) => void
	rtl: boolean
}

/** The book's pages as a grid, to jump to one (the comic reader's 'contents') */
function PagesDialog({ open, onOpenChange, currentPage, onGo, rtl }: PagesDialogProps) {
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
