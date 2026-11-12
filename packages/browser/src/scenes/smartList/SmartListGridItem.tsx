import { FragmentType, graphql, useFragment } from '@stump/graphql'
import { Book, Globe, Library, Rows3, UserRound } from 'lucide-react'

import { CollectionGridItem } from '@/components/collection'
import { useAppContext } from '@/context'
import { useStatLabels } from '@/hooks/useStatLabels'
import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'

import { usePrefetchSmartList } from './graphql'

const LOCALE_BASE_KEY = 'userSmartListsScene.list.card'
const withLocaleKey = (key: string) => `${LOCALE_BASE_KEY}.${key}`

const fragment = graphql(`
	fragment SmartListWebGridItem on SmartList {
		id
		name
		creatorId
		visibility
		creator {
			id
			username
		}
		books(limit: 5) {
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
		meta {
			matchedBooks
			matchedSeries
			matchedLibraries
		}
	}
`)

type Props = {
	data: FragmentType<typeof fragment>
	getLayoutNumber: (id: string, itemCount: number) => number | undefined
}

/**
 * A smart list's card: its first covers, stacked, with its name and what it matches, in words
 * ("344 books", "89 series"; the libraries only when it spans more than one). A list of someone
 * else says who made it (*by Ana*), and a list of the user's own that everyone sees says it is
 * *Shared*: a private list of the user's own, the usual case, says nothing, so the badge means
 * something when it is there.
 */
export default function SmartListGridItem({ data, getLayoutNumber }: Props) {
	const paths = usePaths()
	const { user } = useAppContext()
	const { t } = useTranslate()
	const labels = useStatLabels()
	const { prefetch } = usePrefetchSmartList()
	const { id, name, creatorId, visibility, creator, books, meta } = useFragment(fragment, data)

	const layoutNumber = getLayoutNumber(id, books.length)

	const isMine = creatorId === user.id
	const badge =
		isMine && visibility === 'PUBLIC'
			? {
					key: 'visibility',
					icon: Globe,
					value: t(withLocaleKey('shared')),
					label: t(withLocaleKey('sharedHint')),
				}
			: !isMine
				? {
						key: 'creator',
						icon: UserRound,
						value: creator
							? t(withLocaleKey('by'), { name: creator.username })
							: t(withLocaleKey('shared')),
						label: creator
							? t(withLocaleKey('byHint'), { name: creator.username })
							: t(withLocaleKey('sharedHint')),
					}
				: undefined

	return (
		<CollectionGridItem
			title={name}
			to={paths.smartList(id)}
			onHover={() => prefetch({ id })}
			thumbnails={books.map(({ thumbnail }) => ({
				url: thumbnail.url,
				metadata: thumbnail.metadata,
			}))}
			layoutNumber={layoutNumber}
			stats={[
				{
					key: 'books',
					icon: Book,
					value: labels.number(meta.matchedBooks),
					unit: labels.word('books', meta.matchedBooks),
					label: labels.count('books', meta.matchedBooks),
				},
				{
					key: 'series',
					icon: Rows3,
					value: labels.number(meta.matchedSeries),
					unit: labels.word('series', meta.matchedSeries),
					label: labels.count('series', meta.matchedSeries),
				},
				...(meta.matchedLibraries > 1
					? [
							{
								key: 'libraries',
								icon: Library,
								value: labels.number(meta.matchedLibraries),
								unit: labels.word('libraries', meta.matchedLibraries),
								label: labels.count('libraries', meta.matchedLibraries),
							},
						]
					: []),
				...(badge ? [badge] : []),
			]}
		/>
	)
}
