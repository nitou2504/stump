export const isApple =
	typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.userAgent)

/** How the title bar's search shortcut reads on this device */
export const GLOBAL_SEARCH_SHORTCUT = isApple ? '⌘K' : 'Ctrl K'
