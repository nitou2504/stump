import { IconButton, ToolTip } from '@stump/components'
import { InterfaceLayout } from '@stump/graphql'
import { LayoutGrid, Table } from 'lucide-react'

import { useLayoutMode } from '@/hooks/useLayoutMode'

/**
 * The grid/table toggle of the book and series lists. The layout is a single user
 * preference, so switching it here switches every list (see `useLayoutMode`).
 */
export default function TableOrGridLayout() {
	const { layoutMode, setLayoutMode } = useLayoutMode()

	return (
		<div className="gap-1 flex shrink-0 items-center">
			<ToolTip content="Grid" size="sm">
				<IconButton
					variant="ghost"
					size="sm"
					className="hover:bg-accent"
					onClick={() => setLayoutMode(InterfaceLayout.Grid)}
					disabled={layoutMode === InterfaceLayout.Grid}
					aria-label="Grid"
				>
					<LayoutGrid className="h-4 w-4" />
				</IconButton>
			</ToolTip>

			<ToolTip content="Table" size="sm" align="end">
				<IconButton
					variant="ghost"
					size="sm"
					className="hover:bg-accent"
					onClick={() => setLayoutMode(InterfaceLayout.Table)}
					disabled={layoutMode === InterfaceLayout.Table}
					aria-label="Table"
				>
					<Table className="h-4 w-4" />
				</IconButton>
			</ToolTip>
		</div>
	)
}
