import { PREFETCH_STALE_TIME, useInfiniteSuspenseGraphQL, useSDK } from '@stump/client'
import { Link } from '@stump/components'
import { BookOverviewSceneQuery, graphql } from '@stump/graphql'
import { useQueryClient } from '@tanstack/react-query'
import { Suspense, useCallback, useEffect, useMemo } from 'react'

import BookCard from '@/components/book/BookCard'
import { usePrefetchBook } from '@/components/book/useBookOverview'
import HorizontalCardList from '@/components/HorizontalCardList'
import { DEFAULT_SERIES_BOOKS_ORDER_BY } from '@/components/series/queries'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

const query = graphql(`
	query BooksInSeries(
		$seriesId: ID!
		$filter: MediaFilterInput!
		$orderBy: [MediaOrderBy!]!
		$pagination: Pagination!
	) {
		seriesById(id: $seriesId) {
			id
			resolvedName
		}
		media(filter: $filter, orderBy: $orderBy, pagination: $pagination) {
			nodes {
				id
				...BookCard
			}
			pageInfo {
				__typename
				... on OffsetPaginationInfo {
					currentPage
					totalPages
					totalItems
					pageSize
					pageOffset
					zeroBased
				}
			}
		}
	}
`)

const PAGE_SIZE = 50

const queryKey = (seriesId: string) => ['booksInSeries', seriesId]

/** Every book of the series, in the series page's default order (number, language, then name) */
const queryVariables = (seriesId: string) => ({
	seriesId,
	filter: { seriesId: { eq: seriesId } },
	orderBy: DEFAULT_SERIES_BOOKS_ORDER_BY,
	pagination: { offset: { page: 1, pageSize: PAGE_SIZE } },
})

/**
 * Prefetches the series row of a book page. Cards only know the book, so its overview
 * (which the caller prefetches too, in a single request) tells which series to load
 */
export const usePrefetchBooksInSeries = () => {
	const { sdk } = useSDK()
	const client = useQueryClient()
	const prefetchBook = usePrefetchBook()

	return async (bookId: string) => {
		await prefetchBook(bookId)
		const seriesId = client.getQueryData<BookOverviewSceneQuery>(
			sdk.cacheKey('bookOverview', [bookId]),
		)?.mediaById?.seriesId
		if (!seriesId) return

		const { pagination, ...variables } = queryVariables(seriesId)
		await client.prefetchInfiniteQuery({
			queryKey: queryKey(seriesId),
			initialPageParam: pagination,
			queryFn: ({ pageParam }) => sdk.execute(query, { ...variables, pagination: pageParam }),
			staleTime: PREFETCH_STALE_TIME,
		})
	}
}

type Props = {
	bookId: string
	seriesId: string
}

export default function BooksInSeriesContainer(props: Props) {
	return (
		<Suspense>
			<BooksInSeries {...props} />
		</Suspense>
	)
}

/**
 * The whole series of the book being viewed, with that book marked and scrolled into view
 */
function BooksInSeries({ bookId, seriesId }: Props) {
	const { t } = useTranslate()
	const paths = usePaths()

	const variables = useMemo(() => queryVariables(seriesId), [seriesId])
	const { data, hasNextPage, isFetchingNextPage, fetchNextPage } = useInfiniteSuspenseGraphQL(
		query,
		queryKey(seriesId),
		variables,
	)

	const nodes = useMemo(() => data.pages.flatMap((page) => page.media.nodes), [data.pages])
	const series = data.pages[0]?.seriesById
	const pageInfo = data.pages[0]?.media.pageInfo
	const total = pageInfo?.__typename === 'OffsetPaginationInfo' ? pageInfo.totalItems : nodes.length

	const currentIndex = useMemo(() => nodes.findIndex(({ id }) => id === bookId), [nodes, bookId])

	const handleFetchMore = useCallback(() => {
		if (hasNextPage && !isFetchingNextPage) {
			fetchNextPage()
		}
	}, [fetchNextPage, hasNextPage, isFetchingNextPage])

	// In a long series the book can be past the first page: load until it is there, so the
	// row can start at it
	useEffect(() => {
		if (currentIndex === -1) {
			handleFetchMore()
		}
	}, [currentIndex, handleFetchMore])

	// The book is the series' only one (that this user can see)
	if (total <= 1) return null
	if (currentIndex === -1 && hasNextPage) return null

	const cards = nodes.map((node) => (
		<BookCard
			key={node.id}
			fragment={node}
			fullWidth={false}
			showSeriesPosition
			isCurrent={node.id === bookId}
		/>
	))

	return (
		<HorizontalCardList
			title={
				<span className="gap-x-2 min-w-0 flex flex-wrap items-baseline">
					<span>{t('bookOverviewScene.booksInSeries')}</span>
					{series && (
						<Link
							to={paths.seriesOverview(series.id)}
							variant="muted"
							underline={false}
							className="text-sm font-normal min-w-0 truncate hover:underline"
						>
							{series.resolvedName}
						</Link>
					)}
				</span>
			}
			items={cards}
			onFetchMore={handleFetchMore}
			footerHeight={64 + 17}
			initialFocusIndex={currentIndex}
		/>
	)
}
