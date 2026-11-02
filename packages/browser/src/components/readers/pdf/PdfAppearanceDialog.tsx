import { EPUB_READER_THEMES, type EpubReaderTheme } from '@stump/client'
import {
	Button,
	buttonVariants,
	cn,
	Dialog,
	Heading,
	Label,
	NativeSelect,
	WideSwitch,
} from '@stump/components'
import { Check, Minus, Paintbrush, Plus } from 'lucide-react'
import { Link } from 'react-router'

import { useTheme } from '@/hooks/useTheme'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'
import { useReaderStore } from '@/stores'

import ControlButton from '../epub/controls/ControlButton'
import { resolveReaderTheme } from '../epub/readium/preferences'
import { type FitMode, MAX_ZOOM, MIN_ZOOM, zoomIn, zoomOut } from './geometry'
import type { PdfBookPreferences, PdfScrollMode } from './store'

type Props = {
	bookId: string
	fit: FitMode
	zoom: number
	scrollMode: PdfScrollMode
	invertPagesInDark: boolean
	onChange: (preferences: Partial<PdfBookPreferences>) => void
}

const THEME_LABELS: Record<EpubReaderTheme, string> = {
	auto: 'Auto',
	light: 'Light',
	sepia: 'Sepia',
	dark: 'Dark',
	black: 'Black',
}

/**
 * The PDF reader's appearance: the reader theme (shared with the EPUB reader), fit, zoom,
 * scrolling, whether dark themes invert the pages, and the other ways to open the PDF.
 */
export default function PdfAppearanceDialog({
	bookId,
	fit,
	zoom,
	scrollMode,
	invertPagesInDark,
	onChange,
}: Props) {
	const { translate } = useTranslate()
	const paths = usePaths()
	const { isDarkVariant } = useTheme()
	const theme = useReaderStore((state) => state.settings.theme ?? 'auto')
	const setSettings = useReaderStore((state) => state.setSettings)

	return (
		<Dialog>
			<Dialog.Trigger asChild>
				<ControlButton
					title={translate('shared.epubReader.themeAndOptions')}
					aria-label={translate('shared.epubReader.themeAndOptions')}
				>
					<Paintbrush className="h-4 w-4" />
				</ControlButton>
			</Dialog.Trigger>

			<Dialog.Content
				size="md"
				className="gap-4 z-101 flex max-h-[calc(100dvh-2rem)] flex-col overflow-y-auto bg-muted"
			>
				<Dialog.Header>
					<Heading size="md">{translate('shared.epubReader.appearance')}</Heading>
					<Dialog.Close />
				</Dialog.Header>

				<div>
					<Label>{translate('shared.pdfReader.theme')}</Label>
					<div className="mt-1.5 gap-2 flex flex-wrap" role="radiogroup" aria-label="Reader theme">
						{EPUB_READER_THEMES.map((option) => {
							const colors = resolveReaderTheme(option, isDarkVariant)
							const isActive = theme === option
							return (
								<button
									key={option}
									type="button"
									role="radio"
									aria-checked={isActive}
									onClick={() => setSettings({ theme: option })}
									className={cn(
										'gap-1.5 px-3 py-1.5 text-sm flex items-center rounded-lg border-2 transition-colors',
										isActive ? 'border-primary' : 'border-border hover:border-muted-foreground',
									)}
									style={{ backgroundColor: colors.background, color: colors.text }}
								>
									{isActive && <Check className="h-3.5 w-3.5" />}
									{THEME_LABELS[option]}
								</button>
							)
						})}
					</div>
				</div>

				<WideSwitch
					label={translate('shared.pdfReader.invertPages')}
					description={translate('shared.pdfReader.invertPagesHint')}
					checked={invertPagesInDark}
					onCheckedChange={(checked) => onChange({ invertPagesInDark: checked })}
				/>

				<div className="gap-4 sm:grid-cols-2 grid grid-cols-1">
					<div>
						<Label htmlFor="pdf-fit">{translate('shared.pdfReader.fit.label')}</Label>
						<NativeSelect
							id="pdf-fit"
							size="sm"
							className="mt-1.5"
							value={fit}
							onChange={(event) => onChange({ fit: event.target.value as FitMode, zoom: 1 })}
							options={[
								{ label: translate('shared.pdfReader.fit.width'), value: 'width' },
								{ label: translate('shared.pdfReader.fit.page'), value: 'page' },
							]}
						/>
					</div>
					<div>
						<Label htmlFor="pdf-scroll">
							{translate('shared.readerSettings.readingMode.label')}
						</Label>
						<NativeSelect
							id="pdf-scroll"
							size="sm"
							className="mt-1.5"
							value={scrollMode}
							onChange={(event) => onChange({ scrollMode: event.target.value as PdfScrollMode })}
							options={[
								{ label: translate('shared.epubReader.controls.continuous'), value: 'continuous' },
								{ label: translate('shared.pdfReader.singlePage'), value: 'paged' },
							]}
						/>
					</div>
				</div>

				<div>
					<Label>{translate('shared.pdfReader.zoom')}</Label>
					<div className="mt-1.5 gap-2 flex items-center">
						<Button
							size="sm"
							variant="outline"
							disabled={zoom <= MIN_ZOOM}
							aria-label={translate('shared.pdfReader.zoomOut')}
							onClick={() => onChange({ zoom: zoomOut(zoom) })}
						>
							<Minus className="h-4 w-4" />
						</Button>
						<span className="w-14 text-sm text-center tabular-nums">{Math.round(zoom * 100)}%</span>
						<Button
							size="sm"
							variant="outline"
							disabled={zoom >= MAX_ZOOM}
							aria-label={translate('shared.pdfReader.zoomIn')}
							onClick={() => onChange({ zoom: zoomIn(zoom) })}
						>
							<Plus className="h-4 w-4" />
						</Button>
						<Button size="sm" variant="ghost" onClick={() => onChange({ zoom: 1 })}>
							{translate('shared.pdfReader.zoomReset')}
						</Button>
					</div>
				</div>

				<div className="gap-2 flex flex-col">
					<Label>{translate('shared.pdfReader.openAs')}</Label>
					<div className="gap-2 flex flex-wrap">
						<Button size="sm" variant="outline" onClick={() => onChange({ reader: 'pages' })}>
							{translate('shared.pdfReader.openAsPages')}
						</Button>
						<Link
							className={buttonVariants({ size: 'sm', variant: 'outline' })}
							to={paths.bookReader(bookId, { isPdf: true, isStreaming: false })}
						>
							{translate('shared.pdfReader.openInBrowser')}
						</Link>
					</div>
				</div>
			</Dialog.Content>
		</Dialog>
	)
}
