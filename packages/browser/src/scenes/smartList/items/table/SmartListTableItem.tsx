import { Media } from '@stump/graphql'
import { Cell, flexRender } from '@tanstack/react-table'

import { columnStyle } from '@/components/table/columnLayout'

type Props = {
	/**
	 * The row's visible cells (`row.getVisibleCells()`). Not the row itself: TanStack Table
	 * keeps a row's identity when the columns change, so a memoized row would keep showing
	 * the previous columns' cells, while this array is new whenever the columns change
	 */
	cells: Cell<Media, unknown>[]
	style?: React.CSSProperties
}

export const SmartListTableItem = ({ cells, style }: Props) => {
	return (
		<div
			className="flex w-full border-b border-border bg-background hover:bg-muted/50"
			style={style}
		>
			{cells.map((cell) => (
				<div
					key={cell.id}
					className="px-3 py-1 min-h-9 min-w-0 first:pl-4 last:pr-4 flex items-center overflow-hidden"
					style={columnStyle(cell.column)}
				>
					{flexRender(cell.column.columnDef.cell, cell.getContext())}
				</div>
			))}
		</div>
	)
}
