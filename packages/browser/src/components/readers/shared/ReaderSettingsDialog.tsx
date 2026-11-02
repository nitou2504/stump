import { EPUB_READER_THEMES, type EpubReaderTheme } from '@stump/client'
import { cn, Dialog, Heading, WideSwitch } from '@stump/components'
import { Check } from 'lucide-react'
import { useEffect, useRef } from 'react'

import { useTheme } from '@/hooks/useTheme'
import { useTranslate } from '@/hooks/useTranslate'
import { useReaderStore } from '@/stores'

import { ReaderShortcutsList, type ShortcutRow } from '../annotations'
import { resolveReaderTheme } from '../epub/readium/preferences'
import type { ReaderLayout } from './layout'
import LayoutPicker from './LayoutPicker'
import { useDocumentFullscreen } from './useReaderChrome'

type Props = {
	open: boolean
	onOpenChange: (open: boolean) => void
	/** `shortcuts`: opened with `?`, so the shortcuts list is scrolled into view */
	section?: 'shortcuts' | null
	layout: ReaderLayout
	onLayoutChange: (layout: ReaderLayout) => void
	/** Why a layout can't be used here (shown disabled with the reason), or `null` */
	layoutDisabledReason?: (option: ReaderLayout) => string | null
	/** Reader-specific layout settings under the picker (e.g. reading direction) */
	layoutExtra?: React.ReactNode
	/** The reader's own section: the text for an EPUB, the page for a PDF or comic */
	readerSection: { title: string; content: React.ReactNode }
	/** The reader's own shortcuts, listed before the shared ones */
	shortcuts?: ShortcutRow[]
	/** Last, e.g. other ways to open the book */
	footer?: React.ReactNode
}

/**
 * The settings menu every reader shares, in the same order everywhere: theme, page layout,
 * the reader's own settings (text or page), display, then the keyboard shortcuts.
 */
export default function ReaderSettingsDialog({
	open,
	onOpenChange,
	section,
	layout,
	onLayoutChange,
	layoutDisabledReason,
	layoutExtra,
	readerSection,
	shortcuts,
	footer,
}: Props) {
	const { translate } = useTranslate()
	const shortcutsRef = useRef<HTMLElement>(null)

	useEffect(() => {
		if (!open || section !== 'shortcuts') return undefined
		const frame = window.requestAnimationFrame(() =>
			shortcutsRef.current?.scrollIntoView({ block: 'start' }),
		)
		return () => window.cancelAnimationFrame(frame)
	}, [open, section])

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<Dialog.Content
				size="md"
				className="gap-5 z-101 flex max-h-[calc(100dvh-2rem)] flex-col overflow-y-auto overscroll-contain bg-muted"
				data-testid="reader-settings"
			>
				<Dialog.Header>
					<Heading size="md">{translate('shared.reader.settings.title')}</Heading>
					<Dialog.Close />
				</Dialog.Header>

				<Section title={translate('shared.reader.settings.theme')}>
					<ThemePicker />
				</Section>

				<Section title={translate('shared.reader.layout.label')}>
					<LayoutPicker
						value={layout}
						onChange={onLayoutChange}
						disabledReason={layoutDisabledReason}
					/>
					{layoutExtra}
				</Section>

				<Section title={readerSection.title}>{readerSection.content}</Section>

				<Section title={translate('shared.reader.settings.display')}>
					<DisplaySettings />
				</Section>

				<Section
					ref={shortcutsRef}
					title={translate('shared.readerAnnotations.shortcuts.title')}
					className="pointer-coarse:hidden"
				>
					<ReaderShortcutsList extra={shortcuts} />
				</Section>

				{footer}
			</Dialog.Content>
		</Dialog>
	)
}

type SectionProps = {
	title: string
	children: React.ReactNode
	className?: string
	ref?: React.Ref<HTMLElement>
}

export function Section({ title, children, className, ref }: SectionProps) {
	return (
		<section ref={ref} className={cn('gap-2.5 flex scroll-mt-4 flex-col', className)}>
			<h3 className="text-xs font-medium tracking-wide uppercase text-muted-foreground">
				{title}
			</h3>
			{children}
		</section>
	)
}

const THEME_KEYS: Record<EpubReaderTheme, string> = {
	auto: 'shared.reader.themes.auto',
	light: 'shared.reader.themes.light',
	sepia: 'shared.reader.themes.sepia',
	dark: 'shared.reader.themes.dark',
	black: 'shared.reader.themes.black',
}

/** The reader colour themes, shared by every reader and every book */
export function ThemePicker() {
	const { translate } = useTranslate()
	const { isDarkVariant } = useTheme()
	const theme = useReaderStore((state) => state.settings.theme ?? 'auto')
	const setSettings = useReaderStore((state) => state.setSettings)

	return (
		<div>
			<div
				className="gap-2 grid grid-cols-5"
				role="radiogroup"
				aria-label={translate('shared.reader.settings.theme')}
			>
				{EPUB_READER_THEMES.map((option) => {
					const colors = resolveReaderTheme(option, isDarkVariant)
					const isActive = theme === option
					const label = translate(THEME_KEYS[option])
					return (
						<button
							key={option}
							type="button"
							role="radio"
							aria-checked={isActive}
							aria-label={label}
							title={option === 'auto' ? translate('shared.reader.themes.autoHint') : label}
							onClick={() => setSettings({ theme: option })}
							data-theme-option={option}
							className={cn(
								'h-14 gap-0.5 flex flex-col items-center justify-center rounded-lg border-2 transition-colors',
								isActive ? 'border-primary' : 'border-border hover:border-muted-foreground',
							)}
							style={{ backgroundColor: colors.background, color: colors.text }}
						>
							<span className="font-serif text-base leading-none">Aa</span>
							<span className="gap-0.5 text-[11px] flex items-center leading-none">
								{isActive && <Check className="h-3 w-3" />}
								{label}
							</span>
						</button>
					)
				})}
			</div>
			<p className="mt-1.5 text-xs pointer-coarse:hidden text-muted-foreground">
				{translate('shared.reader.themes.dayNightHint')}
			</p>
		</div>
	)
}

/** Settings about the reader's frame, the same in every reader */
function DisplaySettings() {
	const { translate } = useTranslate()
	const showPageIndicator = useReaderStore((state) => state.settings.showPageIndicator ?? true)
	const setSettings = useReaderStore((state) => state.setSettings)
	const fullscreen = useDocumentFullscreen()

	return (
		<div className="gap-3 flex flex-col">
			<WideSwitch
				label={translate('shared.reader.settings.pageNumber')}
				description={translate('shared.reader.settings.pageNumberHint')}
				checked={showPageIndicator}
				onCheckedChange={(checked) => setSettings({ showPageIndicator: checked })}
			/>
			{fullscreen.supported && (
				<WideSwitch
					label={translate('shared.pdfReader.fullscreen')}
					checked={fullscreen.isFullscreen}
					onCheckedChange={() => fullscreen.toggle()}
				/>
			)}
		</div>
	)
}
