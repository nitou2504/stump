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
import { useId, useRef } from 'react'

import { useTheme } from '@/hooks/useTheme'
import { useTranslate } from '@/hooks/useTranslate'
import { useReaderStore } from '@/stores'

import { resolveReaderTheme } from '../epub/readium/preferences'

/**
 * The small building blocks of the reader settings panel, shared by every reader so the
 * EPUB, PDF and comic panels look and behave the same ("ink and paper"): the panel is
 * printed with the book's own ink on the book's own paper. An option not chosen is drawn in
 * ink at 80% on a track of ink at 7%; the chosen one is inked solid, its glyph knocked out
 * in the paper's colour. Orange only marks keyboard focus and switches that are on.
 */

/** The track options sit in: ink at 7% (never `muted`, which vanishes in dark and black) */
export const TRACK = 'bg-foreground/7'
/** An option not chosen: ink at 80%, darker under a mouse */
export const CELL =
	'text-foreground/80 pointer-fine:hover:bg-foreground/7 pointer-fine:hover:text-foreground'
/** The chosen option: solid ink, the glyph in the paper's colour */
export const CHOSEN = 'bg-foreground text-popover'
/** Keyboard focus: the panel's only orange, a 2px ring with a gap in the panel's colour */
export const FOCUS =
	'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-popover'
/** A label at the start of a row, and the captions under the controls */
const LABEL = 'text-[12px] leading-4 font-medium text-muted-foreground'

type RowProps = {
	label: string
	children: React.ReactNode
	/** A short explanation, shown as the row's tooltip */
	hint?: string
	className?: string
}

/** One line of the panel: a short label, then its control filling the rest */
export function SettingRow({ label, children, hint, className }: RowProps) {
	return (
		<div
			className={cn('gap-2.5 pointer-coarse:min-h-11 flex min-h-[34px] items-center', className)}
			title={hint}
		>
			<span className={cn(LABEL, 'w-[58px] shrink-0 pointer-coarse:w-[62px]')}>{label}</span>
			<div className="min-w-0 flex flex-1 justify-end">{children}</div>
		</div>
	)
}

export type SegmentOption<T extends string | number> = {
	value: T
	/** The option's name, for its tooltip and screen readers */
	label: string
	/** A glyph, or a short word for the `text` variant */
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
	/** `text`: the options are short words (e.g. Off · Number · Bar) instead of glyphs */
	variant?: 'glyph' | 'text'
	className?: string
	testId?: string
}

/**
 * A row of options, one of them chosen (a radio group). The arrow keys move the choice
 * (Home and End jump to the ends), and only the chosen option is in the tab order.
 */
export function Segmented<T extends string | number>({
	label,
	value,
	options,
	onChange,
	variant = 'glyph',
	className,
	testId,
}: SegmentedProps<T>) {
	const groupRef = useRef<HTMLDivElement>(null)
	const enabled = options.filter((option) => !option.disabledReason)
	const focusable = options.find((option) => option.value === value && !option.disabledReason)
		? value
		: enabled[0]?.value

	const onKeyDown = (event: React.KeyboardEvent) => {
		const keys: Record<string, number | 'first' | 'last'> = {
			ArrowLeft: -1,
			ArrowUp: -1,
			ArrowRight: 1,
			ArrowDown: 1,
			Home: 'first',
			End: 'last',
		}
		const move = keys[event.key]
		if (move === undefined || !enabled.length) return
		event.preventDefault()
		// a keyboard choice stays in the panel's control (not the reader's page turns)
		event.stopPropagation()
		const index = enabled.findIndex((option) => option.value === value)
		const next =
			move === 'first'
				? enabled[0]
				: move === 'last'
					? enabled[enabled.length - 1]
					: enabled[(Math.max(0, index) + move + enabled.length) % enabled.length]
		if (!next) return
		onChange(next.value)
		const button = groupRef.current?.querySelector<HTMLButtonElement>(
			`[data-option="${String(next.value)}"]`,
		)
		button?.focus()
	}

	return (
		<div
			ref={groupRef}
			role="radiogroup"
			aria-label={label}
			data-testid={testId}
			onKeyDown={onKeyDown}
			className={cn('gap-0.5 p-0.5 flex rounded-[10px]', TRACK, className)}
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
						tabIndex={option.value === focusable ? 0 : -1}
						data-option={String(option.value)}
						onClick={() => onChange(option.value)}
						{...option.attributes}
						className={cn(
							'pointer-coarse:h-10 min-w-7 flex h-[30px] flex-1 items-center justify-center rounded-lg transition-[background-color,color,transform] duration-100 active:scale-[.97] motion-reduce:transition-none',
							'[&_svg]:size-[17px]',
							variant === 'text' && 'px-2 font-medium text-[12px] whitespace-nowrap',
							checked ? CHOSEN : CELL,
							'disabled:cursor-not-allowed disabled:opacity-30 disabled:active:scale-100 pointer-fine:disabled:hover:bg-transparent',
							FOCUS,
						)}
					>
						{option.icon}
					</button>
				)
			})}
		</div>
	)
}

type TrioGroup<T extends string | number> = Omit<SegmentedProps<T>, 'variant'> & {
	/** The short word under the group, e.g. Spacing */
	caption: string
}

/**
 * Three small choices side by side with a caption under each (spacing, margins and
 * alignment): one row instead of three labelled ones
 */
export function SegmentedTrio({
	groups,
}: {
	// the three groups hold different value types
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	groups: TrioGroup<any>[]
}) {
	return (
		<div className="gap-2 grid grid-cols-3">
			{groups.map(({ caption, ...group }) => (
				<div key={caption} className="min-w-0 flex flex-col">
					<Segmented {...group} className={cn('w-full', group.className)} />
					<span className={cn(LABEL, 'mt-1 text-center text-[11px] leading-[14px]')}>
						{caption}
					</span>
				</div>
			))}
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
		'pointer-coarse:h-10 pointer-coarse:w-14 flex h-[30px] w-[46px] shrink-0 items-center justify-center rounded-lg text-foreground transition-[background-color,transform] duration-100 active:scale-[.97] motion-reduce:transition-none',
		'pointer-fine:hover:bg-foreground/7 [&_svg]:size-[15px]',
		'disabled:cursor-not-allowed disabled:opacity-30 disabled:active:scale-100 pointer-fine:disabled:hover:bg-transparent',
		FOCUS,
	)
	const value = 'text-[13px] font-semibold tabular-nums text-foreground'
	return (
		<div
			role="group"
			aria-label={label}
			data-testid={testId}
			className={cn(
				'p-0.5 pointer-coarse:h-11 flex h-[34px] w-full items-center rounded-[10px]',
				TRACK,
			)}
		>
			<button
				type="button"
				className={buttonClass}
				aria-label={decreaseLabel}
				title={decreaseLabel}
				disabled={!canDecrease}
				onClick={onDecrease}
			>
				{decreaseIcon ?? <Minus aria-hidden />}
			</button>
			{onReset ? (
				<button
					type="button"
					className={cn(
						'h-full flex-1 rounded-lg text-center pointer-fine:hover:bg-foreground/7',
						value,
						FOCUS,
					)}
					onClick={onReset}
					title={resetLabel}
					aria-label={`${display}${resetLabel ? `, ${resetLabel}` : ''}`}
				>
					{display}
				</button>
			) : (
				<span className={cn('flex-1 text-center', value)} aria-live="polite">
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
				{increaseIcon ?? <Plus aria-hidden />}
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
		<div
			className="gap-3 pointer-coarse:min-h-11 flex min-h-[34px] items-center justify-between"
			title={hint}
		>
			<label htmlFor={id} className="cursor-pointer text-[13px] text-foreground">
				{label}
			</label>
			<RawSwitch id={id} checked={checked} onCheckedChange={onCheckedChange} />
		</div>
	)
}

type ChipOption<T extends string> = { value: T; label: string }

/** Options that can each be on or off, as small pills (e.g. what the reading bar shows) */
export function ToggleChips<T extends string>({
	label,
	options,
	value,
	onChange,
	testId,
}: {
	label: string
	options: ChipOption<T>[]
	value: T[]
	onChange: (value: T[]) => void
	testId?: string
}) {
	return (
		<div role="group" aria-label={label} className="gap-1.5 flex flex-wrap" data-testid={testId}>
			{options.map((option) => {
				const on = value.includes(option.value)
				return (
					<button
						key={option.value}
						type="button"
						aria-pressed={on}
						data-option={option.value}
						onClick={() =>
							onChange(
								on
									? value.filter((item) => item !== option.value)
									: // kept in the options' order
										options
											.map((candidate) => candidate.value)
											.filter((item) => item === option.value || value.includes(item)),
							)
						}
						className={cn(
							'gap-1.5 px-2.5 pointer-coarse:h-9 font-medium inline-flex h-[26px] items-center rounded-full text-[12px] transition-colors duration-100',
							on ? CHOSEN : cn(TRACK, CELL),
							FOCUS,
						)}
					>
						{on && (
							<svg aria-hidden viewBox="0 0 16 16" className="size-[13px]" fill="none">
								<path
									d="M3.5 8.5 6.5 11.5 12.5 4.5"
									stroke="currentColor"
									strokeWidth="1.75"
									strokeLinecap="round"
									strokeLinejoin="round"
								/>
							</svg>
						)}
						{option.label}
					</button>
				)
			})}
		</div>
	)
}

/** A small heading inside the panel (e.g. "The bar shows") */
export function PanelSubheading({ children }: { children: React.ReactNode }) {
	return <p className={cn(LABEL, 'mt-1 text-[11px]')}>{children}</p>
}

const THEME_KEYS: Record<EpubReaderTheme, string> = {
	auto: 'shared.reader.themes.auto',
	light: 'shared.reader.themes.light',
	sepia: 'shared.reader.themes.sepia',
	dark: 'shared.reader.themes.dark',
	black: 'shared.reader.themes.black',
}

/**
 * The page colours as small sheets of each paper, each showing "Aa" in its own ink. `auto`
 * is split down the middle, a light half and a dark one: it follows the app.
 */
export function ThemeSwatches() {
	const { translate } = useTranslate()
	const { isDarkVariant } = useTheme()
	const theme = useReaderStore((state) => state.settings.theme ?? 'auto')
	const setSettings = useReaderStore((state) => state.setSettings)
	const groupRef = useRef<HTMLDivElement>(null)

	const choose = (option: EpubReaderTheme) => setSettings({ theme: option })
	const onKeyDown = (event: React.KeyboardEvent) => {
		const step = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 }[event.key]
		if (!step) return
		event.preventDefault()
		event.stopPropagation()
		const index = EPUB_READER_THEMES.indexOf(theme)
		const next =
			EPUB_READER_THEMES[(index + step + EPUB_READER_THEMES.length) % EPUB_READER_THEMES.length]!
		choose(next)
		groupRef.current?.querySelector<HTMLButtonElement>(`[data-theme-option="${next}"]`)?.focus()
	}

	return (
		<div
			ref={groupRef}
			className="gap-2 pointer-coarse:gap-2.5 flex items-center"
			role="radiogroup"
			aria-label={translate('shared.reader.settings.theme')}
			data-testid="reader-theme-swatches"
			onKeyDown={onKeyDown}
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
						tabIndex={isActive ? 0 : -1}
						title={auto ? `${label}: ${translate('shared.reader.themes.autoHint')}` : label}
						onClick={() => choose(option)}
						data-theme-option={option}
						className={cn(
							'w-10 font-medium relative grid h-[30px] shrink-0 place-items-center overflow-hidden rounded-lg font-literata text-[14px] leading-none tracking-[-0.01em] transition-shadow duration-100 pointer-coarse:h-[38px] pointer-coarse:w-[52px] pointer-coarse:rounded-[10px] pointer-coarse:text-[16px]',
							// the paper's edge, drawn over its contents so the split Auto keeps it too
							'after:inset-0 after:pointer-events-none after:absolute after:rounded-[inherit] after:shadow-[inset_0_0_0_1px_color-mix(in_srgb,var(--foreground)_16%,transparent)]',
							// chosen: an ink ring outside a gap in the panel's colour
							isActive && 'shadow-[0_0_0_2px_var(--popover),0_0_0_3.5px_var(--foreground)]',
							'focus-visible:shadow-[0_0_0_2px_var(--popover),0_0_0_4px_var(--primary)] focus-visible:outline-none',
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
 * The Auto paper: two halves side by side, a light page with a dark `A` and a dark page
 * with a light `a` (what the page does: it follows the app between the two)
 */
export function AutoPaperGlyph() {
	const light = resolveReaderTheme('light', false)
	const dark = resolveReaderTheme('dark', true)
	return (
		<span aria-hidden className="inset-0 absolute flex" data-testid="auto-paper-glyph">
			<span
				className="flex flex-1 items-center justify-end pr-px"
				style={{ backgroundColor: light.background, color: light.text }}
			>
				A
			</span>
			<span
				className="flex flex-1 items-center justify-start pl-px"
				style={{ backgroundColor: dark.background, color: dark.text }}
			>
				a
			</span>
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
						icon: <ArrowRight aria-hidden />,
					},
					{
						value: ReadingDirection.Rtl,
						label: translate('shared.reader.direction.rtl'),
						icon: <ArrowLeft aria-hidden />,
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
			return <MoveHorizontal aria-hidden />
		case 'height':
			return <MoveVertical aria-hidden />
		case 'page':
			return <Scan aria-hidden />
		case 'original':
			return (
				<span aria-hidden className="font-semibold text-[11.5px] leading-none tabular-nums">
					1:1
				</span>
			)
	}
}
