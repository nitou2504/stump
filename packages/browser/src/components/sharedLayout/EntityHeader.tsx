import {
	Breadcrumbs,
	Button,
	DropdownMenu,
	DropdownMenuProps,
	Link,
	MiniStatCard,
	Tabs,
} from '@stump/components'
import { EllipsisVertical, Info, Settings } from 'lucide-react'
import { useLocation } from 'react-router'
import { useMediaMatch } from 'rooks'

import { BreadcrumbPageSwitcher } from '../navigation/BreadcrumbPageSwitcher'
import { HeaderIconButton } from './HeaderIconButton'
import { PageHeader } from './PageHeader'

type Stat = React.ComponentProps<typeof MiniStatCard> & { key: string }

type Tab = {
	isActive: boolean
	label: string
	onHover?: () => void
	to: string
}

export type EntityCrumb = {
	label: string
	to: string
}

type Props = {
	/** the name of the entity, will render as the current breadcrumb page */
	name: string
	/**
	 * the parent routes the entity, only the immediate parent
	 * is shown inline and anything else will collapse into a menu
	 */
	parents?: EntityCrumb[]
	tabs: Tab[]
	actions?: DropdownMenuProps['groups']
	stats?: Stat[]
	settingsLink?: string
	onInfoClick?: () => void
	/** extra buttons shown first on the right, e.g. an upload button */
	endActions?: React.ReactNode
}

export function EntityHeader({
	name,
	parents = [],
	tabs,
	actions,
	stats,
	settingsLink,
	onInfoClick,
	endActions,
}: Props) {
	const location = useLocation()
	const isMobile = useMediaMatch('(max-width: 768px)')

	const isSettingsActive = settingsLink ? location.pathname.includes(settingsLink) : false
	const activeTab =
		(isSettingsActive
			? // a lil hacky but basically forcing a non-existing tab to be active so none are
				settingsLink
			: tabs.find((tab) => tab.isActive)?.to) ?? ''
	const activeTabLabel = tabs.find((tab) => tab.isActive)?.label

	// on small screens the tabs often don't fit well so i have them collapse into a trailing switcher
	// crumb. it isn't _perfect_ but is okay enough. frankly as long as mobile is functional enough
	// im happy, i use the mobile app and so RARELY visit web on my phone
	const showTabSwitcher = isMobile && tabs.length > 1 && !!activeTabLabel && !isSettingsActive
	// A single tab is the page itself (e.g. a smart list's 'Items', a book's 'Overview'): it's
	// only worth showing on the settings page, as the way back to it
	const showTabs = !isMobile && (tabs.length > 1 || (tabs.length === 1 && isSettingsActive))

	return (
		<PageHeader>
			<PageHeader.Start>
				<Breadcrumbs
					maxVisible={2}
					segments={[
						...parents,
						{ label: name },
						...(showTabSwitcher && activeTabLabel
							? [
									{
										label: activeTabLabel,
										noShrink: true,
										render: (
											<BreadcrumbPageSwitcher
												title={activeTabLabel}
												groups={[
													{
														items: tabs.map(({ to, label, isActive }) => ({ to, label, isActive })),
													},
												]}
											/>
										),
									},
								]
							: []),
					]}
				/>

				{actions && (
					<DropdownMenu
						align="start"
						contentWrapperClassName="w-48"
						trigger={
							<Button
								variant="ghost"
								size="icon"
								className="size-7 pointer-coarse:size-10 shrink-0"
								aria-label="Options"
							>
								<EllipsisVertical className="h-4 w-4" />
							</Button>
						}
						groups={actions}
					/>
				)}

				{stats && (
					<div className="lg:flex gap-2 hidden shrink-0 items-center">
						{stats.map(({ key, ...stat }) => (
							<MiniStatCard key={key} {...stat} />
						))}
					</div>
				)}
			</PageHeader.Start>

			<PageHeader.End>
				{endActions}
				{onInfoClick && <HeaderIconButton label="Info" icon={Info} onClick={onInfoClick} />}

				{showTabs && (
					<Tabs value={activeTab} size="sm">
						<Tabs.List>
							{tabs.map((tab) => (
								<Tabs.Trigger key={tab.to} value={tab.to} asChild>
									<Link to={tab.to} underline={false} onMouseEnter={tab.onHover}>
										{tab.label}
									</Link>
								</Tabs.Trigger>
							))}
						</Tabs.List>
					</Tabs>
				)}

				{settingsLink && (
					<HeaderIconButton
						label="Settings"
						icon={Settings}
						to={settingsLink}
						isActive={isSettingsActive}
					/>
				)}
			</PageHeader.End>
		</PageHeader>
	)
}
