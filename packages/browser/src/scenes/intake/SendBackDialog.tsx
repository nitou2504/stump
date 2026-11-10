import { Button, Dialog, Label, TextArea } from '@stump/components'
import { useEffect, useId, useState } from 'react'

/** The longest note an approver may leave: the server keeps it with the book */
export const SEND_BACK_NOTE_MAX = 1000

type Props = {
	open: boolean
	/** The title of the book that goes back, to say which one it is */
	title: string
	/** Who gets it back */
	to?: string
	onClose: () => void
	onConfirm: (note: string) => void
	isPending?: boolean
}

/**
 * The note that goes with a book sent back to its uploader: it says what to change, and it is
 * what they see on the book until they send it again
 */
export default function SendBackDialog({ open, title, to, onClose, onConfirm, isPending }: Props) {
	const id = useId()
	const [note, setNote] = useState('')

	// a note is for one book: start empty each time the dialog opens
	useEffect(() => {
		if (open) setNote('')
	}, [open])

	const trimmed = note.trim()

	return (
		<Dialog open={open} onOpenChange={(next) => !next && !isPending && onClose()}>
			<Dialog.Content size="md">
				<Dialog.Header className="pr-8">
					<Dialog.Title>Send back</Dialog.Title>
					<Dialog.Description className="line-clamp-2">
						{to ? `${title} goes back to ${to}` : title}. They change it and send it again.
					</Dialog.Description>
					<Dialog.Close />
				</Dialog.Header>

				<div className="gap-2 flex flex-col">
					<Label htmlFor={`${id}-note`}>What should they change?</Label>
					<TextArea
						id={`${id}-note`}
						rows={4}
						value={note}
						maxLength={SEND_BACK_NOTE_MAX}
						placeholder="e.g. The author is spelled wrong, and the series number is missing"
						onChange={(e) => setNote(e.target.value)}
					/>
				</div>

				<Dialog.Footer className="gap-2 sm:flex grid grid-cols-2">
					<Button type="button" variant="outline" onClick={onClose} disabled={isPending}>
						Cancel
					</Button>
					<Button
						type="button"
						onClick={() => onConfirm(trimmed)}
						disabled={!trimmed}
						isLoading={isPending}
					>
						Send back
					</Button>
				</Dialog.Footer>
			</Dialog.Content>
		</Dialog>
	)
}
