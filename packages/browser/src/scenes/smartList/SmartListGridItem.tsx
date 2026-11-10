import { FragmentType, graphql, useFragment } from '@stump/graphql'
import { Book, Globe, Library, Lock, Rows3, UserRound } from 'lucide-react'

import { CollectionGridItem } from '@/components/collection'
import { useAppContext } from '@/context'
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
 * A smart list's card: its first covers, stacked, with its name and what it matches. A list of the
 * user's own says who sees it (*Shared* with everyone on the server, or *Private*); a list of
 * someone else says who made it (*by Ana*): the lists others share are for everyone to read.
 */
export default function SmartListGridItem({ data, getLayoutNumber }: Props) {
	const paths = usePaths()
	const { user } = useAppContext()
	const { t, translate } = useTranslate()
	const { prefetch } = usePrefetchSmartList()
	const { id, name, creatorId, visibility, creator, books, meta } = useFragment(fragment, data)

	const layoutNumber = getLayoutNumber(id, books.length)

	const isMine = creatorId === user.id
	const badge = isMine
		? visibility === 'PUBLIC'
			? {
					key: 'visibility',
					icon: Globe,
					value: t(withLocaleKey('shared')),
					label: t(withLocaleKey('sharedHint')),
				}
			: {
					key: 'visibility',
					icon: Lock,
					value: t(withLocaleKey('private')),
					label: t(withLocaleKey('privateHint')),
				}
		: {
				key: 'creator',
				icon: UserRound,
				value: creator
					? t(withLocaleKey('by'), { name: creator.username })
					: t(withLocaleKey('shared')),
				label: creator
					? t(withLocaleKey('byHint'), { name: creator.username })
					: t(withLocaleKey('sharedHint')),
			}

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
					key: 'libraries',
					icon: Library,
					value: meta.matchedLibraries,
					label: translate('shared.common.libraries', { count: meta.matchedLibraries }),
				},
				{
					key: 'series',
					icon: Rows3,
					value: meta.matchedSeries,
					label: translate('shared.common.series', { count: meta.matchedSeries }),
				},
				{
					key: 'books',
					icon: Book,
					value: meta.matchedBooks,
					label: translate('shared.common.book', { count: meta.matchedBooks }),
				},
				badge,
			]}
		/>
	)
}
