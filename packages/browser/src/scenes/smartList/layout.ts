import { InterfaceLayout } from '@stump/graphql'
import { create } from 'zustand'
import { createJSONStorage, persist } from 'zustand/middleware'

type SmartListLayoutState = {
	/** The chosen layout per smart list ID */
	layouts: Record<string, InterfaceLayout>
	setLayout: (listId: string, layout: InterfaceLayout) => void
}

/**
 * Remembers, per smart list, whether its books are shown as a table or a cover grid
 */
export const useSmartListLayoutStore = create<SmartListLayoutState>()(
	persist(
		(set) => ({
			layouts: {},
			setLayout: (listId, layout) =>
				set((state) => ({ layouts: { ...state.layouts, [listId]: layout } })),
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
