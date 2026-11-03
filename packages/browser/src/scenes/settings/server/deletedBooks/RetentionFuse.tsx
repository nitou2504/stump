import { cn } from '@stump/components'
import { format } from 'date-fns'

import { countdownLabel, daysLeft, fuseTicks, isImminent } from './countdown'

type Props = {
	/** When the automatic purge will delete the book (null: kept until deleted by hand) */
	purgeAt?: string | null
	retentionDays: number
	now: Date
	className?: string
}

/**
 * How long a book has left in Deleted books, as a fuse: one tick per day of the retention, the
 * days spent faded on the left, the days left inked up to the purge on the right. Red once the
 * purge is today or tomorrow
 */
export default function RetentionFuse({ purgeAt, retentionDays, now, className }: Props) {
	const days = daysLeft(purgeAt, now)
	const ticks = fuseTicks(days, retentionDays)
	const imminent = isImminent(days)
	const when = purgeAt ? format(new Date(purgeAt), 'EEE d MMM yyyy, HH:mm') : null

	return (
		<div
			className={cn('gap-2 flex items-center', className)}
			title={when ? `Deleted permanently by the automatic purge on ${when}` : undefined}
			data-testid="retention-fuse"
		>
			{ticks.length > 0 && (
				<span aria-hidden className="h-3 flex items-end gap-[2px]">
					{ticks.map((tick, index) => (
						<span
							key={index}
							className={cn(
								'w-[3px] rounded-[1px] transition-colors',
								tick === 'spent'
									? 'h-1.5 bg-foreground/15'
									: imminent
										? 'h-3 bg-destructive'
										: 'h-3 bg-foreground/60',
							)}
						/>
					))}
				</span>
			)}
			<span
				className={cn(
					'text-xs tabular-nums',
					imminent ? 'font-medium text-destructive' : 'text-muted-foreground',
				)}
			>
				{countdownLabel(days)}
			</span>
		</div>
	)
}
