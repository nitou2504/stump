import { InterfaceLayout } from '@stump/graphql'
import { useState } from 'react'

import { GridSizeSlider } from '@/components/container/GridSizeSlider'
import TableOrGridLayout from '@/components/TableOrGridLayout'

import { useSmartListContext } from '../../context'
import { useSmartListLayout } from '../../layout'
import GroupByDialog, { GroupByButton } from './GroupByDialog'

/** The table/grid layout state for the current smart list */
export function useSmartListItemsLayout() {
	const {
		list: { id },
	} = useSmartListContext()
	const { layout, setLayout } = useSmartListLayout(id)
	return { isGrid: layout === InterfaceLayout.Grid, layout, setLayout }
}

/**
 * The view controls of a smart list's items: grouping, grid size and table/grid layout.
 * These are the viewer's own choices and don't modify the list.
 */
export default function SmartListLayoutControls() {
	const { isGrid, layout, setLayout } = useSmartListItemsLayout()
	const [isGroupingOpen, setIsGroupingOpen] = useState(false)

	return (
		<div className="gap-2 flex shrink-0 items-center">
			<GroupByButton onClick={() => setIsGroupingOpen(true)} />
			<GroupByDialog isOpen={isGroupingOpen} onClose={() => setIsGroupingOpen(false)} />
			{isGrid && (
				<div className="md:block hidden">
					<GridSizeSlider />
				</div>
			)}
			<TableOrGridLayout layout={layout} setLayout={setLayout} />
		</div>
	)
}
