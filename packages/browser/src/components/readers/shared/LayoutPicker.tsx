import { cn } from '@stump/components'

import { useTranslate } from '@/hooks/useTranslate'

import { LAYOUT_OPTIONS, type ReaderLayout, sameLayout } from './layout'
import { Segmented } from './settingsControls'

type Props = {
	value: ReaderLayout
	onChange: (layout: ReaderLayout) => void
	/** Why an option can't be picked here (it's shown dimmed with this as its tooltip), or `null` */
	disabledReason?: (option: ReaderLayout) => string | null
}

const keyOf = (layout: ReaderLayout) => `${layout.spread}-${layout.flow}`

/**
 * The page layout picker every reader shares: one or two pages, turned or scrolled. Four
 * small drawings of the page arrangement in one row; each names itself in its tooltip.
 */
export default function LayoutPicker({ value, onChange, disabledReason }: Props) {
	const { translate } = useTranslate()
	const current = LAYOUT_OPTIONS.find((option) => sameLayout(option, value))

	return (
		<Segmented
			label={translate('shared.reader.layout.label')}
			testId="reader-layout-picker"
			className="w-full"
			value={current ? keyOf(current) : null}
			onChange={(key) => {
				const option = LAYOUT_OPTIONS.find((candidate) => keyOf(candidate) === key)
				if (option) onChange(option)
			}}
			options={LAYOUT_OPTIONS.map((option) => {
				const pages = translate(`shared.reader.layout.${option.spread}`)
				const flow = translate(`shared.reader.layout.${option.flow}`)
				return {
					value: keyOf(option),
					label: `${pages}, ${flow}`,
					icon: <LayoutGlyph layout={option} />,
					disabledReason: disabledReason?.(option) ?? null,
					attributes: { 'data-layout': keyOf(option) },
				}
			})}
		/>
	)
}

/**
 * A tiny drawing of the layout: pages side by side for turned pages; a column of pages
 * running off the bottom for scrolling
 */
export function LayoutGlyph({ layout }: { layout: ReaderLayout }) {
	const columns = layout.spread === 'double' ? 2 : 1
	const page = (key: string, className?: string) => (
		<span
			key={key}
			className={cn('w-2.5 shrink-0 rounded-[1.5px] border-[1.5px] border-current', className)}
		/>
	)

	return (
		<span aria-hidden className="h-5 w-6 flex items-center justify-center overflow-hidden">
			{layout.flow === 'paged' ? (
				<span className="flex gap-px">
					{Array.from({ length: columns }, (_, i) => page(`p${i}`, 'h-3.5'))}
				</span>
			) : (
				<span className="-mb-2.5 flex flex-col gap-px self-start">
					{[0, 1].map((row) => (
						<span key={row} className="flex gap-px">
							{Array.from({ length: columns }, (_, i) => page(`r${row}-${i}`, 'h-3'))}
						</span>
					))}
				</span>
			)}
		</span>
	)
}
