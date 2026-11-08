import { useGraphQL } from '@stump/client'
import { Button } from '@stump/components'
import { extractErrorMessage, graphql } from '@stump/graphql'
import { BookMarked, Plus } from 'lucide-react'
import { useCallback, useMemo, useRef, useState } from 'react'
import { Helmet } from 'react-helmet'

import { SceneContainer } from '@/components/container'
import GenericEmptyState from '@/components/GenericEmptyState'
import { byName, READING_LISTS_KEY } from '@/components/readingList/queries'
import { CollectionSceneHeader, HeaderIconButton } from '@/components/sharedLayout'
import { useAppContext } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'

import ReadingListFormDialog from './ReadingListFormDialog'
import ReadingListGridItem from './ReadingListGridItem'

const query = graphql(`
	query ReadingListsScene {
		readingLists(pagination: { none: { unpaginated: true } }) {
			nodes {
				id
				name
				creatingUserId
				...ReadingListGridItem
			}
		}
	}
`)

/** Reading lists: the user's lists of books, put together by hand */
export default function ReadingListsScene() {
	const { t } = useTranslate()
	const { user } = useAppContext()
	const { data, isLoading, error } = useGraphQL(query, [READING_LISTS_KEY, 'scene'])
	// only the user's own lists (the API can make a list public, but nothing here shares them)
	const lists = useMemo(
		() =>
			(data?.readingLists.nodes ?? [])
				.filter((list) => list.creatingUserId === user.id)
				.sort(byName),
		[data, user.id],
	)

	const [isCreating, setIsCreating] = useState(false)

	// Each list keeps its own stack layout, and lists with as many covers get different
	// layouts while there are layouts left (like the libraries and smart lists)
	const layoutRegistry = useRef({
		assignments: new Map<string, number>(),
		counters: { 1: 0, 2: 0, 3: 0, 4: 0, 5: 0 },
	})
	const getLayoutNumber = useCallback((listId: string, itemCount: number) => {
		if (itemCount < 1 || itemCount > 5) return undefined

		const { assignments, counters } = layoutRegistry.current
		const assigned = assignments.get(listId)
		if (assigned !== undefined) return assigned

		const category = itemCount as 1 | 2 | 3 | 4 | 5
		const layoutNumber = counters[category]
		assignments.set(listId, layoutNumber)
		counters[category]++
		return layoutNumber
	}, [])

	const title = t('readingLists.title')
	const newList = t('readingLists.newList')

	const renderContent = () => {
		if (isLoading) return null

		if (error) {
			return (
				<GenericEmptyState
					title={t('readingLists.errors.loadLists')}
					subtitle={extractErrorMessage(error)}
				/>
			)
		}

		if (!lists.length) {
			return (
				<div className="gap-4 flex flex-col items-center">
					<GenericEmptyState
						title={t('readingLists.emptyState.heading')}
						subtitle={t('readingLists.emptyState.message')}
					/>
					<Button onClick={() => setIsCreating(true)}>
						<Plus />
						{newList}
					</Button>
				</div>
			)
		}

		return (
			<div className="gap-4 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4 grid grid-cols-1">
				{lists.map((list) => (
					<ReadingListGridItem key={list.id} data={list} getLayoutNumber={getLayoutNumber} />
				))}
			</div>
		)
	}

	return (
		<>
			<Helmet>
				<title>Stump | {title}</title>
			</Helmet>

			<CollectionSceneHeader
				title={title}
				stats={data ? [{ key: 'lists', icon: BookMarked, value: lists.length }] : undefined}
				endActions={
					!!lists.length && (
						<HeaderIconButton label={newList} icon={Plus} onClick={() => setIsCreating(true)} />
					)
				}
			/>

			<SceneContainer>{renderContent()}</SceneContainer>

			<ReadingListFormDialog isOpen={isCreating} onClose={() => setIsCreating(false)} />
		</>
	)
}
