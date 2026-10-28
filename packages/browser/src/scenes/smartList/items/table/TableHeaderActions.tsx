import { ReactNode, useCallback } from 'react'

import { Search } from '@/components/filters'
import { useTranslate } from '@/hooks/useTranslate'

import { useSafeWorkingView } from '../../context'
import { useSmartListViewStore } from '../../store'
import FilterBottomDrawer from './FilterBottomDrawer'
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

	const setFilter = useCallback(
		(value?: string) => updateWorkingView({ search: value || undefined }),
		[updateWorkingView],
	)

	return (
		<header className="-mt-2 mb-2 gap-x-2 px-4 flex w-full items-center bg-background">
			<SavedViewSelector />
			<FilterBottomDrawer />
			{showColumnControls && <TableColumnsBottomDrawer />}
			<Search
				key={selectedView?.id ?? 'default'}
				initialValue={search ?? undefined}
				onChange={(value) => setFilter(value)}
				placeholder={t('userSmartListScene.itemsScene.actionHeader.search.placeholder')}
			/>
			<ViewManagerDropdown />
			{layoutControls}
		</header>
	)
}
