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
	className?: string
}

/** One line of the panel: the label on the left, the control on the right */
export function SettingRow({ label, children, hint, className }: RowProps) {
	return (
		<div className={cn('gap-3 min-h-9 flex items-center', className)} title={hint}>
			<span className="w-16 text-xs shrink-0 text-muted-foreground">{label}</span>
			<div className="min-w-0 flex flex-1 justify-end">{children}</div>
		</div>
	)
}

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
			className={cn('gap-0.5 p-0.5 flex rounded-lg bg-foreground/6', className)}
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
							checked
								? 'shadow-sm bg-background text-foreground'
								: 'text-foreground/65 hover:bg-background/50 hover:text-foreground',
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
	const buttonClass =
		'h-8 w-11 pointer-coarse:h-10 flex items-center justify-center rounded-md text-foreground/80 transition-colors hover:bg-background/60 hover:text-foreground disabled:opacity-35 disabled:hover:bg-transparent'
	return (
		<div
			role="group"
			aria-label={label}
			data-testid={testId}
			className="p-0.5 flex w-full items-center rounded-lg bg-foreground/6"
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
				const light = resolveReaderTheme('light', false)
				const dark = resolveReaderTheme('dark', true)
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
							'size-8 font-serif pointer-coarse:size-10 relative flex shrink-0 items-center justify-center overflow-hidden rounded-full border text-[11px] transition-shadow',
							isActive
								? 'border-transparent ring-2 ring-primary ring-offset-2 ring-offset-background'
								: 'border-foreground/15 hover:ring-1 hover:ring-foreground/30',
						)}
						style={
							auto
								? {
										background: `linear-gradient(135deg, ${light.background} 0 50%, ${dark.background} 50% 100%)`,
									}
								: { backgroundColor: colors.background, color: colors.text }
						}
					>
						{auto ? (
							// the light paper's text on its half, the dark paper's on the other
							<span aria-hidden className="inset-0 absolute">
								<span className="absolute top-[16%] left-[22%]" style={{ color: light.text }}>
									A
								</span>
								<span className="absolute right-[24%] bottom-[18%]" style={{ color: dark.text }}>
									a
								</span>
							</span>
						) : (
							<span aria-hidden>Aa</span>
						)}
					</button>
				)
			})}
		</div>
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
