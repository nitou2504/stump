import { useSyncExternalStore } from 'react'

const WIDE = '(min-width: 768px)'

/** Whether the window is as wide as a tablet; wide when the browser can't say (a test, an old one) */
export function useIsWide() {
	return useSyncExternalStore(
		(notify) => {
			if (typeof window === 'undefined' || !window.matchMedia) return () => undefined
			const query = window.matchMedia(WIDE)
			query.addEventListener('change', notify)
			return () => query.removeEventListener('change', notify)
		},
		() =>
			typeof window !== 'undefined' && window.matchMedia ? window.matchMedia(WIDE).matches : true,
		() => true,
	)
}
