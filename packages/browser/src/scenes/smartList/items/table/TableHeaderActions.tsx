import { ReactNode, useCallback } from 'react'

import { Search, URLFilterDrawer } from '@/components/filters'
import { useTranslate } from '@/hooks/useTranslate'

import { useSafeWorkingView, useSmartListContext } from '../../context'
import { useSmartListViewStore } from '../../store'
import SavedViewSelector from './SavedViewSelector'
import TableColumnsBottomDrawer from './TableColumnsBottomDrawer'
import ViewManagerDropdown from './ViewManagerDropdown'

type Props = {
	/** Extra controls rendered at the end of the header, e.g. the table/grid toggle */
	layoutControls?: ReactNode
	/** Whether the table-only controls (columns) should be shown */
	showColumnControls?: boolean
}

export default function TableHeaderActions({ layoutControls, showColumnControls = true }: Props) {
	const { t } = useTranslate()
	const {
		workingView: { search },
		updateWorkingView,
	} = useSafeWorkingView()

	const selectedView = useSmartListViewStore((state) => state.selectedView)
	const workingView = useSmartListViewStore((state) => state.workingView)
	const {
		list: { views },
	} = useSmartListContext()
	// Only show the saved-view controls once they can do something: the selector when
	// views exist, the save menu once the view (columns, sorting, search) was changed
	const hasSavedViews = !!views?.length
	const canSaveView = !!workingView || !!selectedView

	const setFilter = useCallback(
		(value?: string) => updateWorkingView({ search: value || undefined }),
		[updateWorkingView],
	)

	return (
		<header className="-mt-2 mb-2 gap-x-2 px-4 flex w-full items-center bg-background">
			{hasSavedViews && <SavedViewSelector />}
			<URLFilterDrawer entity="media" />
			{showColumnControls && <TableColumnsBottomDrawer />}
			<Search
				key={selectedView?.id ?? 'default'}
				initialValue={search ?? undefined}
				onChange={(value) => setFilter(value)}
				placeholder={t('userSmartListScene.itemsScene.actionHeader.search.placeholder')}
			/>
			{canSaveView && <ViewManagerDropdown />}
			{layoutControls && <div className="ml-auto">{layoutControls}</div>}
		</header>
	)
}
