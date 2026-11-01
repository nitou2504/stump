import { cn } from '@stump/components'
import {
	ColumnDef,
	flexRender,
	getCoreRowModel,
	getFilteredRowModel,
	getSortedRowModel,
	OnChangeFn,
	SortDirection,
	SortingState,
	useReactTable,
} from '@tanstack/react-table'
import { Fragment, PropsWithChildren } from 'react'

import { columnStyle, tableMinWidth, useTableScroll } from './columnLayout'
import SortIcon from './SortIcon'

type EntityTableSorting =
	| {
			sorting: SortingState
			setSorting: OnChangeFn<SortingState>
	  }
	| {
			sorting?: never
			setSorting?: never
	  }

type EntityTableSearch =
	| {
			globalFilter: string
	  }
	| {
			globalFilter?: never
	  }

type StateOptions = EntityTableSorting & EntityTableSearch

export type EntityTableOptions = {
	enableMultiSort?: boolean
} & StateOptions

export type EntityTableProps<Entity> = {
	/**
	 * The items to render in the table.
	 */
	items: Entity[]
	/**
	 * The columns to render in the table. This is a prop in order to support dynamic columns,
	 * e.g. configurable columns.
	 */
	columns: ColumnDef<Entity>[]
	/**
	 * Additional options for the underlying table.
	 */
	options?: EntityTableOptions
	/**
	 * An optional renderer to wrap the table in. This is useful for providing a custom container,
	 * as needed.
	 */
	render?: (props: PropsWithChildren) => React.ReactNode
}

/**
 * A table component used for rendering:
 *
 * - Media / Books
 * - Series
 * - Authors
 *
 * This is separate from the `Table` component primarily in styling, and serves to provide a
 * unified look and feel for the application WRT the scenes which explore these entities.
 *
 * The table fills the available width, with single-line rows (see `columnLayout.ts`).
 */
export default function EntityTable<Entity>({
	items,
	columns,
	options,
	render,
}: EntityTableProps<Entity>) {
	const { setSorting, enableMultiSort, ...state } = options ?? {}

	const table = useReactTable({
		columns,
		data: items,
		enableMultiSort,
		getCoreRowModel: getCoreRowModel(),
		getFilteredRowModel: getFilteredRowModel(),
		getSortedRowModel: getSortedRowModel(),
		onSortingChange: setSorting,
		state,
	})
	const { rows } = table.getRowModel()

	const minWidth = tableMinWidth(table.getVisibleLeafColumns())
	const { ref: wrapperRef, scrolls } = useTableScroll(minWidth)

	const Container = render ?? Fragment

	return (
		<Container>
			<div
				ref={wrapperRef}
				className={cn('w-full', scrolls ? 'overflow-x-auto' : 'overflow-x-clip')}
			>
				<table className="grid w-full grid-cols-1" style={{ minWidth }}>
					<thead
						className={cn('shadow-sm z-2 grid grid-cols-1 bg-background', {
							'top-0 sticky': !scrolls,
						})}
					>
						<tr className="flex w-full">
							{table.getFlatHeaders().map((header) => {
								const isSortable = header.column.getCanSort()
								return (
									<th
										key={header.id}
										className="h-9 px-2 first:pl-4 last:pr-4 min-w-0 flex items-center overflow-hidden border-r border-r-border text-left last:border-r-0"
										style={columnStyle(header.column)}
									>
										<div
											className={cn('min-w-0 flex items-center', {
												'gap-x-2 cursor-pointer select-none': isSortable,
											})}
											onClick={header.column.getToggleSortingHandler()}
										>
											<div className="min-w-0 truncate">
												{flexRender(header.column.columnDef.header, header.getContext())}
											</div>
											{isSortable && (
												<SortIcon
													direction={(header.column.getIsSorted() as SortDirection) ?? null}
												/>
											)}
										</div>
									</th>
								)
							})}
						</tr>
					</thead>

					<tbody className="grid grid-cols-1">
						{rows.map((row) => (
							<tr key={row.id} className="flex w-full odd:bg-muted">
								{row.getVisibleCells().map((cell) => (
									<td
										className={cn(
											'px-2 py-1 min-h-9 min-w-0 flex items-center overflow-hidden border-r border-r-border last:border-r-0',
											{
												'first:pl-4': !['cover', 'position'].includes(cell.column.id),
												'last:pr-4': true,
											},
										)}
										key={cell.id}
										style={columnStyle(cell.column)}
									>
										{flexRender(cell.column.columnDef.cell, cell.getContext())}
									</td>
								))}
							</tr>
						))}
					</tbody>
				</table>
			</div>
		</Container>
	)
}
