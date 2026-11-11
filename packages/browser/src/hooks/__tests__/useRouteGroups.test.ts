import { UserPermission } from '@stump/graphql'
import { renderHook } from '@testing-library/react'
import { Server } from 'lucide-react'

import { type IAppContext, useAppContext } from '@/context'

import { type RouteGroup, useRouteGroups } from '../useRouteGroups'

vi.mock('@/context', () => ({ useAppContext: vi.fn() }))

const groups: RouteGroup[] = [
	{
		defaultRoute: '/settings/server',
		items: [
			{ icon: Server, label: 'Open', localeKey: 'a', to: '/a' },
			{
				icon: Server,
				label: 'Needs a permission',
				localeKey: 'b',
				permissions: [UserPermission.ManageServer],
				to: '/b',
			},
			{ icon: Server, label: 'Owner only', localeKey: 'c', ownerOnly: true, to: '/c' },
			{
				icon: Server,
				label: 'Owner only and a permission',
				localeKey: 'd',
				ownerOnly: true,
				permissions: [UserPermission.ManageServer],
				to: '/d',
			},
		],
	},
	{
		defaultRoute: '/x',
		items: [
			{ icon: Server, label: 'Only for the owner', localeKey: 'e', ownerOnly: true, to: '/e' },
		],
		label: 'Alone',
	},
]

const labelsFor = (permissions: UserPermission[], isServerOwner: boolean) => {
	vi.mocked(useAppContext).mockReturnValue({
		checkPermission: (permission: UserPermission) => permissions.includes(permission),
		isServerOwner,
	} as unknown as IAppContext)
	const { result } = renderHook(() => useRouteGroups({ routeGroups: groups }))
	return result.current.groups.map((group) => group.items.map(({ label }) => label))
}

describe('useRouteGroups', () => {
	it('offers an owner-only item to the server owner', () => {
		expect(labelsFor([UserPermission.ManageServer], true)).toEqual([
			['Open', 'Needs a permission', 'Owner only', 'Owner only and a permission'],
			['Only for the owner'],
		])
	})

	it('does not offer it to anyone else, whatever they hold, and drops a group left with nothing', () => {
		expect(
			labelsFor(
				[UserPermission.ManageServer, UserPermission.ManageUsers, UserPermission.ReadUsers],
				false,
			),
		).toEqual([['Open', 'Needs a permission']])
	})

	it('still asks the owner-only item for its permission too', () => {
		expect(labelsFor([], true)).toEqual([['Open', 'Owner only'], ['Only for the owner']])
	})
})
