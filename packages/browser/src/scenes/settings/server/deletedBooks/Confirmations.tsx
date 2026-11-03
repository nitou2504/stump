import { ConfirmationModal, Input } from '@stump/components'
import { FileX2, ShieldAlert, Trash2 } from 'lucide-react'
import { ReactNode, useEffect, useId, useState } from 'react'

import { books } from './countdown'
import type { DeletedBookEntry } from './DeletedBookRow'

/** What deleting these books permanently does, point by point */
function Consequences({ entries }: { entries: DeletedBookEntry[] }) {
	const withFile = entries.filter((entry) => entry.fileExists).length
	const missing = entries.length - withFile
	const one = entries.length === 1

	return (
		<ul className="gap-2.5 text-sm flex flex-col">
			{withFile > 0 && (
				<Point icon={<FileX2 aria-hidden />}>
					{one ? 'Its file is' : withFile === 1 ? '1 file is' : `${withFile} files are`} deleted
					from disk, with any original Stump keeps for {one ? 'it' : 'them'}.
				</Point>
			)}
			<Point icon={<Trash2 aria-hidden />}>
				Stump erases {one ? 'its' : 'their'} reading progress, annotations, bookmarks and tags. This
				can’t be undone.
			</Point>
			{missing > 0 && (
				<Point icon={<ShieldAlert aria-hidden />}>
					{missing === 1 ? (one ? 'Its file is' : '1 file is') : `${missing} files are`} already
					gone from the library: only {missing === 1 ? 'its' : 'their'} data is deleted.
				</Point>
			)}
		</ul>
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

type PurgeProps = {
	/** The books to delete; the dialog is open while set */
	entries: DeletedBookEntry[] | null
	isPending: boolean
	onConfirm: () => void
	onClose: () => void
}

/** Confirms deleting one or a few books permanently */
export function PurgeConfirmation({ entries, isPending, onConfirm, onClose }: PurgeProps) {
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
			{list.length > 0 && <Consequences entries={list} />}
		</ConfirmationModal>
	)
}

type EmptyProps = {
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
				<Consequences entries={entries} />
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
