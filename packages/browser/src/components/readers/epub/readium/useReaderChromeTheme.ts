import type { EpubReaderTheme } from '@stump/client'
import { useLayoutEffect } from 'react'

type ChromePalette = {
	background: string
	foreground: string
	/** Panels: cards, dialogs, popovers, inputs */
	surface: string
	muted: string
	mutedForeground: string
	accent: string
	border: string
	colorScheme: 'light' | 'dark'
}

/**
 * The app colours used around the book (header, footer, dialogs, menus) for each explicit
 * reader theme, so the whole reader takes the page colour. `auto` keeps the app theme.
 */
export const READER_CHROME: Record<Exclude<EpubReaderTheme, 'auto'>, ChromePalette> = {
	light: {
		accent: '#eeeeec',
		background: '#ffffff',
		border: '#e4e4e2',
		colorScheme: 'light',
		foreground: '#161719',
		muted: '#f3f3f2',
		mutedForeground: '#6b6f76',
		surface: '#ffffff',
	},
	sepia: {
		accent: '#e7dbbf',
		background: '#f4ecd8',
		border: '#ddd0b2',
		colorScheme: 'light',
		foreground: '#5b4636',
		muted: '#ece2c9',
		mutedForeground: '#8a7560',
		surface: '#f7f0df',
	},
	dark: {
		accent: '#242628',
		background: '#161719',
		border: '#292c30',
		colorScheme: 'dark',
		foreground: '#e8edf4',
		muted: '#1f2123',
		mutedForeground: '#898d94',
		surface: '#1b1c1d',
	},
	black: {
		accent: '#1a1a1a',
		background: '#000000',
		border: '#232323',
		colorScheme: 'dark',
		foreground: '#d4d4d4',
		muted: '#111111',
		mutedForeground: '#8a8a8a',
		surface: '#0b0b0b',
	},
}

/** The app theme variables (see `components/tailwind/themes.css`) set for a palette */
export function chromeVariables(palette: ChromePalette): Record<string, string> {
	const { background, foreground, surface, muted, mutedForeground, accent, border } = palette
	return {
		'--accent': accent,
		'--accent-foreground': foreground,
		'--background': background,
		'--background-gradient-from': background,
		'--background-gradient-to': background,
		'--border': border,
		'--card': surface,
		'--card-foreground': foreground,
		'--color-scrollbar': background,
		'--dialog': surface,
		'--dialog-foreground': foreground,
		'--foreground': foreground,
		'--input': surface,
		'--muted': muted,
		'--muted-foreground': mutedForeground,
		'--popover': surface,
		'--popover-foreground': foreground,
		'--secondary': muted,
		'--secondary-foreground': foreground,
		'--switch': muted,
	}
}

/**
 * While the reader is open with an explicit theme, paint the whole page with it: the
 * app's colour variables are overridden on `<html>` (where the app theme class lives), so
 * the header, footer and portalled dialogs and menus follow too. Restored on unmount or
 * when going back to `auto`.
 */
export function useReaderChromeTheme(theme: EpubReaderTheme | undefined) {
	useLayoutEffect(() => {
		if (!theme || theme === 'auto') return undefined
		const palette = READER_CHROME[theme]
		if (!palette) return undefined

		const root = document.documentElement
		const variables = chromeVariables(palette)
		const previous = new Map<string, string>()
		for (const [name, value] of Object.entries(variables)) {
			previous.set(name, root.style.getPropertyValue(name))
			root.style.setProperty(name, value)
		}
		const previousScheme = root.style.colorScheme
		root.style.colorScheme = palette.colorScheme

		return () => {
			for (const [name, value] of previous) {
				if (value) {
					root.style.setProperty(name, value)
				} else {
					root.style.removeProperty(name)
				}
			}
			root.style.colorScheme = previousScheme
		}
	}, [theme])
}
