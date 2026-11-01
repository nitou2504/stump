import { InterfaceLayout } from '@stump/graphql'
import { useCallback } from 'react'
import { toast } from 'sonner'

import { useUserStore } from '@/stores'

import { usePreferences } from './usePreferences'

/**
 * The grid/table layout of every book and series list (libraries, series, search, smart
 * lists, files). It is one per-user preference (`preferredLayoutMode`, saved on the server),
 * so switching it on any screen switches every screen. The change applies immediately and
 * is reverted if the server rejects it.
 */
export function useLayoutMode() {
	const layoutMode = useUserStore(
		(state) => state.userPreferences?.preferredLayoutMode ?? InterfaceLayout.Grid,
	)
	const hasPreferences = useUserStore((state) => !!state.userPreferences)
	const { update, store } = usePreferences()

	const setLayoutMode = useCallback(
		(layout: InterfaceLayout) => {
			if (layout === layoutMode || !hasPreferences) return
			const previous = layoutMode
			store({ preferredLayoutMode: layout })
			update(
				{ preferredLayoutMode: layout },
				{
					onError: () => {
						store({ preferredLayoutMode: previous })
						toast.error('Failed to save the layout')
					},
				},
			)
		},
		[layoutMode, hasPreferences, store, update],
	)

	return {
		layoutMode,
		isGrid: layoutMode === InterfaceLayout.Grid,
		setLayoutMode,
	}
}
