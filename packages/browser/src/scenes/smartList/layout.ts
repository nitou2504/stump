import { InterfaceLayout, SmartListGrouping } from '@stump/graphql'
import { create } from 'zustand'
import { createJSONStorage, persist } from 'zustand/middleware'

import { FieldGrouping } from './fieldGrouping'

type SmartListLayoutState = {
	/** The chosen layout per smart list ID */
	layouts: Record<string, InterfaceLayout>
	setLayout: (listId: string, layout: InterfaceLayout) => void
	/** The chosen grouping per smart list ID, overriding the list's default grouping */
	groupings: Record<string, SmartListGrouping>
	setGrouping: (listId: string, grouping: SmartListGrouping | undefined) => void
	/** A grouping by a book detail (author, `Asignatura: *`, ...), computed in the browser */
	fieldGroupings: Record<string, FieldGrouping>
	setFieldGrouping: (listId: string, grouping: FieldGrouping | undefined) => void
}

/**
 * Remembers, per smart list, whether its books are shown as a table or a cover grid and
 * how they are grouped. These are personal view choices: they don't change the list.
 */
export const useSmartListLayoutStore = create<SmartListLayoutState>()(
	persist(
		(set) => ({
			layouts: {},
			setLayout: (listId, layout) =>
				set((state) => ({ layouts: { ...state.layouts, [listId]: layout } })),
			groupings: {},
			setGrouping: (listId, grouping) =>
				set((state) => {
					const groupings = { ...state.groupings }
					if (grouping) {
						groupings[listId] = grouping
					} else {
						delete groupings[listId]
					}
					return { groupings }
				}),
			fieldGroupings: {},
			setFieldGrouping: (listId, grouping) =>
				set((state) => {
					const fieldGroupings = { ...state.fieldGroupings }
					if (grouping) {
						fieldGroupings[listId] = grouping
					} else {
						delete fieldGroupings[listId]
					}
					return { fieldGroupings }
				}),
		}),
		{
			name: 'stump-smart-list-layouts',
			storage: createJSONStorage(() => localStorage),
		},
	),
)

export function useSmartListLayout(listId: string) {
	const layout = useSmartListLayoutStore((state) => state.layouts[listId] ?? InterfaceLayout.Table)
	const setLayout = useSmartListLayoutStore((state) => state.setLayout)
	return {
		layout,
		setLayout: (layout: InterfaceLayout) => setLayout(listId, layout),
	}
}

/**
 * The grouping to display a list with: the viewer's choice, falling back to the list's
 * default grouping
 */
export function useSmartListGrouping(listId: string, defaultGrouping?: SmartListGrouping) {
	const override = useSmartListLayoutStore((state) => state.groupings[listId])
	const fieldGrouping = useSmartListLayoutStore((state) => state.fieldGroupings[listId])
	const setGrouping = useSmartListLayoutStore((state) => state.setGrouping)
	const setFieldGrouping = useSmartListLayoutStore((state) => state.setFieldGrouping)
	const fallback = defaultGrouping ?? SmartListGrouping.ByBooks
	return {
		// a field grouping is computed from the plain (ungrouped) book list
		grouping: fieldGrouping ? SmartListGrouping.ByBooks : (override ?? fallback),
		fieldGrouping: fieldGrouping ?? null,
		setGrouping: (grouping: SmartListGrouping) => {
			setFieldGrouping(listId, undefined)
			setGrouping(listId, grouping === fallback ? undefined : grouping)
		},
		setFieldGrouping: (grouping: FieldGrouping) => setFieldGrouping(listId, grouping),
	}
}
