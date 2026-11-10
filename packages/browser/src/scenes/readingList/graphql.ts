import { useSDK } from '@stump/client'
import { graphql } from '@stump/graphql'
import { Api } from '@stump/sdk'
import { useInfiniteQuery } from '@tanstack/react-query'

import { READING_LISTS_KEY } from '@/components/readingList/queries'

/** How many of a list's books a page asks for (the server gives at most 200 at a time) */
export const READING_LIST_PAGE_SIZE = 100

/** What the list's page shows above its books: who made it, who sees it, what it may do */
export const readingListQuery = graphql(`
	query ReadingListScene($id: ID!) {
		readingListById(id: $id) {
			id
			name
			description
			visibility
			canEdit
			creatingUserId
			bookCount
			creator {
				id
				username
				avatarUrl
			}
		}
	}
`)

/** A page of the list's books, in the list's order */
export const readingListBooksQuery = graphql(`
	query ReadingListBooks($id: ID!, $limit: Int!, $offset: Int!) {
		readingListById(id: $id) {
			id
			books(limit: $limit, offset: $offset) {
				id
				resolvedName
				...BookCard
			}
		}
	}
`)

const readingListBookIdsQuery = graphql(`
	query ReadingListBookIds($id: ID!, $limit: Int!, $offset: Int!) {
		readingListById(id: $id) {
			id
			books(limit: $limit, offset: $offset) {
				id
			}
		}
	}
`)

/**
 * The list's books a page at a time: `fetchNextPage` loads the next one (*Load more*) and
 * `hasNextPage` says whether there is one. The key starts with the reading list key, so any
 * change to a list reloads it
 */
export function useReadingListBooks(id: string) {
	const { sdk } = useSDK()
	return useInfiniteQuery({
		queryKey: [READING_LISTS_KEY, 'books', id],
		queryFn: ({ pageParam }) =>
			sdk.execute(readingListBooksQuery, {
				id,
				limit: READING_LIST_PAGE_SIZE,
				offset: pageParam,
			}),
		initialPageParam: 0,
		getNextPageParam: (lastPage, pages) =>
			lastPage.readingListById.books.length < READING_LIST_PAGE_SIZE
				? undefined
				: pages.length * READING_LIST_PAGE_SIZE,
		enabled: !!id,
	})
}

/** The most books of a list that *Save a copy* reads: a list of the server owner's can be longer */
const MAX_COPIED_BOOKS = 10_000
const ID_PAGE_SIZE = 200

/** The ids of every book of the list the user can see, in the list's order */
export async function fetchReadingListBookIds(sdk: Api, id: string): Promise<string[]> {
	const ids: string[] = []
	while (ids.length < MAX_COPIED_BOOKS) {
		const { readingListById } = await sdk.execute(readingListBookIdsQuery, {
			id,
			limit: ID_PAGE_SIZE,
			offset: ids.length,
		})
		const page = readingListById.books
		ids.push(...page.map((book) => book.id))
		if (page.length < ID_PAGE_SIZE) break
	}
	return ids
}
