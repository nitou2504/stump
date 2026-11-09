import { useGraphQL } from '@stump/client'
import { Command, Dialog, ProgressSpinner } from '@stump/components'
import { graphql } from '@stump/graphql'
import { ArrowRight, Dices, Library, Tags, Users } from 'lucide-react'
import { useState } from 'react'
import { useNavigate } from 'react-router'
import { useDebouncedValue } from 'rooks'

import { ThumbnailImage } from '@/components/thumbnail/ThumbnailImage'
import { useSurpriseMe } from '@/hooks/useSurpriseMe'
import { usePaths } from '@/paths'

import { GLOBAL_SEARCH_SHORTCUT } from './shortcut'

const query = graphql(`
	query GlobalSearch($media: MediaFilterInput!, $series: SeriesFilterInput!) {
		media(filter: $media, pagination: { offset: { page: 1, pageSize: 6 } }) {
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
					totalItems
				}
			}
		}
		series(filter: $series, pagination: { offset: { page: 1, pageSize: 3 } }) {
			nodes {
				id
				resolvedName
				mediaCount
				thumbnail {
					url
				}
			}
		}
	}
`)

const MIN_LENGTH = 2

type Props = {
	open: boolean
	onOpenChange: (open: boolean) => void
}

/**
 * The title bar search's palette: books and series as you type (one request, debounced),
 * Enter on "See all" (or with nothing picked) opens the full results on the books page.
 * Empty, it offers Surprise me and the browse pages.
 */
export default function GlobalSearchDialog({ open, onOpenChange }: Props) {
	const paths = usePaths()
	const navigate = useNavigate()
	const { surprise } = useSurpriseMe()

	const [term, setTerm] = useState('')
	const trimmed = term.trim()
	const [debounced] = useDebouncedValue(trimmed, 200)
	const search = debounced ?? ''
	const enabled = open && search.length >= MIN_LENGTH

	const { data, isFetching } = useGraphQL(
		query,
		['globalSearch', search],
		{ media: { search }, series: { search } },
		{ enabled, placeholderData: (previous) => previous, staleTime: 30_000 },
	)
	const hasTerm = trimmed.length >= MIN_LENGTH
	const books = hasTerm ? (data?.media.nodes ?? []) : []
	const series = hasTerm ? (data?.series.nodes ?? []) : []
	const total =
		data?.media.pageInfo.__typename === 'OffsetPaginationInfo' ? data.media.pageInfo.totalItems : 0
	const isWaiting = hasTerm && (isFetching || trimmed !== search)

	const go = (to: string) => {
		onOpenChange(false)
		navigate(to)
	}
	// The books page reads `search` URL-decoded once more than URLSearchParams does
	const resultsHref = (value: string) =>
		`${paths.bookSearch()}?${new URLSearchParams({ search: encodeURIComponent(value) })}`

	return (
		<Dialog
			open={open}
			onOpenChange={(next) => {
				onOpenChange(next)
				if (!next) setTerm('')
			}}
		>
			<Dialog.Content
				size="lg"
				className="p-0 shadow-2xl translate-y-0 gap-0 top-[12vh] overflow-hidden"
				aria-describedby={undefined}
			>
				<Dialog.Title className="sr-only">Search the library</Dialog.Title>
				<Command
					shouldFilter={false}
					loop
					className="p-0 rounded-none bg-dialog"
					onKeyDown={(event) => {
						// Enter with no result highlighted: the full results
						if (event.key !== 'Enter' || !trimmed) return
						const selected = (event.currentTarget as HTMLElement).querySelector(
							'[cmdk-item][data-selected="true"]',
						)
						if (!selected) {
							event.preventDefault()
							go(resultsHref(trimmed))
						}
					}}
				>
					<div className="pr-4 gap-2 flex items-center border-b border-border">
						<Command.Input
							value={term}
							onValueChange={setTerm}
							placeholder="Search books, authors, series…"
							wrapperClassName="min-w-0 flex-1 rounded-none border-0"
							iconClassName="opacity-100"
							className="h-12 text-base sm:text-sm"
						/>
						{isWaiting && <ProgressSpinner size="sm" className="shrink-0" />}
						<kbd className="px-1.5 font-medium rounded leading-5 sm:block hidden shrink-0 border border-border bg-muted font-sans text-[11px] text-muted-foreground">
							Esc
						</kbd>
					</div>

					<Command.List className="p-1 max-h-[min(60vh,28rem)]">
						{!hasTerm && (
							<Command.Group heading="Discover">
								<Command.Item
									value="surprise"
									onSelect={() => {
										onOpenChange(false)
										surprise()
									}}
								>
									<Dices className="mr-2 h-4 w-4 text-muted-foreground" />
									Surprise me
									<span className="text-xs ml-auto text-muted-foreground">
										a random unread book
									</span>
								</Command.Item>
								<Command.Item value="books" onSelect={() => go(paths.bookSearch())}>
									<Library className="mr-2 h-4 w-4 text-muted-foreground" />
									All books
								</Command.Item>
								<Command.Item value="authors" onSelect={() => go(paths.browseIndex('authors'))}>
									<Users className="mr-2 h-4 w-4 text-muted-foreground" />
									Authors
								</Command.Item>
								<Command.Item value="genres" onSelect={() => go(paths.browseIndex('genres'))}>
									<Tags className="mr-2 h-4 w-4 text-muted-foreground" />
									Genres
								</Command.Item>
							</Command.Group>
						)}

						{hasTerm && !isWaiting && !books.length && !series.length && (
							<p className="py-6 text-sm text-center text-muted-foreground">
								Nothing matches “{trimmed}”
							</p>
						)}

						{books.length > 0 && (
							<Command.Group heading="Books">
								{books.map((book) => {
									const rating = book.metadata?.rating != null ? Number(book.metadata.rating) : null
									const detail = [
										book.metadata?.writers?.join(', '),
										rating && rating > 0 ? `★ ${rating.toFixed(1)}` : null,
									]
										.filter(Boolean)
										.join(' · ')
									return (
										<Command.Item
											key={book.id}
											value={`book-${book.id}`}
											onSelect={() => go(paths.bookOverview(book.id))}
											className="gap-3 py-1.5"
										>
											<ThumbnailImage
												src={book.thumbnail.url}
												alt=""
												size={{ width: 28, height: 42 }}
												lazy
											/>
											<div className="min-w-0 flex flex-1 flex-col">
												<span className="line-clamp-1">{book.resolvedName}</span>
												{detail && (
													<span className="text-xs font-normal line-clamp-1 text-muted-foreground">
														{detail}
													</span>
												)}
											</div>
										</Command.Item>
									)
								})}
							</Command.Group>
						)}

						{series.length > 0 && (
							<Command.Group heading="Series">
								{series.map((item) => (
									<Command.Item
										key={item.id}
										value={`series-${item.id}`}
										onSelect={() => go(paths.seriesOverview(item.id))}
										className="gap-3 py-1.5"
									>
										<ThumbnailImage
											src={item.thumbnail.url}
											alt=""
											size={{ width: 28, height: 42 }}
											lazy
										/>
										<span className="min-w-0 line-clamp-1 flex-1">{item.resolvedName}</span>
										<span className="text-xs font-normal shrink-0 text-muted-foreground">
											{item.mediaCount} {item.mediaCount === 1 ? 'book' : 'books'}
										</span>
									</Command.Item>
								))}
							</Command.Group>
						)}

						{hasTerm && books.length > 0 && (
							<Command.Item
								value="all-results"
								onSelect={() => go(resultsHref(trimmed))}
								className="mt-1 text-muted-foreground"
							>
								<ArrowRight className="mr-2 h-4 w-4" />
								{total > books.length ? `See all ${total} books` : 'Open in Explore'}
							</Command.Item>
						)}
					</Command.List>

					<div className="px-3 py-2 gap-3 sm:flex hidden items-center border-t border-border text-[11px] text-muted-foreground">
						<span>↑↓ to move</span>
						<span>↵ to open</span>
						<span className="ml-auto">{GLOBAL_SEARCH_SHORTCUT} anywhere</span>
					</div>
				</Command>
			</Dialog.Content>
		</Dialog>
	)
}
