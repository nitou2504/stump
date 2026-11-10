import { UserPermission } from '@stump/graphql'
import { lazy, Suspense } from 'react'
import { Navigate, Route, Routes } from 'react-router'

import UserPermissionRouteGuard from '@/components/UserPermissionRouteGuard.tsx'

const BookSearchScene = lazy(() => import('../bookSearch/BookSearchScene.tsx'))
const BookOverviewScene = lazy(() => import('./BookOverviewScene.tsx'))
const BookReaderScene = lazy(() => import('./reader/BookReaderScene.tsx'))
const EpubReaderScene = lazy(() => import('./reader/EpubReaderScene.tsx'))
const PDFReaderScene = lazy(() => import('./reader/PDFReaderScene.tsx'))
const BookSettingsScene = lazy(() => import('./settings/BookSettingsScene.tsx'))

export default function BookRouter() {
	return (
		<Suspense>
			<Routes>
				<Route path="/" element={<BookSearchScene />} />
				<Route path=":id" element={<BookOverviewScene />} />
				<Route
					path=":id/manage"
					element={
						<UserPermissionRouteGuard
							permissions={[UserPermission.EditMetadata, UserPermission.MetadataFetchRecordManage]}
							enforceAll={false}
						/>
					}
				>
					<Route path="" element={<BookSettingsScene />} />
				</Route>
				<Route path=":id/epub-reader" element={<EpubReaderScene />} />
				<Route path=":id/pdf-reader" element={<PDFReaderScene />} />
				<Route path=":id/reader" element={<BookReaderScene />} />
				<Route path="*" element={<Navigate to="/404" />} />
			</Routes>
		</Suspense>
	)
}
