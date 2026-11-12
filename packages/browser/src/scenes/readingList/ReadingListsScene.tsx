import { useGraphQL } from '@stump/client'
import { Avatar, Button, Text } from '@stump/components'
import { extractErrorMessage, graphql } from '@stump/graphql'
import { BookMarked, Plus } from 'lucide-react'
import { useCallback, useMemo, useRef, useState } from 'react'
import { Helmet } from 'react-helmet'

import { SceneContainer } from '@/components/container'
import GenericEmptyState from '@/components/GenericEmptyState'
import { byName, READING_LISTS_KEY } from '@/components/readingList/queries'
import { CollectionSceneHeader, HeaderIconButton } from '@/components/sharedLayout'
import { useAppContext } from '@/context'
import { useStatLabels } from '@/hooks/useStatLabels'
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
				bookCount
				creator {
					id
					username
					avatarUrl
				}
				...ReadingListGridItem
			}
		}
	}
`)

const compareNames = (a: string, b: string) =>
	a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' })

/**
 * Reading lists: the user's own lists of books, put together by hand, and the lists other people
 * on the server made public, grouped by who made them. A public list that holds none of the
 * books the user may read isn't shown: there's nothing in it for them
 */
export default function ReadingListsScene() {
	const { t } = useTranslate()
	const labels = useStatLabels()
	const { user } = useAppContext()
	const { data, isLoading, error } = useGraphQL(query, [READING_LISTS_KEY, 'scene'])

	const { mine, others } = useMemo(() => {
		const nodes = data?.readingLists.nodes ?? []
		const mine = nodes.filter((list) => list.creatingUserId === user.id).sort(byName)

		const groups = new Map<
			string,
			{
				creator: { id: string; username: string; avatarUrl?: string | null }
				lists: typeof nodes
			}
		>()
		for (const list of nodes) {
			if (list.creatingUserId === user.id || !list.creator || list.bookCount < 1) continue
			const group = groups.get(list.creator.id) ?? { creator: list.creator, lists: [] }
			group.lists.push(list)
			groups.set(list.creator.id, group)
		}
		const others = [...groups.values()]
			.map((group) => ({ ...group, lists: group.lists.sort(byName) }))
			.sort((a, b) => compareNames(a.creator.username, b.creator.username))

		return { mine, others }
	}, [data, user.id])
	const total = mine.length + others.reduce((count, group) => count + group.lists.length, 0)

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
	const gridClassName = 'gap-4 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4 grid grid-cols-1'

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

		if (!total) {
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
			<div className="gap-8 flex flex-col">
				<section aria-label={t('readingLists.sections.mine')} className="gap-3 flex flex-col">
					<h2 className="text-lg font-semibold">{t('readingLists.sections.mine')}</h2>
					{mine.length ? (
						<div className={gridClassName}>
							{mine.map((list) => (
								<ReadingListGridItem key={list.id} data={list} getLayoutNumber={getLayoutNumber} />
							))}
						</div>
					) : (
						<div className="gap-3 flex flex-wrap items-center">
							<Text variant="muted">{t('readingLists.emptyState.mine')}</Text>
							<Button variant="outline" size="sm" onClick={() => setIsCreating(true)}>
								<Plus />
								{newList}
							</Button>
						</div>
					)}
				</section>

				{!!others.length && (
					<section aria-label={t('readingLists.sections.others')} className="gap-4 flex flex-col">
						<h2 className="text-lg font-semibold">{t('readingLists.sections.others')}</h2>
						{others.map(({ creator, lists }) => (
							<div key={creator.id} className="gap-3 flex flex-col">
								<div className="gap-2 flex items-center">
									<Avatar
										className="size-7"
										src={creator.avatarUrl ?? undefined}
										fallback={creator.username}
									/>
									<Text className="font-medium">{creator.username}</Text>
								</div>
								<div className={gridClassName}>
									{lists.map((list) => (
										<ReadingListGridItem
											key={list.id}
											data={list}
											getLayoutNumber={getLayoutNumber}
										/>
									))}
								</div>
							</div>
						))}
					</section>
				)}
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
				stats={
					data
						? [
								{
									key: 'lists',
									icon: BookMarked,
									value: labels.number(total),
									suffix: labels.word('lists', total),
								},
							]
						: undefined
				}
				endActions={
					!!total && (
						<HeaderIconButton label={newList} icon={Plus} onClick={() => setIsCreating(true)} />
					)
				}
			/>

			<SceneContainer>{renderContent()}</SceneContainer>

			<ReadingListFormDialog isOpen={isCreating} onClose={() => setIsCreating(false)} />
		</>
	)
}
