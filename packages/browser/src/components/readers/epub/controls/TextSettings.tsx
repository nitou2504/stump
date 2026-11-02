import { DEFAULT_EPUB_FONT_SIZE, type EpubTextAlign } from '@stump/client'
import { cn } from '@stump/components'
import { isSupportedFont } from '@stump/sdk'
import { AlignJustify, AlignLeft, BookOpen } from 'lucide-react'

import { useTranslate } from '@/hooks/useTranslate'
import { useBookPreferences } from '@/scenes/book/reader/useBookPreferences'
import { SUPPORTED_FONT_OPTIONS } from '@/scenes/settings/app/preferences/FontSelect'
import { useReaderStore } from '@/stores'

import {
	DirectionRow,
	GLYPH_BUTTON_IDLE,
	GLYPH_BUTTON_PRESSED,
	GLYPH_TRACK,
	Segmented,
	SettingRow,
	Stepper,
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
 * The EPUB reader's own settings, compact like the `Aa` panel of reading apps: the size as
 * a small A and a big A, the typefaces as `Aa` samples in each face, then spacing, margins
 * and alignment as three-way glyph switches.
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
			<SettingRow label={t('size')}>
				<Stepper
					label={t('size')}
					testId="reader-font-size"
					display={String(fontSize)}
					decreaseLabel={t('smaller')}
					increaseLabel={t('larger')}
					decreaseIcon={<span className="font-serif text-[13px] leading-none">A</span>}
					increaseIcon={<span className="font-serif text-[19px] leading-none">A</span>}
					canDecrease={fontSize > MIN_FONT_SIZE}
					canIncrease={fontSize < MAX_FONT_SIZE}
					onDecrease={() => setBookPreferences({ fontSize: clampFontSize(fontSize - 1) })}
					onIncrease={() => setBookPreferences({ fontSize: clampFontSize(fontSize + 1) })}
				/>
			</SettingRow>

			<FontChips
				value={fontFamily}
				onChange={(font) => setBookPreferences({ fontFamily: font })}
				label={t('font')}
				bookLabel={t('bookFont')}
			/>

			<SettingRow label={t('spacing')}>
				<Segmented
					label={t('spacing')}
					testId="reader-line-spacing"
					className="w-full"
					value={nearestOption(LINE_SPACINGS, lineHeight)}
					onChange={(value) => setBookPreferences({ lineHeight: value })}
					options={LINE_SPACINGS.map((value, index) => ({
						value,
						label: t(['spacingTight', 'spacingNormal', 'spacingLoose'][index]!),
						icon: <SpacingGlyph gap={[2, 3.5, 5][index]!} />,
					}))}
				/>
			</SettingRow>

			<SettingRow label={t('margins')}>
				<Segmented
					label={t('margins')}
					testId="reader-margins"
					className="w-full"
					value={nearestOption(PAGE_MARGINS, pageMargins)}
					onChange={(value) => setBookPreferences({ pageMargins: value })}
					options={PAGE_MARGINS.map((value, index) => ({
						value,
						label: t(['marginsNarrow', 'marginsNormal', 'marginsWide'][index]!),
						icon: <MarginGlyph inset={[1.5, 3, 4.5][index]!} />,
					}))}
				/>
			</SettingRow>

			<SettingRow label={t('align')}>
				<Segmented<EpubTextAlign>
					label={translate('shared.reader.text.alignment')}
					testId="reader-text-align"
					className="w-full"
					value={textAlign}
					onChange={(value) => setSettings({ textAlign: value })}
					options={[
						{
							value: 'auto',
							label: translate('shared.reader.text.alignBook'),
							icon: <BookOpen className="h-4 w-4" />,
						},
						{
							value: 'justify',
							label: translate('shared.reader.text.alignJustify'),
							icon: <AlignJustify className="h-4 w-4" />,
						},
						{
							value: 'start',
							label: translate('shared.reader.text.alignStart'),
							icon: <AlignLeft className="h-4 w-4" />,
						},
					]}
				/>
			</SettingRow>
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
	bookLabel: string
}

/**
 * The typefaces as a grid of `Aa` samples, each in its own face, the book's own font first.
 * They all fit (two rows of five), so nothing hides behind a scroll; the picked face's name
 * shows under the row's label and every sample names itself in its tooltip.
 */
function FontChips({ value, onChange, label, bookLabel }: FontChipsProps) {
	const selected = value && isSupportedFont(value.toUpperCase()) ? value.toUpperCase() : ''
	const chips = [{ value: '', label: bookLabel, fontClassName: '' }, ...SUPPORTED_FONT_OPTIONS]
	const current = chips.find((font) => font.value === selected)

	return (
		<SettingRow
			label={label}
			caption={current && (current.value ? shortFontName(current.label) : current.label)}
		>
			<div
				role="radiogroup"
				aria-label={label}
				data-testid="reader-font-family"
				className={cn('gap-0.5 p-0.5 grid w-full grid-cols-5', GLYPH_TRACK)}
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
							data-font={font.value || 'book'}
							onClick={() => onChange(font.value || undefined)}
							className={cn(
								'h-8 pointer-coarse:h-10 flex items-center justify-center rounded-md transition-colors',
								checked ? GLYPH_BUTTON_PRESSED : GLYPH_BUTTON_IDLE,
							)}
						>
							{font.value ? (
								<span className={cn('text-base leading-none', font.fontClassName)}>Aa</span>
							) : (
								// the book's own font: an open book, since its face isn't known here
								<BookOpen aria-hidden className="h-4 w-4" />
							)}
						</button>
					)
				})}
			</div>
		</SettingRow>
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
