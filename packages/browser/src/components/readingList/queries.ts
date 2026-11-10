import { useGraphQLMutation } from '@stump/client'
import { EntityVisibility, graphql } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback } from 'react'

/** Every reading list query key starts with it, so one change refreshes them all */
export const READING_LISTS_KEY = 'readingLists'

/** The user's lists a book is in (only their ids: enough to mark the book page's list button) */
export const bookReadingListIdsQuery = graphql(`
	query BookReadingListIds($id: ID!) {
		mediaById(id: $id) {
			id
			readingListIds
		}
	}
`)

/**
 * The reading lists to add a book to, loaded when the add-to-list menu opens: only the user's
 * own (the lists other people made public are theirs to change)
 */
export const pickerReadingListsQuery = graphql(`
	query PickerReadingLists {
		readingLists(scope: MINE, pagination: { none: { unpaginated: true } }) {
			nodes {
				id
				name
				canEdit
				bookCount
			}
		}
	}
`)

const createMutation = graphql(`
	mutation CreateReadingList($input: ReadingListInput!) {
		createReadingList(input: $input) {
			id
			name
		}
	}
`)

const updateMutation = graphql(`
	mutation UpdateReadingList($id: ID!, $input: UpdateReadingListInput!) {
		updateReadingList(id: $id, input: $input) {
			id
		}
	}
`)

const deleteMutation = graphql(`
	mutation DeleteReadingList($id: String!) {
		deleteReadingList(id: $id) {
			id
		}
	}
`)

const addBooksMutation = graphql(`
	mutation AddBooksToReadingList($id: ID!, $mediaIds: [ID!]!) {
		addBooksToReadingList(id: $id, mediaIds: $mediaIds) {
			id
		}
	}
`)

const removeBooksMutation = graphql(`
	mutation RemoveBooksFromReadingList($id: ID!, $mediaIds: [ID!]!) {
		removeBooksFromReadingList(id: $id, mediaIds: $mediaIds) {
			id
		}
	}
`)

/** Sorts lists by name, the way people read them (`List 2` before `List 10`) */
export const byName = (a: { name: string }, b: { name: string }) =>
	a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' })

/** Who sees a list: only its creator, or everyone on the server */
export const visibilityOf = (isPublic: boolean) =>
	isPublic ? EntityVisibility.Public : EntityVisibility.Private

/**
 * Creating, editing and deleting reading lists, and adding books to them or removing them.
 * Each change refreshes every reading list query once it's saved; errors are thrown to the
 * caller
 */
export function useReadingListActions() {
	const client = useQueryClient()
	const refresh = useCallback(
		() => client.invalidateQueries({ queryKey: [READING_LISTS_KEY] }),
		[client],
	)

	const { mutateAsync: create, isPending: isCreating } = useGraphQLMutation(createMutation)
	const { mutateAsync: update, isPending: isUpdating } = useGraphQLMutation(updateMutation)
	const { mutateAsync: remove, isPending: isDeleting } = useGraphQLMutation(deleteMutation)
	const { mutateAsync: addBooks } = useGraphQLMutation(addBooksMutation)
	const { mutateAsync: removeBooks } = useGraphQLMutation(removeBooksMutation)

	const createList = useCallback(
		async (input: {
			name: string
			description?: string
			visibility?: EntityVisibility
			mediaIds?: string[]
		}) => {
			const { createReadingList } = await create({ input })
			await refresh()
			return createReadingList
		},
		[create, refresh],
	)

	const updateList = useCallback(
		async (
			id: string,
			input: { name: string; description?: string; visibility?: EntityVisibility },
		) => {
			await update({ id, input })
			await refresh()
		},
		[update, refresh],
	)

	/**
	 * The server owner's way to take someone else's public list back to private: the name is
	 * only there because the API asks for one (it keeps the list's own)
	 */
	const unpublishList = useCallback(
		async (list: { id: string; name: string }) => {
			await update({
				id: list.id,
				input: { name: list.name, visibility: EntityVisibility.Private },
			})
			await refresh()
		},
		[update, refresh],
	)

	const deleteList = useCallback(
		async (id: string) => {
			await remove({ id })
			await refresh()
		},
		[remove, refresh],
	)

	const addBook = useCallback(
		async (id: string, mediaId: string) => {
			await addBooks({ id, mediaIds: [mediaId] })
			await refresh()
		},
		[addBooks, refresh],
	)

	const removeBook = useCallback(
		async (id: string, mediaId: string) => {
			await removeBooks({ id, mediaIds: [mediaId] })
			await refresh()
		},
		[removeBooks, refresh],
	)

	return {
		createList,
		updateList,
		unpublishList,
		deleteList,
		addBook,
		removeBook,
		isSaving: isCreating || isUpdating,
		isDeleting,
	}
}
