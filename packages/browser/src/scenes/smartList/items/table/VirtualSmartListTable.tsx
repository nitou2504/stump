import { cn } from '@stump/components'
import { Media } from '@stump/graphql'
import {
	flexRender,
	getCoreRowModel,
	getFilteredRowModel,
	getSortedRowModel,
	SortDirection,
	SortingState,
	useReactTable,
} from '@tanstack/react-table'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useCallback, useMemo, useRef } from 'react'

import { SortIcon } from '@/components/table'

import { useScrollElement } from '../../../../hooks/useScrollElement'
import { useSafeWorkingView } from '../../context'
import { naturalBookOrder } from './bookOrder'
import { bookFuzzySearch, buildColumns } from './mediaColumns'
import SmartListBookGrid from './SmartListBookGrid'
import SmartListLayoutControls, { useSmartListItemsLayout } from './SmartListLayoutControls'
import { SmartListTableItem } from './SmartListTableItem'
import TableHeaderActions from './TableHeaderActions'

type Props = {
	books: Media[]
}

const HEADER_HEIGHT = 40
const ROW_HEIGHT = 48

export default function VirtualSmartListTable({ books: unsortedBooks }: Props) {
	// Start from the natural (title) order instead of the query's arbitrary order
	const books = useMemo(() => [...unsortedBooks].sort(naturalBookOrder(false)), [unsortedBooks])
	const containerRef = useRef<HTMLDivElement>(null)
	const scrollElement = useScrollElement(containerRef)

	const { workingView, updateWorkingView } = useSafeWorkingView()

	const search = workingView.search

	const columns = useMemo(() => buildColumns(workingView.bookColumns), [workingView.bookColumns])

	const sorting = useMemo(() => workingView?.bookSorting ?? [], [workingView])
	const setSorting = useCallback(
		(updaterOrValue: SortingState | ((old: SortingState) => SortingState)) => {
			if (typeof updaterOrValue === 'function') {
				const updated = updaterOrValue(sorting)
				updateWorkingView({
					bookSorting: updated.length ? updated : undefined,
				})
			} else {
				updateWorkingView({
					bookSorting: updaterOrValue.length ? updaterOrValue : undefined,
				})
			}
		},
		[updateWorkingView, sorting],
	)

	const table = useReactTable({
		columns,
		data: books,
		getCoreRowModel: getCoreRowModel(),
		getFilteredRowModel: getFilteredRowModel(),
		getSortedRowModel: getSortedRowModel(),
		getRowId: (book) => book.id,
		// Search every book detail (writers, genres, tags, ...), not only the visible columns
		globalFilterFn: ({ original }, _columnId, value) => bookFuzzySearch(original, String(value)),
		onSortingChange: setSorting,
		state: {
			globalFilter: search,
			sorting,
		},
	})

	const rows = table.getRowModel().rows

	const rowVirtualizer = useVirtualizer({
		count: rows.length,
		estimateSize: () => ROW_HEIGHT,
		getScrollElement: () => scrollElement,
		overscan: 10,
	})

	const virtualItems = rowVirtualizer.getVirtualItems()
	const totalSize = rowVirtualizer.getTotalSize()

	const { isGrid } = useSmartListItemsLayout()
	const layoutControls = <SmartListLayoutControls />

	if (isGrid) {
		return (
			<div ref={containerRef} className="flex w-full flex-col">
				<TableHeaderActions layoutControls={layoutControls} showColumnControls={false} />
				<SmartListBookGrid books={rows.map((row) => row.original)} scrollElement={scrollElement} />
			</div>
		)
	}

	return (
		<div ref={containerRef} className="flex w-full flex-col">
			<TableHeaderActions layoutControls={layoutControls} />

			<div
				className="top-0 sticky z-30 flex w-full border-b border-border bg-background"
				style={{ height: HEADER_HEIGHT }}
			>
				{table.getHeaderGroups().map((headerGroup) => (
					<div key={headerGroup.id} className="flex w-full bg-muted">
						{headerGroup.headers.map((header) => {
							const isSortable = header.column.getCanSort()
							return (
								<div
									key={header.id}
									className={cn(
										'px-4 py-2 text-sm font-medium flex items-center overflow-hidden bg-muted text-foreground',
										{
											'gap-x-2 cursor-pointer select-none': isSortable,
										},
									)}
									onClick={header.column.getToggleSortingHandler()}
									style={{
										flex: `0 0 ${header.getSize()}px`,
										width: header.getSize(),
										minWidth: header.column.columnDef.minSize,
										maxWidth: header.column.columnDef.maxSize,
									}}
								>
									{header.isPlaceholder
										? null
										: flexRender(header.column.columnDef.header, header.getContext())}
									{isSortable && (
										<SortIcon direction={(header.column.getIsSorted() as SortDirection) ?? null} />
									)}
								</div>
							)
						})}
					</div>
				))}
			</div>

			<div className="relative w-full" style={{ height: totalSize }}>
				{/* Spacer to push items down to their virtual position */}
				{virtualItems.length > 0 && virtualItems[0] && (
					<div style={{ height: virtualItems[0].start }} />
				)}

				{virtualItems.map((virtualItem) => {
					const row = rows[virtualItem.index]
					if (!row) return null

					return (
						<div
							key={virtualItem.key}
							ref={rowVirtualizer.measureElement}
							data-index={virtualItem.index}
						>
							<SmartListTableItem row={row} />
						</div>
					)
				})}
			</div>
		</div>
	)
}
