import {
	PREFETCH_STALE_TIME,
	queryClient,
	useGraphQL,
	useGraphQLFetcher,
	useGraphQLMutation,
	useSDK,
	useSuspenseGraphQL,
} from '@stump/client'
import {
	EntityVisibility,
	graphql,
	MediaFilterInput,
	SaveSmartListInput,
	SmartList,
	SmartListFilterGroupInput,
	SmartListGrouping,
	SmartListItemsQuery,
	SmartListMetaQuery,
} from '@stump/graphql'
import { Api } from '@stump/sdk'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useMemo } from 'react'
export type SmartListParsed = Omit<SmartList, 'filters'> & {
	filters: Array<SmartListFilterGroupInput>
}

export const DEFAULT_META_CACHE_TIME = 1000 * 60 * 15 // 15 minutes

const listByIdQuery = graphql(`
	query SmartListById($id: ID!) {
		smartListById(id: $id) {
			id
			creatorId
			canEdit
			creator {
				id
				username
			}
			description
			defaultGrouping
			filters
			joiner
			name
			visibility
			views {
				id
				listId
				name
				bookColumns {
					id
					position
				}
				bookSorting {
					id
					desc
				}
				groupColumns {
					id
					position
				}
				groupSorting {
					id
					desc
				}
				search
			}
		}
	}
`)

const smartListMetaQuery = graphql(`
	query SmartListMeta($id: ID!) {
		smartListMeta(id: $id) {
			matchedBooks
			matchedSeries
			matchedLibraries
		}
	}
`)

const smartListUpdateMutation = graphql(`
	mutation UpdateSmartList($id: ID!, $input: SaveSmartListInput!) {
		updateSmartList(id: $id, input: $input) {
			__typename
		}
	}
`)

const smartListItemsQuery = graphql(`
	query SmartListItems(
		$id: ID!
		$filter: MediaFilterInput
		$grouping: SmartListGrouping
		$limit: Int
		$offset: Int
	) {
		smartListItems(id: $id, filter: $filter, grouping: $grouping, limit: $limit, offset: $offset) {
			__typename
			... on SmartListGrouped {
				items {
					entity {
						__typename
						... on Series {
							id
							name
						}
						... on Library {
							id
							name
						}
					}
					books {
						...BookCard
						...BookMetadata
						path
						tags {
							id
							name
						}
					}
				}
			}
			... on SmartListUngrouped {
				books {
					...BookCard
					...SmartListItemBookMetadata
					path
					tags {
						id
						name
					}
				}
			}
		}
	}
`)

export const smartListMediaItemMetadataFragment = graphql(`
	fragment SmartListItemBookMetadata on Media {
		metadata {
			ageRating
			rating
			characters
			colorists
			coverArtists
			editors
			genres
			inkers
			letterers
			links
			pencillers
			publisher
			teams
			writers
			year
			month
			day
			format
			identifierAmazon
			identifierCalibre
			identifierGoogle
			identifierIsbn
			identifierMobiAsin
			identifierUuid
			language
			notes
			number
			pageCount
			series
			seriesGroup
			storyArc
			storyArcNumber
			title
			titleSort
			volume
		}
	}
`)

/**
 * The server hands out a smart list at most 200 books at a time, so a list is read page by page
 * (see {@link fetchSmartListItems}). A query's cost is checked before it runs, and a page costs
 * its size times the cost of one book: 137 for what the book fragments below select (both
 * groupings are in the one document), against a limit of 20,000. 100 books a page cost 13,700,
 * which leaves room for a few more fields. crates/graphql/src/complexity.rs checks this
 * document with this page size, so a heavier page fails there before it fails here.
 */
export const SMART_LIST_PAGE_SIZE = 100

/** A smart list longer than this many pages (50,000 books) is cut, rather than read for ever */
const SMART_LIST_MAX_PAGES = 500

type SmartListItems = SmartListItemsQuery['smartListItems']
type SmartListGroup = Extract<SmartListItems, { __typename: 'SmartListGrouped' }>['items'][number]

const bookCount = (page: SmartListItems) =>
	page.__typename === 'SmartListGrouped'
		? page.items.reduce((count, item) => count + item.books.length, 0)
		: page.books.length

// The books' fields come with the BookCard fragment, so their types don't show the id that
// every book has (the scene casts them to Media for the same reason)
const bookId = (book: object) => (book as { id: string }).id

/**
 * Puts the pages of a smart list back together: the books of an ungrouped list one after the
 * other, and a group (a series, a library) that runs over two pages in one group again. A book
 * is only shown once.
 */
export function mergeSmartListPages(pages: [SmartListItems, ...SmartListItems[]]): SmartListItems {
	const seen = new Set<string>()
	const newBooks = <T extends object>(books: T[]) => {
		const fresh: T[] = []
		for (const book of books) {
			const id = bookId(book)
			if (seen.has(id)) continue
			seen.add(id)
			fresh.push(book)
		}
		return fresh
	}

	const [first] = pages
	if (first.__typename === 'SmartListUngrouped') {
		return {
			...first,
			books: pages.flatMap((page) =>
				page.__typename === 'SmartListUngrouped' ? newBooks(page.books) : [],
			),
		}
	}

	const groups = new Map<string, SmartListGroup>()
	for (const page of pages) {
		if (page.__typename !== 'SmartListGrouped') continue
		for (const item of page.items) {
			const key = item.entity ? `${item.entity.__typename}:${item.entity.id}` : ''
			const books = newBooks(item.books)
			const group = groups.get(key)
			if (group) {
				group.books.push(...books)
			} else {
				groups.set(key, { ...item, books })
			}
		}
	}
	return { ...first, items: [...groups.values()] }
}

/**
 * Reads a whole smart list: page after page, until one comes back with fewer books than a page
 * holds. The server gives the books of every page in the same order, so no book is left out.
 */
export async function fetchSmartListItems(
	sdk: Api,
	variables: { id: string; filter?: MediaFilterInput; grouping?: SmartListGrouping },
): Promise<SmartListItems> {
	const fetchPage = async (page: number) => {
		const { smartListItems } = await sdk.execute(smartListItemsQuery, {
			...variables,
			limit: SMART_LIST_PAGE_SIZE,
			offset: page * SMART_LIST_PAGE_SIZE,
		})
		return smartListItems
	}

	let page = await fetchPage(0)
	const pages: [SmartListItems, ...SmartListItems[]] = [page]
	for (
		let next = 1;
		next < SMART_LIST_MAX_PAGES && bookCount(page) >= SMART_LIST_PAGE_SIZE;
		next++
	) {
		page = await fetchPage(next)
		pages.push(page)
	}
	return mergeSmartListPages(pages)
}

const smartListItemsKey = (
	sdk: Api,
	id: string,
	filter?: MediaFilterInput,
	grouping?: SmartListGrouping,
) => [sdk.cacheKeys.smartListItems, id, filter ?? null, grouping ?? null]

export function useSmartListItems({
	id,
	filter,
	grouping,
	enabled = true,
}: {
	id: string
	filter?: MediaFilterInput
	grouping?: SmartListGrouping
	enabled?: boolean
}) {
	const { sdk } = useSDK()
	const activeFilter = !!filter && Object.keys(filter).length > 0 ? filter : undefined
	const { data: items, isLoading } = useGraphQLFetcher(
		smartListItemsKey(sdk, id, activeFilter, grouping),
		(sdk) => fetchSmartListItems(sdk, { id, filter: activeFilter, grouping }),
		// Keep showing the current books while a new filter loads
		{ enabled: enabled && !!id, placeholderData: (prev) => prev },
	)

	return {
		items,
		isLoading,
	}
}

export function usePrefetchSmartList() {
	const { sdk } = useSDK()
	const prefetch = useCallback(
		({ id }: { id: string }) => {
			Promise.all([
				queryClient.prefetchQuery({
					queryKey: [sdk.cacheKeys.smartListById, id],
					queryFn: async () => {
						const response = await sdk.execute(listByIdQuery, { id })
						return response
					},
					staleTime: PREFETCH_STALE_TIME,
				}),
				queryClient.prefetchQuery({
					queryKey: [sdk.cacheKeys.smartListMeta, id],
					queryFn: async () => {
						const response = await sdk.execute(smartListMetaQuery, { id })
						return response
					},
					staleTime: DEFAULT_META_CACHE_TIME,
				}),
				queryClient.prefetchQuery({
					// the key of the list as the scene asks for it first, or nothing would find this
					queryKey: smartListItemsKey(sdk, id),
					queryFn: () => fetchSmartListItems(sdk, { id }),
					staleTime: PREFETCH_STALE_TIME,
				}),
			])
		},
		[sdk],
	)

	return { prefetch }
}

export function useUpdateSmartList({ id, list }: { id: string; list?: SmartListParsed }): {
	update: (updates: Partial<SaveSmartListInput>) => Promise<void>
} {
	const { sdk } = useSDK()
	// A list is saved whole: its current details with the changes on top. Only what the input takes
	// is sent (the list also carries who made it, whether the viewer can edit it, its views...)
	const currentDetails = useMemo(() => {
		if (!list) return {}

		const { name, description, joiner, defaultGrouping, visibility, filters } = list
		return {
			name,
			description,
			defaultGrouping,
			visibility,
			joiner: joiner?.toUpperCase() as typeof joiner,
			filters: filters?.map((group) => ({
				...group,
				joiner: group.joiner.toUpperCase() as typeof group.joiner,
			})),
		}
	}, [list])

	const client = useQueryClient()
	const { mutate } = useGraphQLMutation(smartListUpdateMutation, {
		mutationKey: [sdk.cacheKeys.smartListUpdate, id],
		onSuccess: () => {
			client.invalidateQueries({ queryKey: [sdk.cacheKeys.smartListById, id || ''] })
		},
	})
	const update = useCallback(
		async (updates: Partial<SaveSmartListInput>) => {
			mutate({ id, input: { ...currentDetails, ...updates } as SaveSmartListInput })
		},
		[mutate, id, currentDetails],
	)

	return {
		update,
	}
}

/**
 * Takes someone else's public smart list back to private: what the server owner may do with a
 * list of a friend (moderation), apart from deleting it. The server applies the visibility only,
 * so the rest of the input is just what the mutation needs to be well formed. The lists in the
 * sidebar and on the lists page are refreshed; the list itself is not (it is not the server
 * owner's to see once it is private, so the page it was on has to be left first)
 */
export function useUnpublishSmartList(list?: SmartListParsed) {
	const { sdk } = useSDK()
	const client = useQueryClient()
	const { mutateAsync, isPending } = useGraphQLMutation(smartListUpdateMutation, {
		mutationKey: [sdk.cacheKeys.smartListUpdate, list?.id, 'unpublish'],
		onSuccess: () => {
			client.invalidateQueries({ queryKey: [sdk.cacheKeys.smartLists] })
		},
	})

	const unpublish = useCallback(async () => {
		if (!list) return
		await mutateAsync({
			id: list.id,
			input: {
				name: list.name,
				description: list.description,
				defaultGrouping: list.defaultGrouping,
				joiner: list.joiner?.toUpperCase() as typeof list.joiner,
				filters: [],
				visibility: EntityVisibility.Private,
			},
		})
	}, [mutateAsync, list])

	return { unpublish, isPending }
}

const setVisibilityMutation = graphql(`
	mutation SetSmartListsVisibility($ids: [ID!]!, $visibility: EntityVisibility!) {
		setSmartListsVisibility(ids: $ids, visibility: $visibility) {
			id
			visibility
		}
	}
`)

/**
 * Makes some of the user's own smart lists public or private in one go. The server changes only
 * what is theirs, all or none; the lists everywhere are refreshed after it.
 */
export function useSetSmartListsVisibility() {
	const { sdk } = useSDK()
	const client = useQueryClient()
	const { mutateAsync, isPending } = useGraphQLMutation(setVisibilityMutation, {
		mutationKey: [sdk.cacheKeys.smartListUpdate, 'visibility'],
		onSuccess: () => {
			client.invalidateQueries({ queryKey: [sdk.cacheKeys.smartLists] })
			client.invalidateQueries({ queryKey: [sdk.cacheKeys.smartListById] })
		},
	})

	const setVisibility = useCallback(
		async (ids: string[], visibility: EntityVisibility) => {
			await mutateAsync({ ids, visibility })
		},
		[mutateAsync],
	)

	return { setVisibility, isPending }
}

type UseSmartListByIdParams = {
	id: string
}

type UseSmartListByIdReturn = {
	list?: SmartListParsed
	isLoading: boolean
}

export function useSmartListById({ id }: UseSmartListByIdParams): UseSmartListByIdReturn {
	const { sdk } = useSDK()
	const { data: { smartListById: listUnparsed } = {}, isLoading: isLoading } = useSuspenseGraphQL(
		listByIdQuery,
		[sdk.cacheKeys.smartListById, id || ''],
		{
			id: id || '',
		},
	)

	const list = useMemo(() => {
		if (listUnparsed) {
			const filtersFromJson = JSON.parse(listUnparsed.filters) as Array<SmartListFilterGroupInput>
			return { ...listUnparsed, filters: filtersFromJson } as SmartListParsed
		} else {
			return undefined
		}
	}, [listUnparsed])

	return {
		list,
		isLoading,
	}
}

type UseSmartListMetaParams = {
	id: string
	staleTime?: number
}

type UseSmartListMetaReturn = {
	meta?: SmartListMetaQuery['smartListMeta']
	isLoading: boolean
}

export function useSmartListMeta({
	id,
	staleTime,
}: UseSmartListMetaParams): UseSmartListMetaReturn {
	const { sdk } = useSDK()
	const options = staleTime ? { staleTime } : {}
	const { data: { smartListMeta: meta } = {}, isLoading: isLoading } = useGraphQL(
		smartListMetaQuery,
		[sdk.cacheKeys.smartListMeta, id || ''],
		{
			id: id || '',
		},
		{ ...options, enabled: !!id },
	)

	return {
		meta,
		isLoading,
	}
}
