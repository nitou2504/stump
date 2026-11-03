import { ConfirmationModal, Input } from '@stump/components'
import { useEffect, useId, useState } from 'react'

import { books } from './countdown'
import type { DeletedBookEntry } from './DeletedBookRow'

/** What deleting these books permanently does, in one line */
function Consequences({ entries }: { entries: DeletedBookEntry[] }) {
	const missing = entries.filter((entry) => !entry.fileExists).length
	const one = entries.length === 1

	return (
		<p className="text-sm leading-relaxed">
			{one
				? 'Deletes its file, progress, notes and bookmarks.'
				: 'Deletes their files, progress, notes and bookmarks.'}{' '}
			{missing > 0 &&
				(one
					? 'Its file is already missing. '
					: `${missing === 1 ? '1 file is' : `${missing} files are`} already missing. `)}
			<strong>This can’t be undone.</strong>
		</p>
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
					: `Empty Deleted books (${books(entries.length)})?`
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
