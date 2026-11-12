import { cn } from '@stump/components'
import { AccessRole } from '@stump/graphql'
import { Suspense, useMemo, useRef } from 'react'
import { Helmet } from 'react-helmet'
import { Outlet, useLocation, useParams } from 'react-router'
import { useMediaMatch } from 'rooks'

import { SceneContainer } from '@/components/container'
import { GenericSettingsHeader } from '@/components/settings'
import { usePreferences } from '@/hooks/usePreferences'
import { useTranslate } from '@/hooks/useTranslate'

import { SmartListContext } from './context'
import { useSmartListById, useSmartListMeta, useUpdateSmartList } from './graphql'
import { useSmartListGrouping } from './layout'
import { createRouteGroups } from './settings/routes'
import SmartListSettingsSideBar from './settings/SmartListSettingsSideBar'
import { createSmartListViewStore, SmartListViewStoreContext } from './store'
import UserSmartListHeader from './UserSmartListHeader'

const LOCALE_BASE_KEY = 'userSmartListScene.layout'
const withLocaleKey = (key: string) => `${LOCALE_BASE_KEY}.${key}`

export default function UserSmartListLayout() {
	const location = useLocation()

	const { id } = useParams<{ id: string }>()
	const { t } = useTranslate()

	const {
		preferences: { enableHideScrollbar },
	} = usePreferences()

	const isSettings = useMemo(() => location.pathname.includes('settings'), [location.pathname])
	const isMobile = useMediaMatch('(max-width: 768px)')

	const displaySideBar = !isMobile && isSettings

	if (!id) {
		throw new Error(t(withLocaleKey('missingIdError')))
	}

	const { list, isLoading: isLoadingList } = useSmartListById({ id })
	const { meta, isLoading: isLoadingMeta } = useSmartListMeta({ id })

	const { update: updateSmartList } = useUpdateSmartList({
		id,
		list,
	})

	const { grouping, setGrouping, fieldGrouping, setFieldGrouping } = useSmartListGrouping(
		id,
		list?.defaultGrouping,
	)

	/**
	 * The access role of the current user for this smart list. The server says who may change a
	 * list (`canEdit`: the user who made it); everyone else reads it, the server owner too (who
	 * can only take a public list of someone else back to private, or delete it: the header has
	 * the menu for that). This is used to determine what actions the user can take on the list
	 *
	 * TODO: Support actual roles from the backend, i.e. Writer, CoCreator, Creator, Reader
	 */
	const viewerRole = useMemo<AccessRole>(
		() => (list?.canEdit ? AccessRole.CoCreator : AccessRole.Reader),
		[list?.canEdit],
	)

	// Create scoped store with default grouping from the list
	// eslint-disable-next-line react-hooks/refs
	const store = useRef(
		createSmartListViewStore({
			defaultGrouping: list?.defaultGrouping,
		}),
	).current

	const renderHeader = () =>
		isSettings ? (
			<GenericSettingsHeader
				localeBase="smartListSettingsScene"
				routeGroups={createRouteGroups(list?.id ?? '', viewerRole)}
				entityName={list?.name}
			/>
		) : (
			<UserSmartListHeader />
		)

	if (isLoadingList || isLoadingMeta) {
		return null
	}

	// TODO: redirect for these?
	if (!list || !meta) {
		throw new Error(t(withLocaleKey('smartListNotFound')))
	}

	return (
		<SmartListViewStoreContext.Provider value={store}>
			<SmartListContext.Provider
				value={{
					list,
					meta,
					patchSmartList: updateSmartList,
					viewerRole,
					grouping,
					setGrouping,
					fieldGrouping,
					setFieldGrouping,
				}}
			>
				<div className="relative flex flex-1">
					<Helmet>
						<title>Stump | {list.name}</title>
					</Helmet>
					{displaySideBar && <SmartListSettingsSideBar />}

					<div className="min-w-0 flex flex-1 flex-col">
						{renderHeader()}

						<SceneContainer
							className={cn('gap-4 md:pb-0 relative flex flex-1 flex-col', {
								'md:hide-scrollbar': !!enableHideScrollbar,
								'p-0': !isSettings,
							})}
						>
							<Suspense fallback={null}>
								<Outlet />
							</Suspense>
						</SceneContainer>
					</div>
				</div>
			</SmartListContext.Provider>
		</SmartListViewStoreContext.Provider>
	)
}
