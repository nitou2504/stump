import { Link, ProgressBar } from '@stump/components'
import { IntakeKind } from '@stump/graphql'
import { Loader2 } from 'lucide-react'

import { usePaths } from '@/paths'

import CoverTile, { BookCover } from './CoverTile'
import type { IntakeItemData } from './queries'
import { destinationPaths } from './reviewState'

/** A book the server is reading: its name and a bar that says something is happening */
export function AnalysingRow({ item }: { item: IntakeItemData }) {
	return (
		<li className="gap-3 px-3 py-2.5 flex items-center">
			<CoverTile
				size="sm"
				title={item.name}
				format={item.files[0]?.format}
				series={item.kind === IntakeKind.Series}
			/>
			<div className="min-w-0 gap-1.5 flex flex-1 flex-col">
				<span className="text-sm font-medium truncate">{item.name}</span>
				<ProgressBar
					isIndeterminate
					size="sm"
					variant="primary"
					aria-label={`Reading ${item.name}`}
				/>
			</div>
			<span className="text-xs shrink-0 text-muted-foreground">Reading…</span>
		</li>
	)
}

/**
 * A book that is in the library: its cover (the book's own once the scan has made it), where it
 * went, and a way to open it
 */
export function FiledRow({ item, now }: { item: IntakeItemData; now: number }) {
	const paths = usePaths()
	const isSeries = item.kind === IntakeKind.Series
	const meta = item.decision?.metadata
	const title = (isSeries || item.decision?.seriesKind ? meta?.series : meta?.title) || item.name
	// somebody who files nothing themselves has no destination: the paths are the filing's
	const planned = destinationPaths(item)
	const where = planned.length ? planned : (item.result?.paths ?? [])
	const first = item.mediaIds[0]
	const recentlyFiled =
		!now || (!!item.filedAt && now - new Date(item.filedAt).getTime() < 5 * 60 * 1000)
	const format = item.files[0]?.format

	return (
		<li className="gap-3 px-3 py-2.5 flex items-center">
			{first ? (
				<BookCover mediaId={first} title={title} format={format} series={isSeries} />
			) : (
				<CoverTile size="sm" title={title} format={format} series={isSeries} />
			)}
			<div className="min-w-0 flex flex-1 flex-col">
				<span className="text-sm font-medium truncate">{title}</span>
				<span
					className="font-mono text-xs break-all text-muted-foreground"
					title={where.join('\n')}
				>
					{where.length > 1
						? item.destination
							? `${item.destination.relativeDir}/ (${where.length} files)`
							: `${where.length} files`
						: where[0]}
				</span>
				{!!item.result?.notes.length && (
					<span className="text-xs text-warning">{item.result.notes.join(' · ')}</span>
				)}
			</div>
			{first ? (
				<Link to={paths.bookOverview(first)} className="text-sm shrink-0">
					Open
				</Link>
			) : recentlyFiled ? (
				<span className="gap-1 text-xs flex shrink-0 items-center text-muted-foreground">
					<Loader2 className="h-3 w-3 animate-spin motion-reduce:animate-none" aria-hidden />{' '}
					scanning
				</span>
			) : (
				<span className="text-xs shrink-0 text-muted-foreground">not scanned yet</span>
			)}
		</li>
	)
}
