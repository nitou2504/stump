import type { EpubReaderTheme } from '@stump/client'
import type { CSSProperties } from 'react'

import { READER_THEME_COLORS } from '../epub/readium/preferences'

export type PdfTheme = Exclude<EpubReaderTheme, 'auto'>

/** `auto` follows the app's light/dark theme, like the EPUB reader */
export function resolvePdfTheme(
	theme: EpubReaderTheme | undefined,
	isDarkVariant: boolean,
): PdfTheme {
	if (!theme || theme === 'auto') return isDarkVariant ? 'dark' : 'light'
	return theme
}

export function isDarkTheme(theme: PdfTheme): boolean {
	return theme === 'dark' || theme === 'black'
}

export type PdfThemeLook = {
	/** The reader's background around the pages */
	background: string
	/** The theme's text colour, used for underline, strikethrough and notes */
	text: string
	/** CSS filter for the page picture */
	imageFilter: string | undefined
	/** A colour multiplied over the picture (sepia), or `undefined` */
	tint: string | undefined
	/** The page's own background while its picture loads */
	pagePlaceholder: string
	/** How a highlight blends with the page underneath */
	highlightBlend: CSSProperties['mixBlendMode']
}

/**
 * How pages look in a reader theme. Dark themes invert the page picture (black text on white
 * becomes light text on black; `hue-rotate` keeps colours roughly their hue) unless the user
 * turned that off for illustrated books; then only the frame is dark.
 */
export function pdfThemeLook(theme: PdfTheme, invertPagesInDark: boolean): PdfThemeLook {
	const colors = READER_THEME_COLORS[theme]
	const dark = isDarkTheme(theme)
	const invert = dark && invertPagesInDark

	let imageFilter: string | undefined
	if (invert) {
		imageFilter =
			theme === 'black'
				? 'invert(1) hue-rotate(180deg)'
				: 'invert(1) hue-rotate(180deg) brightness(0.9)'
	}

	return {
		background: colors.background,
		text: colors.text,
		imageFilter,
		tint: theme === 'sepia' ? colors.background : undefined,
		pagePlaceholder: invert
			? theme === 'black'
				? '#000000'
				: '#1c1d1f'
			: theme === 'sepia'
				? colors.background
				: '#ffffff',
		highlightBlend: invert ? 'screen' : 'multiply',
	}
}
