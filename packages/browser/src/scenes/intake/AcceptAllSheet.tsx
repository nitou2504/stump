import { Button, Sheet } from '@stump/components'
import { useMemo, useState } from 'react'

import type { IntakeItemData } from './queries'
import { destinationPaths, effectiveGenres, genreChips, toggleGenre } from './reviewState'
import { TagChip } from './TagPicker'
import { useTagReviewStore } from './tagReviewStore'

type Entry = { item: IntakeItemData; genres: string[] }

type Props = {
	items: IntakeItemData[]
	open: boolean
	onClose: () => void
	onConfirm: (entries: Entry[]) => void
	isPending?: boolean
}

/**
 * The "does this look right?" step of "Add all ready": each book's destination and the tags
 * that will be written into it (guessed ones highlighted), with every tag untickable before
 * confirming.
 */
export default function AcceptAllSheet({ items, open, onClose, onConfirm, isPending }: Props) {
	return (
		<Sheet
			open={open}
			onClose={onClose}
			title={`Add ${items.length} ${items.length === 1 ? 'book' : 'books'}`}
			description="Untick any genre that doesn’t fit."
			contentClassName="flex w-full flex-col sm:max-w-lg"
		>
			{open && (
				<AcceptAllList
					items={items}
					onClose={onClose}
					onConfirm={onConfirm}
					isPending={isPending}
				/>
			)}
		</Sheet>
	)
}

function AcceptAllList({ items, onClose, onConfirm, isPending }: Omit<Props, 'open'>) {
	const reviewed = useTagReviewStore((store) => store.reviewed)
	const [selection, setSelection] = useState<Record<string, string[]>>(() =>
		Object.fromEntries(items.map((item) => [item.id, effectiveGenres(item, !!reviewed[item.id])])),
	)
	const rows = useMemo(
		() =>
			items.map((item) => {
				const chosen = selection[item.id] ?? []
				// the tags proposed by default or chosen: what the uploader may want to untick
				const chips = genreChips(item, chosen).filter((chip) => chip.chosen || chip.preselected)
				return { chips, chosen, item }
			}),
		[items, selection],
	)
	const guessedTicked = rows.reduce(
		(sum, row) => sum + row.chips.filter((c) => c.chosen && c.origin === 'guessed').length,
		0,
	)

	return (
		<>
			<ul className="px-4 min-h-0 flex-1 divide-y divide-border overflow-y-auto">
				{rows.map(({ item, chips, chosen }) => {
					const meta = item.decision?.metadata
					const asSeries = !!item.decision?.seriesKind || item.files.length > 1
					const title = (asSeries ? meta?.series : meta?.title) || item.name
					const where = destinationPaths(item)
					return (
						<li key={item.id} className="gap-1 py-2.5 flex flex-col">
							<div className="gap-x-2 min-w-0 flex flex-wrap items-baseline">
								<span className="font-medium">{title}</span>
								{!!meta?.authors.length && (
									<span className="text-sm truncate text-muted-foreground">
										{meta.authors.join(', ')}
									</span>
								)}
							</div>
							<span
								className="font-mono text-xs truncate text-muted-foreground"
								title={where.join('\n')}
							>
								{where.length > 1
									? `${item.destination?.relativeDir}/ (${where.length} files)`
									: where[0]}
							</span>
							<div className="gap-1 flex flex-wrap">
								{chips.length === 0 && (
									<span className="text-xs text-muted-foreground">No genres</span>
								)}
								{chips.map((chip) => (
									<TagChip
										key={chip.value}
										chip={chip}
										size="sm"
										isOn={chosen.includes(chip.value)}
										disabled={isPending}
										onToggle={() =>
											setSelection((prev) => ({
												...prev,
												[item.id]: toggleGenre(prev[item.id] ?? [], chip.value),
											}))
										}
									/>
								))}
							</div>
						</li>
					)
				})}
			</ul>
			<div className="gap-2 px-4 py-3 flex items-center justify-end border-t border-border">
				{guessedTicked > 0 && (
					<span className="text-xs mr-auto text-warning">
						{guessedTicked} guessed {guessedTicked === 1 ? 'genre' : 'genres'} ticked
					</span>
				)}
				<Button variant="ghost" onClick={onClose} disabled={isPending}>
					Cancel
				</Button>
				<Button
					onClick={() => onConfirm(rows.map(({ item, chosen }) => ({ genres: chosen, item })))}
					disabled={isPending || !items.length}
					isLoading={isPending}
				>
					Add {items.length} {items.length === 1 ? 'book' : 'books'}
				</Button>
			</div>
		</>
	)
}
