import { Alert, AlertDescription, ConfirmationModal, Input } from '@stump/components'
import { FolderInput, History, ShieldAlert, Trash2 } from 'lucide-react'
import { ReactNode, useEffect, useId, useState } from 'react'

import { books, todaysQuarantineFolder } from './countdown'
import type { DeletedBookEntry } from './DeletedBookRow'

type QuarantineProps = {
	quarantineDir?: string | null
	quarantineReady: boolean
	now: Date
}

/** What deleting these books permanently does, point by point */
function Consequences({
	entries,
	quarantineDir,
	quarantineReady,
	now,
}: QuarantineProps & { entries: DeletedBookEntry[] }) {
	const withFile = entries.filter((entry) => entry.fileExists).length
	const missing = entries.length - withFile
	const one = entries.length === 1
	const folder = quarantineDir ? todaysQuarantineFolder(quarantineDir, now) : null

	return (
		<div className="gap-3 text-sm flex flex-col">
			<ul className="gap-2.5 flex flex-col">
				{withFile > 0 && quarantineReady && (
					<Point icon={<FolderInput aria-hidden />}>
						{one ? 'Its file moves' : withFile === 1 ? '1 file moves' : `${withFile} files move`} to{' '}
						<code className="font-mono text-xs break-all">{folder}</code>, listed in that folder’s{' '}
						<code className="font-mono text-xs">restore_log.csv</code>. No book file is erased.
					</Point>
				)}
				<Point icon={<Trash2 aria-hidden />}>
					Stump erases {one ? 'its' : 'their'} reading progress, annotations, bookmarks and tags.
					This can’t be undone in Stump.
				</Point>
				{withFile > 0 && quarantineReady && (
					<Point icon={<History aria-hidden />}>
						{one ? 'Its file' : 'A file'} can be put back from <em>Deleted permanently</em> below;
						it returns as a new book.
					</Point>
				)}
				{missing > 0 && (
					<Point icon={<ShieldAlert aria-hidden />}>
						{missing === 1 ? (one ? 'Its file is' : '1 file is') : `${missing} files are`} already
						gone from the library: only {missing === 1 ? 'its' : 'their'} data is deleted.
					</Point>
				)}
			</ul>
			{withFile > 0 && !quarantineReady && (
				<Alert variant="warning">
					<ShieldAlert />
					<AlertDescription>
						<p>
							No quarantine folder is ready (<code className="font-mono">STUMP_QUARANTINE_DIR</code>
							), and Stump never deletes book files:{' '}
							{withFile === 1 ? 'the book' : `the ${withFile} books`} with a file will stay in
							Deleted books.
						</p>
					</AlertDescription>
				</Alert>
			)}
		</div>
	)
}

function Point({ icon, children }: { icon: ReactNode; children: ReactNode }) {
	return (
		<li className="gap-2.5 flex items-start">
			<span className="mt-0.5 [&_svg]:size-4 shrink-0 text-muted-foreground">{icon}</span>
			<span className="leading-relaxed">{children}</span>
		</li>
	)
}

type PurgeProps = QuarantineProps & {
	/** The books to delete; the dialog is open while set */
	entries: DeletedBookEntry[] | null
	isPending: boolean
	onConfirm: () => void
	onClose: () => void
}

/** Confirms deleting one or a few books permanently */
export function PurgeConfirmation({ entries, isPending, onConfirm, onClose, ...rest }: PurgeProps) {
	const list = entries ?? []
	const one = list.length === 1 ? list[0] : undefined
	return (
		<ConfirmationModal
			isOpen={!!entries}
			onClose={onClose}
			onConfirm={onConfirm}
			confirmIsLoading={isPending}
			confirmVariant="destructive"
			confirmText={one ? 'Delete permanently' : `Delete ${list.length} permanently`}
			title={
				one
					? `Delete “${one.book.resolvedName}” permanently?`
					: `Delete ${list.length} books permanently?`
			}
			description={one ? 'It leaves Deleted books for good.' : 'They leave Deleted books for good.'}
		>
			{list.length > 0 && <Consequences entries={list} {...rest} />}
		</ConfirmationModal>
	)
}

type EmptyProps = QuarantineProps & {
	isOpen: boolean
	entries: DeletedBookEntry[]
	/** Set when the list is filtered to one library */
	libraryName?: string
	isPending: boolean
	onConfirm: () => void
	onClose: () => void
}

/** Confirms emptying Deleted books (or one library's part of it): the user types what happens */
export function EmptyConfirmation({
	isOpen,
	entries,
	libraryName,
	isPending,
	onConfirm,
	onClose,
	...rest
}: EmptyProps) {
	const formId = useId()
	const [typed, setTyped] = useState('')
	useEffect(() => {
		setTyped('')
	}, [isOpen])
	const phrase = `delete ${books(entries.length)}`
	const matches = typed.trim().toLowerCase() === phrase

	return (
		<ConfirmationModal
			isOpen={isOpen}
			onClose={onClose}
			formId={formId}
			confirmDisabled={!matches}
			confirmIsLoading={isPending}
			confirmVariant="destructive"
			confirmText={libraryName ? 'Delete them all permanently' : 'Empty Deleted books'}
			title={
				libraryName
					? `Delete all ${books(entries.length)} from ${libraryName} permanently?`
					: 'Empty Deleted books?'
			}
			description={
				libraryName
					? `Every book removed from ${libraryName} leaves Deleted books for good.`
					: `All ${books(entries.length)} in Deleted books leave it for good.`
			}
		>
			<form
				id={formId}
				className="gap-4 flex flex-col"
				onSubmit={(event) => {
					event.preventDefault()
					if (matches) onConfirm()
				}}
			>
				<Consequences entries={entries} {...rest} />
				<label className="gap-2 text-sm flex flex-col">
					<span>
						Type <strong className="font-mono">{phrase}</strong> to confirm
					</span>
					<Input
						value={typed}
						onChange={(event) => setTyped(event.target.value)}
						placeholder={phrase}
						autoComplete="off"
						autoFocus
						fullWidth
						className="text-base md:text-sm"
					/>
				</label>
			</form>
		</ConfirmationModal>
	)
}
