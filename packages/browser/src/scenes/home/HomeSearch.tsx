import { useGraphQL } from '@stump/client'
import { cn, Text } from '@stump/components'
import { graphql, MediaFilterInput } from '@stump/graphql'
import { SearchIcon } from 'lucide-react'
import { useMemo, useRef, useState } from 'react'
import { useNavigate } from 'react-router'
import { useDebouncedValue } from 'rooks'

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
 * A search box at the top of the home page: shows the first few matching books while
 * typing, and opens the full results (the books page) on Enter or "See all".
 */
export default function HomeSearch() {
	const paths = usePaths()
	const navigate = useNavigate()
	const containerRef = useRef<HTMLDivElement>(null)

	const [value, setValue] = useState('')
	const [isOpen, setIsOpen] = useState(false)
	const [debounced] = useDebouncedValue(value.trim(), 300)
	const searchFilter = useSearchMediaFilter(debounced || undefined)
	const filter = useMemo<MediaFilterInput>(() => ({ _or: searchFilter ?? [] }), [searchFilter])

	const { data, isFetching } = useGraphQL(
		query,
		['homeSearch', debounced],
		{ filter },
		{
			enabled: debounced.length >= 2,
			placeholderData: (prev) => prev,
		},
	)
	const books = debounced.length >= 2 ? (data?.media.nodes ?? []) : []
	const totalPages =
		data?.media.pageInfo.__typename === 'OffsetPaginationInfo' ? data.media.pageInfo.totalPages : 0

	// The books page reads `search` URL-decoded once more than URLSearchParams does
	const resultsHref = (search: string) =>
		`${paths.bookSearch()}?${new URLSearchParams({ search: encodeURIComponent(search) })}`

	const showDropdown = isOpen && debounced.length >= 2

	return (
		<div
			ref={containerRef}
			className="max-w-2xl relative w-full"
			onBlur={(e) => {
				if (!containerRef.current?.contains(e.relatedTarget as Node)) setIsOpen(false)
			}}
		>
			<form
				role="search"
				onSubmit={(e) => {
					e.preventDefault()
					if (value.trim()) navigate(resultsHref(value.trim()))
				}}
			>
				<label className="gap-2 px-3 h-10 flex items-center rounded-lg border border-input bg-background focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/30">
					<SearchIcon className="h-4 w-4 shrink-0 text-muted-foreground" />
					<input
						type="search"
						value={value}
						placeholder="Search books, authors…"
						aria-label="Search books"
						className="text-sm min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted-foreground"
						onChange={(e) => {
							setValue(e.target.value)
							setIsOpen(true)
						}}
						onFocus={() => setIsOpen(true)}
						onKeyDown={(e) => e.key === 'Escape' && setIsOpen(false)}
					/>
				</label>
			</form>

			{showDropdown && (
				<div className="left-0 right-0 mt-1 p-1 shadow-lg absolute z-50 rounded-lg border border-border bg-popover">
					{books.length === 0 ? (
						<Text size="sm" variant="muted" className="px-3 py-2">
							{isFetching ? 'Searching…' : `No books match “${debounced}”`}
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
								to={resultsHref(debounced)}
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
