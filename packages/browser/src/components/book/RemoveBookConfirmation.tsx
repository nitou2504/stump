import { useGraphQLMutation } from '@stump/client'
import { Alert, AlertDescription, ConfirmationModal } from '@stump/components'
import { graphql } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { Info } from 'lucide-react'
import { useNavigate } from 'react-router'
import { toast } from 'sonner'

const mutation = graphql(`
	mutation RemoveBookFromLibrary($id: ID!) {
		deleteMedia(id: $id) {
			id
		}
	}
`)

type Props = {
	bookId: string
	name: string
	/** Where to go once the book is gone (its page no longer exists) */
	redirectTo: string
	isOpen: boolean
	onClose: () => void
}

/**
 * Confirms removing a book from its library. Stump soft-deletes it: the file stays on
 * disk, rescans keep it hidden, and it can be restored from the library's settings.
 */
export default function RemoveBookConfirmation({
	bookId,
	name,
	redirectTo,
	isOpen,
	onClose,
}: Props) {
	const navigate = useNavigate()
	const client = useQueryClient()
	const { mutate, isPending } = useGraphQLMutation(mutation, {
		onSuccess: () => {
			toast.success(`Removed “${name}”`)
			onClose()
			navigate(redirectTo, { replace: true })
			client.invalidateQueries()
		},
		onError: () => toast.error('Failed to remove the book'),
	})

	return (
		<ConfirmationModal
			title="Remove from library"
			description={`Remove “${name}” from the library?`}
			isOpen={isOpen}
			onClose={onClose}
			onConfirm={() => mutate({ id: bookId })}
			confirmIsLoading={isPending}
			confirmText="Remove"
			confirmVariant="destructive"
		>
			<Alert>
				<Info />
				<AlertDescription>
					The file is not deleted from disk. The book disappears from Stump (including OPDS and
					smart lists) and can be restored from the library&apos;s settings → Danger zone →
					Deletion.
				</AlertDescription>
			</Alert>
		</ConfirmationModal>
	)
}
