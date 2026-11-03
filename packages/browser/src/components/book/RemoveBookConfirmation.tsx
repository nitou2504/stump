import { useGraphQL, useGraphQLMutation } from '@stump/client'
import { Alert, AlertDescription, ConfirmationModal, Link } from '@stump/components'
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

const retentionQuery = graphql(`
	query RemoveBookRetention {
		deletedBooksSettings {
			retentionDays
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
 * Confirms removing a book from its library. Stump soft-deletes it: it goes to Deleted books
 * (Settings), hidden everywhere with its file still on disk, until a library manager restores it
 * or it is deleted permanently once the retention is over. Afterwards the user is taken back to
 * the page they opened the book from.
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
	const { data } = useGraphQL(retentionQuery, ['deletedBooksSettings', 'retention'], undefined, {
		enabled: isOpen,
	})
	const days = data?.deletedBooksSettings.retentionDays

	const stay =
		days === undefined
			? 'It stays in Deleted books'
			: days === 0
				? 'It stays in Deleted books until it is deleted permanently by hand'
				: `It stays in Deleted books for ${days === 1 ? '1 day' : `${days} days`}, then it is deleted permanently (its file moves to the quarantine folder)`
	const deletedBooks = canRestore ? (
		<Link to={paths.deletedBooks()} onClick={onClose} className="underline">
			Settings → Deleted books
		</Link>
	) : (
		'Settings → Deleted books'
	)

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
					<p>
						{stay}. Until then {canRestore ? 'you' : 'a library manager'} can restore it, with its
						reading progress and notes, from {deletedBooks}.
					</p>
					<p className="mt-1.5">
						It disappears from Stump right away, OPDS and smart lists included. Its file stays on
						disk.
					</p>
				</AlertDescription>
			</Alert>
		</ConfirmationModal>
	)
}
