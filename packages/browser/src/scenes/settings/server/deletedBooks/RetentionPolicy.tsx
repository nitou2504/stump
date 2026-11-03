import { Alert, AlertDescription, AlertTitle, Link, NativeSelect } from '@stump/components'
import { DeletedBooksSettingsQuery } from '@stump/graphql'
import { format, formatDistanceToNow, isToday, isTomorrow } from 'date-fns'
import { CalendarClock, FolderInput, History, ShieldAlert } from 'lucide-react'
import { ReactNode } from 'react'

import { usePaths } from '@/paths'

import { books, retentionLabel, retentionOptions } from './countdown'

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
 * The rules of Deleted books, up front: how long books wait, when the purge runs and what it last
 * did, and where files go
 */
export default function RetentionPolicy({
	settings,
	canEditRetention,
	isSaving,
	onRetentionChange,
}: Props) {
	const paths = usePaths()
	const { retentionDays, quarantineDir, quarantineReady, nextPurgeAt, lastPurge, lastPurgeAt } =
		settings
	const never = retentionDays === 0

	return (
		<section
			aria-label="How Deleted books works"
			className="gap-4 p-4 sm:p-5 flex flex-col rounded-lg border border-border bg-muted/30"
		>
			<p className="text-sm max-w-prose leading-relaxed">
				{never ? (
					<>Removed books wait here until you delete them permanently. </>
				) : (
					<>
						Removed books wait here <strong>{retentionLabel(retentionDays)}</strong>, then the daily
						purge deletes them permanently.{' '}
					</>
				)}
				Restoring brings a book back exactly as it was: reading progress, annotations, bookmarks,
				tags and smart lists included. Deleting one permanently moves its file to the quarantine
				folder and erases it from Stump.
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

			{quarantineReady && quarantineDir ? (
				<p className="gap-2 text-xs flex items-start text-muted-foreground">
					<FolderInput className="size-4 mt-px shrink-0" aria-hidden />
					<span>
						Files deleted permanently go to{' '}
						<code className="font-mono break-all text-foreground">
							{quarantineDir.replace(/\/+$/, '')}/deleted/&lt;date&gt;/
						</code>
						, each listed in that folder’s <code className="font-mono">restore_log.csv</code>.
						Nothing is erased from disk.
					</span>
				</p>
			) : (
				<Alert variant="warning">
					<ShieldAlert />
					<AlertTitle>Permanent deletion can’t move files</AlertTitle>
					<AlertDescription>
						{quarantineDir ? (
							<>
								The quarantine folder <code>{quarantineDir}</code> doesn’t exist (is it mounted?).
							</>
						) : (
							<>
								No quarantine folder is set (<code>STUMP_QUARANTINE_DIR</code>).
							</>
						)}{' '}
						Stump never deletes book files, so books stay here until you restore them; only books
						whose file is already gone can be deleted permanently.
					</AlertDescription>
				</Alert>
			)}
		</section>
	)
}

function lastPurgeResult(summary: NonNullable<Settings['lastPurge']>): string {
	const gone = summary.purged + summary.missingFiles
	if (gone === 0 && summary.failed === 0) return 'nothing was due'
	const parts = []
	if (gone > 0) parts.push(`${books(gone)} deleted permanently`)
	if (summary.failed > 0) parts.push(`${books(summary.failed)} kept`)
	return parts.join(', ')
}

function Fact({ icon, label, children }: { icon: ReactNode; label: string; children: ReactNode }) {
	return (
		<div className="gap-2 text-sm flex items-center">
			<span className="[&_svg]:size-4 text-muted-foreground">{icon}</span>
			<span className="text-muted-foreground">{label}</span>
			<span className="font-medium">{children}</span>
		</div>
	)
}
