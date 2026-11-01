import { useGraphQL } from '@stump/client'
import { cn, Text } from '@stump/components'
import { graphql, MediaFilterInput } from '@stump/graphql'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router'

import { Search } from '@/components/filters'
import { useSearchMediaFilter } from '@/components/filters/useFilterScene'
import { ThumbnailImage } from '@/components/thumbnail/ThumbnailImage'
import { Link } from '@/context'
import { usePaths } from '@/paths'

const query = graphql(`
	query HomeSearch($filter: MediaFilterInput!) {
		media(filter: $filter, pagination: { offset: { page: 1, pageSize: 6 } }) {
			nodes {
				id
				resolvedName
				thumbnail {
					url
				}
				metadata {
					writers
					rating
				}
			}
			pageInfo {
				__typename
				... on OffsetPaginationInfo {
					totalPages
				}
			}
		}
	}
`)

/**
 * The home page's search, in the same toolbar and box as the list pages' searches: shows the
 * first few matching books while typing, and opens the full results (the books page) on Enter
 * or "See all".
 */
export default function HomeSearch() {
	const paths = usePaths()
	const navigate = useNavigate()
	const containerRef = useRef<HTMLDivElement>(null)

	// The query is kept in the URL (`?q=`), so coming back from a result reopens the search
	const [searchParams, setSearchParams] = useSearchParams()
	const initialQuery = searchParams.get('q') ?? ''
	// Already debounced by the search box
	const [term, setTerm] = useState(initialQuery)
	const [isOpen, setIsOpen] = useState(!!initialQuery)

	useEffect(() => {
		if ((searchParams.get('q') ?? '') === term) return
		setSearchParams(
			(prev) => {
				if (term) {
					prev.set('q', term)
				} else {
					prev.delete('q')
				}
				return prev
			},
			{ replace: true },
		)
	}, [term, searchParams, setSearchParams])
	const searchFilter = useSearchMediaFilter(term || undefined)
	const filter = useMemo<MediaFilterInput>(() => ({ _or: searchFilter ?? [] }), [searchFilter])

	const { data, isFetching } = useGraphQL(
		query,
		['homeSearch', term],
		{ filter },
		{
			enabled: term.length >= 2,
			placeholderData: (prev) => prev,
		},
	)
	const books = term.length >= 2 ? (data?.media.nodes ?? []) : []
	const totalPages =
		data?.media.pageInfo.__typename === 'OffsetPaginationInfo' ? data.media.pageInfo.totalPages : 0

	// The books page reads `search` URL-decoded once more than URLSearchParams does
	const resultsHref = (search: string) =>
		`${paths.bookSearch()}?${new URLSearchParams({ search: encodeURIComponent(search) })}`

	const showDropdown = isOpen && term.length >= 2

	return (
		<div
			ref={containerRef}
			role="search"
			className="min-w-0 relative flex flex-1 items-center"
			onFocus={() => setIsOpen(true)}
			onInput={() => setIsOpen(true)}
			onBlur={(e) => {
				if (!containerRef.current?.contains(e.relatedTarget as Node)) setIsOpen(false)
			}}
			onKeyDown={(e) => {
				if (e.key === 'Escape') {
					setIsOpen(false)
				} else if (e.key === 'Enter' && e.target instanceof HTMLInputElement) {
					// Enter opens the full results right away, without waiting for the debounce
					const value = e.target.value.trim()
					if (value) navigate(resultsHref(value))
				}
			}}
		>
			<Search
				initialValue={initialQuery}
				placeholder="Search books, authors…"
				onChange={(value) => setTerm(value.trim())}
				isLoading={isFetching && term.length >= 2}
			/>

			{showDropdown && (
				<div className="left-0 mt-1 p-1 shadow-lg sm:w-2/5 sm:min-w-96 absolute top-full z-50 w-full rounded-lg border border-border bg-popover">
					{books.length === 0 ? (
						<Text size="sm" variant="muted" className="px-3 py-2">
							{isFetching ? 'Searching…' : `No books match “${term}”`}
						</Text>
					) : (
						<>
							{books.map((book) => {
								const rating = book.metadata?.rating != null ? Number(book.metadata.rating) : null
								return (
									<Link
										key={book.id}
										to={paths.bookOverview(book.id)}
										className="gap-3 px-2 py-1.5 flex items-center rounded-md hover:bg-accent focus:bg-accent focus:outline-none"
										onClick={() => setIsOpen(false)}
									>
										<ThumbnailImage
											src={book.thumbnail.url}
											alt={book.resolvedName}
											size={{ width: 32, height: 48 }}
											lazy
										/>
										<div className="min-w-0 flex flex-1 flex-col">
											<Text size="sm" className="font-medium line-clamp-1">
												{book.resolvedName}
											</Text>
											<Text size="xs" variant="muted" className="line-clamp-1">
												{[
													book.metadata?.writers?.join(', '),
													rating && rating > 0 ? `★ ${rating.toFixed(1)}` : null,
												]
													.filter(Boolean)
													.join(' · ')}
											</Text>
										</div>
									</Link>
								)
							})}
							<Link
								to={resultsHref(term)}
								className={cn(
									'mt-1 px-2 py-1.5 text-sm block rounded-md border-t border-border text-muted-foreground',
									'hover:bg-accent hover:text-foreground',
								)}
								onClick={() => setIsOpen(false)}
							>
								{totalPages > 1 ? 'See all results' : 'Open in Explore'} →
							</Link>
						</>
					)}
				</div>
			)}
		</div>
	)
}
