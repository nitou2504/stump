import { FragmentType, graphql, useFragment } from '@stump/graphql'
import { Book } from 'lucide-react'

import { CollectionGridItem } from '@/components/collection'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

const fragment = graphql(`
	fragment ReadingListGridItem on ReadingList {
		id
		name
		bookCount
		books(limit: 5) {
			id
			thumbnail {
				url
				metadata {
					averageColor
					colors {
						color
						percentage
					}
					thumbhash
				}
			}
		}
	}
`)

type Props = {
	data: FragmentType<typeof fragment>
	getLayoutNumber: (id: string, itemCount: number) => number | undefined
}

/** A reading list's card: its first covers, stacked, with its name and how many books it has */
export default function ReadingListGridItem({ data, getLayoutNumber }: Props) {
	const paths = usePaths()
	const { translate } = useTranslate()
	const { id, name, books, bookCount } = useFragment(fragment, data)

	return (
		<CollectionGridItem
			title={name}
			to={paths.readingList(id)}
			thumbnails={books.map(({ thumbnail }) => ({
				url: thumbnail.url,
				metadata: thumbnail.metadata,
			}))}
			layoutNumber={getLayoutNumber(id, books.length)}
			stats={[
				{
					key: 'books',
					icon: Book,
					value: bookCount,
					label: translate('shared.common.xBooks', { count: bookCount }),
				},
			]}
		/>
	)
}
