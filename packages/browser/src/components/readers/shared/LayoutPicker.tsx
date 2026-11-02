import { cn } from '@stump/components'

import { useTranslate } from '@/hooks/useTranslate'

import { LAYOUT_OPTIONS, type ReaderLayout, sameLayout } from './layout'

type Props = {
	value: ReaderLayout
	onChange: (layout: ReaderLayout) => void
	/** Why an option can't be picked here (it's shown disabled with this hint), or `null` */
	disabledReason?: (option: ReaderLayout) => string | null
}

/**
 * The page layout picker every reader shares: one or two pages, turned or scrolled, as a
 * 2×2 grid of small drawings of the page arrangement.
 */
export default function LayoutPicker({ value, onChange, disabledReason }: Props) {
	const { translate } = useTranslate()
	const reasons = LAYOUT_OPTIONS.map((option) => disabledReason?.(option) ?? null)
	const hints = [...new Set(reasons.filter((reason): reason is string => !!reason))]

	return (
		<div>
			<div
				role="radiogroup"
				aria-label={translate('shared.reader.layout.label')}
				className="gap-2 grid grid-cols-2"
				data-testid="reader-layout-picker"
			>
				{LAYOUT_OPTIONS.map((option, index) => {
					const reason = reasons[index]
					const checked = sameLayout(option, value)
					const pages = translate(`shared.reader.layout.${option.spread}`)
					const flow = translate(`shared.reader.layout.${option.flow}`)
					return (
						<button
							key={`${option.spread}-${option.flow}`}
							type="button"
							role="radio"
							aria-checked={checked}
							aria-label={`${pages}, ${flow}`}
							title={reason ?? `${pages}, ${flow}`}
							disabled={!!reason}
							onClick={() => onChange(option)}
							data-layout={`${option.spread}-${option.flow}`}
							className={cn(
								'gap-3 px-3 py-2.5 flex items-center rounded-lg border-2 text-left transition-colors',
								checked
									? 'border-primary bg-background'
									: 'border-border bg-background/60 hover:border-muted-foreground',
								'disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:border-border',
							)}
						>
							<LayoutGlyph layout={option} active={checked} />
							<span className="min-w-0 flex flex-col">
								<span className="text-sm font-medium leading-tight">{pages}</span>
								<span className="text-xs leading-tight text-muted-foreground">{flow}</span>
							</span>
						</button>
					)
				})}
			</div>
			{hints.map((hint) => (
				<p key={hint} className="mt-1.5 text-xs text-muted-foreground">
					{hint}
				</p>
			))}
		</div>
	)
}

/**
 * A tiny drawing of the layout: pages side by side for turned pages; a column of pages
 * running off the bottom for scrolling
 */
function LayoutGlyph({ layout, active }: { layout: ReaderLayout; active: boolean }) {
	const pageClass = cn('rounded-[2px] border', active ? 'border-primary bg-primary/15' : 'border-muted-foreground/60 bg-muted')
	const columns = layout.spread === 'double' ? 2 : 1
	const page = (key: string) => <span key={key} className={cn(pageClass, 'h-6 w-4 shrink-0')} />

	return (
		<span
			aria-hidden
			className="h-9 w-12 p-1 relative flex shrink-0 justify-center overflow-hidden rounded-md bg-foreground/5"
		>
			{layout.flow === 'paged' ? (
				<span className="gap-0.5 flex items-center">{Array.from({ length: columns }, (_, i) => page(`p${i}`))}</span>
			) : (
				<span className="gap-0.5 flex flex-col">
					{[0, 1].map((row) => (
						<span key={row} className="gap-0.5 flex">
							{Array.from({ length: columns }, (_, i) => page(`r${row}-${i}`))}
						</span>
					))}
				</span>
			)}
		</span>
	)
}
