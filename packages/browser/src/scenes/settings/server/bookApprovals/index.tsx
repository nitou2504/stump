import { Navigate } from 'react-router'

import { usePaths } from '@/paths'

import { BOOK_APPROVALS_AVAILABLE } from './availability'
import BookApprovalsScene from './BookApprovalsScene'

/**
 * The "Book approvals" settings page, for those who approve the books other people add (the
 * server owner and library managers: the route is only theirs). While the page isn't available
 * the route goes home
 */
export default function BookApprovalsRoute() {
	const paths = usePaths()

	if (!BOOK_APPROVALS_AVAILABLE) {
		return <Navigate to={paths.home()} replace />
	}

	return <BookApprovalsScene />
}
