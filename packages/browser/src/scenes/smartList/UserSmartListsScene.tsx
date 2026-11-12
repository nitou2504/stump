import { useGraphQL, useSDK } from '@stump/client'
import { ConfirmationModal } from '@stump/components'
import { EntityVisibility, graphql } from '@stump/graphql'
import { Globe, GlobeLock, ListFilter } from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import { toast } from 'sonner'

import { SceneContainer } from '@/components/container'
import GenericEmptyState from '@/components/GenericEmptyState'
import { CollectionSceneHeader } from '@/components/sharedLayout'
import { useAppContext } from '@/context'
import { useStatLabels } from '@/hooks/useStatLabels'
import { useTranslate } from '@/hooks/useTranslate'

import { useSetSmartListsVisibility } from './graphql'
import SmartListGridItem from './SmartListGridItem'

const LOCALE_BASE_KEY = `userSmartListsScene`
const withLocaleKey = (key: string) => `${LOCALE_BASE_KEY}.${key}`

const query = graphql(`
	query SmartListsWithSearch($input: SmartListsInput!) {
		smartLists(input: $input) {
			id
			creatorId
			visibility
			meta {
				matchedBooks
			}
			...SmartListWebGridItem
		}
	}
`)

const errorMessage = (error: unknown) =>
	error instanceof Error && error.message ? error.message : undefined

/**
 * The user's smart lists and the lists others made public, by name. A list of someone else that
 * matches none of the books the user may read isn't shown: there's nothing in it for them. The
 * menu next to the title shares all of the user's own lists, or makes them private, after asking.
 */
export default function UserSmartListsScene() {
	const { t, translate } = useTranslate()
	const labels = useStatLabels()
	const { sdk } = useSDK()
	const { user } = useAppContext()
	const { data, isLoading } = useGraphQL(query, [sdk.cacheKeys.smartLists, 'scene'], {
		input: {},
	})
	const { setVisibility, isPending } = useSetSmartListsVisibility()
	const [confirming, setConfirming] = useState<EntityVisibility | null>(null)

	const allLists = data?.smartLists ?? []
	const smartLists = allLists.filter(
		(list) => list.creatorId === user.id || list.meta.matchedBooks > 0,
	)
	// what each way of changing the visibility would change: the user's lists that aren't there yet
	const mine = allLists.filter((list) => list.creatorId === user.id)
	const toShare = mine.filter((list) => list.visibility === EntityVisibility.Private)
	const toHide = mine.filter((list) => list.visibility === EntityVisibility.Public)
	const shareActions = [
		...(toShare.length
			? [
					{
						label: t(withLocaleKey('share.all')),
						leftIcon: <Globe />,
						onClick: () => setConfirming(EntityVisibility.Public),
					},
				]
			: []),
		...(toHide.length
			? [
					{
						label: t(withLocaleKey('share.makePrivate')),
						leftIcon: <GlobeLock />,
						onClick: () => setConfirming(EntityVisibility.Private),
					},
				]
			: []),
	]

	const confirmVisibility = async () => {
		const target = confirming
		setConfirming(null)
		if (!target) return

		const lists = target === EntityVisibility.Public ? toShare : toHide
		try {
			await setVisibility(
				lists.map(({ id }) => id),
				target,
			)
			toast.success(
				t(withLocaleKey(target === EntityVisibility.Public ? 'share.shared' : 'share.madePrivate')),
			)
		} catch (error) {
			toast.error(t(withLocaleKey('share.error')), { description: errorMessage(error) })
		}
	}

	// This makes sure we do not repeat layout variants if possible:
	//
	// For example, there are only two layouts for lists with 3 books,
	// and if only the 1st and 3rd lists have 3 books, then if we used
	// the list index they would have both used the same layout variant
	const layoutRegistry = useRef({
		// The layout variant number (non-modulo) assigned to a smart list ID
		assignments: new Map<string, number>(),
		// Record the smallest unused layout variant number (non-modulo) for each catagory
		counters: { 1: 0, 2: 0, 3: 0, 4: 0, 5: 0 },
	})

	const getLayoutNumber = useCallback((smartListId: string, itemCount: number) => {
		if (itemCount < 1 || itemCount > 5) return undefined

		const { assignments, counters } = layoutRegistry.current

		if (assignments.has(smartListId)) {
			return assignments.get(smartListId)!
		}

		const category = itemCount as 1 | 2 | 3 | 4 | 5
		const layoutNumber = counters[category]
		assignments.set(smartListId, layoutNumber)
		counters[category]++

		return layoutNumber
	}, [])

	const renderContent = () => {
		if (isLoading) return null

		if (!smartLists.length) {
			return (
				<GenericEmptyState
					title={t(withLocaleKey('list.emptyState.heading'))}
					subtitle={t(withLocaleKey('list.emptyState.noListsMessage'))}
				/>
			)
		}

		return (
			<div className="gap-4 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4 grid grid-cols-1">
				{smartLists.map((list) => (
					<SmartListGridItem key={list.id} data={list} getLayoutNumber={getLayoutNumber} />
				))}
			</div>
		)
	}

	return (
		<>
			<CollectionSceneHeader
				title={translate('shared.common.smartLists')}
				docsLink="https://stumpapp.dev/docs/guides/features/smart-list"
				actions={shareActions.length ? [{ items: shareActions }] : undefined}
				stats={
					data
						? [
								{
									key: 'lists',
									icon: ListFilter,
									value: labels.number(smartLists.length),
									suffix: labels.word('lists', smartLists.length),
								},
							]
						: undefined
				}
			/>

			<SceneContainer>{renderContent()}</SceneContainer>

			<ConfirmationModal
				isOpen={confirming !== null}
				onClose={() => setConfirming(null)}
				onConfirm={confirmVisibility}
				confirmIsLoading={isPending}
				title={t(
					withLocaleKey(
						confirming === EntityVisibility.Public
							? 'share.sharing.title'
							: 'share.unsharing.title',
					),
				)}
				description={t(
					withLocaleKey(
						confirming === EntityVisibility.Public
							? 'share.sharing.description'
							: 'share.unsharing.description',
					),
				)}
				confirmText={t(
					withLocaleKey(
						confirming === EntityVisibility.Public
							? 'share.sharing.confirm'
							: 'share.unsharing.confirm',
					),
				)}
			/>
		</>
	)
}
