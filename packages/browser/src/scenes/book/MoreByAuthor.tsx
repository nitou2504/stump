import { useGraphQL } from '@stump/client'
import { graphql, MediaFilterInput } from '@stump/graphql'
import { useMemo } from 'react'

import BookCard from '@/components/book/BookCard'
import HorizontalCardList from '@/components/HorizontalCardList'
import { noop } from '@/utils/misc'

const query = graphql(`
	query MoreByAuthorQuery($filter: MediaFilterInput!, $pagination: Pagination) {
		media(filter: $filter, pagination: $pagination) {
			nodes {
				id
				seriesId
				...BookCard
			}
		}
	}
`)

type Props = {
	bookId: string
	/** The book's series, whose books are already listed under "Next in series" */
	seriesId?: string | null
	author: string
}

/**
 * Other books by the book's (first) author. In metadata-organized libraries most books
 * have no series, so this takes over from the folder-based "Next in series" that used to
 * list the rest of an author's folder.
 */
export default function MoreByAuthor({ bookId, seriesId, author }: Props) {
	const filter = useMemo<MediaFilterInput>(
		() => ({
			id: { neq: bookId },
			metadata: { writers: { hasAnyOf: [author] } },
		}),
		[bookId, author],
	)
	const { data } = useGraphQL(query, ['moreByAuthor', bookId, author], {
		filter,
		pagination: { offset: { page: 1, pageSize: 40 } },
	})

	const nodes = useMemo(
		() => (data?.media.nodes ?? []).filter((node) => !seriesId || node.seriesId !== seriesId),
		[data, seriesId],
	)

	if (!nodes.length) return null

	return (
		<HorizontalCardList
			title={`More by ${author}`}
			items={nodes.map((node) => (
				<BookCard key={node.id} fragment={node} fullWidth={false} />
			))}
			onFetchMore={noop}
			footerHeight={64 + 17}
		/>
	)
}
