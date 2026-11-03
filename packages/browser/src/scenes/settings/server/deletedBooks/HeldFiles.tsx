import { useGraphQL } from '@stump/client'
import { Button, ConfirmationModal } from '@stump/components'
import { HeldFilesQuery } from '@stump/graphql'
import { formatDistanceToNow } from 'date-fns'
import { Trash2, Undo2 } from 'lucide-react'
import { useState } from 'react'

import { SettingsSection } from '@/components/sharedLayout'

import { formatBytes, heldFileLabel } from './countdown'
import { Cover } from './DeletedBookRow'
import { heldFilesQuery } from './queries'
import RetentionFuse from './RetentionFuse'

type Held = HeldFilesQuery['heldFiles'][number]

type Props = {
	libraryId?: string
	retentionDays: number
	now: Date
	onPutBack: (id: number) => Promise<boolean>
	onDelete: (ids: number[]) => Promise<boolean>
	busy: boolean
}

/**
 * The files Stump took out of a library and keeps for the retention: the originals that
 * post-processing replaced (16 greys, OCR, shrinking). Put back returns one to its book (the
 * version it replaces waits here in turn); Delete permanently deletes it from disk
 */
export default function HeldFiles({
	libraryId,
	retentionDays,
	now,
	onPutBack,
	onDelete,
	busy,
}: Props) {
	const { data } = useGraphQL(heldFilesQuery, ['heldFiles', libraryId ?? ''], {
		libraryId: libraryId || undefined,
	})
	const rows = data?.heldFiles ?? []
	const [putting, setPutting] = useState<Held | null>(null)
	const [deleting, setDeleting] = useState<Held[] | null>(null)

	if (rows.length === 0) return null
	const total = rows.reduce((sum, row) => sum + row.bytes, 0)

	return (
		<SettingsSection
			title="Replaced originals"
			description={`${rows.length === 1 ? '1 file' : `${rows.length} files`}, ${formatBytes(total)}`}
		>
			<ul className="divide-y divide-border overflow-hidden rounded-lg border border-border">
				{rows.map((row) => (
					<li
						key={row.id}
						className="gap-3 px-3 py-3 sm:px-4 md:flex-row md:items-start md:gap-6 flex flex-col"
						data-testid="held-file"
					>
						<div className="min-w-0 gap-3 flex flex-1 items-start">
							{row.book ? (
								<Cover url={row.book.thumbnail.url} title={row.title} />
							) : (
								<Cover url="" title={row.title} />
							)}
							<div className="min-w-0 gap-0.5 flex flex-1 flex-col">
								<p className="font-medium leading-snug line-clamp-2" title={row.relativePath}>
									{row.title}
								</p>
								<p className="text-sm text-muted-foreground">
									{heldFileLabel(row.reason, row.steps)}
									{' · '}
									{formatBytes(row.bytes)}
									{row.libraryName ? ` · ${row.libraryName}` : ''}
								</p>
							</div>
						</div>
						<div className="gap-1.5 md:w-72 md:items-end flex shrink-0 flex-col">
							<div className="gap-x-3 gap-y-1 md:flex-col md:items-end flex flex-wrap items-center justify-between">
								<span className="text-xs text-muted-foreground" title={row.heldAt}>
									Replaced {row.heldBy ? `by ${row.heldBy} ` : ''}
									{formatDistanceToNow(new Date(row.heldAt), { addSuffix: true })}
								</span>
								<RetentionFuse purgeAt={row.purgeAt} retentionDays={retentionDays} now={now} />
							</div>
							<div className="gap-2 mt-1 flex">
								<Button
									size="sm"
									variant="outline"
									className="md:flex-none flex-1"
									disabled={busy || !row.canPutBack}
									title={row.canPutBack ? undefined : 'Its book or its file is gone'}
									onClick={() => setPutting(row)}
								>
									<Undo2 aria-hidden />
									Put back
								</Button>
								<Button
									size="sm"
									variant="destructive"
									className="md:flex-none flex-1"
									disabled={busy}
									onClick={() => setDeleting([row])}
								>
									<Trash2 aria-hidden />
									Delete permanently
								</Button>
							</div>
						</div>
					</li>
				))}
			</ul>
			{rows.length > 1 && (
				<div>
					<Button
						size="sm"
						variant="ghost"
						className="text-destructive hover:text-destructive"
						disabled={busy}
						onClick={() => setDeleting(rows)}
					>
						<Trash2 aria-hidden />
						Delete all {rows.length} permanently
					</Button>
				</div>
			)}

			<ConfirmationModal
				isOpen={!!putting}
				onClose={() => setPutting(null)}
				onConfirm={async () => {
					if (putting && (await onPutBack(putting.id))) setPutting(null)
				}}
				confirmIsLoading={busy}
				confirmText="Put back"
				title={`Put back the original of “${putting?.title}”?`}
				description="The current version waits here instead, so you can undo this."
			/>
			<ConfirmationModal
				isOpen={!!deleting}
				onClose={() => setDeleting(null)}
				onConfirm={async () => {
					if (deleting && (await onDelete(deleting.map((row) => row.id)))) setDeleting(null)
				}}
				confirmIsLoading={busy}
				confirmVariant="destructive"
				confirmText="Delete permanently"
				title={
					deleting?.length === 1
						? `Delete the original of “${deleting[0]?.title}” permanently?`
						: `Delete ${deleting?.length ?? 0} originals permanently?`
				}
				description="The books keep their current files. This can’t be undone."
			/>
		</SettingsSection>
	)
}
