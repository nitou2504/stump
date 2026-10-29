import { useGraphQL, useGraphQLMutation } from '@stump/client'
import { Button, Heading, Text } from '@stump/components'
import { graphql } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { useLibraryManagement } from '../../context'

const query = graphql(`
	query LibraryRemovedBooks($libraryId: ID!) {
		removedMedia(libraryId: $libraryId) {
			id
			resolvedName
			path
		}
	}
`)

const restoreMutation = graphql(`
	mutation RestoreRemovedBook($id: ID!) {
		restoreMedia(id: $id)
	}
`)

/**
 * Books removed from this library from their book page. Their files are still on disk;
 * restoring makes them visible again.
 */
export default function RemovedBooks() {
	const {
		library: { id },
	} = useLibraryManagement()
	const client = useQueryClient()
	const { data, isLoading } = useGraphQL(query, ['removedMedia', id], { libraryId: id })
	const { mutate: restore, isPending } = useGraphQLMutation(restoreMutation, {
		onSuccess: () => {
			toast.success('Book restored')
			client.invalidateQueries()
		},
		onError: () => toast.error('Failed to restore the book'),
	})

	const books = data?.removedMedia ?? []

	return (
		<div className="gap-4 flex flex-col">
			<div>
				<Heading size="sm">Removed books</Heading>
				<Text size="sm" variant="muted">
					Books removed from their book page. Their files are still on disk; restore one to show it
					in the library again.
				</Text>
			</div>

			{!isLoading && books.length === 0 && (
				<Text size="sm" variant="muted">
					No books have been removed from this library.
				</Text>
			)}

			{books.length > 0 && (
				<ul className="divide-y divide-border rounded-lg border border-border">
					{books.map((book) => (
						<li key={book.id} className="gap-3 px-3 py-2 flex items-center justify-between">
							<div className="min-w-0">
								<Text size="sm" className="font-medium line-clamp-1">
									{book.resolvedName}
								</Text>
								<Text size="xs" variant="muted" className="line-clamp-1">
									{book.path}
								</Text>
							</div>
							<Button
								size="sm"
								variant="outline"
								disabled={isPending}
								onClick={() => restore({ id: book.id })}
							>
								Restore
							</Button>
						</li>
					))}
				</ul>
			)}
		</div>
	)
}
