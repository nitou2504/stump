import {
	Button,
	CheckBox,
	Drawer,
	IconButton,
	Label,
	RawSwitch,
	Text,
	ToolTip,
} from '@stump/components'
import { TableProperties } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'

import { useSafeWorkingView, useSmartListContext } from '../../context'
import { getColumnOptionMap as getGroupColumnOptionMap } from './groupColumns'
import { columnOptionMap as mediaColumnOptionMap } from './mediaColumns'

export default function TableColumnsBottomDrawer() {
	const [isOpen, setIsOpen] = useState(false)
	const { grouping: defaultGrouping } = useSmartListContext()
	const {
		workingView: {
			bookColumns: selectedBookColumns,
			groupColumns: selectedGroupColumns,
			enableMultiSort,
		},
		updateWorkingView,
	} = useSafeWorkingView()

	const isGrouped = !defaultGrouping || defaultGrouping !== 'BY_BOOKS'

	/**
	 * The local state to track the selected columns for the book table(s). This will be used to
	 * update the context working view once the user clicks save
	 */
	const [bookColumnState, setBookColumnState] = useState<Record<string, boolean>>(() => {
		const state: Record<string, boolean> = {}
		selectedBookColumns.forEach(({ id }) => {
			state[id] = true
		})
		return state
	})

	/**
	 * The local state to track the selected columns of the parent table (if any)
	 */
	const [groupByEntityColumnState, setGroupByEntityColumnState] = useState<Record<string, boolean>>(
		() => {
			const state: Record<string, boolean> = {}
			selectedGroupColumns.forEach(({ id }) => {
				state[id] = true
			})
			return state
		},
	)

	/**
	 * The local state to track whether multi-sort is enabled
	 */
	const [multiSort, setMultiSort] = useState(() => enableMultiSort ?? false)

	/**
	 * The options available to the user to select from
	 */
	const bookColumnOptions = useMemo(
		() =>
			Object.entries(mediaColumnOptionMap).map(([key, label]) => ({
				isSelected: bookColumnState[key] ?? false,
				label,
				value: key,
			})),
		[bookColumnState],
	)

	const isGroupedBySeries = defaultGrouping === 'BY_SERIES'
	const groupColumnOptions = useMemo(() => {
		if (isGrouped) {
			const all = getGroupColumnOptionMap(isGroupedBySeries)
			return Object.entries(all).map(([key, label]) => ({
				isSelected: groupByEntityColumnState[key] ?? false,
				label,
				value: key,
			}))
		}

		return []
	}, [isGrouped, isGroupedBySeries, groupByEntityColumnState])

	/**
	 * A callback to update the local state when a book column is selected or deselected
	 * @param id The ID of the column to update
	 */
	const handleChangeBookColumnState = (id: string) => {
		setBookColumnState((state) => ({ ...state, [id]: !state[id] }))
	}

	/**
	 * A callback to update the local state when a group column is selected or deselected
	 * @param id
	 */
	const handleChangeGroupColumnState = (id: string) => {
		setGroupByEntityColumnState((state) => ({ ...state, [id]: !state[id] }))
	}

	/**
	 * A callback to update the working view with the current local state
	 */
	const handleSave = () => {
		const bookColumns = Object.entries(bookColumnState)
			.filter(([, isSelected]) => isSelected)
			.map(([id], idx) => ({ id, position: idx }))

		const groupColumns = Object.entries(groupByEntityColumnState)
			.filter(([, isSelected]) => isSelected)
			.map(([id], idx) => ({ id, position: idx }))

		if (!bookColumns.length || (isGrouped && !groupColumns.length)) {
			toast.error('You must select at least one column')
			return
		}

		const enableMultiSort = multiSort || undefined

		updateWorkingView({ bookColumns: bookColumns, enableMultiSort, groupColumns: groupColumns })
		setIsOpen(false)
	}

	/**
	 * An effect to update the local book column state whenever the working view changes
	 */
	useEffect(() => {
		setBookColumnState(() => {
			const newState: Record<string, boolean> = {}
			selectedBookColumns.forEach(({ id }) => {
				newState[id] = true
			})
			return newState
		})
	}, [selectedBookColumns])

	// FIXME(smart-list): This is really buggy. Move all of these fucking updates out of effects,
	// such a strong code smell
	/**
	 * An effect to update the local group column state whenever the working view changes
	 */
	useEffect(() => {
		if (selectedGroupColumns.length === 0 && Object.keys(groupByEntityColumnState).length === 0) {
			return
		}
		if (
			selectedGroupColumns.length === Object.keys(groupByEntityColumnState).length &&
			selectedGroupColumns.every(({ id }) => groupByEntityColumnState[id])
		) {
			return
		}

		setGroupByEntityColumnState(() => {
			const newState: Record<string, boolean> = {}
			selectedGroupColumns.forEach(({ id }) => {
				newState[id] = true
			})
			return newState
		})
	}, [selectedGroupColumns, groupByEntityColumnState])

	/**
	 * An effect to update the local multi-sort state whenever the working view changes
	 */
	useEffect(() => {
		setMultiSort(enableMultiSort ?? false)
	}, [enableMultiSort])

	const handleOpenChanged = (nowOpen: boolean) => {
		if (!nowOpen) {
			setIsOpen(false)
		}
	}

	return (
		<Drawer open={isOpen} onOpenChange={handleOpenChanged}>
			<ToolTip content="Adjust columns">
				<Drawer.Trigger asChild onClick={() => setIsOpen(true)}>
					<IconButton variant="ghost" aria-label="Adjust columns">
						<TableProperties className="h-4 w-4 text-muted-foreground" />
					</IconButton>
				</Drawer.Trigger>
			</ToolTip>
			<Drawer.Content
				className="max-h-[90dvh]"
				// Don't refocus the trigger on close: its tooltip would pop up and stay
				onCloseAutoFocus={(event) => event.preventDefault()}
			>
				<div className="max-w-2xl min-h-0 mx-auto flex w-full flex-col">
					<Drawer.Header className="shrink-0">
						<Drawer.Title>Adjust columns</Drawer.Title>
						<Drawer.Description>Choose the columns of this list&apos;s table</Drawer.Description>
					</Drawer.Header>
					<div className="gap-y-6 p-4 pb-0 min-h-0 flex flex-col overflow-y-auto">
						{isGrouped && (
							<div>
								<Label>Group columns</Label>
								<Text size="sm" variant="muted">
									This only affects the parent table by which books are grouped
								</Text>
								<div className="mt-3 gap-x-2 gap-y-4 sm:grid-cols-3 md:grid-cols-4 grid grid-cols-2">
									{groupColumnOptions.map(({ label, value, isSelected }) => (
										<CheckBox
											id={value}
											checked={isSelected}
											key={value}
											label={label}
											onClick={() => handleChangeGroupColumnState(value)}
										/>
									))}
								</div>
							</div>
						)}

						<div>
							<Label>Book columns</Label>
							<Text size="sm" variant="muted">
								This only affects the nested book table(s)
							</Text>
							<div className="mt-3 gap-x-2 gap-y-4 sm:grid-cols-3 md:grid-cols-4 grid grid-cols-2">
								{bookColumnOptions.map(({ label, value, isSelected }) => (
									<CheckBox
										id={value}
										checked={isSelected}
										key={value}
										label={label}
										onClick={() => handleChangeBookColumnState(value)}
									/>
								))}
							</div>
						</div>

						<div className="flex items-center justify-between">
							<div className="gap-2 flex grow flex-col text-left">
								<Label htmlFor="enable_multi_sort">Enable multi-sort</Label>
								<Text size="sm" variant="muted">
									Sorting by a column removes the previous sort. This setting allows combining them,
									instead
								</Text>
							</div>

							<div className="w-6" />

							<RawSwitch
								id="enable_multi_sort"
								checked={multiSort}
								onClick={() => setMultiSort((state) => !state)}
							/>
						</div>
					</div>
					<Drawer.Footer className="shrink-0 flex-row">
						<Drawer.Close asChild>
							<Button variant="outline" className="flex-1">
								Cancel
							</Button>
						</Drawer.Close>
						<Button className="flex-1" onClick={handleSave}>
							Save
						</Button>
					</Drawer.Footer>
				</div>
			</Drawer.Content>
		</Drawer>
	)
}
