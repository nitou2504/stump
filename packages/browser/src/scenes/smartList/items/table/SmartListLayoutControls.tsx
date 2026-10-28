import { NativeSelect } from '@stump/components'
import { InterfaceLayout, SmartListGrouping } from '@stump/graphql'

import { GridSizeSlider } from '@/components/container/GridSizeSlider'
import TableOrGridLayout from '@/components/TableOrGridLayout'

import { useSmartListContext } from '../../context'
import { useSmartListLayout } from '../../layout'

const GROUPING_OPTIONS = [
	{ label: 'No grouping', value: SmartListGrouping.ByBooks },
	{ label: 'Group by series', value: SmartListGrouping.BySeries },
	{ label: 'Group by library', value: SmartListGrouping.ByLibrary },
]

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
	const { grouping, setGrouping } = useSmartListContext()
	const { isGrid, layout, setLayout } = useSmartListItemsLayout()

	return (
		<div className="gap-2 flex shrink-0 items-center">
			<NativeSelect
				title="Grouping"
				className="h-8 py-0 w-[160px]"
				options={GROUPING_OPTIONS}
				value={grouping}
				onChange={(e) => setGrouping(e.target.value as SmartListGrouping)}
			/>
			{isGrid && (
				<div className="md:block hidden">
					<GridSizeSlider />
				</div>
			)}
			<TableOrGridLayout layout={layout} setLayout={setLayout} />
		</div>
	)
}
