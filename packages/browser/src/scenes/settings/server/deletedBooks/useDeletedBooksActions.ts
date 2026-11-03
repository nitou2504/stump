import { useGraphQLMutation } from '@stump/client'
import { DeletedBooksPurgeResult } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback } from 'react'
import { toast } from 'sonner'

import { books } from './countdown'
import {
	emptyDeletedBooksMutation,
	purgeDeletedBooksMutation,
	restoreDeletedBooksMutation,
	restorePurgedBookMutation,
	updateRetentionMutation,
} from './queries'

type PurgeSummary = Pick<DeletedBooksPurgeResult, 'purged' | 'missingFiles'> & {
	kept: Pick<DeletedBooksPurgeResult['kept'][number], 'title' | 'error'>[]
}

const errorMessage = (error: unknown, fallback: string) =>
	error instanceof Error && error.message ? error.message : fallback

/** The toasts after a permanent deletion: what went, and what was kept with why */
export function announcePurge(result: PurgeSummary, singleTitle?: string) {
	const gone = result.purged + result.missingFiles
	if (gone > 0) {
		const what = gone === 1 && singleTitle ? `“${singleTitle}”` : books(gone)
		toast.success(`Deleted ${what} permanently`, {
			description:
				result.purged > 0
					? `${result.purged === 1 ? 'Its file is' : 'Their files are'} in the quarantine folder.`
					: 'Its file was already gone: only its data was deleted.',
		})
	}
	if (result.kept.length > 0) {
		const [first] = result.kept
		toast.error(
			result.kept.length === 1
				? `Kept “${first?.title}” in Deleted books`
				: `Kept ${books(result.kept.length)} in Deleted books`,
			{ description: first?.error, duration: 10000 },
		)
	}
}

/** Restoring, deleting permanently, emptying, the retention, putting files back */
export function useDeletedBooksActions() {
	const client = useQueryClient()

	// a restored book shows up again everywhere: lists, series, search
	const refreshAll = useCallback(() => client.invalidateQueries(), [client])
	const refreshTrash = useCallback(
		() =>
			Promise.all([
				client.invalidateQueries({ queryKey: ['deletedBooks'] }),
				client.invalidateQueries({ queryKey: ['purgedBooks'] }),
				client.invalidateQueries({ queryKey: ['deletedBooksSettings'] }),
			]),
		[client],
	)

	const { mutateAsync: restoreAsync, isPending: isRestoring } = useGraphQLMutation(
		restoreDeletedBooksMutation,
	)
	const { mutateAsync: purgeAsync, isPending: isPurging } =
		useGraphQLMutation(purgeDeletedBooksMutation)
	const { mutateAsync: emptyAsync, isPending: isEmptying } =
		useGraphQLMutation(emptyDeletedBooksMutation)
	const { mutateAsync: retentionAsync, isPending: isSavingRetention } =
		useGraphQLMutation(updateRetentionMutation)
	const { mutateAsync: putBackAsync, isPending: isPuttingBack } =
		useGraphQLMutation(restorePurgedBookMutation)

	const restore = useCallback(
		async (ids: string[], singleTitle?: string) => {
			try {
				const { restoreDeletedBooks: count } = await restoreAsync({ ids })
				toast.success(
					count === 1 && singleTitle ? `Restored “${singleTitle}”` : `Restored ${books(count)}`,
					{ description: 'Back in the library with its progress, notes and tags.' },
				)
				await refreshAll()
				return true
			} catch (error) {
				toast.error(errorMessage(error, 'Failed to restore'))
				return false
			}
		},
		[restoreAsync, refreshAll],
	)

	const purge = useCallback(
		async (ids: string[], singleTitle?: string) => {
			try {
				const { purgeDeletedBooks: result } = await purgeAsync({ ids })
				announcePurge(result, singleTitle)
				await refreshTrash()
				return true
			} catch (error) {
				toast.error(errorMessage(error, 'Failed to delete permanently'))
				return false
			}
		},
		[purgeAsync, refreshTrash],
	)

	const empty = useCallback(
		async (libraryId?: string) => {
			try {
				const { emptyDeletedBooks: result } = await emptyAsync({ libraryId })
				announcePurge(result)
				await refreshTrash()
				return true
			} catch (error) {
				toast.error(errorMessage(error, 'Failed to empty Deleted books'))
				return false
			}
		},
		[emptyAsync, refreshTrash],
	)

	const setRetention = useCallback(
		async (days: number) => {
			try {
				await retentionAsync({ days })
				toast.success(
					days === 0
						? 'Deleted books are kept until you delete them'
						: `Deleted books are kept ${days === 1 ? '1 day' : `${days} days`}`,
				)
				await refreshTrash()
			} catch (error) {
				toast.error(errorMessage(error, 'Failed to save the retention'))
			}
		},
		[retentionAsync, refreshTrash],
	)

	const putBack = useCallback(
		async (id: number) => {
			try {
				const { restorePurgedBook: book } = await putBackAsync({ id })
				toast.success(`“${book.title}” is back in ${book.libraryName ?? 'its library'}`, {
					description: 'Its folder is being scanned; it returns as a new book.',
				})
				await refreshTrash()
				return true
			} catch (error) {
				toast.error(errorMessage(error, 'Failed to put the file back'))
				return false
			}
		},
		[putBackAsync, refreshTrash],
	)

	return {
		restore,
		purge,
		empty,
		setRetention,
		putBack,
		isRestoring,
		isPurging: isPurging || isEmptying,
		isSavingRetention,
		isPuttingBack,
	}
}
