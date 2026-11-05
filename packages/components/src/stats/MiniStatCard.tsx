import { LucideIcon } from 'lucide-react'

export type MiniStatCardProps = {
	icon?: LucideIcon
	value: string | number
	suffix?: string
	/** A tooltip, e.g. the full name of an abbreviated value */
	title?: string
}

// TODO: yoinked from expo but ignored colors for now, put somewhere else!

export function MiniStatCard({ icon: Icon, value, suffix, title }: MiniStatCardProps) {
	return (
		<div
			className="gap-1.5 px-1.5 py-1 flex shrink-0 items-center rounded-lg bg-muted"
			title={title}
		>
			{Icon && (
				<div className="h-5 w-5 flex shrink-0 items-center justify-center rounded-md bg-muted-foreground/15">
					<Icon className="h-3 w-3 text-muted-foreground" />
				</div>
			)}
			<span className="text-xs font-semibold text-foreground tabular-nums">{value}</span>
			{suffix && <span className="text-xs font-medium text-foreground opacity-50">{suffix}</span>}
		</div>
	)
}
