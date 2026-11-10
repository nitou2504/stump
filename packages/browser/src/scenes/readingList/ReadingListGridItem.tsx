import { FragmentType, graphql, useFragment } from '@stump/graphql'
import { Book, Globe } from 'lucide-react'

import { CollectionGridItem } from '@/components/collection'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

const fragment = graphql(`
	fragment ReadingListGridItem on ReadingList {
		id
		name
		visibility
		canEdit
		bookCount
		creator {
			id
			username
		}
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

/**
 * A reading list's card: its first covers, stacked, with its name and how many books it has. A
 * public list has a globe: *Public* on the user's own, the name of the person who made it on
 * someone else's
 */
export default function ReadingListGridItem({ data, getLayoutNumber }: Props) {
	const paths = usePaths()
	const { t, translate } = useTranslate()
	const { id, name, books, bookCount, visibility, canEdit, creator } = useFragment(fragment, data)

	const isPublic = visibility === 'PUBLIC'
	const badge = isPublic
		? [
				{
					key: 'public',
					icon: Globe,
					value: canEdit
						? t('readingLists.public')
						: (creator?.username ?? t('readingLists.public')),
					label: canEdit
						? t('readingLists.publicHint')
						: t('readingLists.list.publicBy', { name: creator?.username }),
				},
			]
		: []

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
				...badge,
			]}
		/>
	)
}
