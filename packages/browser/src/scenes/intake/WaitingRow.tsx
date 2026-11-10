import { Badge, Button } from '@stump/components'
import { IntakeKind, IntakeState } from '@stump/graphql'
import { formatDistanceToNow } from 'date-fns'
import { Loader2, Undo2 } from 'lucide-react'

import { FormatIcon } from './IntakeItemCard'
import type { IntakeItemData } from './queries'

type Props = {
	item: IntakeItemData
	/** Takes the book back from the approval queue, to change it */
	onWithdraw: (ids: string[]) => void
	disabled?: boolean
}

/**
 * A book the uploader can't change now: waiting for an approver (it can be taken back) or being
 * written into the library after an approval. Nothing of where it goes: that is the approver's
 */
export default function WaitingRow({ item, onWithdraw, disabled }: Props) {
	const isSeries = item.kind === IntakeKind.Series
	const meta = item.decision?.metadata
	const title = (isSeries || item.decision?.seriesKind ? meta?.series : meta?.title) || item.name
	const filing = item.state === IntakeState.Filing

	return (
		<li className="gap-3 px-3 py-2.5 flex items-center">
			<FormatIcon
				format={item.files[0]?.format}
				series={isSeries}
				className="h-4 w-4 shrink-0 text-muted-foreground"
			/>
			<div className="min-w-0 flex flex-1 flex-col">
				<span className="text-sm font-medium truncate">{title}</span>
				<span className="text-xs truncate text-muted-foreground">
					{item.libraryName}
					{item.submittedAt &&
						` · sent ${formatDistanceToNow(new Date(item.submittedAt), { addSuffix: true })}`}
				</span>
			</div>
			{filing ? (
				<Badge variant="success" size="xs" className="gap-1 flex items-center">
					<Loader2 className="h-3 w-3 animate-spin" aria-hidden /> being added
				</Badge>
			) : (
				<Button
					variant="ghost"
					size="sm"
					disabled={disabled}
					onClick={() => onWithdraw([item.id])}
					title="Take it back to change it"
				>
					<Undo2 /> Take back
				</Button>
			)}
		</li>
	)
}
