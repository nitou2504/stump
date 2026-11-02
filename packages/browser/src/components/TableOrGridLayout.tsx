import { IconButton, ToolTip } from '@stump/components'
import { InterfaceLayout } from '@stump/graphql'
import { LayoutGrid, Table } from 'lucide-react'

import { useLayoutMode } from '@/hooks/useLayoutMode'

type LayoutToggleProps = {
	/** Whether the list currently shows a grid (the button then offers the table) */
	isGrid: boolean
	onToggle: () => void
}

/**
 * One button that switches a list between grid and table. It shows the layout it switches
 * to, like the label it reads out: "Show as table" while the grid is on screen.
 */
export function LayoutToggle({ isGrid, onToggle }: LayoutToggleProps) {
	const label = isGrid ? 'Show as table' : 'Show as grid'
	const Icon = isGrid ? Table : LayoutGrid

	return (
		<ToolTip content={label} size="sm" align="end">
			<IconButton
				variant="ghost"
				size="sm"
				className="shrink-0 hover:bg-accent"
				onClick={onToggle}
				aria-label={label}
				data-layout={isGrid ? 'grid' : 'table'}
			>
				<Icon className="h-4 w-4" />
			</IconButton>
		</ToolTip>
	)
}

/**
 * The grid/table toggle of the book and series lists. The layout is a single user
 * preference, so switching it here switches every list (see `useLayoutMode`).
 */
export default function TableOrGridLayout() {
	const { isGrid, setLayoutMode } = useLayoutMode()

	return (
		<LayoutToggle
			isGrid={isGrid}
			onToggle={() => setLayoutMode(isGrid ? InterfaceLayout.Table : InterfaceLayout.Grid)}
		/>
	)
}
