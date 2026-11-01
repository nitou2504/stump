import { cn } from '@stump/components'
import { Media, SmartListGroupedItem } from '@stump/graphql'
import {
	ExpandedState,
	flexRender,
	getCoreRowModel,
	getExpandedRowModel,
	getFilteredRowModel,
	getSortedRowModel,
	Row,
	SortDirection,
	SortingState,
	useReactTable,
} from '@tanstack/react-table'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useCallback, useLayoutEffect, useMemo, useRef, useState } from 'react'

import { SortIcon } from '@/components/table'
import { columnStyle, tableMinWidth, useTableScroll } from '@/components/table/columnLayout'
import { useLayoutMode } from '@/hooks/useLayoutMode'
import { useTranslate } from '@/hooks/useTranslate'

import { useScrollElement } from '../../../../hooks/useScrollElement'
import { useSafeWorkingView, useSmartListContext } from '../../context'
import { naturalBookOrder, naturalGroupOrder } from './bookOrder'
import { buildColumns as buildGroupColumns } from './groupColumns'
import GroupedSmartListGrid, { BookGroup, useGroupHref } from './GroupedSmartListGrid'
import { bookFuzzySearch, buildColumns as buildBookColumns } from './mediaColumns'
import SmartListLayoutControls from './SmartListLayoutControls'
import { SmartListTableItem } from './SmartListTableItem'
import TableHeaderActions from './TableHeaderActions'

type Props = {
	items: SmartListGroupedItem[]
	/**
	 * Set for viewer-side groupings by a book detail (see `fieldGrouping.ts`): the group
	 * column's label, the label of books without a value, and groups keep their given order
	 * and don't link anywhere
	 */
	fieldGroup?: { label: string; ungroupedLabel: string }
}

type FlatItem =
	| { type: 'group'; row: Row<SmartListGroupedItem> }
	| { type: 'book-header'; groupId: string; groupIndex: number }
	| { type: 'book'; row: Row<Media>; groupId: string; groupIndex: number }

const HEADER_HEIGHT = 40
const GROUP_ROW_HEIGHT = 40
const BOOK_HEADER_HEIGHT = 40

export default function GroupedVirtualSmartListTable({ items, fieldGroup }: Props) {
	const { translate } = useTranslate()
	const { grouping } = useSmartListContext()
	const containerRef = useRef<HTMLDivElement>(null)
	const listRef = useRef<HTMLDivElement>(null)
	const scrollElement = useScrollElement(containerRef)

	const { workingView, updateWorkingView } = useSafeWorkingView()
	const search = workingView.search
	const [expanded, setExpanded] = useState<ExpandedState>({})

	const isGroupedBySeries = grouping === 'BY_SERIES'
	const standaloneLabel = fieldGroup?.ungroupedLabel ?? translate('shared.common.standaloneBooks')
	const groupColumns = useMemo(
		() =>
			buildGroupColumns(
				isGroupedBySeries,
				workingView.groupColumns,
				standaloneLabel,
				fieldGroup?.label,
			),
		[isGroupedBySeries, workingView.groupColumns, standaloneLabel, fieldGroup?.label],
	)

	const groupSorting = useMemo(() => workingView?.groupSorting ?? [], [workingView])
	const setGroupSorting = useCallback(
		(updaterOrValue: SortingState | ((old: SortingState) => SortingState)) => {
			if (typeof updaterOrValue === 'function') {
				const updated = updaterOrValue(groupSorting)
				updateWorkingView({
					groupSorting: updated.length ? updated : undefined,
				})
			} else {
				updateWorkingView({
					groupSorting: updaterOrValue.length ? updaterOrValue : undefined,
				})
			}
		},
		[updateWorkingView, groupSorting],
	)

	// Groups default to name order (standalone books last) rather than query order
	const sortedItems = useMemo(
		() => (fieldGroup ? items : [...items].sort(naturalGroupOrder)),
		[items, fieldGroup],
	)

	const groupTable = useReactTable({
		columns: groupColumns,
		data: sortedItems,
		getCoreRowModel: getCoreRowModel(),
		getExpandedRowModel: getExpandedRowModel(),
		getFilteredRowModel: getFilteredRowModel(),
		getRowCanExpand: () => true,
		getRowId: (row) => row.entity?.id ?? 'standalone',
		getSortedRowModel: getSortedRowModel(),
		globalFilterFn: ({ original: { books, entity } }, _columnId, searchValue) =>
			books.some((book) => bookFuzzySearch(book, searchValue)) ||
			(entity?.name ?? standaloneLabel).toLowerCase().includes(searchValue.toLowerCase()),
		onExpandedChange: setExpanded,
		onSortingChange: setGroupSorting,
		// while searching every matching group is open (see `visibleBooks`)
		state: { expanded: search ? true : expanded, globalFilter: search, sorting: groupSorting },
	})

	// Start from the natural order (series position, then title) so the default view
	// isn't the query's arbitrary order
	const allBooks = useMemo(
		() =>
			[
				...new Map(items.flatMap((item) => item.books).map((book) => [book.id, book])).values(),
			].sort(naturalBookOrder(isGroupedBySeries)),
		[items, isGroupedBySeries],
	)
	const bookColumns = useMemo(
		() => buildBookColumns(workingView.bookColumns),
		[workingView.bookColumns],
	)

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

	const bookTable = useReactTable({
		columns: bookColumns,
		data: allBooks,
		getCoreRowModel: getCoreRowModel(),
		getSortedRowModel: getSortedRowModel(),
		getRowId: (book) => book.id,
		onSortingChange: setSorting,
		state: {
			sorting,
		},
	})

	const bookRows = bookTable.getRowModel().rows
	const { bookRowsById, bookSortMap } = useMemo(() => {
		const rowMap = new Map<string, Row<Media>>()
		const sortMap = new Map<string, number>()

		bookRows.forEach((row, index) => {
			rowMap.set(row.original.id, row)
			sortMap.set(row.original.id, index)
		})

		return { bookRowsById: rowMap, bookSortMap: sortMap }
	}, [bookRows])

	const groupRows = groupTable.getRowModel().rows

	// While searching, matching groups are open and show only the matching books (all of
	// them when the group's own name matches), so a search lands on the books themselves
	const visibleBooks = useCallback(
		({ books, entity }: SmartListGroupedItem) => {
			if (!search) return books
			const groupName = entity?.name ?? standaloneLabel
			if (groupName.toLowerCase().includes(search.toLowerCase())) return books
			return books.filter((book) => bookFuzzySearch(book, search))
		},
		[search, standaloneLabel],
	)

	const flatData = useMemo(() => {
		const flat: FlatItem[] = []

		groupRows.forEach((groupRow) => {
			const groupIndex = flat.length
			flat.push({ row: groupRow, type: 'group' })

			if (groupRow.getIsExpanded()) {
				flat.push({ groupId: groupRow.id, groupIndex, type: 'book-header' })

				const sortedBooks = [...visibleBooks(groupRow.original)].sort((a, b) => {
					const indexA = bookSortMap.get(a.id) ?? 0
					const indexB = bookSortMap.get(b.id) ?? 0
					return indexA - indexB
				})

				sortedBooks.forEach((book) => {
					const bookRow = bookRowsById.get(book.id)
					if (bookRow) {
						flat.push({ groupId: groupRow.id, groupIndex, row: bookRow, type: 'book' })
					}
				})
			}
		})
		return flat
	}, [groupRows, bookRowsById, bookSortMap, visibleBooks])

	// The same groups for the grid layout: every group's (search-matching) books, in the
	// table's group and book order
	const { isGrid } = useLayoutMode()
	const groupHref = useGroupHref()
	const gridGroups = useMemo<BookGroup[]>(
		() =>
			groupRows
				.map((groupRow) => ({
					books: [...visibleBooks(groupRow.original)].sort(
						(a, b) => (bookSortMap.get(a.id) ?? 0) - (bookSortMap.get(b.id) ?? 0),
					),
					href: fieldGroup ? undefined : groupHref(groupRow.original.entity),
					id: groupRow.id,
					isExpanded: groupRow.getIsExpanded(),
					name: groupRow.original.entity?.name ?? standaloneLabel,
					onToggle: () => groupRow.toggleExpanded(),
				}))
				.filter(({ books }) => books.length > 0),
		[groupRows, visibleBooks, bookSortMap, groupHref, standaloneLabel, expanded, fieldGroup],
	)

	// Fill the width; scroll sideways only when the book columns don't fit
	const minWidth = tableMinWidth(bookTable.getVisibleLeafColumns())
	const { ref: scrollRef, scrolls } = useTableScroll(minWidth)

	const rowVirtualizer = useVirtualizer({
		count: flatData.length,
		estimateSize: (index) => {
			const item = flatData[index]
			if (!item) return 40
			if (item.type === 'group') return GROUP_ROW_HEIGHT
			if (item.type === 'book-header') return BOOK_HEADER_HEIGHT
			return 48 // Estimated book row height
		},
		getScrollElement: () => scrollElement,
		overscan: 10,
	})

	const virtualItems = rowVirtualizer.getVirtualItems()
	const totalSize = rowVirtualizer.getTotalSize()

	const activeStickyGroupIndex = useMemo(() => {
		if (virtualItems.length === 0) return -1
		const firstItem = virtualItems[0]
		if (!firstItem) return -1
		const item = flatData[firstItem.index]
		if (!item) return -1
		if (item.type === 'group') return firstItem.index
		if (item.type === 'book-header' || item.type === 'book') return item.groupIndex
		return -1
	}, [virtualItems, flatData])

	const isGroupHeaderVisible = virtualItems.some((vi) => vi.index === activeStickyGroupIndex)
	const isBookHeaderVisible = virtualItems.some((vi) => vi.index === activeStickyGroupIndex + 1)

	const prevExpandedRef = useRef<ExpandedState>(expanded)
	/**
	 * I hate this effect, it isn't even perfect, but without it there is a much more
	 * annoying and jarring scroll jump when collapsing groups near or past the top of the scroll
	 * position. I'm pretty sure it is because when a group collapses, the virtualizer recalculates
	 * the positions of all items, and if the collapsed group was above the current scroll position,
	 * everything shifts up, causing a jump.
	 *
	 * This effect attempts to detect when a group has been collapsed, and if so, checks if that group
	 * is above the current scroll position. If it is, it adjusts the scroll position to account
	 * for the removed height of the collapsed group and its children.
	 */
	useLayoutEffect(() => {
		const prevExpanded = prevExpandedRef.current as Record<string, boolean>
		const currentExpanded = expanded as Record<string, boolean>

		const collapsedIds = Object.keys(prevExpanded).filter(
			(id) => prevExpanded[id] && !currentExpanded[id],
		)

		if (collapsedIds.length > 0 && scrollElement && listRef.current) {
			const collapsedId = collapsedIds[0]
			const groupIndex = flatData.findIndex(
				(item) => item.type === 'group' && item.row.id === collapsedId,
			)

			if (groupIndex !== -1) {
				const listRect = listRef.current.getBoundingClientRect()
				const scrollRect = scrollElement.getBoundingClientRect()
				const listStart = listRect.top - scrollRect.top + scrollElement.scrollTop

				const offsetResult = rowVirtualizer.getOffsetForIndex(groupIndex)
				const groupOffset =
					typeof offsetResult === 'number' ? offsetResult : (offsetResult?.[0] ?? 0)
				const targetScroll = Math.max(0, groupOffset + listStart - HEADER_HEIGHT)

				if (scrollElement.scrollTop > targetScroll) {
					scrollElement.scrollTo({ top: targetScroll })
				}
			}
		}

		prevExpandedRef.current = expanded
	}, [expanded, flatData, rowVirtualizer, scrollElement])

	if (isGrid) {
		return (
			<div ref={containerRef} className="flex w-full flex-col">
				<TableHeaderActions
					layoutControls={<SmartListLayoutControls />}
					showColumnControls={false}
				/>
				<GroupedSmartListGrid
					groups={gridGroups}
					label={fieldGroup?.label ?? (isGroupedBySeries ? 'Series' : 'Library')}
					isAllExpanded={groupTable.getIsAllRowsExpanded()}
					onToggleAll={() => groupTable.toggleAllRowsExpanded()}
					showSeriesPosition={isGroupedBySeries}
				/>
			</div>
		)
	}

	return (
		<div ref={containerRef} className="flex w-full flex-col">
			<TableHeaderActions layoutControls={<SmartListLayoutControls />} />

			<div
				ref={scrollRef}
				className={cn('w-full', scrolls ? 'overflow-x-auto' : 'overflow-x-clip')}
			>
				<div style={{ minWidth }}>
					<div
						className="top-0 sticky z-30 flex w-full border-b border-border bg-muted"
						style={{ height: HEADER_HEIGHT }}
					>
						{groupTable.getHeaderGroups().map((headerGroup) => (
							<div key={headerGroup.id} className="flex w-full">
								{headerGroup.headers.map((header) => {
									const isSortable = header.column.getCanSort()
									return (
										<div
											key={header.id}
											// TODO: Fix the hitbox for sorting, not the entire header just the content
											className={cn(
												'px-4 py-2 text-sm font-medium flex w-full items-center bg-muted text-foreground last:justify-end',
												{
													'gap-x-2 cursor-pointer select-none': isSortable,
												},
											)}
											onClick={header.column.getToggleSortingHandler()}
										>
											{header.isPlaceholder
												? null
												: flexRender(header.column.columnDef.header, header.getContext())}
											{isSortable && (
												<SortIcon
													direction={(header.column.getIsSorted() as SortDirection) ?? null}
												/>
											)}
										</div>
									)
								})}
							</div>
						))}
					</div>

					{activeStickyGroupIndex !== -1 && !isGroupHeaderVisible && (
						<div
							className="sticky z-20 flex w-full border-b border-border bg-muted"
							style={{
								height: GROUP_ROW_HEIGHT,
								top: HEADER_HEIGHT,
							}}
						>
							{(() => {
								const item = flatData[activeStickyGroupIndex]
								if (item?.type !== 'group') return null
								return item.row.getVisibleCells().map((cell) => (
									<div
										key={cell.id}
										className="px-4 py-2 flex w-full items-center last:justify-end"
									>
										{flexRender(cell.column.columnDef.cell, cell.getContext())}
									</div>
								))
							})()}
						</div>
					)}

					{activeStickyGroupIndex !== -1 && !isBookHeaderVisible && (
						<div
							className="sticky z-10 flex w-full border-b border-border bg-background"
							style={{
								height: BOOK_HEADER_HEIGHT,
								top: HEADER_HEIGHT + GROUP_ROW_HEIGHT,
							}}
						>
							{(() => {
								const item = flatData[activeStickyGroupIndex]
								if (item?.type !== 'group' || !item.row.getIsExpanded()) return null

								return bookTable.getHeaderGroups().map((headerGroup) => (
									<div key={headerGroup.id} className="flex w-full">
										{headerGroup.headers.map((header) => {
											const isSortable = header.column.getCanSort()
											return (
												<div
													key={header.id}
													className={cn(
														'px-3 py-2 text-xs first:pl-4 last:pr-4 min-w-0 flex items-center overflow-hidden text-muted-foreground',
														{
															'gap-x-2 cursor-pointer select-none': isSortable,
														},
													)}
													onClick={header.column.getToggleSortingHandler()}
													style={columnStyle(header.column)}
												>
													{header.isPlaceholder
														? null
														: flexRender(header.column.columnDef.header, header.getContext())}
													{isSortable && (
														<SortIcon
															direction={(header.column.getIsSorted() as SortDirection) ?? null}
														/>
													)}
												</div>
											)
										})}
									</div>
								))
							})()}
						</div>
					)}

					<div
						ref={listRef}
						className="relative w-full bg-background"
						style={{ height: totalSize }}
					>
						{/* Spacer to push items down to their virtual position */}
						{virtualItems.length > 0 && virtualItems[0] && (
							<div style={{ height: virtualItems[0].start }} />
						)}

						{virtualItems.map((virtualItem) => {
							const item = flatData[virtualItem.index]
							if (!item) return null

							if (item.type === 'group') {
								return (
									<div
										key={virtualItem.key}
										ref={rowVirtualizer.measureElement}
										className={cn('z-20 flex w-full border-b border-border', {
											'sticky bg-muted': item.row.getIsExpanded(),
										})}
										data-index={virtualItem.index}
										style={{
											height: GROUP_ROW_HEIGHT,
											top: HEADER_HEIGHT,
										}}
									>
										{item.row.getVisibleCells().map((cell) => (
											<div
												key={cell.id}
												className="px-4 py-2 flex w-full items-center last:justify-end"
											>
												{flexRender(cell.column.columnDef.cell, cell.getContext())}
											</div>
										))}
									</div>
								)
							}

							if (item.type === 'book-header') {
								return (
									<div
										key={virtualItem.key}
										ref={rowVirtualizer.measureElement}
										className="sticky z-10 flex w-full border-b border-border bg-background"
										data-index={virtualItem.index}
										style={{
											height: BOOK_HEADER_HEIGHT,
											top: HEADER_HEIGHT + GROUP_ROW_HEIGHT,
										}}
									>
										{bookTable.getHeaderGroups().map((headerGroup) => (
											<div key={headerGroup.id} className="flex w-full">
												{headerGroup.headers.map((header) => {
													const isSortable = header.column.getCanSort()
													return (
														<div
															key={header.id}
															className={cn(
																'px-3 py-2 text-xs first:pl-4 last:pr-4 min-w-0 flex items-center overflow-hidden text-muted-foreground',
																{
																	'gap-x-2 cursor-pointer select-none': isSortable,
																},
															)}
															onClick={header.column.getToggleSortingHandler()}
															style={columnStyle(header.column)}
														>
															{header.isPlaceholder
																? null
																: flexRender(header.column.columnDef.header, header.getContext())}
															{isSortable && (
																<SortIcon
																	direction={(header.column.getIsSorted() as SortDirection) ?? null}
																/>
															)}
														</div>
													)
												})}
											</div>
										))}
									</div>
								)
							}

							if (item.type === 'book') {
								return (
									<div
										key={virtualItem.key}
										ref={rowVirtualizer.measureElement}
										data-index={virtualItem.index}
									>
										<SmartListTableItem cells={item.row.getVisibleCells()} />
									</div>
								)
							}

							return null
						})}
					</div>
				</div>
			</div>
		</div>
	)
}
