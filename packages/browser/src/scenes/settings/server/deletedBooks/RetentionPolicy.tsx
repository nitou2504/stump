import { Link, NativeSelect } from '@stump/components'
import { DeletedBooksSettingsQuery } from '@stump/graphql'
import { format, formatDistanceToNow, isToday, isTomorrow } from 'date-fns'
import { CalendarClock, FolderArchive, History } from 'lucide-react'
import { ReactNode } from 'react'

import { usePaths } from '@/paths'

import { books, formatBytes, retentionLabel, retentionOptions } from './countdown'

type Settings = DeletedBooksSettingsQuery['deletedBooksSettings']

type Props = {
	settings: Settings
	canEditRetention: boolean
	isSaving: boolean
	onRetentionChange: (days: number) => void
}

/** "today 23:00", "tomorrow 04:00", "Thu 6 Nov, 04:00" */
export function when(iso: string): string {
	const date = new Date(iso)
	const time = format(date, 'HH:mm')
	if (isToday(date)) return `today ${time}`
	if (isTomorrow(date)) return `tomorrow ${time}`
	return format(date, 'EEE d MMM, HH:mm')
}

/**
 * The rules of Deleted books, up front: how long books and replaced originals wait, when the purge
 * runs and what it last did, and where Stump keeps the originals
 */
export default function RetentionPolicy({
	settings,
	canEditRetention,
	isSaving,
	onRetentionChange,
}: Props) {
	const paths = usePaths()
	const { retentionDays, trashDir, heldFiles, heldBytes, nextPurgeAt, lastPurge, lastPurgeAt } =
		settings
	const never = retentionDays === 0

	return (
		<section
			aria-label="How Deleted books works"
			className="gap-4 p-4 sm:p-5 flex flex-col rounded-lg border border-border bg-muted/30"
		>
			<p className="text-sm max-w-prose leading-relaxed">
				{never ? (
					<>
						Removed books, and the originals post-processing replaced, wait here until you delete
						them permanently.{' '}
					</>
				) : (
					<>
						Removed books, and the originals post-processing replaced, wait here{' '}
						<strong>{retentionLabel(retentionDays)}</strong>, then they are deleted
						permanently.{' '}
					</>
				)}
				Restore brings a book back as it was, with its progress, notes and tags; Put back returns an
				original to its book. Delete permanently deletes the file from disk and erases the book from
				Stump.
			</p>

			<div className="gap-x-8 gap-y-3 sm:flex-row sm:flex-wrap sm:items-center flex flex-col">
				<label className="gap-2 text-sm flex items-center">
					<span className="text-muted-foreground">Keep removed books</span>
					{canEditRetention ? (
						<NativeSelect
							size="sm"
							className="w-36 text-base md:text-sm"
							value={String(retentionDays)}
							options={retentionOptions(retentionDays)}
							disabled={isSaving}
							aria-label="How long removed books are kept"
							onChange={(event) => onRetentionChange(Number(event.target.value))}
						/>
					) : (
						<strong title="Set by a server administrator">
							{never ? 'until deleted by hand' : retentionLabel(retentionDays)}
						</strong>
					)}
				</label>

				<Fact icon={<CalendarClock aria-hidden />} label="Next purge">
					{never || !nextPurgeAt ? 'off' : when(nextPurgeAt)}
				</Fact>

				<Fact icon={<History aria-hidden />} label="Last purge">
					{lastPurgeAt && lastPurge ? (
						<span title={new Date(lastPurgeAt).toLocaleString()}>
							{formatDistanceToNow(new Date(lastPurgeAt), { addSuffix: true })}:{' '}
							{lastPurgeResult(lastPurge)}
							{lastPurge.jobId && (
								<>
									{' '}
									<Link to={paths.serverLogs(lastPurge.jobId)} className="underline">
										logs
									</Link>
								</>
							)}
						</span>
					) : (
						'not run yet'
					)}
				</Fact>
			</div>

			{lastPurge && lastPurge.errors.length > 0 && (
				<ul className="gap-1 text-xs flex flex-col text-destructive">
					{lastPurge.errors.slice(0, 3).map((error) => (
						<li key={error.mediaId}>
							Kept “{error.title}”: {error.error}
						</li>
					))}
				</ul>
			)}

			{trashDir && (
				<p className="gap-2 text-xs flex items-start text-muted-foreground">
					<FolderArchive className="size-4 mt-px shrink-0" aria-hidden />
					<span>
						Replaced originals are kept in{' '}
						<code className="font-mono break-all text-foreground">
							{trashDir.replace(/\/+$/, '')}/&lt;date&gt;/
						</code>
						{heldFiles > 0
							? ` (${heldFiles === 1 ? '1 file' : `${heldFiles} files`}, ${formatBytes(heldBytes)})`
							: ''}
						. Removed books keep their file where it is until they are deleted permanently.
					</span>
				</p>
			)}
		</section>
	)
}

function lastPurgeResult(summary: NonNullable<Settings['lastPurge']>): string {
	const gone = summary.purged + summary.missingFiles
	if (gone === 0 && summary.failed === 0 && summary.originals === 0) return 'nothing was due'
	const parts = []
	if (gone > 0) parts.push(`${books(gone)} deleted permanently`)
	if (summary.originals > 0)
		parts.push(
			`${summary.originals === 1 ? '1 original' : `${summary.originals} originals`} deleted`,
		)
	if (summary.failed > 0) parts.push(`${books(summary.failed)} kept`)
	return parts.join(', ')
}

function Fact({ icon, label, children }: { icon: ReactNode; label: string; children: ReactNode }) {
	return (
		<div className="gap-2 text-sm flex items-start">
			<span className="[&_svg]:size-4 mt-0.5 text-muted-foreground">{icon}</span>
			<span className="whitespace-nowrap text-muted-foreground">{label}</span>
			<span className="font-medium">{children}</span>
		</div>
	)
}
