import type { EpubReaderTheme } from '@stump/client'

type KeyLike = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>

/**
 * Whether a key press opens the reader's own search: Ctrl/Cmd+F (instead of the browser's
 * find bar, which can't see inside the book), `/` (Firefox's quick find otherwise opens) and
 * Ctrl/Cmd+K (the old shortcut, kept as an alias). The caller checks the focus isn't in an
 * input first (`isEditableTarget`) and prevents the default.
 */
export function isReaderSearchShortcut(event: KeyLike): boolean {
	// some synthetic key events (autofill) have no key
	if (event.altKey || typeof event.key !== 'string') return false
	const key = event.key.toLowerCase()
	if (event.ctrlKey || event.metaKey) return key === 'f' || key === 'k'
	return event.key === '/'
}

/** The keys the readers show for their search, in the shortcuts list */
export const SEARCH_SHORTCUT_KEYS = ['Ctrl', 'F']

export const DAY_NIGHT_SHORTCUT = 'd'
export const COPY_SHORTCUT = 'c'

const DARK_THEMES: ReadonlyArray<EpubReaderTheme> = ['dark', 'black']

export function isDarkReaderTheme(theme: EpubReaderTheme, isDarkVariant: boolean): boolean {
	return theme === 'auto' ? isDarkVariant : DARK_THEMES.includes(theme)
}

/**
 * The quick day/night switch: from a dark theme to light, from a light one (sepia included)
 * to dark. `auto` switches to the opposite of what is shown now.
 */
export function nextDayNightTheme(theme: EpubReaderTheme, isDarkVariant: boolean): EpubReaderTheme {
	return isDarkReaderTheme(theme, isDarkVariant) ? 'light' : 'dark'
}

/**
 * Puts text on the clipboard. The Clipboard API needs a secure context (https or localhost);
 * on a plain-http address the old `execCommand('copy')` path is used instead.
 */
export async function copyText(text: string): Promise<boolean> {
	if (!text) return false
	try {
		if (typeof navigator !== 'undefined' && navigator.clipboard && window.isSecureContext) {
			await navigator.clipboard.writeText(text)
			return true
		}
	} catch {
		// fall through to the fallback
	}
	try {
		const textarea = document.createElement('textarea')
		textarea.value = text
		textarea.setAttribute('readonly', '')
		textarea.style.position = 'fixed'
		textarea.style.top = '-1000px'
		textarea.style.opacity = '0'
		document.body.appendChild(textarea)
		textarea.select()
		const copied = document.execCommand('copy')
		textarea.remove()
		return copied
	} catch {
		return false
	}
}
