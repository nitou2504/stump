import { Badge, Button, cn, Sheet } from '@stump/components'

import type { IntakeItemData } from './queries'
import { chosenTags, destinationPaths } from './reviewState'

type Props = {
	items: IntakeItemData[]
	open: boolean
	onClose: () => void
	onConfirm: () => void
	isPending?: boolean
}

/**
 * Before "Add all ready" files several books at once: where each one goes and the tags
 * that will be written into it, with the guessed ones marked so they get a last look.
 */
export default function AcceptAllSheet({ items, open, onClose, onConfirm, isPending }: Props) {
	const guessedCount = items.reduce(
		(sum, item) => sum + chosenTags(item).filter((tag) => tag.origin === 'guessed').length,
		0,
	)

	return (
		<Sheet
			open={open}
			onClose={onClose}
			title={`Add ${items.length} ${items.length === 1 ? 'book' : 'books'} to the library`}
			description={
				guessedCount
					? `${guessedCount} ${guessedCount === 1 ? 'tag was' : 'tags were'} guessed from similar books (in italics). Check them before adding.`
					: 'Each book is written with these details and filed here.'
			}
			contentClassName="flex w-full flex-col sm:max-w-xl"
			footer={
				<div className="gap-2 flex w-full justify-end">
					<Button variant="ghost" onClick={onClose} disabled={isPending}>
						Cancel
					</Button>
					<Button onClick={onConfirm} disabled={isPending || !items.length} isLoading={isPending}>
						Add {items.length} {items.length === 1 ? 'book' : 'books'}
					</Button>
				</div>
			}
		>
			<ul className="px-4 min-h-0 flex-1 divide-y divide-border overflow-y-auto">
				{items.map((item) => {
					const meta = item.decision?.metadata
					const asSeries = !!item.decision?.seriesKind || item.files.length > 1
					const title = (asSeries ? meta?.series : meta?.title) || item.name
					const where = destinationPaths(item)
					const tags = chosenTags(item)
					return (
						<li key={item.id} className="gap-1 py-3 flex flex-col">
							<span className="font-medium">{title}</span>
							{!!meta?.authors.length && (
								<span className="text-sm text-muted-foreground">{meta.authors.join(', ')}</span>
							)}
							<span className="font-mono text-xs break-all text-muted-foreground">
								{where.length > 1
									? `${item.destination?.relativeDir}/ (${where.length} files)`
									: where[0]}
							</span>
							<div className="mt-1 gap-1 flex flex-wrap">
								{tags.length === 0 && (
									<span className="text-xs text-muted-foreground">No tags</span>
								)}
								{tags.map((tag) => (
									<Badge
										key={tag.value}
										variant={tag.origin === 'guessed' ? 'warning' : 'secondary'}
										size="xs"
										className={cn(tag.origin === 'guessed' && 'italic')}
										title={tag.origin === 'guessed' ? 'Guessed from similar books' : undefined}
									>
										{tag.value}
									</Badge>
								))}
							</div>
						</li>
					)
				})}
			</ul>
		</Sheet>
	)
}
