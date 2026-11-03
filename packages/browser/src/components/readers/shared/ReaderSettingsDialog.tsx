import { cn, Dialog } from '@stump/components'
import { ChevronDown, Maximize, Minimize } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'

import { ReaderShortcutsHelp, type ShortcutRow } from '../annotations'
import { HELP_SHORTCUT } from '../annotations/styles'
import type { ReaderLayout } from './layout'
import LayoutPicker from './LayoutPicker'
import {
	type ReaderKind,
	READING_BAR_ITEMS_FOR,
	type ReadingBarItem,
	type ReadingProgressDisplay,
	useReadingProgress,
} from './readingProgress'
import {
	type BookDefaultsProps,
	BookDefaultsRow,
	FOCUS,
	PanelSubheading,
	Segmented,
	SettingRow,
	ThemeSwatches,
	ToggleChips,
} from './settingsControls'
import { useDocumentFullscreen } from './useReaderChrome'

type Props = {
	open: boolean
	onOpenChange: (open: boolean) => void
	/** `shortcuts`: opened with `?`, so the keyboard shortcuts sheet shows instead */
	section?: 'shortcuts' | null
	/** Which reader's reading progress the More section sets */
	reader: ReaderKind
	layout: ReaderLayout
	onLayoutChange: (layout: ReaderLayout) => void
	/** Why a layout can't be used here (shown dimmed with the reason), or `null` */
	layoutDisabledReason?: (option: ReaderLayout) => string | null
	/** The reader's own rows, under the paper: the text for an EPUB, the page for a PDF or comic */
	children: React.ReactNode
	/** Rarely used reader settings, first under More (e.g. reading direction, cover alone) */
	more?: React.ReactNode
	/** The reader's own shortcuts, listed before the shared ones */
	shortcuts?: ShortcutRow[]
	/** Last under More, e.g. other ways to open the book */
	footer?: React.ReactNode
	/**
	 * The book's settings against its kind's defaults: under More, with "Make default" and
	 * "Reset this book"; a dot by More shows when the book has settings of its own
	 */
	bookDefaults?: BookDefaultsProps
}

/** A drag down the phone sheet past this share of its height (or a flick) closes it */
const SHEET_CLOSE_SHARE = 0.3
const SHEET_CLOSE_SPEED = 0.5

/**
 * The reading settings every reader shares, as a small panel over the page, like the `Aa`
 * panel of reading apps ("ink and paper": it's printed with the book's own ink on its own
 * paper). No backdrop and no dimming, so every change shows on the page behind it.
 *
 * - Wider screens: a 320px card under the `Aa` button, clear of the text column.
 * - Phones: a sheet from the bottom edge, about half the screen, with a handle to drag it
 *   away; the top half of the page stays readable above it.
 *
 * Always in the same order: the papers (and full screen), the reader's own controls, the
 * pages, then a footer with More (the reader's rarer settings, the reading progress) and the
 * keyboard shortcuts. `?` opens the shortcuts directly (`section: 'shortcuts'`).
 */
export default function ReaderSettingsDialog({
	open,
	onOpenChange,
	section,
	reader,
	layout,
	onLayoutChange,
	layoutDisabledReason,
	children,
	more,
	shortcuts,
	footer,
	bookDefaults,
}: Props) {
	const { translate } = useTranslate()
	const [moreOpen, setMoreOpen] = useState(false)
	const [shortcutsRequested, setShortcutsRequested] = useState(false)
	const showShortcuts = open && (section === 'shortcuts' || shortcutsRequested)

	useEffect(() => {
		if (!open) setShortcutsRequested(false)
	}, [open])

	// Closed with Escape: the keyboard goes back to the `Aa` button it came from
	const escapedRef = useRef(false)
	useEffect(() => {
		if (open || !escapedRef.current) return
		escapedRef.current = false
		const button = document.querySelector<HTMLButtonElement>('[data-testid=reader-settings-button]')
		window.requestAnimationFrame(() => button?.focus())
	}, [open])

	const contentRef = useRef<HTMLDivElement>(null)
	// the phone sheet follows a finger on its handle
	const moveSheet = useCallback((offset: number, animate: boolean) => {
		const element = contentRef.current
		if (!element) return
		element.style.transition = animate ? 'transform 200ms cubic-bezier(.32,.72,0,1)' : 'none'
		element.style.transform = offset > 0 ? `translateY(${offset}px)` : ''
	}, [])
	const sheetHeight = useCallback(() => contentRef.current?.getBoundingClientRect().height ?? 0, [])

	return (
		<>
			<Dialog open={open && !showShortcuts} onOpenChange={onOpenChange}>
				<Dialog.Content
					ref={contentRef}
					size="sm"
					aria-describedby={undefined}
					// the page stays visible (and undimmed) behind the panel; a click on it closes the panel
					overlayClassName="z-100 bg-transparent backdrop-blur-none supports-backdrop-filter:backdrop-blur-none"
					className={cn(
						'gap-0 z-101 flex flex-col overflow-y-auto overscroll-contain border border-border bg-popover text-popover-foreground ring-0',
						// phones: a sheet from the bottom edge
						'px-5 pt-2 bottom-0 left-0 translate-x-0 translate-y-0 top-auto max-h-[88dvh] w-full max-w-none rounded-t-[18px] rounded-b-none border-x-0 border-b-0',
						'pb-[calc(10px+env(safe-area-inset-bottom))]',
						'shadow-[0_-1px_0_var(--border),0_-18px_40px_-18px_rgb(0_0_0/0.35)]',
						'duration-[260ms] ease-[cubic-bezier(.32,.72,0,1)] data-[state=closed]:zoom-out-100 data-[state=closed]:slide-out-to-bottom data-[state=open]:zoom-in-100 data-[state=open]:slide-in-from-bottom',
						// wider screens: a card under the settings button in the top right corner
						'sm:px-4 sm:pt-3 sm:pb-2 sm:right-3 sm:bottom-auto sm:left-auto sm:max-h-[calc(100dvh-72px)] sm:w-[320px] sm:max-w-[320px] sm:rounded-[14px] sm:border sm:top-[calc(56px+env(safe-area-inset-top))]',
						'sm:shadow-[0_1px_2px_rgb(0_0_0/0.06),0_18px_44px_-16px_rgb(0_0_0/0.32)] dark:sm:shadow-[0_0_0_1px_rgb(0_0_0/0.35),0_22px_50px_-14px_rgb(0_0_0/0.75)]',
						'sm:origin-top-right sm:duration-150 sm:ease-[cubic-bezier(.2,.8,.2,1)] sm:data-[state=open]:slide-in-from-top-1 sm:data-[state=open]:slide-in-from-bottom-0 sm:data-[state=open]:zoom-in-[0.985] sm:data-[state=closed]:slide-out-to-bottom-0 sm:data-[state=closed]:zoom-out-[0.985] sm:data-[state=closed]:duration-100',
						'motion-reduce:duration-75 motion-reduce:data-[state=closed]:zoom-out-100 motion-reduce:data-[state=open]:slide-in-from-bottom-0 motion-reduce:data-[state=open]:slide-in-from-top-0 motion-reduce:data-[state=open]:zoom-in-100',
					)}
					data-testid="reader-settings"
					onEscapeKeyDown={() => {
						escapedRef.current = true
					}}
				>
					<Dialog.Title className="sr-only">
						{translate('shared.reader.settings.title')}
					</Dialog.Title>
					{/* phones: the handle, to drag the sheet away */}
					<SheetHandle height={sheetHeight} move={moveSheet} onClose={() => onOpenChange(false)} />

					<div className="gap-2 flex items-center justify-between">
						<ThemeSwatches />
						<FullscreenButton />
					</div>
					<div className="-mx-5 sm:-mx-4 my-2.5 h-px shrink-0 bg-border" />

					<div className="gap-2 pointer-coarse:gap-3 flex flex-col">
						{children}

						<SettingRow label={translate('shared.reader.settings.layout')}>
							<LayoutPicker
								value={layout}
								onChange={onLayoutChange}
								disabledReason={layoutDisabledReason}
							/>
						</SettingRow>
					</div>

					<div className="mt-2 pt-1.5 sm:border-t flex shrink-0 items-center justify-between border-border">
						<button
							type="button"
							onClick={() => setMoreOpen((value) => !value)}
							aria-expanded={moreOpen}
							className={cn(
								'h-7 gap-1 pointer-coarse:h-10 font-medium inline-flex items-center rounded-md text-[12.5px] text-muted-foreground hover:text-foreground',
								FOCUS,
							)}
							data-testid="reader-settings-more"
						>
							{translate(moreOpen ? 'shared.reader.settings.less' : 'shared.reader.settings.more')}
							{bookDefaults?.differs && !moreOpen && (
								<span
									aria-hidden
									className="size-1.5 rounded-full bg-foreground/70"
									data-testid="reader-settings-more-dot"
								/>
							)}
							<ChevronDown
								aria-hidden
								className={cn(
									'size-3.5 transition-transform duration-150 motion-reduce:transition-none',
									moreOpen && 'rotate-180',
								)}
							/>
						</button>
						<button
							type="button"
							onClick={() => setShortcutsRequested(true)}
							className={cn(
								'h-7 inline-flex items-center gap-[7px] rounded-md text-[12.5px] text-muted-foreground hover:text-foreground pointer-coarse:hidden',
								FOCUS,
							)}
							data-testid="reader-settings-shortcuts"
						>
							{translate('shared.reader.settings.shortcuts')}
							<kbd className="font-semibold inline-grid h-[18px] min-w-[18px] place-items-center rounded-[5px] bg-foreground/7 px-[5px] text-[11px] text-foreground shadow-[inset_0_-1px_0_var(--border)]">
								{HELP_SHORTCUT}
							</kbd>
						</button>
					</div>

					{moreOpen && (
						<div
							className="mt-1.5 gap-2 pointer-coarse:gap-3 flex animate-in flex-col duration-150 fade-in-0 motion-reduce:animate-none"
							data-testid="reader-settings-more-content"
						>
							{more}
							<ReadingProgressSettings reader={reader} />
							{bookDefaults && <BookDefaultsRow {...bookDefaults} />}
							{footer}
						</div>
					)}
				</Dialog.Content>
			</Dialog>

			<ReaderShortcutsHelp
				open={showShortcuts}
				onOpenChange={(next) => {
					if (!next) onOpenChange(false)
				}}
				extra={shortcuts}
			/>
		</>
	)
}

const ITEM_KEYS: Record<ReadingBarItem, string> = {
	chapter: 'shared.reader.progress.items.chapter',
	page: 'shared.reader.progress.items.page',
	pagesLeft: 'shared.reader.progress.items.pagesLeft',
	percent: 'shared.reader.progress.items.percent',
	clock: 'shared.reader.progress.items.clock',
}

/**
 * The reading progress, under More: off, the small page number or the reading bar, and what
 * the bar shows. Where the bar goes is the reader's own (an EPUB keeps it at the page's foot,
 * a PDF or comic shows it with the top bar): nothing to choose there.
 */
function ReadingProgressSettings({ reader }: { reader: ReaderKind }) {
	const { translate } = useTranslate()
	const { display, items, setProgress } = useReadingProgress(reader)
	const t = (key: string) => translate(`shared.reader.progress.${key}`)

	return (
		<>
			<SettingRow label={t('label')}>
				<Segmented<ReadingProgressDisplay>
					label={t('label')}
					testId="reader-progress"
					variant="text"
					className="w-full"
					value={display}
					onChange={(value) => setProgress({ display: value })}
					options={(['off', 'number', 'bar'] as const).map((value) => ({
						value,
						label: t(value),
						icon: t(value),
					}))}
				/>
			</SettingRow>
			{display === 'bar' && (
				<>
					<PanelSubheading>{t('barShows')}</PanelSubheading>
					<ToggleChips<ReadingBarItem>
						label={t('barShows')}
						testId="reader-bar-items"
						options={READING_BAR_ITEMS_FOR[reader].map((value) => ({
							value,
							label: translate(ITEM_KEYS[value]),
						}))}
						value={items}
						onChange={(value) => setProgress({ items: value })}
					/>
				</>
			)}
		</>
	)
}

/**
 * The phone sheet's handle: the sheet follows a finger dragging it down, and closes past 30%
 * of its height or on a quick flick; otherwise it springs back
 */
function SheetHandle({
	height,
	move,
	onClose,
}: {
	/** The sheet's height when the drag starts */
	height: () => number
	/** Moves the sheet down by `offset` px (0: back in place) */
	move: (offset: number, animate: boolean) => void
	onClose: () => void
}) {
	const dragRef = useRef<{ y: number; at: number; height: number } | null>(null)

	return (
		<div
			aria-hidden
			className="sm:hidden -mx-5 pt-1 pb-2.5 shrink-0 cursor-grab touch-none"
			data-testid="reader-settings-handle"
			onPointerDown={(event) => {
				;(event.target as Element).setPointerCapture?.(event.pointerId)
				dragRef.current = { y: event.clientY, at: performance.now(), height: height() }
			}}
			onPointerMove={(event) => {
				const drag = dragRef.current
				if (drag) move(Math.max(0, event.clientY - drag.y), false)
			}}
			onPointerUp={(event) => {
				const drag = dragRef.current
				dragRef.current = null
				if (!drag) return
				const offset = Math.max(0, event.clientY - drag.y)
				const speed = offset / Math.max(1, performance.now() - drag.at)
				if (offset > drag.height * SHEET_CLOSE_SHARE || speed > SHEET_CLOSE_SPEED) {
					move(drag.height, true)
					window.setTimeout(() => {
						onClose()
						move(0, false)
					}, 180)
				} else {
					move(0, true)
				}
			}}
			onPointerCancel={() => {
				dragRef.current = null
				move(0, true)
			}}
		>
			<span className="h-1 w-9 mx-auto block rounded-[3px] bg-foreground/18" />
		</div>
	)
}

/** The reader's own full screen (the whole page, so its menus still show), also on F11 */
function FullscreenButton() {
	const { translate } = useTranslate()
	const fullscreen = useDocumentFullscreen()
	if (!fullscreen.supported) return null
	const label = `${translate('shared.pdfReader.fullscreen')} (F11)`
	return (
		<button
			type="button"
			onClick={fullscreen.toggle}
			aria-pressed={fullscreen.isFullscreen}
			aria-label={label}
			title={label}
			className={cn(
				'size-8 pointer-coarse:size-11 flex shrink-0 items-center justify-center rounded-lg text-foreground/80 hover:bg-foreground/7 hover:text-foreground [&_svg]:size-[17px]',
				FOCUS,
			)}
			data-testid="reader-fullscreen"
		>
			{fullscreen.isFullscreen ? <Minimize aria-hidden /> : <Maximize aria-hidden />}
		</button>
	)
}
