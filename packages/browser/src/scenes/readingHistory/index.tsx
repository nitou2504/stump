import { Navigate, useParams } from 'react-router'

import { usePaths } from '@/paths'

import { MY_READING_AVAILABLE } from './availability'
import ReadingHistoryPage from './ReadingHistoryPage'

/**
 * The "My reading" page (`/reading`, and `/reading/:userId` for a friend's, if they share it).
 * While the page isn't available the route goes home, so a link to it never shows an unfinished
 * screen
 */
export default function ReadingHistoryScene() {
	const paths = usePaths()
	const { userId } = useParams()

	if (!MY_READING_AVAILABLE) {
		return <Navigate to={paths.home()} replace />
	}

	// a page of its own for each person, so one's search and page never carry over to another's
	return <ReadingHistoryPage key={userId ?? 'me'} userId={userId} />
}
