import { type IEpubPreferences, TextAlignment } from '@readium/navigator'
import type { EpubReaderTheme, EpubTextAlign } from '@stump/client'
import { ReadingMode, SupportedFont } from '@stump/graphql'

import { toFamilyName } from '../themes'

type ColumnCountPreference = 'auto' | 1 | 2

type PreferenceSource = {
	fontSize?: number
	lineHeight?: number
	fontFamily?: string
	readingMode?: ReadingMode
	isDarkVariant?: boolean
	/**
	 * Desired paginated column count. `'auto'` picks 1 or 2 columns from `stageWidth`.
	 * Has no effect when `readingMode` is continuous scroll.
	 */
	columnCount?: ColumnCountPreference
	/**
	 * Margin (roughly rem) to apply around page content. Readium's `EpubPreferences` has no
	 * dedicated "page margins" concept — the closest analog is `pageGutter`, the space between
	 * columns — so we map it there. Centering the viewport itself is handled by the caller via
	 * CSS `max-width`, not by this mapping.
	 */
	pageMargins?: number
	/**
	 * Current stage (viewport) width in CSS px, used to resolve `'auto'` column count.
	 */
	stageWidth?: number
	/** The reader colour theme; `auto` follows `isDarkVariant` */
	theme?: EpubReaderTheme
	textAlign?: EpubTextAlign
}

type ThemeColors = { background: string; text: string; link: string }

/** The reader colour themes. `auto` resolves to light or dark with the app theme. */
export const READER_THEME_COLORS: Record<Exclude<EpubReaderTheme, 'auto'>, ThemeColors> = {
	light: { background: '#ffffff', text: '#161719', link: '#2563eb' },
	sepia: { background: '#f4ecd8', text: '#5b4636', link: '#8a5a2b' },
	dark: { background: '#161719', text: '#E8EDF4', link: '#4299E1' },
	black: { background: '#000000', text: '#d4d4d4', link: '#60a5fa' },
}

export function resolveReaderTheme(
	theme: EpubReaderTheme | undefined,
	isDarkVariant: boolean | undefined,
): ThemeColors {
	if (!theme || theme === 'auto') {
		return READER_THEME_COLORS[isDarkVariant ? 'dark' : 'light']
	}
	return READER_THEME_COLORS[theme] ?? READER_THEME_COLORS.light
}

const DEFAULT_PX = 16

/** Below this stage width two columns are too narrow, so one is shown */
const TWO_COLUMN_MIN_WIDTH = 900

/**
 * Resolve an effective Readium `columnCount` from a Stump column preference: two columns (a
 * two-page spread) only when picked and the stage is wide enough, otherwise one. `'auto'`
 * (the old default, which picked two on wide screens) reads as one, like the other readers'
 * single-page default.
 *
 * Continuous scroll is represented elsewhere by `scroll: true` (Readium scrolls one column);
 * see {@link bookPreferencesToEpubPreferences}, which clears the column count for it.
 */
export function resolveEffectiveColumnCount(
	columnCount: ColumnCountPreference | undefined,
	stageWidth: number | undefined,
): number | null {
	if (columnCount !== 2) return 1
	if (stageWidth == null) return 1
	return stageWidth < TWO_COLUMN_MIN_WIDTH ? 1 : 2
}

/**
 * Map Stump book preferences to Readium EpubPreferences.
 * Stored fontSize is px; Readium expects a scale factor around 1.0.
 */
export function bookPreferencesToEpubPreferences({
	fontSize,
	lineHeight,
	fontFamily,
	readingMode,
	isDarkVariant,
	columnCount,
	pageMargins,
	stageWidth,
	theme,
	textAlign,
}: PreferenceSource): IEpubPreferences {
	const isContinuous = readingMode === ReadingMode.ContinuousVertical

	const preferences: IEpubPreferences = {
		scroll: isContinuous,
		// Paginated: resolve to an explicit column count so resize does not auto-reflow into
		// arbitrary CSS columns. Continuous scroll does not paginate, so `columnCount` is
		// explicitly cleared to `null` (not `undefined`) — Readium's `EpubPreferences.merging`
		// only overwrites a key when the incoming value is not `undefined`, so `null` is required
		// to actually clear a previously-set explicit column count when switching modes.
		columnCount: isContinuous ? null : resolveEffectiveColumnCount(columnCount, stageWidth),
	}

	if (fontSize != null && fontSize > 0) {
		preferences.fontSize = clamp(fontSize / DEFAULT_PX, 0.7, 4)
	}

	if (lineHeight != null && lineHeight > 0) {
		preferences.lineHeight = clamp(lineHeight, 1, 2.5)
	}

	preferences.fontFamily = fontFamily ? toFamilyName(fontFamily as SupportedFont) : null

	if (!isContinuous && pageMargins != null && pageMargins > 0) {
		preferences.pageGutter = clamp(pageMargins * DEFAULT_PX, 0, DEFAULT_PX * 4)
	}

	const colors = resolveReaderTheme(theme, isDarkVariant)
	preferences.backgroundColor = colors.background
	preferences.textColor = colors.text
	preferences.linkColor = colors.link

	// `null` (not `undefined`) so switching back to `auto` clears a previous override
	preferences.textAlign =
		textAlign === 'justify'
			? TextAlignment.justify
			: textAlign === 'start'
				? TextAlignment.start
				: null
	preferences.hyphens = textAlign === 'justify' ? true : null

	return preferences
}

function clamp(value: number, min: number, max: number): number {
	return Math.min(max, Math.max(min, value))
}

/**
 * Collect origins that must be allowed in the Readium iframe CSP / injectables.
 * Prefer origins found on the RWPM absolute hrefs, and always include the app origin
 * so injected font stylesheets work.
 */
export function collectAllowedDomains(
	hrefs: Array<string | undefined | null>,
	extraOrigins: Array<string | undefined | null> = [],
): string[] {
	const origins = new Set<string>()

	for (const href of hrefs) {
		if (!href) continue
		try {
			origins.add(new URL(href).origin)
		} catch {
			// ignore relative / invalid
		}
	}

	for (const origin of extraOrigins) {
		if (!origin) continue
		try {
			origins.add(new URL(origin).origin)
		} catch {
			origins.add(origin)
		}
	}

	if (typeof window !== 'undefined') {
		origins.add(window.location.origin)
	}

	return [...origins]
}
