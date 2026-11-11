import { UserPermission } from '@stump/graphql'
import { Api } from '@stump/sdk'
import { QueryClient } from '@tanstack/react-query'

const flags = vi.hoisted(() => ({ approvals: false }))

// read when the routes are made, so a test can turn the page on
vi.mock('../server/bookApprovals/availability', () => ({
	get BOOK_APPROVALS_AVAILABLE() {
		return flags.approvals
	},
}))

import { createRouteGroups } from '../routes'

const items = () =>
	createRouteGroups(
		new QueryClient(),
		new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' }),
	).flatMap((group) => group.items)

describe('settings routes', () => {
	beforeEach(() => {
		flags.approvals = false
	})

	it('do not list the book approvals while the page is not built', () => {
		expect(items().map(({ label }) => label)).not.toContain('Book approvals')
	})

	it('list the book approvals for those who may manage libraries once it is', () => {
		flags.approvals = true

		const item = items().find(({ label }) => label === 'Book approvals')

		expect(item).toMatchObject({
			localeKey: 'server/bookApprovals',
			permissions: [UserPermission.ManageLibrary],
			to: '/settings/book-approvals',
		})
	})

	it('list Public access for the server owner only, in Management after Users', () => {
		const groups = createRouteGroups(
			new QueryClient(),
			new Api({ baseURL: 'http://localhost:10801', authMethod: 'session' }),
		)
		const management = groups.find(({ label }) => label === 'Management')!
		const labels = management.items.map(({ label }) => label)

		expect(management.items.find(({ label }) => label === 'Public access')).toMatchObject({
			localeKey: 'server/access',
			ownerOnly: true,
			to: '/settings/access',
		})
		// a permission is not what it is gated by
		expect(
			management.items.find(({ label }) => label === 'Public access')?.permissions,
		).toBeUndefined()
		expect(labels.indexOf('Public access')).toBe(labels.indexOf('Users') + 1)
	})
})
