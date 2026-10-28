import { cn } from '@stump/components'
import { Media, MediaFilterInput, SmartListGroupedItem } from '@stump/graphql'

import { SceneContainer } from '@/components/container'
import { FilterContext } from '@/components/filters/context'
import { useFilterScene } from '@/components/filters/useFilterScene'
import { useTranslate } from '@/hooks/useTranslate'

import { useSmartListContext } from '../context'
import { useSmartListItems } from '../graphql'
import GroupedVirtualSmartListTable from './table/GroupedVirtualSmartListTable'
import VirtualSmartListTable from './table/VirtualSmartListTable'

export default function UserSmartListItemsScene() {
	const { t } = useTranslate()
	const {
		list: { id },
	} = useSmartListContext()

	// Ad-hoc filters (genre, writer, language, ...) on top of the list's own rules, kept in
	// the URL like the library/series book views
	const filterScene = useFilterScene()
	const filters = filterScene.filters as MediaFilterInput
	const { items, isLoading } = useSmartListItems({ id, filter: filters })

	if (isLoading) {
		return null
	}

	if (!items) {
		throw new Error(t('userSmartListScene.itemsScene.smartListNotFound'))
	}

	const renderContent = () => {
		const isGrouped = 'items' in items

		if (isGrouped) {
			return <GroupedVirtualSmartListTable items={items.items as SmartListGroupedItem[]} />
		} else {
			return <VirtualSmartListTable books={items.books as Media[]} />
		}
	}

	return (
		<FilterContext.Provider value={filterScene}>
			<SceneContainer className={cn('p-0 py-4')}>{renderContent()}</SceneContainer>
		</FilterContext.Provider>
	)
}
