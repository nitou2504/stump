import {
	closestCenter,
	DndContext,
	DragEndEvent,
	DragOverEvent,
	DragOverlay,
	DragStartEvent,
	KeyboardSensor,
	MouseSensor,
	TouchSensor,
	useDroppable,
	useSensor,
	useSensors,
} from '@dnd-kit/core'
import {
	arrayMove,
	rectSortingStrategy,
	SortableContext,
	sortableKeyboardCoordinates,
	useSortable,
} from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { Button, cn, IconButton, SheetPrimitive, Text, ToolTip } from '@stump/components'
import { ColumnSort } from '@stump/sdk'
import partition from 'lodash/partition'
import { Columns, Eye, EyeOff, GripVertical } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { useMediaMatch } from 'rooks'
import { toast } from 'sonner'

import { useTranslate } from '@/hooks/useTranslate'

import { bookTableColumnMap } from '../book/table'
import { seriesTableColumnMap } from '../series/table'

type Props = {
	entity: 'media' | 'series' | 'library'
	configuration: ColumnSort[]
	onSave: (columns: ColumnSort[]) => void
}

const TITLE = 'Adjust columns'

/**
 * The column picker of the book and series tables: which columns are shown (drag between
 * Visible and Hidden, or use the eye buttons) and in which order (drag within Visible).
 */
export default function EntityTableColumnConfiguration({ entity, configuration, onSave }: Props) {
	const { translate } = useTranslate()

	const [isOpen, setIsOpen] = useState(false)

	const isMobile = useMediaMatch('(max-width: 768px)')
	const sensors = useSensors(
		useSensor(MouseSensor, {
			activationConstraint: {
				// Require pointer to move by 5 pixels before activating draggable
				// Allows nested onClicks/buttons/interactions to be accessed
				distance: 5,
			},
		}),
		// On touch screens a long press starts a drag, so the list can still be scrolled
		useSensor(TouchSensor, {
			activationConstraint: { delay: 250, tolerance: 5 },
		}),
		useSensor(KeyboardSensor, {
			coordinateGetter: sortableKeyboardCoordinates,
		}),
	)

	const columnMap = useMemo<Record<string, string>>(() => {
		if (entity === 'media') {
			return bookTableColumnMap
		} else if (entity === 'series') {
			return seriesTableColumnMap
		}

		return {}
	}, [entity])

	const resolved = useMemo(
		() => resolveConfiguration(configuration, columnMap),
		[configuration, columnMap],
	)

	const toBuckets = (columns: ResolvedColumn[]): [ResolvedColumn[], ResolvedColumn[]] =>
		partition(columns, (column) => column.selected)

	const [[visible, hidden], setBuckets] = useState(() => toBuckets(resolved))
	const [activeColumnId, setActiveColumnId] = useState<string | null>(null)

	const allColumns = useMemo(() => [...visible, ...hidden], [visible, hidden])
	const activeColumn = useMemo(
		() => allColumns.find((column) => column.id === activeColumnId) ?? null,
		[allColumns, activeColumnId],
	)

	/**
	 * Opens the picker on the current configuration, dropping unsaved changes of a
	 * previous visit
	 */
	const handleOpenChange = (open: boolean) => {
		if (open) {
			setBuckets(toBuckets(resolveConfiguration(configuration, columnMap)))
		}
		setIsOpen(open)
	}

	/**
	 * Moves a column into either visible or hidden list
	 */
	const handleMoveColumn = (id: string, toVisible: boolean) => {
		setBuckets((prev) => {
			const [prevVisible, prevHidden] = prev
			const source = toVisible ? prevHidden : prevVisible
			const destination = toVisible ? prevVisible : prevHidden
			const moved = source.find((column) => column.id === id)

			if (!moved) {
				return prev
			}

			const updatedSource = source.filter((column) => column.id !== id)
			const updatedDestination = [...destination, { ...moved, selected: toVisible }]

			return toVisible ? [updatedDestination, updatedSource] : [updatedSource, updatedDestination]
		})
	}

	/**
	 * A callback to persist the current local state to the parent component.
	 */
	const handleSave = useCallback(() => {
		if (!visible.length) {
			toast.error('Show at least one column')
			return
		}
		const onlySelected = visible.map(({ id }, idx) => ({
			id,
			position: idx,
		}))
		setIsOpen(false)
		// Note: I did this to push to end of event loop so animation isn't interrupted
		setTimeout(() => onSave(onlySelected))
	}, [visible, onSave])

	/**
	 * A callback to handle the end of a drag event. If the column is dragged over another column,
	 * the columns will be re-ordered according to the new position.
	 */
	const handleDragEnd = (event: DragEndEvent) => {
		setActiveColumnId(null)
		const { active, over } = event
		if (!over?.id) return

		const activeId = String(active.id)
		const overId = String(over.id)

		const findContainer = (id: string): 'visible' | 'hidden' | null => {
			if (id === 'visible-container') return 'visible'
			if (id === 'hidden-container') return 'hidden'
			if (visible.some((column) => column.id === id)) return 'visible'
			if (hidden.some((column) => column.id === id)) return 'hidden'
			return null
		}

		const sourceContainer = findContainer(activeId)
		const destinationContainer = findContainer(overId)

		if (!sourceContainer || !destinationContainer) return

		if (sourceContainer === destinationContainer) {
			setBuckets((prev) => {
				const [prevVisible, prevHidden] = prev
				const source = sourceContainer === 'visible' ? prevVisible : prevHidden
				const oldIndex = source.findIndex((column) => column.id === activeId)
				const newIndex =
					overId === `${sourceContainer}-container`
						? source.length - 1
						: source.findIndex((column) => column.id === overId)

				if (oldIndex === -1 || newIndex === -1 || oldIndex === newIndex) {
					return prev
				}

				const moved = arrayMove(source, oldIndex, newIndex)
				return sourceContainer === 'visible' ? [moved, prevHidden] : [prevVisible, moved]
			})
		} else {
			setBuckets((prev) => {
				const [prevVisible, prevHidden] = prev
				const sourceList = sourceContainer === 'visible' ? prevVisible : prevHidden
				const destinationList = destinationContainer === 'visible' ? prevVisible : prevHidden

				const sourceIndex = sourceList.findIndex((column) => column.id === activeId)
				if (sourceIndex === -1) return prev

				const movedColumn = sourceList[sourceIndex]
				if (!movedColumn) return prev

				const source = sourceList.filter((column) => column.id !== activeId)
				const destination = [...destinationList]

				const destinationIndex =
					overId === `${destinationContainer}-container`
						? destination.length
						: destination.findIndex((column) => column.id === overId)

				if (destinationIndex === -1) {
					destination.push({
						...movedColumn,
						selected: destinationContainer === 'visible',
					})
				} else {
					destination.splice(destinationIndex, 0, {
						...movedColumn,
						selected: destinationContainer === 'visible',
					})
				}

				return destinationContainer === 'visible' ? [destination, source] : [source, destination]
			})
		}
	}

	const handleDragOver = (event: DragOverEvent) => {
		const { active, over } = event
		if (!over?.id) return

		const activeId = String(active.id)
		const overId = String(over.id)

		setBuckets((prev) => {
			const [prevVisible, prevHidden] = prev
			const findContainer = (id: string): 'visible' | 'hidden' | null => {
				if (id === 'visible-container') return 'visible'
				if (id === 'hidden-container') return 'hidden'
				if (prevVisible.some((column) => column.id === id)) return 'visible'
				if (prevHidden.some((column) => column.id === id)) return 'hidden'
				return null
			}

			const sourceContainer = findContainer(activeId)
			const destinationContainer = findContainer(overId)

			if (!sourceContainer || !destinationContainer || sourceContainer === destinationContainer) {
				return prev
			}

			const sourceList = sourceContainer === 'visible' ? prevVisible : prevHidden
			const destinationList = destinationContainer === 'visible' ? prevVisible : prevHidden

			const sourceIndex = sourceList.findIndex((column) => column.id === activeId)
			if (sourceIndex === -1) return prev

			const movedColumn = sourceList[sourceIndex]
			if (!movedColumn) return prev

			const source = sourceList.filter((column) => column.id !== activeId)
			const destination = [...destinationList]

			const destinationIndex =
				overId === `${destinationContainer}-container`
					? destination.length
					: destination.findIndex((column) => column.id === overId)

			const moved = {
				...movedColumn,
				selected: destinationContainer === 'visible',
			}

			if (destinationIndex === -1) {
				destination.push(moved)
			} else {
				destination.splice(destinationIndex, 0, moved)
			}

			return destinationContainer === 'visible' ? [destination, source] : [source, destination]
		})
	}

	const handleDragStart = ({ active }: DragStartEvent) => setActiveColumnId(String(active.id))

	const handleDragCancel = () => {
		setActiveColumnId(null)
	}

	/**
	 * The IDs of all columns in the current configuration, used for sorting and re-ordering.
	 */
	const visibleIdentifiers = useMemo(() => visible.map((column) => column.id), [visible])
	const hiddenIdentifiers = useMemo(() => hidden.map((column) => column.id), [hidden])

	return (
		<SheetPrimitive open={isOpen} onOpenChange={handleOpenChange}>
			<ToolTip content={TITLE} size="sm" align="start">
				<SheetPrimitive.Trigger asChild>
					<IconButton size="xs" variant="ghost" aria-label={TITLE}>
						<Columns className="h-4 w-4" />
					</IconButton>
				</SheetPrimitive.Trigger>
			</ToolTip>
			<SheetPrimitive.Content
				size={isMobile ? 'full' : 'default'}
				className="gap-0 md:max-w-xl md:min-w-md"
				closeIcon
				// Don't refocus the trigger on close: its tooltip would pop up and stay
				onCloseAutoFocus={(event) => event.preventDefault()}
			>
				<SheetPrimitive.Header className="pr-10 shrink-0">
					<SheetPrimitive.Title>{TITLE}</SheetPrimitive.Title>
					<SheetPrimitive.Description>
						Choose the columns of this table. Drag them to change their order.
					</SheetPrimitive.Description>
				</SheetPrimitive.Header>

				<div className="min-h-0 flex-1 overflow-y-auto">
					<DndContext
						sensors={sensors}
						collisionDetection={closestCenter}
						onDragStart={handleDragStart}
						onDragOver={handleDragOver}
						onDragCancel={handleDragCancel}
						onDragEnd={handleDragEnd}
					>
						<div className="gap-3 p-4 sm:grid-cols-2 grid grid-cols-1 items-start">
							<ColumnBucket
								title="Visible"
								containerId="visible-container"
								items={visible}
								identifiers={visibleIdentifiers}
								onMoveToOtherList={(id) => handleMoveColumn(id, false)}
							/>
							<ColumnBucket
								title="Hidden"
								containerId="hidden-container"
								items={hidden}
								identifiers={hiddenIdentifiers}
								onMoveToOtherList={(id) => handleMoveColumn(id, true)}
							/>
						</div>
						<DragOverlay>{activeColumn && <ColumnOverlay column={activeColumn} />}</DragOverlay>
					</DndContext>
				</div>

				<div className="gap-3 p-4 flex shrink-0 border-t border-border">
					<Button variant="outline" className="flex-1" onClick={() => setIsOpen(false)}>
						{translate('shared.common.cancel')}
					</Button>
					<Button className="flex-1" onClick={handleSave}>
						{translate('shared.common.save')}
					</Button>
				</div>
			</SheetPrimitive.Content>
		</SheetPrimitive>
	)
}

const resolveConfiguration = (configuration: ColumnSort[], columnMap: Record<string, string>) =>
	Object.entries(columnMap)
		.map(([key, label], idx) => {
			const configPosition = configuration.findIndex((column) => column.id === key)
			return {
				id: key,
				label,
				position: configPosition === -1 ? configuration.length + idx : configPosition,
				selected: configPosition !== -1,
			}
		})
		.sort((a, b) => a.position - b.position)

type ResolvedColumn = ReturnType<typeof resolveConfiguration>[number]

type DraggableColumnProps = {
	column: ResolvedColumn
	moveToOtherList: () => void
	buttonLabel: string
	disabled?: boolean
}
function DraggableColumn({ column, moveToOtherList, buttonLabel }: DraggableColumnProps) {
	const { attributes, isDragging, listeners, setNodeRef, transform, transition } = useSortable({
		id: column.id,
		transition: {
			duration: 250,
			easing: 'cubic-bezier(0.25, 1, 0.5, 1)',
		},
	})

	const style = {
		transform: CSS.Transform.toString(transform),
		transition,
	}

	const VisibilityIcon = column.selected ? Eye : EyeOff

	return (
		<div
			ref={setNodeRef}
			style={style}
			{...attributes}
			{...listeners}
			className={cn(
				'gap-2 py-1 pl-2 pr-1 flex shrink-0 cursor-grab items-center rounded-md border border-border bg-muted active:cursor-grabbing',
				{ 'opacity-40': isDragging },
			)}
		>
			<GripVertical className="h-4 w-4 shrink-0 text-muted-foreground" />
			<Text size="sm" className="min-w-0 flex-1 truncate">
				{column.label}
			</Text>
			<IconButton
				size="xs"
				variant="ghost"
				title={buttonLabel}
				aria-label={`${buttonLabel}: ${column.label}`}
				onClick={moveToOtherList}
				className={column.selected ? 'text-foreground' : 'text-muted-foreground'}
			>
				<VisibilityIcon className="h-4 w-4" />
			</IconButton>
		</div>
	)
}

function ColumnOverlay({ column }: { column: ReturnType<typeof resolveConfiguration>[number] }) {
	return (
		<div className="min-w-40 gap-2 py-1 pl-2 pr-1 shadow-sm h-9 flex cursor-grabbing items-center rounded-md border border-border bg-muted">
			<GripVertical className="h-4 w-4 shrink-0 text-muted-foreground" />
			<Text size="sm">{column.label}</Text>
		</div>
	)
}

type ColumnBucketProps = {
	title: string
	containerId: string
	items: ReturnType<typeof resolveConfiguration>
	identifiers: string[]
	onMoveToOtherList: (id: string) => void
}

function ColumnBucket({
	title,
	containerId,
	items,
	identifiers,
	onMoveToOtherList,
}: ColumnBucketProps) {
	const { setNodeRef } = useDroppable({ id: containerId })
	const moveLabel = title === 'Visible' ? 'Hide' : 'Show'

	return (
		<div ref={setNodeRef} className="p-3 rounded-md border border-border bg-background">
			<div className="mb-2 flex items-center justify-between">
				<Text size="sm" variant="secondary">
					{title}
				</Text>
				<Text size="xs" variant="muted">
					{items.length}
				</Text>
			</div>

			<SortableContext items={identifiers} strategy={rectSortingStrategy}>
				<div className="min-h-20 gap-1.5 flex flex-col">
					{items.map((column) => (
						<DraggableColumn
							column={column}
							key={column.id}
							moveToOtherList={() => onMoveToOtherList(column.id)}
							buttonLabel={moveLabel}
						/>
					))}
					{items.length === 0 && (
						<div className="min-h-10 rounded px-2 py-3 flex items-center justify-center border border-dashed border-border">
							<Text size="xs" variant="muted">
								Drop columns here
							</Text>
						</div>
					)}
				</div>
			</SortableContext>
		</div>
	)
}
