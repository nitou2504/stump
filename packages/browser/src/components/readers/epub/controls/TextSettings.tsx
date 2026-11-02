import { DEFAULT_EPUB_FONT_SIZE, type EpubTextAlign } from '@stump/client'
import { cn } from '@stump/components'
import { isSupportedFont } from '@stump/sdk'
import { AlignJustify, AlignLeft, BookOpen } from 'lucide-react'
import { useEffect, useRef } from 'react'

import { useTranslate } from '@/hooks/useTranslate'
import { useBookPreferences } from '@/scenes/book/reader/useBookPreferences'
import { SUPPORTED_FONT_OPTIONS } from '@/scenes/settings/app/preferences/FontSelect'
import { useReaderStore } from '@/stores'

import {
	CELL,
	CHOSEN,
	DirectionRow,
	FOCUS,
	SegmentedTrio,
	SettingRow,
	Stepper,
	TRACK,
} from '../../shared/settingsControls'
import { useEpubReaderContext } from '../context'

/** The font size's bounds, in CSS pixels */
export const MIN_FONT_SIZE = 10
export const MAX_FONT_SIZE = 48

/** Line spacing: tight, normal and loose (Readium's line height) */
export const LINE_SPACINGS = [1.2, 1.5, 1.8] as const
/** Page margins: narrow, normal and wide (Readium's page gutter multiplier) */
export const PAGE_MARGINS = [0.5, 1, 1.5] as const

/** The option closest to a stored value (older versions stored other steps) */
export function nearestOption<T extends number>(options: readonly T[], value: number): T {
	let best = options[0]!
	for (const option of options) {
		if (Math.abs(option - value) < Math.abs(best - value)) best = option
	}
	return best
}

export function clampFontSize(size: number): number {
	return Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, Math.round(size)))
}

/** A typeface's short name for its chip (the full name is the tooltip) */
export function shortFontName(label: string): string {
	const words = label.split(' ')
	// 'Libre Baskerville', 'Hina Mincho': the second word is the one people know
	if ((words[0] === 'Libre' || words[0] === 'Hina') && words[1]) return words[1]
	return words[0] ?? label
}

/**
 * The EPUB reader's own settings, compact like the `Aa` panel of reading apps: the
 * typefaces as `Aa` samples in each face (every one in view), the size as a small A and a
 * big A, then spacing, margins and alignment as three small glyph switches in one row.
 */
export default function TextSettings() {
	const { translate } = useTranslate()
	const {
		readerMeta: { bookEntity: book },
	} = useEpubReaderContext()
	const {
		bookPreferences: {
			fontSize = DEFAULT_EPUB_FONT_SIZE,
			fontFamily,
			lineHeight = 1.5,
			pageMargins = 1,
		},
		setBookPreferences,
	} = useBookPreferences({ book })
	const textAlign = useReaderStore((state) => state.settings.textAlign ?? 'auto')
	const setSettings = useReaderStore((state) => state.setSettings)
	const t = (key: string) => translate(`shared.reader.settings.${key}`)

	return (
		<>
			<FontChips
				value={fontFamily}
				onChange={(font) => setBookPreferences({ fontFamily: font })}
				label={t('font')}
				bookLabel={t('bookFont')}
				bookName={t('bookFontName')}
			/>

			<SettingRow label={t('size')}>
				<Stepper
					label={t('size')}
					testId="reader-font-size"
					display={String(fontSize)}
					decreaseLabel={t('smaller')}
					increaseLabel={t('larger')}
					decreaseIcon={
						<span className="font-medium font-literata text-[13px] leading-none">A</span>
					}
					increaseIcon={
						<span className="font-medium font-literata text-[20px] leading-none">A</span>
					}
					canDecrease={fontSize > MIN_FONT_SIZE}
					canIncrease={fontSize < MAX_FONT_SIZE}
					onDecrease={() => setBookPreferences({ fontSize: clampFontSize(fontSize - 1) })}
					onIncrease={() => setBookPreferences({ fontSize: clampFontSize(fontSize + 1) })}
				/>
			</SettingRow>

			<SegmentedTrio
				groups={[
					{
						caption: t('spacing'),
						label: t('spacing'),
						testId: 'reader-line-spacing',
						value: nearestOption(LINE_SPACINGS, lineHeight),
						onChange: (value: number) => setBookPreferences({ lineHeight: value }),
						options: LINE_SPACINGS.map((value, index) => ({
							value,
							label: t(['spacingTight', 'spacingNormal', 'spacingLoose'][index]!),
							icon: <SpacingGlyph gap={[2, 3.5, 5][index]!} />,
						})),
					},
					{
						caption: t('margins'),
						label: t('margins'),
						testId: 'reader-margins',
						value: nearestOption(PAGE_MARGINS, pageMargins),
						onChange: (value: number) => setBookPreferences({ pageMargins: value }),
						options: PAGE_MARGINS.map((value, index) => ({
							value,
							label: t(['marginsNarrow', 'marginsNormal', 'marginsWide'][index]!),
							icon: <MarginGlyph inset={[1.5, 3, 4.5][index]!} />,
						})),
					},
					{
						caption: t('align'),
						label: translate('shared.reader.text.alignment'),
						testId: 'reader-text-align',
						value: textAlign,
						onChange: (value: EpubTextAlign) => setSettings({ textAlign: value }),
						options: [
							{
								value: 'auto',
								label: translate('shared.reader.text.alignBook'),
								icon: <BookOpen aria-hidden />,
							},
							{
								value: 'justify',
								label: translate('shared.reader.text.alignJustify'),
								icon: <AlignJustify aria-hidden />,
							},
							{
								value: 'start',
								label: translate('shared.reader.text.alignStart'),
								icon: <AlignLeft aria-hidden />,
							},
						],
					},
				]}
			/>
		</>
	)
}

/** Reading direction, for the EPUB panel's More (most books set their own) */
export function EpubReadingDirection() {
	const {
		readerMeta: { bookEntity: book },
	} = useEpubReaderContext()
	const {
		bookPreferences: { readingDirection },
		setBookPreferences,
	} = useBookPreferences({ book })
	return (
		<DirectionRow
			value={readingDirection}
			onChange={(value) => setBookPreferences({ readingDirection: value })}
		/>
	)
}

type FontChipsProps = {
	value: string | undefined
	onChange: (font: string | undefined) => void
	label: string
	/** The book's own font: its chip's tooltip */
	bookLabel: string
	/** ...and its name above the grid when it's the one in use */
	bookName: string
}

/** Faces that run large or small at the same size, set so every `Aa` looks the same size */
const CHIP_FACE_SIZE: Record<string, string> = {
	LIBREBASKERVILLE: 'text-[15.5px] pointer-coarse:text-[19px]',
	OPENDYSLEXIC: 'text-[13.5px] pointer-coarse:text-[17px]',
}

/**
 * The typefaces as `Aa` samples, each in its own face, the book's own font first (an open
 * book: its face isn't known here). The face in use is named once, above them. With a mouse
 * every face is in view (two rows of five, nothing behind a scroll); on a touch screen they
 * are one row that scrolls under the finger.
 */
function FontChips({ value, onChange, label, bookLabel, bookName }: FontChipsProps) {
	const selected = value && isSupportedFont(value.toUpperCase()) ? value.toUpperCase() : ''
	const chips = [{ value: '', label: bookLabel, fontClassName: '' }, ...SUPPORTED_FONT_OPTIONS]
	const current = chips.find((font) => font.value === selected)
	const groupRef = useRef<HTMLDivElement>(null)

	// a touch screen's row scrolls: bring the face in use into view when the panel opens
	useEffect(() => {
		const chosen = groupRef.current?.querySelector<HTMLElement>('[aria-checked="true"]')
		chosen?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' })
	}, [])

	const onKeyDown = (event: React.KeyboardEvent) => {
		const step = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 }[event.key]
		if (!step) return
		event.preventDefault()
		event.stopPropagation()
		const index = Math.max(
			0,
			chips.findIndex((font) => font.value === selected),
		)
		const next = chips[(index + step + chips.length) % chips.length]!
		onChange(next.value || undefined)
		groupRef.current
			?.querySelector<HTMLButtonElement>(`[data-font="${next.value || 'book'}"]`)
			?.focus()
	}

	return (
		<div className="flex flex-col">
			<div className="mb-1.5 gap-2 flex items-baseline justify-between">
				<span className="leading-4 font-medium text-[12px] text-muted-foreground">{label}</span>
				<span
					className="min-w-0 font-semibold truncate text-[12.5px] text-foreground"
					data-testid="reader-font-name"
				>
					{current?.value ? current.label : bookName}
				</span>
			</div>
			<div
				ref={groupRef}
				role="radiogroup"
				aria-label={label}
				data-testid="reader-font-family"
				onKeyDown={onKeyDown}
				className={cn(
					'gap-1.5 grid grid-cols-5',
					// one row that scrolls under the finger on a touch screen, past the sheet's edge
					'pointer-coarse:-mr-5 pointer-coarse:gap-2 pointer-coarse:pr-5 pointer-coarse:flex pointer-coarse:overflow-x-auto pointer-coarse:overscroll-x-contain pointer-coarse:[scrollbar-width:none] pointer-coarse:[&::-webkit-scrollbar]:hidden',
				)}
			>
				{chips.map((font) => {
					const checked = font.value === selected
					return (
						<button
							key={font.value || 'book'}
							type="button"
							role="radio"
							aria-checked={checked}
							aria-label={font.label}
							title={font.label}
							tabIndex={checked ? 0 : -1}
							data-font={font.value || 'book'}
							onClick={() => onChange(font.value || undefined)}
							className={cn(
								'flex h-[34px] shrink-0 items-center justify-center rounded-lg text-[17px] leading-none transition-[background-color,color,transform] duration-100 active:scale-[.97] motion-reduce:transition-none pointer-coarse:h-[46px] pointer-coarse:w-[58px] pointer-coarse:text-[21px]',
								'pointer-coarse:[&_svg]:size-5 [&_svg]:size-[17px]',
								checked ? CHOSEN : cn(TRACK, CELL),
								FOCUS,
							)}
						>
							{font.value ? (
								<span aria-hidden className={cn(font.fontClassName, CHIP_FACE_SIZE[font.value])}>
									Aa
								</span>
							) : (
								<BookOpen aria-hidden />
							)}
						</button>
					)
				})}
			</div>
		</div>
	)
}

/** Three lines with a gap between them: the line spacing */
function SpacingGlyph({ gap }: { gap: number }) {
	const top = 9 - gap - 1
	return (
		<svg aria-hidden width="18" height="18" viewBox="0 0 18 18" fill="currentColor">
			{[top, 9 - 0.75, 9 + gap - 0.5].map((y) => (
				<rect key={y} x="2" y={y} width="14" height="1.5" rx="0.75" />
			))}
		</svg>
	)
}

/** A page with lines of text inside its margins */
function MarginGlyph({ inset }: { inset: number }) {
	return (
		<svg aria-hidden width="16" height="18" viewBox="0 0 16 18" fill="none" stroke="currentColor">
			<rect x="0.75" y="0.75" width="14.5" height="16.5" rx="1.5" strokeWidth="1.5" />
			{[5, 8, 11, 14].map((y) => (
				<line
					key={y}
					x1={1 + inset}
					x2={15 - inset}
					y1={y - 1}
					y2={y - 1}
					strokeWidth="1.25"
					strokeLinecap="round"
				/>
			))}
		</svg>
	)
}
