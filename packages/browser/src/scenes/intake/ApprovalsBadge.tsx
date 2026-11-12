import { cn } from '@stump/components'
import { UserPermission } from '@stump/graphql'

import { useAppContext } from '@/context'
import { BOOK_APPROVALS_AVAILABLE } from '@/scenes/settings/server/bookApprovals/availability'

import { usePendingIntakeCount } from './hooks'

type Props = {
	/**
	 * `corner`: a round badge on the corner of a button (the sidebar's settings button).
	 * `inline`: a pill after a label (the settings navigation)
	 */
	variant?: 'corner' | 'inline'
	/**
	 * Show it to the server owner too. On the settings button the owner has one badge, the count
	 * of *Needs your attention*, which already holds the books that wait; beside *Book approvals*
	 * the count of books is what the owner wants to see
	 */
	forOwner?: boolean
}

/**
 * How many books wait for approval, for those who approve them (the server owner and library
 * managers). Nothing for anybody else, and nothing when none wait
 */
export default function ApprovalsBadge({ variant = 'corner', forOwner = false }: Props) {
	const { checkPermission, isServerOwner } = useAppContext()
	const count = usePendingIntakeCount(
		BOOK_APPROVALS_AVAILABLE && checkPermission(UserPermission.ManageLibrary),
	)

	if (!count || (isServerOwner && !forOwner)) return null

	return (
		<>
			<span
				aria-hidden
				className={cn(
					'h-4 min-w-4 px-1 font-semibold leading-4 rounded-full bg-primary text-center text-[10px] text-primary-foreground',
					variant === 'corner' ? '-right-1 -top-1 absolute' : 'ml-2 inline-block',
				)}
			>
				{count > 99 ? '99+' : count}
			</span>
			<span className="sr-only">{count} books waiting for approval</span>
		</>
	)
}
