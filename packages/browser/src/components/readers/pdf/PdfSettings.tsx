import { buttonVariants, cn } from '@stump/components'
import { Link } from 'react-router'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import ReadingBar, { useClockText } from '../shared/ReadingBar'
import type { ReadingBarItem } from '../shared/readingProgress'
import { FitGlyph, Segmented, SettingRow, Stepper, SwitchRow } from '../shared/settingsControls'
import { type FitMode, MAX_ZOOM, MIN_ZOOM, zoomIn, zoomOut } from './geometry'
import { chapterTitleForPage, type FlatOutlineEntry } from './outline'
import type { PdfBookPreferences } from './store'

type PageSettingsProps = {
	fit: FitMode
	zoom: number
	invertPagesInDark: boolean
	/** Each page shown without its blank margins */
	trimMargins: boolean
	/** A dark or black page colour: the only themes where pages can be inverted */
	darkTheme: boolean
	onChange: (preferences: Partial<PdfBookPreferences>) => void
}

/**
 * The PDF reader's own rows of the reading settings: how the page fits, zoom, and in the
 * dark and black themes whether the pages are inverted too; whether blank margins are trimmed
 */
export function PdfPageSettings({
	fit,
	zoom,
	invertPagesInDark,
	trimMargins,
	darkTheme,
	onChange,
}: PageSettingsProps) {
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
				label={translate('shared.pdfReader.trimMargins')}
				hint={translate('shared.pdfReader.trimMarginsHint')}
				checked={trimMargins}
				onCheckedChange={(checked) => onChange({ trimMargins: checked })}
			/>
			{darkTheme && (
				<SwitchRow
					label={translate('shared.pdfReader.invertPages')}
					hint={translate('shared.pdfReader.invertPagesHint')}
					checked={invertPagesInDark}
					onCheckedChange={(checked) => onChange({ invertPagesInDark: checked })}
				/>
			)}
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

type ReadingBarProps = {
	outline: FlatOutlineEntry[]
	/** The page on screen, from 1 */
	page: number
	pageCount: number
	items: ReadingBarItem[]
	visible: boolean
	emphasized: boolean
	onClick: () => void
	label: string
}

/**
 * The PDF's reading bar: ticks where the outline's top-level chapters start, the chapter on
 * the left, 'Page 74 of 315' and the pages left in the chapter (to the next top-level entry)
 * on the right. A PDF without an outline shows the page and the percentage only.
 */
export function PdfReadingBar({
	outline,
	page,
	pageCount,
	items,
	visible,
	emphasized,
	onClick,
	label,
}: ReadingBarProps) {
	const { translate } = useTranslate()
	const clock = useClockText(items.includes('clock'))
	const total = Math.max(1, pageCount)
	const tops = outline
		.filter((entry) => entry.depth === 0 && entry.page != null)
		.map((entry) => entry.page!)
	const starts = [...new Set(tops)].sort((a, b) => a - b)
	const nextChapter = starts.find((start) => start > page)
	const chapter = chapterTitleForPage(outline, page)
	const t = (key: string, options?: Record<string, unknown>) =>
		translate(`shared.reader.progress.${key}`, options)

	const right: string[] = []
	if (items.includes('page')) right.push(t('pageOf', { page, total }))
	if (items.includes('pagesLeft') && starts.length) {
		const left = (nextChapter ?? total + 1) - page - 1
		right.push(t('leftInChapter', { count: Math.max(0, left) }))
	}
	if (items.includes('percent')) right.push(`${Math.round((page / total) * 100)}%`)
	if (items.includes('clock')) right.push(clock)
	const left = items.includes('chapter') ? chapter || undefined : undefined

	return (
		<ReadingBar
			chapters={starts.map((start) => (start - 1) / total)}
			position={pageCount > 1 ? (page - 1) / (pageCount - 1) : 1}
			left={left}
			right={right}
			onClick={onClick}
			label={[left, ...right, label].filter(Boolean).join(', ')}
			visible={visible}
			emphasized={emphasized}
			band
			className="sm:inset-x-auto sm:left-1/2 sm:w-[min(680px,calc(100%-96px))] sm:-translate-x-1/2 inset-x-[22px]"
		/>
	)
}
