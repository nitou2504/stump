import { useGraphQL, useSDK } from '@stump/client'
import { graphql, type TypedDocumentString } from '@stump/graphql'
import {
	keepPreviousData,
	useMutation,
	useQueryClient,
	type UseQueryResult,
} from '@tanstack/react-query'

/** What a query document answers with */
type ResultOf<T> = T extends TypedDocumentString<infer Result, never> ? Result : never

/** The start of the keys of every query of the page: changing the sharing reloads them all */
export const READING_HISTORY_KEY = 'readingHistory'

/** How many finished books a page holds (the server gives at most 200 a page) */
export const FINISHED_PAGE_SIZE = 48

/** What the person lets the others see, and who lets them see theirs */
const people = graphql(`
	query ReadingHistoryPeople {
		myReadingPrivacy {
			shareReadingHistory
		}
		readersSharingHistory {
			id
			username
			avatarUrl
		}
	}
`)

/**
 * Whose history this is. It fails the same way for a person who doesn't exist and one who
 * doesn't share, so the page can't tell them apart either
 */
const reader = graphql(`
	query ReadingHistoryReader($userId: ID) {
		readingHistory(userId: $userId) {
			isSelf
			reader {
				id
				username
				avatarUrl
			}
		}
	}
`)

const readingNow = graphql(`
	query ReadingHistoryNow($userId: ID) {
		readingHistory(userId: $userId) {
			reading {
				percentage
				lastReadOn
				readthroughNumber
				media {
					id
					resolvedName
					thumbnail {
						url
					}
				}
			}
		}
	}
`)

const finished = graphql(`
	query ReadingHistoryFinished(
		$userId: ID
		$search: String
		$year: Int
		$page: Int!
		$pageSize: Int!
	) {
		readingHistory(userId: $userId) {
			years(search: $search) {
				year
				count
			}
			finished(search: $search, year: $year, pagination: { page: $page, pageSize: $pageSize }) {
				nodes {
					readthroughNumber
					startedOn
					finishedOn
					media {
						id
						resolvedName
						thumbnail {
							url
						}
						metadata {
							writers
						}
					}
				}
				pageInfo {
					totalItems
					totalPages
					currentPage
				}
			}
		}
	}
`)

const updatePrivacy = graphql(`
	mutation UpdateMyReadingPrivacy($share: Boolean!) {
		updateMyReadingPrivacy(shareReadingHistory: $share) {
			shareReadingHistory
		}
	}
`)

/** What the person lets the others see, and who lets them see theirs */
export function useReadingPeople(): UseQueryResult<ResultOf<typeof people>> {
	return useGraphQL(people, [READING_HISTORY_KEY, 'people'])
}

/** `userId` is a friend's, or nothing for the person's own */
export function useReader(userId?: string): UseQueryResult<ResultOf<typeof reader>> {
	return useGraphQL(reader, [READING_HISTORY_KEY, 'reader', userId ?? 'me'], { userId })
}

export function useReadingNow(userId?: string): UseQueryResult<ResultOf<typeof readingNow>> {
	return useGraphQL(readingNow, [READING_HISTORY_KEY, 'now', userId ?? 'me'], { userId })
}

export type FinishedFilters = {
	search?: string
	year?: number
	page: number
}

/**
 * A page of the finished books and the years to pick from. The page before stays on screen while
 * the next one loads, so the list doesn't flash while searching or turning pages
 */
export function useFinished(
	userId: string | undefined,
	{ search, year, page }: FinishedFilters,
): UseQueryResult<ResultOf<typeof finished>> {
	return useGraphQL(
		finished,
		[READING_HISTORY_KEY, 'finished', userId ?? 'me', search ?? '', year ?? 0, page],
		{ userId, search: search || undefined, year, page, pageSize: FINISHED_PAGE_SIZE },
		{ placeholderData: keepPreviousData },
	)
}

/** Lets everyone see the person's reading, or stops it; every query of the page reloads after */
export function useUpdateReadingPrivacy() {
	const { sdk } = useSDK()
	const client = useQueryClient()

	return useMutation({
		mutationFn: (share: boolean) => sdk.execute(updatePrivacy, { share }),
		onSuccess: () => client.invalidateQueries({ queryKey: [READING_HISTORY_KEY] }),
	})
}
