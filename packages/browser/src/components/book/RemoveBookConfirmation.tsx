import { useGraphQLMutation } from '@stump/client'
import { Alert, AlertDescription, ConfirmationModal } from '@stump/components'
import { graphql, UserPermission } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { Info } from 'lucide-react'
import { useCallback } from 'react'
import { toast } from 'sonner'

import { useAppContext } from '@/context'
import { useLeavePages } from '@/hooks/useHistoryTrail'
import { usePaths } from '@/paths'

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
	/**
	 * Where to go once the book is gone when the history doesn't show which page the book
	 * was opened from
	 */
	fallbackPath: string
	isOpen: boolean
	onClose: () => void
}

/**
 * Confirms removing a book from its library. Stump soft-deletes it: the file stays on
 * disk, rescans keep it hidden, and it can be restored from the library's settings.
 * Afterwards the user is taken back to the page they opened the book from.
 */
export default function RemoveBookConfirmation({
	bookId,
	name,
	fallbackPath,
	isOpen,
	onClose,
}: Props) {
	const paths = usePaths()
	const client = useQueryClient()
	const leavePages = useLeavePages()
	const { checkPermission } = useAppContext()

	const isBookPage = useCallback(
		(path: string) => {
			const bookPath = paths.bookOverview(bookId)
			return path === bookPath || path.startsWith(`${bookPath}/`) || path.startsWith(`${bookPath}?`)
		},
		[paths, bookId],
	)

	const { mutate, isPending } = useGraphQLMutation(mutation, {
		onSuccess: () => {
			toast.success(`Removed “${name}”`)
			onClose()
			// Pages not on screen are fetched again when shown, so they no longer list the book.
			// The ones on screen are not refetched: the book's page stays visible until the page
			// we go back to is ready, and it would not find the book anymore
			client.removeQueries({ type: 'inactive' })
			client.invalidateQueries({ refetchType: 'none' })
			leavePages(isBookPage, fallbackPath)
		},
		onError: () => toast.error('Failed to remove the book'),
	})

	const canRestore = checkPermission(UserPermission.ManageLibrary)

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
					smart lists) and {canRestore ? 'can be restored' : 'a library manager can restore it'}{' '}
					from the library&apos;s settings → Danger zone → Deletion.
				</AlertDescription>
			</Alert>
		</ConfirmationModal>
	)
}
