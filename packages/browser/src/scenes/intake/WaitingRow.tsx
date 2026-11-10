import { Button } from '@stump/components'
import { IntakeKind, IntakeState } from '@stump/graphql'
import { formatDistanceToNow } from 'date-fns'
import { Undo2 } from 'lucide-react'

import CoverTile from './CoverTile'
import IntakeStatusChip from './IntakeStatus'
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
			<CoverTile size="sm" title={title} format={item.files[0]?.format} series={isSeries} />
			<div className="min-w-0 gap-0.5 flex flex-1 flex-col">
				<span className="text-sm font-medium truncate">{title}</span>
				<span className="text-xs truncate text-muted-foreground">
					{item.libraryName}
					{item.submittedAt &&
						` · sent ${formatDistanceToNow(new Date(item.submittedAt), { addSuffix: true })}`}
				</span>
			</div>
			{filing ? (
				<IntakeStatusChip item={item} />
			) : (
				<>
					<IntakeStatusChip item={item} className="sm:inline-flex hidden" />
					<Button
						variant="ghost"
						size="sm"
						disabled={disabled}
						onClick={() => onWithdraw([item.id])}
						title="Take it back to change it"
					>
						<Undo2 /> Take back
					</Button>
				</>
			)}
		</li>
	)
}
