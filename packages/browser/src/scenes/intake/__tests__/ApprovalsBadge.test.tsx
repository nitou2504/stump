import { UserPermission } from '@stump/graphql'
import { render, screen } from '@testing-library/react'

import { AppContext, IAppContext } from '@/context'

import ApprovalsBadge from '../ApprovalsBadge'

const pending = vi.hoisted(() => ({ asked: vi.fn(), count: 0 }))
vi.mock('../hooks', () => ({
	usePendingIntakeCount: (enabled: boolean) => {
		pending.asked(enabled)
		return enabled ? pending.count : 0
	},
}))
const flags = vi.hoisted(() => ({ available: true }))
vi.mock('@/scenes/settings/server/bookApprovals/availability', () => ({
	get BOOK_APPROVALS_AVAILABLE() {
		return flags.available
	},
}))

function renderBadge(permissions: UserPermission[]) {
	const app = {
		checkPermission: (permission: UserPermission) => permissions.includes(permission),
	} as unknown as IAppContext
	return render(
		<AppContext.Provider value={app}>
			<ApprovalsBadge />
		</AppContext.Provider>,
	)
}

describe('ApprovalsBadge', () => {
	beforeEach(() => {
		vi.clearAllMocks()
		pending.count = 0
		flags.available = true
	})

	it('shows how many books wait, for those who manage libraries', () => {
		pending.count = 3

		renderBadge([UserPermission.ManageLibrary])

		expect(pending.asked).toHaveBeenCalledWith(true)
		expect(screen.getByText('3')).toBeInTheDocument()
		expect(screen.getByText('3 books waiting for approval')).toBeInTheDocument()
	})

	it('does not even ask for the count of somebody who approves nothing', () => {
		pending.count = 3

		const { container } = renderBadge([UserPermission.AddBooks])

		expect(pending.asked).toHaveBeenCalledWith(false)
		expect(container).toBeEmptyDOMElement()
	})

	it('shows nothing when nothing waits', () => {
		const { container } = renderBadge([UserPermission.ManageLibrary])

		expect(container).toBeEmptyDOMElement()
	})

	it('caps a long count', () => {
		pending.count = 120

		renderBadge([UserPermission.ManageLibrary])

		expect(screen.getByText('99+')).toBeInTheDocument()
	})

	it('shows nothing while the page is turned off', () => {
		flags.available = false
		pending.count = 5

		const { container } = renderBadge([UserPermission.ManageLibrary])

		expect(pending.asked).toHaveBeenCalledWith(false)
		expect(container).toBeEmptyDOMElement()
	})
})
