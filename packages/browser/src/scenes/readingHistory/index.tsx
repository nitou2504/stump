import { Navigate } from 'react-router'

import { usePaths } from '@/paths'

import { MY_READING_AVAILABLE } from './availability'

/**
 * The "My reading" page (`/reading`, and `/reading/:userId` for a friend's, if they share it).
 * It is a stub until package A2 fills it in: while the page isn't available the route goes home,
 * so a link to it never shows an unfinished screen
 */
export default function ReadingHistoryScene() {
	const paths = usePaths()

	if (!MY_READING_AVAILABLE) {
		return <Navigate to={paths.home()} replace />
	}

	return null
}
