import { FragmentType, Media } from '@stump/graphql'
import { VirtuosoGrid } from 'react-virtuoso'

import BookCard, { BookCardFragment } from '@/components/book/BookCard'
import { VirtualGridItem, VirtualGridList } from '@/components/container/DynamicCardGrid'

type Props = {
	/** The (already filtered and sorted) books to render */
	books: Media[]
	/** The element which scrolls the page, the grid is virtualized against it */
	scrollElement: HTMLElement | null
}

/**
 * A virtualized cover grid for the books of an ungrouped smart list. It renders the same
 * rows as the table (so the view's search and sorting still apply), just as cards.
 */
export default function SmartListBookGrid({ books, scrollElement }: Props) {
	if (!scrollElement) return null

	return (
		<div className="px-4 w-full">
			<VirtuosoGrid
				customScrollParent={scrollElement}
				totalCount={books.length}
				components={{ List: VirtualGridList, Item: VirtualGridItem }}
				computeItemKey={(index) => books[index]?.id ?? index}
				itemContent={(index) => {
					const book = books[index]
					if (!book) return null
					return <BookCard fragment={book as unknown as FragmentType<typeof BookCardFragment>} />
				}}
				increaseViewportBy={600}
			/>
		</div>
	)
}
