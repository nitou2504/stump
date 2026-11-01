import { lazy } from 'react'
import { Route, Routes } from 'react-router'

import { AppLayout } from './AppLayout.tsx'
import { RouterProvider } from './context/RouterContext.tsx'
import { useAppStore } from './stores'

// Every section is its own chunk, so a first visit downloads the app shell plus the page it
// opens, not every screen of the app
const HomeScene = lazy(() => import('./scenes/home'))
const LibraryRouter = lazy(() => import('./scenes/library/LibraryRouter.tsx'))
const SeriesRouter = lazy(() => import('./scenes/series/SeriesRouter.tsx'))
const BookRouter = lazy(() => import('./scenes/book/BookRouter.tsx'))
const BookClubRouter = lazy(() => import('./scenes/bookClub/BookClubRouter.tsx'))
const SmartListRouter = lazy(() => import('./scenes/smartList/SmartListRouter.tsx'))
const SettingsRouter = lazy(() => import('./scenes/settings/SettingsRouter.tsx'))
const FourOhFour = lazy(() => import('./scenes/error/FourOhFour.tsx'))
const ServerConnectionErrorScene = lazy(
	() => import('./scenes/error/ServerConnectionErrorScene.tsx'),
)
const LoginOrClaimScene = lazy(() => import('./scenes/auth'))
const BrowseIndexScene = lazy(() => import('./scenes/browse'))

type AppRouterProps = {
	basePath?: string
}

export function AppRouter({ basePath }: AppRouterProps = {}) {
	const baseUrl = useAppStore((state) => state.baseUrl)

	if (!baseUrl) {
		throw new Error('Base URL is not set')
	}

	// TODO(ux):
	// - explore needs to either go away or transform into some kind of suggestion system
	//    - e.g., explore most commonly-read genres, least-common, recently added, old-but-unread,
	//      popular for other users on server (who opt in to sharing), etc
	//    - basically looking at https://dribbble.com/shots/23501959-Goodreads-Redesign-Interface
	//    - i imagine this could eventually be customizable
	// - dedicated books/series/libraries pages, the former-two being basically what exists today for explore
	// - root-level reading timeline
	// - root-level stats? could also have a home section for stats, with a few aggregates?
	// - root-level authors, once i revisit proper author tracking via db table(s)
	return (
		<RouterProvider basePath={basePath}>
			<Routes>
				<Route path="/" element={<AppLayout />}>
					<Route path="" element={<HomeScene />} />
					<Route path="libraries/*" element={<LibraryRouter />} />
					<Route path="series/*" element={<SeriesRouter />} />
					<Route path="books/*" element={<BookRouter />} />
					<Route path="browse/:index" element={<BrowseIndexScene />} />
					<Route path="clubs/*" element={<BookClubRouter />} />
					<Route path="/smart-lists/*" element={<SmartListRouter />} />
					<Route path="settings/*" element={<SettingsRouter />} />
				</Route>

				<Route path="/auth" element={<LoginOrClaimScene />} />
				<Route path="/server-connection-error" element={<ServerConnectionErrorScene />} />
				<Route path="*" element={<FourOhFour />} />
			</Routes>
		</RouterProvider>
	)
}
