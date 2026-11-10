import { Navigate } from 'react-router'

import { usePaths } from '@/paths'

import { BOOK_APPROVALS_AVAILABLE } from './availability'

/**
 * The "Book approvals" settings page, for those who approve the books other people add (the
 * server owner and library managers: the route is only theirs). It is a stub until package D2
 * fills it in: while the page isn't available the route goes home
 */
export default function BookApprovalsScene() {
	const paths = usePaths()

	if (!BOOK_APPROVALS_AVAILABLE) {
		return <Navigate to={paths.home()} replace />
	}

	return null
}
