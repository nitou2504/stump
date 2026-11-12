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

describe('useRouteGroups, a page for people who have devices of their own', () => {
	const personal: RouteGroup[] = [
		{
			defaultRoute: '/settings/account',
			items: [
				{ icon: Server, label: 'Account', localeKey: 'a', to: '/a' },
				{ icon: Server, label: 'My devices', localeKey: 'b', nonOwnerOnly: true, to: '/b' },
				{
					icon: Server,
					label: 'Reading apps',
					localeKey: 'c',
					nonOwnerOnly: true,
					permissions: [UserPermission.DownloadFile],
					to: '/c',
				},
			],
			label: 'Personal',
		},
		{
			defaultRoute: '/x',
			items: [{ icon: Server, label: 'Only friends', localeKey: 'd', nonOwnerOnly: true, to: '/d' }],
			label: 'Friends',
		},
	]

	const labels = (permissions: UserPermission[], isServerOwner: boolean) => {
		vi.mocked(useAppContext).mockReturnValue({
			checkPermission: (permission: UserPermission) => permissions.includes(permission),
			isServerOwner,
		} as unknown as IAppContext)
		const { result } = renderHook(() => useRouteGroups({ routeGroups: personal }))
		return result.current.groups.map((group) => group.items.map(({ label }) => label))
	}

	it('offers it to anybody but the server owner', () => {
		expect(labels([UserPermission.DownloadFile], false)).toEqual([
			['Account', 'My devices', 'Reading apps'],
			['Only friends'],
		])
	})

	it('still asks for the permission, and does not offer it to the owner whatever they hold', () => {
		expect(labels([], false)).toEqual([['Account', 'My devices'], ['Only friends']])
		expect(labels([UserPermission.DownloadFile], true)).toEqual([['Account']])
	})
})
