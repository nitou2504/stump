import { Badge, Button, CheckBox, cn } from '@stump/components'
import { DeletedBooksQuery } from '@stump/graphql'
import { formatDistanceToNow } from 'date-fns'
import { Book, FileX2, RotateCcw, Trash2 } from 'lucide-react'
import { useState } from 'react'

import { EntityImage } from '@/components/entity'

import RetentionFuse from './RetentionFuse'

export type DeletedBookEntry = DeletedBooksQuery['deletedBooks'][number]

type Props = {
	entry: DeletedBookEntry
	selected: boolean
	onSelect: (selected: boolean) => void
	onRestore: () => void
	onPurge: () => void
	retentionDays: number
	now: Date
	busy: boolean
}

/** One book in Deleted books: what it is, where it was, who removed it, how long it has left */
export default function DeletedBookRow({
	entry,
	selected,
	onSelect,
	onRestore,
	onPurge,
	retentionDays,
	now,
	busy,
}: Props) {
	const { book } = entry
	const title = book.resolvedName
	const authors = book.metadata?.writers?.join(', ')
	const removed = formatDistanceToNow(new Date(entry.deletedAt), { addSuffix: true })

	return (
		<li
			className={cn(
				'gap-3 px-3 py-3 sm:px-4 md:flex-row md:items-start md:gap-6 flex flex-col transition-colors',
				selected ? 'bg-primary/5' : 'hover:bg-muted/40',
			)}
			data-testid="deleted-book"
		>
			<div className="min-w-0 gap-3 flex flex-1 items-start">
				<CheckBox
					className="mt-1"
					checked={selected}
					onCheckedChange={(checked) => onSelect(checked === true)}
					aria-label={`Select “${title}”`}
				/>
				<Cover url={book.thumbnail.url} title={title} />
				<div className="min-w-0 gap-0.5 flex flex-1 flex-col">
					<p className="font-medium leading-snug line-clamp-2" title={entry.relativePath}>
						{title}
					</p>
					<p className="text-sm truncate text-muted-foreground">
						{authors ? `${authors} · ` : ''}
						{entry.libraryName}
					</p>
					{!entry.fileExists && (
						<Badge variant="warning" size="sm" className="mt-1 gap-1">
							<FileX2 className="size-3" aria-hidden />
							File missing
						</Badge>
					)}
				</div>
			</div>

			<div className="gap-1.5 md:w-72 md:items-end flex shrink-0 flex-col">
				<div className="gap-x-3 gap-y-1 md:flex-col md:items-end flex flex-wrap items-center justify-between">
					<span className="text-xs text-muted-foreground" title={entry.deletedAt}>
						Removed {entry.deletedBy ? `by ${entry.deletedBy} ` : ''}
						{removed}
					</span>
					<RetentionFuse purgeAt={entry.purgeAt} retentionDays={retentionDays} now={now} />
				</div>
				<div className="gap-2 mt-1 flex">
					<Button
						size="sm"
						variant="outline"
						className="md:flex-none flex-1"
						onClick={onRestore}
						disabled={busy}
					>
						<RotateCcw aria-hidden />
						Restore
					</Button>
					<Button
						size="sm"
						variant="destructive"
						className="md:flex-none flex-1"
						onClick={onPurge}
						disabled={busy}
					>
						<Trash2 aria-hidden />
						Delete permanently
					</Button>
				</div>
			</div>
		</li>
	)
}

export function Cover({ url, title }: { url: string; title: string }) {
	const [failed, setFailed] = useState(false)
	const frame = 'h-[60px] w-10 shrink-0 rounded-sm border-[0.5px] border-border shadow-sm'
	if (failed) {
		return (
			<div className={cn(frame, 'flex items-center justify-center bg-sidebar')} aria-hidden>
				<Book className="h-4 w-4 text-muted-foreground" />
			</div>
		)
	}
	return (
		<EntityImage
			src={url}
			alt=""
			title={title}
			loading="lazy"
			className={cn(frame, 'object-cover')}
			onError={() => setFailed(true)}
		/>
	)
}
