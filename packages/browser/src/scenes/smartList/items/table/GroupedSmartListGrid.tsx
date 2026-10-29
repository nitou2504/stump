import { cn, Heading, Text } from '@stump/components'
import { FragmentType, Media } from '@stump/graphql'
import { ChevronDown } from 'lucide-react'

import BookCard, { BookCardFragment } from '@/components/book/BookCard'
import DynamicCardGrid from '@/components/container/DynamicCardGrid'
import { Link } from '@/context'
import { usePaths } from '@/paths'

export type BookGroup = {
	id: string
	name: string
	/** Where the group's name links to, e.g. the series */
	href?: string
	books: Media[]
	/** Whether the group's books are shown (shared with the table's expansion state) */
	isExpanded: boolean
	onToggle: () => void
}

type Props = {
	groups: BookGroup[]
	/** The grouping's label, e.g. `Series` */
	label: string
	isAllExpanded: boolean
	onToggleAll: () => void
	/** Show each book's number in its series on the covers */
	showSeriesPosition?: boolean
}

const Chevron = ({ isExpanded }: { isExpanded: boolean }) => (
	<ChevronDown
		className={cn('h-4 w-4 shrink-0 text-muted-foreground transition-transform duration-200', {
			'rotate-180': isExpanded,
		})}
	/>
)

/**
 * A cover grid for grouped smart lists: one section per series/library, with its books
 * as cards. Groups and books arrive already filtered and sorted by the current view.
 */
export default function GroupedSmartListGrid({
	groups,
	label,
	isAllExpanded,
	onToggleAll,
	showSeriesPosition,
}: Props) {
	if (!groups.length) {
		return (
			<Text variant="muted" className="py-8 text-center">
				No books match
			</Text>
		)
	}

	return (
		<div className="pb-8 flex w-full flex-col">
			<div className="px-4 py-2 gap-x-1 flex items-center border-b border-border bg-muted">
				<button
					type="button"
					className="gap-x-1 flex items-center"
					onClick={onToggleAll}
					title={isAllExpanded ? 'Collapse all' : 'Expand all'}
				>
					<Chevron isExpanded={isAllExpanded} />
					<Text size="sm" variant="muted">
						{label}
					</Text>
				</button>
				<Text size="sm" variant="muted" className="ml-auto">
					Books
				</Text>
			</div>

			{groups.map(({ id, name, href, books, isExpanded, onToggle }) => (
				<section key={id} className="flex flex-col border-b border-border">
					<div className="px-4 py-2 gap-x-1 flex items-center">
						<button
							type="button"
							className="gap-x-1 min-w-0 flex items-center"
							onClick={onToggle}
							title={isExpanded ? 'Collapse' : 'Expand'}
							aria-expanded={isExpanded}
						>
							<Chevron isExpanded={isExpanded} />
							<Heading size="xs" className="font-normal line-clamp-1 text-left">
								{name}
							</Heading>
						</button>
						{href && (
							<Link
								to={href}
								className="ml-2 text-xs text-muted-foreground hover:text-foreground hover:underline"
							>
								Open
							</Link>
						)}
						<Text size="sm" variant="muted" className="ml-auto">
							{books.length}
						</Text>
					</div>

					{isExpanded && (
						<div className="px-4 pb-6">
							<DynamicCardGrid
								count={books.length}
								renderItem={(index) => {
									const book = books[index]
									if (!book) return null
									return (
										<BookCard
											key={book.id}
											fragment={book as unknown as FragmentType<typeof BookCardFragment>}
											showSeriesPosition={showSeriesPosition}
										/>
									)
								}}
							/>
						</div>
					)}
				</section>
			))}
		</div>
	)
}

export const useGroupHref = () => {
	const paths = usePaths()
	return (entity: { __typename?: string; id: string } | null | undefined) => {
		if (!entity) return undefined
		if (entity.__typename === 'Series') return paths.seriesOverview(entity.id)
		if (entity.__typename === 'Library') return paths.library(entity.id)
		return undefined
	}
}
