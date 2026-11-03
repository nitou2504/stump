import { useGraphQL } from '@stump/client'
import { Button } from '@stump/components'
import { formatDistanceToNow } from 'date-fns'
import { useState } from 'react'

import { SettingsSection } from '@/components/sharedLayout'

import { formatBytes } from './countdown'
import { purgedBooksQuery } from './queries'

const PAGE = 20

/** The books deleted permanently, latest first: a log of what went, from where, and who did it */
export default function PurgedBooks() {
	const [limit, setLimit] = useState(PAGE)
	const { data } = useGraphQL(purgedBooksQuery, ['purgedBooks', limit], { limit })
	const rows = data?.purgedBooks ?? []

	return (
		<SettingsSection
			title="Recently deleted permanently"
			description="The latest books deleted for good, with their files. A log: they can’t be brought back."
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
									{row.originalPath}
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
								{row.bytes == null && (
									<span className="text-xs text-muted-foreground">Its file was already gone</span>
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
		</SettingsSection>
	)
}
