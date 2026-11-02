import { useState } from 'react'

import { GridSizeSlider } from '@/components/container/GridSizeSlider'
import TableOrGridLayout from '@/components/TableOrGridLayout'
import { useLayoutMode } from '@/hooks/useLayoutMode'

import GroupByDialog, { GroupByButton } from './GroupByDialog'

/**
 * The view controls of a smart list's items: grouping, grid size and the table/grid toggle.
 * These are the viewer's own choices and don't modify the list. Table or grid is the
 * global layout preference, shared with every other book and series list.
 */
export default function SmartListLayoutControls() {
	const { isGrid } = useLayoutMode()
	const [isGroupingOpen, setIsGroupingOpen] = useState(false)

	return (
		<div className="gap-1 sm:gap-2 flex shrink-0 items-center">
			<GroupByButton onClick={() => setIsGroupingOpen(true)} />
			<GroupByDialog isOpen={isGroupingOpen} onClose={() => setIsGroupingOpen(false)} />
			{isGrid && (
				<div className="md:block hidden">
					<GridSizeSlider />
				</div>
			)}
			<TableOrGridLayout />
		</div>
	)
}
