import {
	MediaFilterInput,
	MediaMetadataModelOrdering,
	MediaModelOrdering,
	MediaOrderBy,
	OrderDirection,
	SeriesFilterInput,
	SeriesModelOrdering,
	SeriesOrderBy,
} from '@stump/graphql'
import { toUrlParams } from '@stump/sdk'
import { useCallback, useMemo } from 'react'
import { useSearchParams } from 'react-router'
import { useMediaMatch } from 'rooks'

import { FilterInput, IFilterContext, Ordering, OrderingField } from './context'

type Return = IFilterContext
type UseFilterSceneOptions = {
	persistedOrdering?: Ordering
	setPersistedOrdering?: (ordering: Ordering) => void
	defaultOrdering?: Ordering
}

export const SERIES_POSITION_ORDERING: Ordering = {
	orderBy: MediaMetadataModelOrdering.Number,
	direction: OrderDirection.Asc,
}

export const DEFAULT_SERIES_ORDER_BY: SeriesOrderBy[] = [
	{ series: { field: SeriesModelOrdering.Name, direction: OrderDirection.Asc } },
] as SeriesOrderBy[]

export const DEFAULT_MEDIA_ORDER_BY: MediaOrderBy[] = [
	{ media: { field: MediaModelOrdering.Name, direction: OrderDirection.Asc } },
] as MediaOrderBy[]

export const useURLPageParams = () => {
	const [searchParams, setSearchParams] = useSearchParams()

	const is3XLScreenOrBigger = useMediaMatch('(min-width: 1600px)')
	const defaultPageSize = is3XLScreenOrBigger ? 40 : 20

	const pagination = useMemo(
		() => ({
			page: searchParams.get('page') ? parseInt(searchParams.get('page') as string) : 1,
			pageSize: searchParams.get('pageSize')
				? parseInt(searchParams.get('pageSize') as string)
				: defaultPageSize,
		}),
		[searchParams, defaultPageSize],
	)

	const setPage = useCallback(
		(page: number) => {
			setSearchParams((prev) => {
				prev.set('page', page.toString())
				return prev
			})
		},
		[setSearchParams],
	)

	const setPageSize = useCallback(
		(pageSize: number) => {
			setSearchParams((prev) => {
				prev.set('pageSize', pageSize.toString())
				return prev
			})
		},
		[setSearchParams],
	)

	return { ...pagination, setPage, setPageSize }
}

/**
 * The `search` URL param. It is stored URL-encoded once more than URLSearchParams does
 * (links like the home search's "See all" rely on that); values that aren't valid encoded
 * strings are read as they are.
 */
export function readSearchParam(params: URLSearchParams): string {
	const value = params.get('search')
	if (!value) return ''
	try {
		return decodeURIComponent(value)
	} catch {
		return value
	}
}

export function writeSearchParam(params: URLSearchParams, value: string | undefined) {
	if (value) {
		params.set('search', encodeURIComponent(value))
	} else {
		params.delete('search')
	}
}

export const useURLKeywordSearch = () => {
	const [searchParams, setSearchParams] = useSearchParams()

	const search = useMemo(() => readSearchParam(searchParams), [searchParams])

	const setSearch = useCallback(
		(newSearch: string) => {
			setSearchParams(
				(prev) => {
					writeSearchParam(prev, newSearch)
					return prev
				},
				{ replace: true },
			)
		},
		[setSearchParams],
	)

	const removeSearch = useCallback(() => {
		setSearchParams(
			(prev) => {
				prev.delete('search')
				return prev
			},
			{ replace: true },
		)
	}, [setSearchParams])

	return { search, setSearch, removeSearch }
}

/**
 * The server-side search for books: every word of `search` must appear in the file name or
 * path, title, series, writers, publisher, genres or tags, ignoring case and accents
 */
export function useSearchMediaFilter(search: string | undefined): MediaFilterInput[] | undefined {
	return useMemo(() => (search?.trim() ? [{ search }] : undefined), [search])
}

/** The server-side search for series: every word in the series name or title */
export function useSearchSeriesFilter(search: string | undefined): SeriesFilterInput[] | undefined {
	return useMemo(() => (search?.trim() ? [{ search }] : undefined), [search])
}

export function useFilterScene({
	persistedOrdering,
	setPersistedOrdering,
	defaultOrdering,
}: UseFilterSceneOptions = {}): Return {
	const [searchParams, setSearchParams] = useSearchParams()
	// The search lives in the URL, so going back to the page restores it
	const search = useMemo(() => readSearchParam(searchParams) || undefined, [searchParams])

	const is3XLScreenOrBigger = useMediaMatch('(min-width: 1600px)')
	const defaultPageSize = is3XLScreenOrBigger ? 40 : 20

	/**
	 * An object representation of the url params without the excluded keys, such as
	 * orderBy, direction, search, page, and pageSize.
	 */
	const filters = useMemo(() => {
		const filtersJsonStr = searchParams.get('filters')
		const filters: FilterInput = filtersJsonStr ? JSON.parse(filtersJsonStr) : {}
		return filters
	}, [searchParams])

	/**
	 * An object representation of the ordering params
	 */
	const ordering = useMemo(() => {
		const orderBy = searchParams.get('orderBy') as OrderingField | null
		const direction = searchParams.get('direction') as OrderDirection | null

		return orderBy && direction
			? { orderBy, direction }
			: (persistedOrdering ?? defaultOrdering ?? {})
	}, [searchParams, persistedOrdering, defaultOrdering])

	/**
	 * An object representation of the pagination params
	 */
	const pagination = useMemo(
		() => ({
			page: searchParams.get('page') ? parseInt(searchParams.get('page') as string) : 1,
			pageSize: searchParams.get('pageSize')
				? parseInt(searchParams.get('pageSize') as string)
				: defaultPageSize,
		}),
		[searchParams, defaultPageSize],
	)

	const setOrdering = useCallback(
		(newOrdering: Ordering) => {
			setPersistedOrdering?.(newOrdering)
			setSearchParams(
				toUrlParams(
					{
						...pagination,
						...newOrdering,
						filters: JSON.stringify(filters),
						search: searchParams.get('search') ?? undefined,
					},
					undefined,
					{ removeEmpty: true },
				),
			)
		},
		[setSearchParams, searchParams, pagination, filters, setPersistedOrdering],
	)

	const setPage = useCallback(
		(page: number) => {
			setSearchParams((prev) => {
				prev.set('page', page.toString())
				return prev
			})
		},
		[setSearchParams],
	)

	/**
	 * Replace the current filters with the provided filters
	 */
	const handleSetFilters = useCallback(
		(newFilters: FilterInput) => {
			setSearchParams(
				toUrlParams(
					{
						...ordering,
						...pagination,
						filters: JSON.stringify(newFilters),
						search: searchParams.get('search') ?? undefined,
					},
					undefined,
					{ removeEmpty: true },
				),
			)
		},
		[ordering, pagination, searchParams, setSearchParams],
	)

	/**
	 * Sets a single filter in the url with the provided value
	 */
	const handleSetSearch = useCallback(
		(value: string) => {
			setSearchParams(
				(prev) => {
					writeSearchParam(prev, value)
					prev.delete('page')
					return prev
				},
				// typing doesn't add history entries; back leaves the page with its search
				{ replace: true },
			)
		},
		[setSearchParams],
	)

	/**
	 * Removes a filter from the url
	 */
	const removeSearch = useCallback(() => {
		setSearchParams(
			(prev) => {
				prev.delete('search')
				prev.delete('page')
				return prev
			},
			{ replace: true },
		)
	}, [setSearchParams])

	return {
		filters,
		ordering,
		pagination,
		removeSearch,
		search,
		setSearch: handleSetSearch,
		setFilters: handleSetFilters,
		setOrdering,
		setPage,
	}
}

const MEDIA_METADATA_ORDER_FIELDS: string[] = [
	MediaMetadataModelOrdering.Number,
	MediaMetadataModelOrdering.Volume,
	MediaMetadataModelOrdering.Year,
	MediaMetadataModelOrdering.Rating,
	MediaMetadataModelOrdering.AgeRating,
]

export function useMediaURLOrderBy(ordering: Ordering): MediaOrderBy[] {
	return useMemo(() => {
		if (!ordering || !ordering.orderBy || !ordering.direction) {
			return DEFAULT_MEDIA_ORDER_BY
		}

		if (MEDIA_METADATA_ORDER_FIELDS.includes(ordering.orderBy)) {
			const metadataOrderBy = {
				metadata: {
					field: ordering.orderBy as MediaMetadataModelOrdering,
					direction: ordering.direction as OrderDirection,
				},
			} as MediaOrderBy
			return ordering.orderBy === MediaMetadataModelOrdering.Number
				? [metadataOrderBy, ...DEFAULT_MEDIA_ORDER_BY]
				: [metadataOrderBy]
		}

		return [
			{
				media: {
					field: ordering.orderBy as MediaModelOrdering,
					direction: ordering.direction as OrderDirection,
				},
			},
		] as MediaOrderBy[]
	}, [ordering])
}
