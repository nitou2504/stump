import { EPUB_READER_THEMES, type EpubReaderTheme } from '@stump/client'
import { cn, RawSwitch } from '@stump/components'
import { ReadingDirection } from '@stump/graphql'
import {
	ArrowLeft,
	ArrowRight,
	Minus,
	MoveHorizontal,
	MoveVertical,
	Plus,
	Scan,
} from 'lucide-react'
import { useId } from 'react'

import { useTheme } from '@/hooks/useTheme'
import { useTranslate } from '@/hooks/useTranslate'
import { useReaderStore } from '@/stores'

import { resolveReaderTheme } from '../epub/readium/preferences'

/**
 * The small building blocks of the reader settings panel, shared by every reader so the
 * EPUB, PDF and comic panels look and behave the same: a row with a short label and its
 * control, segmented glyph buttons, a stepper, a switch row and the theme swatches.
 */

type RowProps = {
	label: string
	children: React.ReactNode
	/** A short explanation, shown as the row's tooltip */
	hint?: string
	/** A second line under the label, e.g. the name of the picked typeface */
	caption?: string
	className?: string
}

/** One line of the panel: the label on the left, the control on the right */
export function SettingRow({ label, children, hint, caption, className }: RowProps) {
	return (
		<div className={cn('gap-3 min-h-9 flex items-center', className)} title={hint}>
			<span className="w-16 gap-0.5 flex shrink-0 flex-col">
				<span className="text-xs text-muted-foreground">{label}</span>
				{caption && (
					<span className="text-[11px] leading-tight truncate text-foreground" title={caption}>
						{caption}
					</span>
				)}
			</span>
			<div className="min-w-0 flex flex-1 justify-end">{children}</div>
		</div>
	)
}

/**
 * The look of a pressed and an unpressed glyph button, shared by every control of the panel
 * (segments, typefaces, steppers) so they read the same in every page colour: the unpressed
 * glyphs in full-contrast text (never faded), the pressed one lifted onto the panel's ground
 * with an accent outline
 */
export const GLYPH_BUTTON_IDLE =
	'text-foreground/85 hover:bg-background/70 hover:text-foreground'
export const GLYPH_BUTTON_PRESSED =
	'shadow-sm bg-background text-foreground ring-1 ring-inset ring-primary/70'
/** The track the glyph buttons sit in */
export const GLYPH_TRACK = 'rounded-lg bg-foreground/[0.07]'

export type SegmentOption<T extends string | number> = {
	value: T
	/** The option's name, for its tooltip and screen readers */
	label: string
	icon: React.ReactNode
	/** Why it can't be picked here (it's shown dimmed with this as its tooltip) */
	disabledReason?: string | null
	/** Extra attributes, e.g. `{ 'data-layout': 'single-paged' }` for tests */
	attributes?: Record<string, string>
}

type SegmentedProps<T extends string | number> = {
	label: string
	value: T | null
	options: SegmentOption<T>[]
	onChange: (value: T) => void
	className?: string
	testId?: string
}

/** A row of glyph buttons, one of them pressed (a radio group) */
export function Segmented<T extends string | number>({
	label,
	value,
	options,
	onChange,
	className,
	testId,
}: SegmentedProps<T>) {
	return (
		<div
			role="radiogroup"
			aria-label={label}
			data-testid={testId}
			className={cn('gap-0.5 p-0.5 flex', GLYPH_TRACK, className)}
		>
			{options.map((option) => {
				const checked = option.value === value
				return (
					<button
						key={String(option.value)}
						type="button"
						role="radio"
						aria-checked={checked}
						aria-label={option.label}
						title={option.disabledReason ?? option.label}
						disabled={!!option.disabledReason}
						onClick={() => onChange(option.value)}
						{...option.attributes}
						className={cn(
							'h-8 min-w-9 px-2 pointer-coarse:h-10 flex flex-1 items-center justify-center rounded-md transition-colors',
							checked ? GLYPH_BUTTON_PRESSED : GLYPH_BUTTON_IDLE,
							'disabled:cursor-not-allowed disabled:opacity-35 disabled:hover:bg-transparent',
						)}
					>
						{option.icon}
					</button>
				)
			})}
		</div>
	)
}

type StepperProps = {
	label: string
	/** What the middle shows, e.g. `22` or `125%` */
	display: string
	onDecrease: () => void
	onIncrease: () => void
	canDecrease?: boolean
	canIncrease?: boolean
	decreaseLabel: string
	increaseLabel: string
	/** Glyphs for the two ends; a minus and a plus without them */
	decreaseIcon?: React.ReactNode
	increaseIcon?: React.ReactNode
	/** Tapping the value, e.g. to reset a zoom */
	onReset?: () => void
	resetLabel?: string
	testId?: string
}

/** Smaller ← value → larger, as one compact control */
export function Stepper({
	label,
	display,
	onDecrease,
	onIncrease,
	canDecrease = true,
	canIncrease = true,
	decreaseLabel,
	increaseLabel,
	decreaseIcon,
	increaseIcon,
	onReset,
	resetLabel,
	testId,
}: StepperProps) {
	const buttonClass = cn(
		'h-8 w-11 pointer-coarse:h-10 flex items-center justify-center rounded-md transition-colors disabled:opacity-35 disabled:hover:bg-transparent',
		GLYPH_BUTTON_IDLE,
	)
	return (
		<div
			role="group"
			aria-label={label}
			data-testid={testId}
			className={cn('p-0.5 flex w-full items-center', GLYPH_TRACK)}
		>
			<button
				type="button"
				className={buttonClass}
				aria-label={decreaseLabel}
				title={decreaseLabel}
				disabled={!canDecrease}
				onClick={onDecrease}
			>
				{decreaseIcon ?? <Minus className="h-3.5 w-3.5" />}
			</button>
			{onReset ? (
				<button
					type="button"
					className="h-8 text-sm pointer-coarse:h-10 flex-1 rounded-md text-center tabular-nums hover:bg-background/60"
					onClick={onReset}
					title={resetLabel}
					aria-label={`${display}${resetLabel ? `, ${resetLabel}` : ''}`}
				>
					{display}
				</button>
			) : (
				<span className="text-sm flex-1 text-center tabular-nums" aria-live="polite">
					{display}
				</span>
			)}
			<button
				type="button"
				className={buttonClass}
				aria-label={increaseLabel}
				title={increaseLabel}
				disabled={!canIncrease}
				onClick={onIncrease}
			>
				{increaseIcon ?? <Plus className="h-3.5 w-3.5" />}
			</button>
		</div>
	)
}

type SwitchRowProps = {
	label: string
	checked: boolean
	onCheckedChange: (checked: boolean) => void
	hint?: string
}

/** A label and a small switch on one line */
export function SwitchRow({ label, checked, onCheckedChange, hint }: SwitchRowProps) {
	const id = useId()
	return (
		<div className="gap-3 min-h-9 flex items-center justify-between" title={hint}>
			<label htmlFor={id} className="text-sm cursor-pointer text-foreground/90">
				{label}
			</label>
			<RawSwitch id={id} checked={checked} onCheckedChange={onCheckedChange} />
		</div>
	)
}

const THEME_KEYS: Record<EpubReaderTheme, string> = {
	auto: 'shared.reader.themes.auto',
	light: 'shared.reader.themes.light',
	sepia: 'shared.reader.themes.sepia',
	dark: 'shared.reader.themes.dark',
	black: 'shared.reader.themes.black',
}

/**
 * The page colours as small round swatches of paper, each showing its text colour. `auto`
 * is split between the light and dark papers: it follows the app.
 */
export function ThemeSwatches() {
	const { translate } = useTranslate()
	const { isDarkVariant } = useTheme()
	const theme = useReaderStore((state) => state.settings.theme ?? 'auto')
	const setSettings = useReaderStore((state) => state.setSettings)

	return (
		<div
			className="gap-1.5 flex items-center"
			role="radiogroup"
			aria-label={translate('shared.reader.settings.theme')}
			data-testid="reader-theme-swatches"
		>
			{EPUB_READER_THEMES.map((option) => {
				const isActive = theme === option
				const label = translate(THEME_KEYS[option])
				const colors = resolveReaderTheme(option, isDarkVariant)
				const auto = option === 'auto'
				return (
					<button
						key={option}
						type="button"
						role="radio"
						aria-checked={isActive}
						aria-label={label}
						title={auto ? `${label}: ${translate('shared.reader.themes.autoHint')}` : label}
						onClick={() => setSettings({ theme: option })}
						data-theme-option={option}
						className={cn(
							'size-8 font-serif pointer-coarse:size-10 relative flex shrink-0 items-center justify-center rounded-full text-[11px] transition-shadow',
							// the Auto swatch draws its own edge (an SVG circle), the papers a border
							!auto && 'border border-foreground/15',
							isActive
								? 'ring-2 ring-primary ring-offset-2 ring-offset-background'
								: 'hover:ring-1 hover:ring-foreground/30',
						)}
						style={auto ? undefined : { backgroundColor: colors.background, color: colors.text }}
					>
						{auto ? <AutoPaperGlyph /> : <span aria-hidden>Aa</span>}
					</button>
				)
			})}
		</div>
	)
}

/**
 * The Auto swatch: one round paper cut diagonally, light on one half and dark on the other,
 * with `Aa` running across the cut in each half's own ink (what the page does: it follows the
 * app between the two). The papers are one SVG, so the halves meet cleanly at the circle's
 * edge; the letters are the same text as the other swatches', clipped to each half.
 */
export function AutoPaperGlyph() {
	const light = resolveReaderTheme('light', false)
	const dark = resolveReaderTheme('dark', true)
	// the cut runs from the top-right corner to the bottom-left one: light above, dark below
	const lightHalf = 'polygon(0 0, 100% 0, 0 100%)'
	const darkHalf = 'polygon(100% 0, 100% 100%, 0 100%)'
	const letters = (color: string, clipPath: string) => (
		<span className="inset-0 absolute flex items-center justify-center" style={{ color, clipPath }}>
			Aa
		</span>
	)
	return (
		<span
			aria-hidden
			className="inset-0 absolute overflow-hidden rounded-full"
			data-testid="auto-paper-glyph"
		>
			<svg viewBox="0 0 32 32" className="inset-0 absolute size-full">
				<path d="M0 0 L32 0 L0 32 Z" fill={light.background} />
				<path d="M32 0 L32 32 L0 32 Z" fill={dark.background} />
				{/* the same hairline edge as the other papers */}
				<circle
					cx="16"
					cy="16"
					r="15.5"
					fill="none"
					stroke="currentColor"
					strokeOpacity="0.15"
					className="text-foreground"
				/>
			</svg>
			{letters(light.text, lightHalf)}
			{letters(dark.text, darkHalf)}
		</span>
	)
}

type DirectionRowProps = {
	value: ReadingDirection | undefined
	onChange: (value: ReadingDirection) => void
}

/** Left to right or right to left (manga), as two arrows */
export function DirectionRow({ value, onChange }: DirectionRowProps) {
	const { translate } = useTranslate()
	const label = translate('shared.reader.settings.direction')
	return (
		<SettingRow label={label}>
			<Segmented
				label={label}
				testId="reader-direction"
				className="w-full"
				value={value ?? ReadingDirection.Ltr}
				onChange={onChange}
				options={[
					{
						value: ReadingDirection.Ltr,
						label: translate('shared.reader.direction.ltr'),
						icon: <ArrowRight className="h-4 w-4" />,
					},
					{
						value: ReadingDirection.Rtl,
						label: translate('shared.reader.direction.rtl'),
						icon: <ArrowLeft className="h-4 w-4" />,
					},
				]}
			/>
		</SettingRow>
	)
}

export type FitGlyphMode = 'width' | 'height' | 'page' | 'original'

/**
 * How a page fits the screen: as wide as the screen (↔), as tall as it (↕), wholly inside
 * it (the corners), or at its original size (1:1)
 */
export function FitGlyph({ mode }: { mode: FitGlyphMode }) {
	switch (mode) {
		case 'width':
			return <MoveHorizontal aria-hidden className="h-4 w-4" />
		case 'height':
			return <MoveVertical aria-hidden className="h-4 w-4" />
		case 'page':
			return <Scan aria-hidden className="h-4 w-4" />
		case 'original':
			return (
				<span aria-hidden className="font-medium text-[11px] leading-none tabular-nums">
					1:1
				</span>
			)
	}
}
