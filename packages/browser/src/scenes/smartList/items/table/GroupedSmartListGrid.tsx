import { Heading, Text } from '@stump/components'
import { FragmentType, Media } from '@stump/graphql'

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
}

type Props = {
	groups: BookGroup[]
}

/**
 * A cover grid for grouped smart lists: one section per series/library, with its books
 * as cards. Groups and books arrive already filtered and sorted by the current view.
 */
export default function GroupedSmartListGrid({ groups }: Props) {
	if (!groups.length) {
		return (
			<Text variant="muted" className="py-8 text-center">
				No books match
			</Text>
		)
	}

	return (
		<div className="gap-8 px-4 pb-8 flex w-full flex-col">
			{groups.map(({ id, name, href, books }) => (
				<section key={id} className="gap-2 flex flex-col">
					<Heading size="sm" className="gap-2 pb-1 flex items-baseline border-b border-border">
						{href ? (
							<Link to={href} className="hover:underline">
								{name}
							</Link>
						) : (
							name
						)}
						<span className="text-sm font-normal text-muted-foreground">{books.length}</span>
					</Heading>
					<DynamicCardGrid
						count={books.length}
						renderItem={(index) => {
							const book = books[index]
							if (!book) return null
							return (
								<BookCard
									key={book.id}
									fragment={book as unknown as FragmentType<typeof BookCardFragment>}
								/>
							)
						}}
					/>
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
