import { Navigate, Route, Routes } from 'react-router'

import ReadingListScene from './ReadingListScene'
import ReadingListsScene from './ReadingListsScene'

export default function ReadingListRouter() {
	return (
		<Routes>
			<Route path="" element={<ReadingListsScene />} />
			<Route path=":id" element={<ReadingListScene />} />
			<Route path="*" element={<Navigate to="/404" replace />} />
		</Routes>
	)
}
