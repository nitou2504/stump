import { cn } from '@stump/components'
import { Media, MediaFilterInput, SmartListGroupedItem } from '@stump/graphql'
import { useMemo } from 'react'

import { SceneContainer } from '@/components/container'
import { FilterContext } from '@/components/filters/context'
import { useFilterScene } from '@/components/filters/useFilterScene'
import { useTranslate } from '@/hooks/useTranslate'

import { useSmartListContext } from '../context'
import { FieldGrouping, groupBooksByField, groupingLabel } from '../fieldGrouping'
import { useSmartListItems } from '../graphql'
import GroupedVirtualSmartListTable from './table/GroupedVirtualSmartListTable'
import VirtualSmartListTable from './table/VirtualSmartListTable'

export default function UserSmartListItemsScene() {
	const { t } = useTranslate()
	const {
		list: { id },
		grouping,
		fieldGrouping,
	} = useSmartListContext()

	// Ad-hoc filters (genre, writer, language, ...) on top of the list's own rules, kept in
	// the URL like the library/series book views
	const filterScene = useFilterScene()
	const filters = filterScene.filters as MediaFilterInput
	const { items, isLoading } = useSmartListItems({ id, filter: filters, grouping })

	const fieldGrouped = useMemo(() => {
		if (!fieldGrouping || !items || !('books' in items)) return null
		return toGroupedItems(items.books as Media[], fieldGrouping)
	}, [items, fieldGrouping])

	if (isLoading) {
		return null
	}

	if (!items) {
		throw new Error(t('userSmartListScene.itemsScene.smartListNotFound'))
	}

	const renderContent = () => {
		if (fieldGrouped) {
			return (
				<GroupedVirtualSmartListTable items={fieldGrouped.items} fieldGroup={fieldGrouped.labels} />
			)
		}

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

/**
 * Shapes a viewer-side field grouping like the server's grouped items, with books lacking
 * the field in a last, entity-less group
 */
function toGroupedItems(books: Media[], grouping: FieldGrouping) {
	const { groups, ungrouped } = groupBooksByField(books, grouping)
	const label = groupingLabel(grouping)
	const items = groups.map(({ key, name, books }) => ({
		books,
		entity: { __typename: 'Library', id: `field:${key}`, name },
	})) as unknown as SmartListGroupedItem[]
	if (ungrouped.length) {
		items.push({ books: ungrouped, entity: null } as unknown as SmartListGroupedItem)
	}
	return { items, labels: { label, ungroupedLabel: `No ${label}` } }
}
