import { buttonVariants, cn } from '@stump/components'
import { Link } from 'react-router'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { FitGlyph, Segmented, SettingRow, Stepper, SwitchRow } from '../shared/settingsControls'
import { type FitMode, MAX_ZOOM, MIN_ZOOM, zoomIn, zoomOut } from './geometry'
import type { PdfBookPreferences } from './store'

type PageSettingsProps = {
	fit: FitMode
	zoom: number
	invertPagesInDark: boolean
	onChange: (preferences: Partial<PdfBookPreferences>) => void
}

/** The PDF reader's own rows of the reading settings: how the page fits, zoom, inverting */
export function PdfPageSettings({ fit, zoom, invertPagesInDark, onChange }: PageSettingsProps) {
	const { translate } = useTranslate()

	return (
		<>
			<SettingRow label={translate('shared.pdfReader.fit.label')}>
				<Segmented<FitMode>
					label={translate('shared.pdfReader.fit.label')}
					testId="reader-fit"
					className="w-full"
					value={fit}
					onChange={(value) => onChange({ fit: value, zoom: 1 })}
					options={[
						{
							value: 'width',
							label: translate('shared.pdfReader.fit.width'),
							icon: <FitGlyph mode="width" />,
						},
						{
							value: 'page',
							label: translate('shared.pdfReader.fit.page'),
							icon: <FitGlyph mode="page" />,
						},
					]}
				/>
			</SettingRow>
			<SettingRow label={translate('shared.pdfReader.zoom')}>
				<Stepper
					label={translate('shared.pdfReader.zoom')}
					testId="reader-zoom"
					display={`${Math.round(zoom * 100)}%`}
					decreaseLabel={translate('shared.pdfReader.zoomOut')}
					increaseLabel={translate('shared.pdfReader.zoomIn')}
					canDecrease={zoom > MIN_ZOOM}
					canIncrease={zoom < MAX_ZOOM}
					onDecrease={() => onChange({ zoom: zoomOut(zoom) })}
					onIncrease={() => onChange({ zoom: zoomIn(zoom) })}
					onReset={() => onChange({ zoom: 1 })}
					resetLabel={translate('shared.pdfReader.zoomReset')}
				/>
			</SettingRow>
			<SwitchRow
				label={translate('shared.pdfReader.invertPages')}
				hint={translate('shared.pdfReader.invertPagesHint')}
				checked={invertPagesInDark}
				onCheckedChange={(checked) => onChange({ invertPagesInDark: checked })}
			/>
		</>
	)
}

/** The other ways to open the PDF, last under More */
export function PdfOpenAs({
	bookId,
	onChange,
}: {
	bookId: string
	onChange: (preferences: Partial<PdfBookPreferences>) => void
}) {
	const { translate } = useTranslate()
	const paths = usePaths()
	const linkClass = cn(buttonVariants({ size: 'sm', variant: 'ghost' }), 'h-8 px-2 text-xs')

	return (
		<div
			className="gap-1 pt-1 flex flex-wrap items-center"
			title={translate('shared.pdfReader.openAs')}
		>
			<button type="button" className={linkClass} onClick={() => onChange({ reader: 'pages' })}>
				{translate('shared.pdfReader.openAsPages')}
			</button>
			<Link
				className={linkClass}
				to={paths.bookReader(bookId, { isPdf: true, isStreaming: false })}
			>
				{translate('shared.pdfReader.openInBrowser')}
			</Link>
		</div>
	)
}
