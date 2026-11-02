import { PDF_EXTENSION } from '@stump/client'
import { Button, cn, Dialog, Heading, Label, WideSwitch } from '@stump/components'
import { ReadingDirection, ReadingImageScaleFit, ReadingMode } from '@stump/graphql'
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
import { useReaderChrome } from '../../shared/useReaderChrome'
import { useImageBaseReaderContext } from '../context'
import BrightnessControl from './BrightnessControl'
import ImageScalingSelect from './ImageScalingSelect'
import NextInSeries from './NextInSeries'
import ReadingDirectionSelect from './ReadingDirectionSelect'

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

	const layout = useMemo<ReaderLayout>(
		() => ({
			flow: readingMode === ReadingMode.Paged ? 'paged' : 'continuous',
			spread:
				readingMode === ReadingMode.Paged && doublePageBehavior && doublePageBehavior !== 'off'
					? 'double'
					: 'single',
		}),
		[readingMode, doublePageBehavior],
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
			{ keys: ['←', '→'], label: translate('shared.readerAnnotations.shortcuts.turnPage') },
			{ keys: ['Space'], label: translate('shared.reader.shortcuts.toggleBar') },
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
			<ReaderPageIndicator text={indicator} className="fixed" />
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
				layoutExtra={
					<div className="gap-3 flex flex-col">
						<div>
							<Label htmlFor="reading-direction">{translate('shared.reader.direction.label')}</Label>
							<div className="mt-1.5">
								<ReadingDirectionSelect
									direction={readingDirection}
									onChange={(direction) => setBookPreferences({ readingDirection: direction })}
								/>
							</div>
						</div>
						{layout.spread === 'double' && (
							<WideSwitch
								label={translate('shared.reader.layout.coverAlone')}
								description={translate('shared.reader.layout.coverAloneHint')}
								checked={coverAlone ?? true}
								onCheckedChange={(checked) => setBookPreferences({ coverAlone: checked })}
							/>
						)}
					</div>
				}
				readerSection={{
					title: translate('shared.reader.settings.page'),
					content: <ImagePageSettings />,
				}}
				shortcuts={shortcuts}
				footer={
					isPdf ? (
						<Button size="sm" variant="outline" onClick={() => setPdfPreferences(book.id, { reader: 'pdf' })}>
							{translate('shared.pdfReader.openAsText')}
						</Button>
					) : null
				}
			/>

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

function ImagePageSettings() {
	const { book } = useImageBaseReaderContext()
	const {
		bookPreferences: { imageScaling },
		setBookPreferences,
	} = useBookPreferences({ book })
	const { translate } = useTranslate()
	return (
		<div className="gap-3 flex flex-col">
			<div>
				<Label>{translate('shared.reader.fit.label')}</Label>
				<div className="mt-1.5">
					<ImageScalingSelect
						value={imageScaling?.scaleToFit ?? ReadingImageScaleFit.Height}
						onChange={(scaleToFit) => setBookPreferences({ imageScaling: { scaleToFit } })}
					/>
				</div>
			</div>
			<BrightnessControl />
		</div>
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
			<Dialog.Content size="lg" className="z-101 flex max-h-[calc(100dvh-2rem)] flex-col" data-testid="reader-pages">
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
							<span className="text-xs tabular-nums text-muted-foreground">{page}</span>
						</button>
					))}
				</div>
			</Dialog.Content>
		</Dialog>
	)
}
