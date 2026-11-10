import { UserPermission } from '@stump/graphql'

import { useAppContext } from '@/context'
import { BOOK_APPROVALS_AVAILABLE } from '@/scenes/settings/server/bookApprovals/availability'

import { usePendingIntakeCount } from './hooks'

/**
 * How many books wait for approval, on the settings button of the sidebar, for those who approve
 * them (the server owner and library managers). Nothing for anybody else, and nothing when none
 * wait
 */
export default function ApprovalsBadge() {
	const { checkPermission } = useAppContext()
	const count = usePendingIntakeCount(
		BOOK_APPROVALS_AVAILABLE && checkPermission(UserPermission.ManageLibrary),
	)

	if (!count) return null

	return (
		<>
			<span
				aria-hidden
				className="-right-1 -top-1 h-4 min-w-4 px-1 font-semibold leading-4 absolute rounded-full bg-primary text-center text-[10px] text-primary-foreground"
			>
				{count > 99 ? '99+' : count}
			</span>
			<span className="sr-only">{count} books waiting for approval</span>
		</>
	)
}
