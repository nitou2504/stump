import { Button, Dialog } from '@stump/components'

import type { IntakeItemData } from '@/scenes/intake/queries'

type Props = {
	open: boolean
	/** The books that would be approved: the ones on the page with nothing left to fix */
	items: IntakeItemData[]
	/** How many on the page are left out because something is left to fix */
	skipped: number
	onClose: () => void
	onConfirm: () => void
	isPending?: boolean
}

const titleOf = (item: IntakeItemData) =>
	(item.decision?.seriesKind || item.files.length > 1
		? item.decision?.metadata.series
		: item.decision?.metadata.title) || item.name

/** Asks before approving a whole page: each book goes into the library as it is shown */
export default function ApproveAllDialog({
	open,
	items,
	skipped,
	onClose,
	onConfirm,
	isPending,
}: Props) {
	return (
		<Dialog open={open} onOpenChange={(next) => !next && !isPending && onClose()}>
			<Dialog.Content size="md" className="max-h-[min(90dvh,40rem)] overflow-hidden">
				<Dialog.Header className="pr-8">
					<Dialog.Title>
						Approve {items.length} {items.length === 1 ? 'book' : 'books'}
					</Dialog.Title>
					<Dialog.Description>
						Each one is added to the library the way the page shows it.
						{skipped > 0 &&
							` ${skipped} ${skipped === 1 ? 'book has' : 'books have'} something to fix and stay${skipped === 1 ? 's' : ''} here.`}
					</Dialog.Description>
					<Dialog.Close />
				</Dialog.Header>

				<ul className="max-h-72 divide-y divide-border overflow-y-auto rounded-lg border border-border">
					{items.map((item) => (
						<li key={item.id} className="gap-3 px-3 py-2 flex items-baseline">
							<span className="text-sm min-w-0 flex-1 truncate">{titleOf(item)}</span>
							<span className="text-xs shrink-0 text-muted-foreground">
								{item.uploader?.username ?? ''}
							</span>
						</li>
					))}
				</ul>

				<Dialog.Footer className="gap-2 sm:flex grid grid-cols-2">
					<Button type="button" variant="outline" onClick={onClose} disabled={isPending}>
						Cancel
					</Button>
					<Button type="button" onClick={onConfirm} disabled={!items.length} isLoading={isPending}>
						Approve {items.length} {items.length === 1 ? 'book' : 'books'}
					</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog>
	)
}
