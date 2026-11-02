import { Button, buttonVariants, Label, NativeSelect, WideSwitch } from '@stump/components'
import { Minus, Plus } from 'lucide-react'
import { Link } from 'react-router'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { Section } from '../shared/ReaderSettingsDialog'
import { type FitMode, MAX_ZOOM, MIN_ZOOM, zoomIn, zoomOut } from './geometry'
import type { PdfBookPreferences } from './store'

type PageSettingsProps = {
	fit: FitMode
	zoom: number
	invertPagesInDark: boolean
	onChange: (preferences: Partial<PdfBookPreferences>) => void
}

/** The PDF reader's own section of the reader settings: how the page fits, zoom, inverting */
export function PdfPageSettings({ fit, zoom, invertPagesInDark, onChange }: PageSettingsProps) {
	const { translate } = useTranslate()

	return (
		<div className="gap-4 flex flex-col">
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
			</div>

			<WideSwitch
				label={translate('shared.pdfReader.invertPages')}
				description={translate('shared.pdfReader.invertPagesHint')}
				checked={invertPagesInDark}
				onCheckedChange={(checked) => onChange({ invertPagesInDark: checked })}
			/>
		</div>
	)
}

/** The other ways to open the PDF, last in its settings */
export function PdfOpenAs({
	bookId,
	onChange,
}: {
	bookId: string
	onChange: (preferences: Partial<PdfBookPreferences>) => void
}) {
	const { translate } = useTranslate()
	const paths = usePaths()

	return (
		<Section title={translate('shared.pdfReader.openAs')}>
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
		</Section>
	)
}
