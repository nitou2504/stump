import { ReactNode, useCallback, useEffect, useRef } from 'react'
import { useSearchParams } from 'react-router'
import { useMediaMatch } from 'rooks'

import { ListToolbar, ListToolbarControls, Search, URLFilterDrawer } from '@/components/filters'
import { readSearchParam, writeSearchParam } from '@/components/filters/useFilterScene'
import { useTranslate } from '@/hooks/useTranslate'

import { useSafeWorkingView, useSmartListContext } from '../../context'
import { useSmartListViewStore } from '../../store'
import SavedViewSelector from './SavedViewSelector'
import SmartListMoreMenu from './SmartListMoreMenu'
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

	// The search is mirrored in the URL (`?search=`), so going back to the list restores it
	const [searchParams, setSearchParams] = useSearchParams()
	const urlSearch = readSearchParam(searchParams)
	const restoredRef = useRef(false)
	useEffect(() => {
		// only once, when arriving with a search in the URL
		if (restoredRef.current) return
		restoredRef.current = true
		if (urlSearch && !search) {
			updateWorkingView({ search: urlSearch })
		}
	}, [urlSearch, search, updateWorkingView])

	const setFilter = useCallback(
		(value?: string) => {
			updateWorkingView({ search: value || undefined })
			setSearchParams(
				(prev) => {
					writeSearchParam(prev, value)
					return prev
				},
				{ replace: true },
			)
		},
		[updateWorkingView, setSearchParams],
	)

	// On a phone the toolbar is one row: the less frequent controls (saved views, columns) move
	// into a '⋯' menu, and the search folds into an icon that opens across the row
	const isNarrow = useMediaMatch('(max-width: 639px)')

	return (
		// Like the other lists: the search first, the controls at the end of the line
		<ListToolbar className="-mt-2 mb-2 gap-x-2 gap-y-2 px-4 sm:flex-wrap flex w-full items-center justify-end bg-background">
			<Search
				key={selectedView?.id ?? 'default'}
				initialValue={search ?? (urlSearch || undefined)}
				onChange={(value) => setFilter(value)}
				placeholder={t('userSmartListScene.itemsScene.actionHeader.search.placeholder')}
				className="mr-auto"
			/>
			<ListToolbarControls className="gap-x-1 sm:gap-x-2 flex shrink-0 items-center">
				{!isNarrow && hasSavedViews && <SavedViewSelector />}
				{canSaveView && <ViewManagerDropdown compact={isNarrow} />}
				<URLFilterDrawer entity="media" />
				{!isNarrow && showColumnControls && <TableColumnsBottomDrawer />}
				{layoutControls}
				{isNarrow && (
					<SmartListMoreMenu
						hasSavedViews={hasSavedViews}
						showColumnControls={showColumnControls}
					/>
				)}
			</ListToolbarControls>
		</ListToolbar>
	)
}
