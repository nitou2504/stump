import { cn, Dialog } from '@stump/components'
import { ChevronDown, Keyboard, Maximize, Minimize } from 'lucide-react'
import { useEffect, useState } from 'react'

import { useTranslate } from '@/hooks/useTranslate'
import { useReaderStore } from '@/stores'

import { ReaderShortcutsHelp, type ShortcutRow } from '../annotations'
import { HELP_SHORTCUT } from '../annotations/styles'
import type { ReaderLayout } from './layout'
import LayoutPicker from './LayoutPicker'
import { SettingRow, SwitchRow, ThemeSwatches } from './settingsControls'
import { useDocumentFullscreen } from './useReaderChrome'

type Props = {
	open: boolean
	onOpenChange: (open: boolean) => void
	/** `shortcuts`: opened with `?`, so the keyboard shortcuts sheet shows instead */
	section?: 'shortcuts' | null
	layout: ReaderLayout
	onLayoutChange: (layout: ReaderLayout) => void
	/** Why a layout can't be used here (shown dimmed with the reason), or `null` */
	layoutDisabledReason?: (option: ReaderLayout) => string | null
	/** The reader's own rows, under the theme: the text for an EPUB, the page for a PDF or comic */
	children: React.ReactNode
	/** Rarely used reader settings, under More (e.g. reading direction, cover alone) */
	more?: React.ReactNode
	/** The reader's own shortcuts, listed before the shared ones */
	shortcuts?: ShortcutRow[]
	/** Last under More, e.g. other ways to open the book */
	footer?: React.ReactNode
}

/**
 * The reading settings every reader shares, as a small panel over the page (like a Kindle's
 * or Apple Books' `Aa` panel): no backdrop, so every change shows on the page behind it.
 * A card under the settings button on wider screens, a sheet from the bottom on phones.
 *
 * Always in the same order: the page colours (and full screen), the reader's own controls,
 * the page layout, then More (the reader's rarer settings, the page number, the keyboard
 * shortcuts and the footer). The shortcuts open in a sheet of their own; `?` opens it
 * directly (`section: 'shortcuts'`).
 */
export default function ReaderSettingsDialog({
	open,
	onOpenChange,
	section,
	layout,
	onLayoutChange,
	layoutDisabledReason,
	children,
	more,
	shortcuts,
	footer,
}: Props) {
	const { translate } = useTranslate()
	const [moreOpen, setMoreOpen] = useState(false)
	const [shortcutsRequested, setShortcutsRequested] = useState(false)
	const showShortcuts = open && (section === 'shortcuts' || shortcutsRequested)

	useEffect(() => {
		if (!open) setShortcutsRequested(false)
	}, [open])

	const showPageIndicator = useReaderStore((state) => state.settings.showPageIndicator ?? true)
	const setSettings = useReaderStore((state) => state.setSettings)

	return (
		<>
			<Dialog open={open && !showShortcuts} onOpenChange={onOpenChange}>
				<Dialog.Content
					size="sm"
					aria-describedby={undefined}
					// the page stays visible (and undimmed) behind the panel; a click on it closes the panel
					overlayClassName="z-100 bg-transparent backdrop-blur-none supports-backdrop-filter:backdrop-blur-none"
					className={cn(
						'gap-2 p-3 shadow-2xl z-101 flex flex-col overflow-y-auto overscroll-contain bg-popover text-popover-foreground',
						// phones: a sheet from the bottom edge
						'bottom-0 left-0 translate-x-0 translate-y-0 top-auto max-h-[78dvh] w-full max-w-none rounded-b-none',
						'pb-[max(0.75rem,env(safe-area-inset-bottom))]',
						// wider screens: a card under the settings button in the top right corner
						'sm:right-3 sm:bottom-auto sm:left-auto sm:max-h-[calc(100dvh-5rem)] sm:w-[22rem] sm:max-w-[22rem] sm:rounded-b-xl sm:top-[calc(3.5rem+env(safe-area-inset-top))]',
					)}
					data-testid="reader-settings"
				>
					<Dialog.Title className="sr-only">
						{translate('shared.reader.settings.title')}
					</Dialog.Title>
					<span
						aria-hidden
						className="sm:hidden h-1 w-9 mb-1 shrink-0 self-center rounded-full bg-foreground/20"
					/>

					<div className="gap-2 flex items-center justify-between">
						<ThemeSwatches />
						<FullscreenButton />
					</div>
					<div className="my-0.5 h-px shrink-0 bg-border/70" />

					{children}

					<SettingRow label={translate('shared.reader.settings.layout')}>
						<LayoutPicker
							value={layout}
							onChange={onLayoutChange}
							disabledReason={layoutDisabledReason}
						/>
					</SettingRow>

					<button
						type="button"
						onClick={() => setMoreOpen((value) => !value)}
						aria-expanded={moreOpen}
						className="gap-1 py-1 text-xs flex items-center self-start text-muted-foreground hover:text-foreground"
						data-testid="reader-settings-more"
					>
						{translate(moreOpen ? 'shared.reader.settings.less' : 'shared.reader.settings.more')}
						<ChevronDown
							className={cn('h-3.5 w-3.5 transition-transform', moreOpen && 'rotate-180')}
						/>
					</button>

					{moreOpen && (
						<div className="gap-1 flex flex-col" data-testid="reader-settings-more-content">
							{more}
							<SwitchRow
								label={translate('shared.reader.settings.pageNumber')}
								hint={translate('shared.reader.settings.pageNumberHint')}
								checked={showPageIndicator}
								onCheckedChange={(checked) => setSettings({ showPageIndicator: checked })}
							/>
							<button
								type="button"
								onClick={() => setShortcutsRequested(true)}
								className="gap-2 min-h-9 text-sm flex items-center justify-between text-foreground/90 hover:text-foreground pointer-coarse:hidden"
								data-testid="reader-settings-shortcuts"
							>
								<span className="gap-2 flex items-center">
									<Keyboard className="h-4 w-4 text-muted-foreground" />
									{translate('shared.readerAnnotations.shortcuts.title')}
								</span>
								<kbd className="px-1.5 font-mono text-xs rounded border border-border bg-muted">
									{HELP_SHORTCUT}
								</kbd>
							</button>
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
			className="size-8 pointer-coarse:size-10 flex shrink-0 items-center justify-center rounded-md text-foreground/70 hover:bg-foreground/6 hover:text-foreground"
			data-testid="reader-fullscreen"
		>
			{fullscreen.isFullscreen ? (
				<Minimize className="h-4 w-4" />
			) : (
				<Maximize className="h-4 w-4" />
			)}
		</button>
	)
}
