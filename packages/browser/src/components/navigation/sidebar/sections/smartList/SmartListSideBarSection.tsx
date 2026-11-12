import { useSDK, useSuspenseGraphQL } from '@stump/client'
import { Accordion } from '@stump/components'
import { FilterableArrangementEntityLink, graphql } from '@stump/graphql'
import { ListFilter } from 'lucide-react'
import { useLocation } from 'react-router'

import { useTranslate } from '@/hooks/useTranslate'
import { usePaths } from '@/paths'
import { usePrefetchSmartList } from '@/scenes/smartList/graphql'

import { EntityOptionProps } from '../../../types'
import { CollapsedSidebarSection, useIsSidebarIconMode } from '../../CollapsedSidebarItems'
import SideBarButtonLink from '../../SideBarButtonLink'
import { SectionEmptyMessage } from '../SectionEmptyMessage'

// `input: {}` is the user's own lists and the public ones of everyone else (without an input only
// the user's own are listed), by name
const query = graphql(`
	query SmartListSideBarSection {
		smartLists(input: {}) {
			id
			name
		}
	}
`)

type Props = EntityOptionProps

/**
 * Past this many lists the section starts folded, with their number on it (a reader with dozens
 * of lists would otherwise have the sidebar to themselves): *See all* is then always offered, and
 * the section is open when the reader is on one of the lists
 */
const FOLD_AFTER = 6

export default function SmartListSideBarSection({
	links = [FilterableArrangementEntityLink.Create],
}: Props) {
	const location = useLocation()
	const paths = usePaths()

	const { t, translate } = useTranslate()
	const { sdk } = useSDK()
	const {
		data: { smartLists: lists },
	} = useSuspenseGraphQL(query, sdk.cacheKey('smartLists'))

	const { prefetch } = usePrefetchSmartList()

	const isCurrentList = (id: string) => location.pathname.startsWith(paths.smartList(id))
	const isLong = (lists?.length ?? 0) > FOLD_AFTER
	const isOnAList = (lists ?? []).some(({ id }) => isCurrentList(id))

	const isIconMode = useIsSidebarIconMode()
	if (isIconMode) {
		return (
			<CollapsedSidebarSection
				label={translate('shared.common.smartLists')}
				icon={ListFilter}
				emptyMessage={t('sidebar.buttons.noSmartlists')}
				items={(lists ?? []).map(({ id, name }) => ({
					key: id,
					label: name,
					to: paths.smartList(id),
					isActive: isCurrentList(id),
					onMouseEnter: () => prefetch({ id }),
				}))}
				actions={[
					...(links.includes(FilterableArrangementEntityLink.ShowAll)
						? [{ key: 'all', label: t('sidebar.buttons.seeAll'), to: paths.smartLists() }]
						: []),
					...(links.includes(FilterableArrangementEntityLink.Create)
						? [
								{
									key: 'create',
									label: t('sidebar.buttons.createSmartlist'),
									to: paths.smartListCreate(),
								},
							]
						: []),
				]}
			/>
		)
	}

	const renderLists = () => {
		if (!lists || !lists.length) {
			return <SectionEmptyMessage message={t('sidebar.buttons.noSmartlists')} />
		}

		return lists.map(({ id, name }) => {
			return (
				<SideBarButtonLink
					key={id}
					to={paths.smartList(id)}
					isActive={isCurrentList(id)}
					className="pl-2 pr-0"
					onMouseEnter={() => prefetch({ id })}
				>
					{name}
				</SideBarButtonLink>
			)
		})
	}

	return (
		<Accordion
			type="single"
			collapsible
			className="py-1 w-full"
			defaultValue={!isLong || isOnAList ? 'smartlists' : undefined}
		>
			<Accordion.Item value="smartlists" className="border-none">
				<Accordion.Trigger
					noUnderline
					asLabel
					className="px-1 py-1 text-sm font-medium tracking-wide text-muted-foreground"
				>
					<span>
						{translate('shared.common.smartLists')}
						{isLong && <span className="ml-1.5 font-normal opacity-60">{lists?.length}</span>}
					</span>
				</Accordion.Trigger>
				<Accordion.Content containerClassName="flex flex-col space-y-1.5">
					{(isLong || links.includes(FilterableArrangementEntityLink.ShowAll)) && (
						<SideBarButtonLink
							to={paths.smartLists()}
							isActive={location.pathname === paths.smartLists()}
							variant="action"
						>
							{t('sidebar.buttons.seeAll')}
						</SideBarButtonLink>
					)}
					<div className="space-y-1">{renderLists()}</div>
					{links.includes(FilterableArrangementEntityLink.Create) && (
						<SideBarButtonLink
							to={paths.smartListCreate()}
							isActive={location.pathname === paths.smartListCreate()}
							variant="action"
						>
							{t('sidebar.buttons.createSmartlist')}
						</SideBarButtonLink>
					)}
				</Accordion.Content>
			</Accordion.Item>
		</Accordion>
	)
}
