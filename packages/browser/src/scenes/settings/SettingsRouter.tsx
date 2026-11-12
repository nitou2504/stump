import { UserPermission } from '@stump/graphql'
import { lazy } from 'react'
import { Navigate, Route, Routes } from 'react-router'

import { useAppContext } from '@/context'
import { usePaths } from '@/paths'
import { useAppStore } from '@/stores/app.ts'

import EmailSettingsRouter from './server/email/EmailSettingsRouter.tsx'
import UsersRouter from './server/users/UsersRouter.tsx'
import SettingsLayout from './SettingsLayout.tsx'

const GeneralSettingsScene = lazy(() => import('./app/general/GeneralSettingsScene.tsx'))
const AppearanceSettingsScene = lazy(() => import('./app/preferences/AppearanceSettingsScene.jsx'))
const ReaderDefaultSettingsScene = lazy(() => import('./app/reader/ReaderDefaultSettingsScene.tsx'))
const DesktopSettingsScene = lazy(() => import('./app/desktop'))
const APIKeySettingsScene = lazy(() => import('./app/apiKeys'))
const MyDevicesScene = lazy(() => import('./app/devices/MyDevicesScene.tsx'))
const ReadingAppsScene = lazy(() => import('./app/devices/ReadingAppsScene.tsx'))

const GeneralServerSettingsScene = lazy(
	() => import('./server/general/GeneralServerSettingsScene.tsx'),
)
const MetadataIntegrationsScene = lazy(() => import('./server/metadataIntegrations/index.ts'))
const ServerLogsScene = lazy(() => import('./server/logs/ServerLogsScene.tsx'))
const JobSettingsScene = lazy(() => import('./server/jobs/JobSettingsScene.tsx'))
const TagSettingsScene = lazy(() => import('./server/tags'))
const RecentlyAddedScene = lazy(() => import('./server/recentlyAdded'))
const DeletedBooksScene = lazy(() => import('./server/deletedBooks'))
const BookApprovalsScene = lazy(() => import('./server/bookApprovals'))
const PublicAccessScene = lazy(() => import('./server/access'))

/**
 * The main router for the settings scene(s). Mostly just a collection of nested routers
 */
export default function SettingsRouter() {
	const { checkPermission, isServerOwner } = useAppContext()
	const paths = usePaths()

	const isDesktop = useAppStore((store) => store.platform !== 'browser')

	const apiKeys = checkPermission(UserPermission.AccessApiKeys)
	const canDownload = checkPermission(UserPermission.DownloadFile)
	const canManageServer = checkPermission(UserPermission.ManageServer)
	const canReadUsers = checkPermission(UserPermission.ReadUsers)
	const canManageEmail = checkPermission(UserPermission.EmailerManage)
	const canReadProviders = checkPermission(UserPermission.MetadataProviderRead)
	const canManageLibrary = checkPermission(UserPermission.ManageLibrary)

	return (
		<Routes>
			<Route element={<SettingsLayout />}>
				<Route path="" element={<Navigate to={paths.settings('account')} replace />} />

				<Route path="account" element={<GeneralSettingsScene />} />
				<Route path="preferences" element={<AppearanceSettingsScene />} />
				<Route path="reader" element={<ReaderDefaultSettingsScene />} />
				{apiKeys && <Route path="api-keys" element={<APIKeySettingsScene />} />}
				{/* a person's own devices and reading apps: the owner has none of their own */}
				{!isServerOwner && <Route path="devices" element={<MyDevicesScene />} />}
				{!isServerOwner && canDownload && (
					<Route path="reading-apps" element={<ReadingAppsScene />} />
				)}
				{isDesktop && <Route path="desktop" element={<DesktopSettingsScene />} />}

				{canManageServer && <Route path="server" element={<GeneralServerSettingsScene />} />}
				{canManageServer && <Route path="logs" element={<ServerLogsScene />} />}
				{canManageServer && <Route path="jobs" element={<JobSettingsScene />} />}
				{canReadUsers && <Route path="users/*" element={<UsersRouter />} />}
				{/* how people come in from the internet: the owner's alone */}
				{isServerOwner && <Route path="access" element={<PublicAccessScene />} />}
				{canManageEmail && <Route path="email/*" element={<EmailSettingsRouter />} />}
				{canReadProviders && (
					<Route path="metadata-integrations" element={<MetadataIntegrationsScene />} />
				)}
				{canManageLibrary && <Route path="tags" element={<TagSettingsScene />} />}
				{canReadUsers && <Route path="recently-added" element={<RecentlyAddedScene />} />}
				{canManageLibrary && <Route path="deleted-books" element={<DeletedBooksScene />} />}
				{/* the books other people add are approved by the owner and library managers */}
				{canManageLibrary && <Route path="book-approvals" element={<BookApprovalsScene />} />}

				<Route path="*" element={<Navigate to={paths.settings('account')} replace />} />
			</Route>
		</Routes>
	)
}
