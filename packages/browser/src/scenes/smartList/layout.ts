import { InterfaceLayout, SmartListGrouping } from '@stump/graphql'
import { create } from 'zustand'
import { createJSONStorage, persist } from 'zustand/middleware'

type SmartListLayoutState = {
	/** The chosen layout per smart list ID */
	layouts: Record<string, InterfaceLayout>
	setLayout: (listId: string, layout: InterfaceLayout) => void
	/** The chosen grouping per smart list ID, overriding the list's default grouping */
	groupings: Record<string, SmartListGrouping>
	setGrouping: (listId: string, grouping: SmartListGrouping | undefined) => void
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
	const setGrouping = useSmartListLayoutStore((state) => state.setGrouping)
	const fallback = defaultGrouping ?? SmartListGrouping.ByBooks
	return {
		grouping: override ?? fallback,
		setGrouping: (grouping: SmartListGrouping) =>
			setGrouping(listId, grouping === fallback ? undefined : grouping),
	}
}
