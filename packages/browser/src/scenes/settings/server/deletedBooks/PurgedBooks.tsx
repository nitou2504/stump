import { useGraphQL } from '@stump/client'
import { Button, ConfirmationModal } from '@stump/components'
import { PurgedBooksQuery } from '@stump/graphql'
import { formatDistanceToNow } from 'date-fns'
import { Undo2 } from 'lucide-react'
import { useState } from 'react'

import { SettingsSection } from '@/components/sharedLayout'

import { formatBytes } from './countdown'
import { purgedBooksQuery } from './queries'

type Purged = PurgedBooksQuery['purgedBooks'][number]

const PAGE = 20

type Props = {
	onPutBack: (id: number) => Promise<boolean>
	isPuttingBack: boolean
}

/**
 * The books deleted permanently, latest first: where each file went, and a way to put it back
 * into its library
 */
export default function PurgedBooks({ onPutBack, isPuttingBack }: Props) {
	const [limit, setLimit] = useState(PAGE)
	const { data } = useGraphQL(purgedBooksQuery, ['purgedBooks', limit], { limit })
	const rows = data?.purgedBooks ?? []
	const [confirming, setConfirming] = useState<Purged | null>(null)

	return (
		<SettingsSection
			title="Recently deleted permanently"
			description="The latest books deleted for good. Their files wait in the quarantine folder and can be put back."
		>
			{rows.length === 0 ? (
				<p className="text-sm text-muted-foreground">Nothing has been deleted permanently yet.</p>
			) : (
				<ul className="divide-y divide-border rounded-lg border border-border">
					{rows.map((row) => (
						<li
							key={row.id}
							className="gap-2 px-3 py-3 sm:px-4 md:flex-row md:items-start md:gap-6 flex flex-col"
							data-testid="purged-book"
						>
							<div className="min-w-0 gap-0.5 flex flex-1 flex-col">
								<p className="font-medium leading-snug">{row.title}</p>
								<p className="text-sm truncate text-muted-foreground">
									{[row.authors.join(', '), row.libraryName, formatBytes(row.bytes)]
										.filter(Boolean)
										.join(' · ')}
								</p>
								<p
									className="font-mono text-xs break-all text-muted-foreground/80"
									title={row.originalPath}
								>
									{row.quarantinePath ?? row.originalPath}
								</p>
							</div>
							<div className="gap-1.5 md:w-72 md:items-end flex shrink-0 flex-col">
								<span className="text-xs text-muted-foreground" title={row.purgedAt}>
									Deleted permanently{' '}
									{row.automatic
										? 'by the automatic purge'
										: row.purgedBy
											? `by ${row.purgedBy}`
											: ''}{' '}
									{formatDistanceToNow(new Date(row.purgedAt), { addSuffix: true })}
								</span>
								{row.restoredAt ? (
									<span className="text-xs text-success">
										Put back {row.restoredBy ? `by ${row.restoredBy} ` : ''}
										{formatDistanceToNow(new Date(row.restoredAt), { addSuffix: true })}
									</span>
								) : !row.quarantinePath ? (
									<span className="text-xs text-muted-foreground">Its file was already gone</span>
								) : row.canRestore ? (
									<Button
										size="sm"
										variant="outline"
										className="mt-1"
										disabled={isPuttingBack}
										onClick={() => setConfirming(row)}
									>
										<Undo2 aria-hidden />
										Put back in the library
									</Button>
								) : (
									<span className="text-xs text-muted-foreground">
										Can’t be put back: its file left the quarantine folder, or another file took its
										place
									</span>
								)}
							</div>
						</li>
					))}
				</ul>
			)}

			{rows.length >= limit && (
				<div>
					<Button variant="outline" size="sm" onClick={() => setLimit((l) => l + PAGE)}>
						Show more
					</Button>
				</div>
			)}

			<p className="text-xs max-w-prose text-muted-foreground">
				By hand: each day’s folder in the quarantine folder has a <code>restore_log.csv</code> with
				every file’s <code>original_path</code>. Move a file back there and scan its library; it
				returns as a new book.
			</p>

			<ConfirmationModal
				isOpen={!!confirming}
				onClose={() => setConfirming(null)}
				onConfirm={async () => {
					if (confirming && (await onPutBack(confirming.id))) setConfirming(null)
				}}
				confirmIsLoading={isPuttingBack}
				confirmText="Put back"
				title={`Put “${confirming?.title}” back in the library?`}
				description={
					<>
						Its file moves from the quarantine folder back to{' '}
						<code className="font-mono text-xs break-all">{confirming?.originalPath}</code> and its
						folder is scanned. It returns as a new book: its reading progress, annotations and tags
						were erased when it was deleted.
					</>
				}
			/>
		</SettingsSection>
	)
}
