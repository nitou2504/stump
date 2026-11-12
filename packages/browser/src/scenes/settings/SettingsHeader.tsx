import { Breadcrumb } from '@stump/components'
import { useCallback, useMemo } from 'react'
import { useLocation } from 'react-router'
import { useMediaMatch } from 'rooks'

import { BreadcrumbPageSwitcher } from '@/components/navigation/BreadcrumbPageSwitcher'
import AccessWarningsBadge from '@/components/navigation/sidebar/AccessWarningsBadge'
import { usePageSwitcherGroups } from '@/components/navigation/usePageSwitcherGroups'
import { PageHeader } from '@/components/sharedLayout'
import { useRouterContext } from '@/context'
import { useTranslate } from '@/hooks/useTranslate'
import ApprovalsBadge from '@/scenes/intake/ApprovalsBadge'
import { useAppStore } from '@/stores'

import SettingsNavigation from './SettingsNavigation'
import { useSettingsRoutes } from './useSettingsRoutes'

const toLowerCase = (key: string) => key.toLowerCase()
const isNotDesktopRoute = (to: string) => !to.includes('desktop')

type Props = {
	renderNavigation?: boolean
}

/**
 * A header for the settings scene, which pulls a header and optional description from
 * the locale files based on the current route
 */
export default function SettingsHeader({ renderNavigation }: Props) {
	const { t } = useTranslate()
	const { basePath } = useRouterContext()

	const location = useLocation()

	const { groups } = useSettingsRoutes()

	const isMobile = useMediaMatch('(max-width: 768px)')
	const platform = useAppStore((store) => store.platform)

	const isActiveRoute = useCallback(
		(to: string, pathname: string) => pathname.startsWith(`${basePath}${to}`),
		[basePath],
	)
	const routeGroups = useMemo(
		() =>
			groups.map((group) => ({
				...group,
				items: group.items.map((item) => ({ ...item, to: `${basePath}${item.to}` })),
			})),
		[groups, basePath],
	)
	const pageGroups = usePageSwitcherGroups({
		localeBase: 'settingsScene',
		routeGroups,
		isActive: isActiveRoute,
		formatKey: toLowerCase,
		filter: platform === 'browser' ? isNotDesktopRoute : undefined,
	})
	// the phone's settings menu says, as the sidebar does, which pages have something to look at
	const switcherGroups = useMemo(
		() =>
			pageGroups.map((group) => ({
				...group,
				items: group.items.map((item) => ({
					...item,
					badge: item.to.endsWith('/settings/access') ? (
						<AccessWarningsBadge variant="inline" />
					) : item.to.endsWith('/settings/book-approvals') ? (
						<ApprovalsBadge variant="inline" forOwner />
					) : undefined,
				})),
			})),
		[pageGroups],
	)

	/**
	 * The active route based on the current location
	 */
	const activeRouteGroup = useMemo(
		() =>
			groups
				.flatMap((group) => group.items)
				.find((page) => location.pathname.startsWith(`${basePath}${page.to}`)),
		[location.pathname, groups, basePath],
	)

	/**
	 * The active sub-item (e.g. create user), if any
	 */
	const activeSubItem = useMemo(() => {
		const path = basePath ? location.pathname.replace(basePath, '') : location.pathname
		return activeRouteGroup?.subItems?.find((subItem) => subItem.matcher(path))
	}, [activeRouteGroup?.subItems, location.pathname, basePath])

	/**
	 * The active route's locale key, which is used to pull the title. A matched sub-item takes precedence
	 */
	const activeRouteKey = activeSubItem?.localeKey || activeRouteGroup?.localeKey || null
	const backlink = activeSubItem?.backlink ?? null

	const translatedHeader = t(`settingsScene.${activeRouteKey}.title`)

	const showNavigation = renderNavigation && !isMobile

	return (
		<>
			<PageHeader>
				<Breadcrumb>
					<Breadcrumb.List>
						<Breadcrumb.Item>
							<BreadcrumbPageSwitcher
								title={backlink ? t(`settingsScene.${backlink.localeKey}`) : translatedHeader}
								groups={switcherGroups}
								to={backlink ? `${basePath}${backlink.to}` : undefined}
							/>
						</Breadcrumb.Item>
						{backlink && (
							<>
								<Breadcrumb.Separator />
								<Breadcrumb.Item>
									<Breadcrumb.Page>{translatedHeader}</Breadcrumb.Page>
								</Breadcrumb.Item>
							</>
						)}
					</Breadcrumb.List>
				</Breadcrumb>
			</PageHeader>

			{showNavigation && (
				<div className="px-4 pt-4">
					<SettingsNavigation />
				</div>
			)}
		</>
	)
}
