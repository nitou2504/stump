/**
 * A tab that stays open across a deploy still runs the build it was loaded with. The build's files
 * have hashed names, so when that tab asks for a page it has not loaded yet (the Explore page's code
 * is only fetched on the first visit) the files it names are gone from the server, the dynamic import
 * fails and nothing catches it: the app's top-level error screen appears, and it keeps appearing on
 * every click until the tab is reloaded.
 *
 * Vite reports that failure with a cancelable `vite:preloadError` event on the window. Taking the
 * current build is a reload, so reload once.
 *
 * It must never loop: a failure within {@link RELOAD_GUARD_MS} of the reload it caused means the
 * reload did not cure it (the files really are missing, for instance in the middle of a deploy), and
 * is left to surface as the error screen. Without a working `sessionStorage` the guard cannot be kept,
 * so nothing is reloaded. Offline, a reload would swap the app's own error screen for the browser's
 * offline page, so nothing is reloaded either.
 */
export const STALE_BUILD_RELOAD_KEY = 'stump:stale-build-reload'
export const RELOAD_GUARD_MS = 15_000

type StaleBuildWindow = Pick<
	Window,
	'addEventListener' | 'removeEventListener' | 'sessionStorage' | 'location' | 'navigator'
>

/** Starts listening on the window. Returns the function that stops it */
export function reloadOnStaleBuild(win: StaleBuildWindow = window): () => void {
	const onPreloadError = (event: Event) => {
		if (win.navigator.onLine === false) return

		const now = Date.now()
		let reloadedAt = 0
		try {
			reloadedAt = Number(win.sessionStorage.getItem(STALE_BUILD_RELOAD_KEY)) || 0
		} catch {
			return
		}
		// a reload a moment ago did not cure it (a clock that went backwards counts as long ago)
		if (reloadedAt > 0 && now >= reloadedAt && now - reloadedAt < RELOAD_GUARD_MS) return

		try {
			win.sessionStorage.setItem(STALE_BUILD_RELOAD_KEY, String(now))
		} catch {
			return
		}
		event.preventDefault()
		win.location.reload()
	}

	win.addEventListener('vite:preloadError', onPreloadError)
	return () => win.removeEventListener('vite:preloadError', onPreloadError)
}
