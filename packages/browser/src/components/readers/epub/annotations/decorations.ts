import { type Decoration, DecorationStyleType } from '@readium/navigator'
import { Locator, LocatorLocations, LocatorText } from '@readium/shared'
import { AnnotationStyle } from '@stump/graphql'

import { annotationTint } from '../../annotations/styles'
import type { ReaderLocator } from '../context'
import { hrefsMatch } from '../readium/locator'
import type { EpubAnnotation } from './types'

/** The reader theme's colours, which underline, strikethrough and invert are drawn with */
export type DecorationTheme = { text: string; background: string }

const LIGHT_THEME: DecorationTheme = { text: '#161719', background: '#ffffff' }

/**
 * The Readium decoration style for an annotation style. With the CSS Custom Highlight API
 * (every current browser), a highlight draws `tint` behind the text and picks black or white
 * text for contrast, so `invert` is a highlight in the theme's text colour: the text comes
 * out in (about) the theme's background colour. Highlights keep their exact colour (Readium
 * would otherwise darken a light yellow on a light page into olive).
 */
export function decorationStyleFor(style: AnnotationStyle, theme: DecorationTheme = LIGHT_THEME) {
	const tint = annotationTint(style, theme)
	switch (style) {
		case AnnotationStyle.Underline:
			return { type: DecorationStyleType.Underline, tint }
		case AnnotationStyle.Strikethrough:
			return { type: DecorationStyleType.Strikethrough, tint }
		case AnnotationStyle.Invert:
			return { type: DecorationStyleType.Highlight, tint, enforceContrast: false }
		case AnnotationStyle.Highlight:
		default:
			return { type: DecorationStyleType.Highlight, tint, enforceContrast: false }
	}
}

/**
 * Map a persisted annotation into a Readium Decoration for the navigator.
 */
export function annotationToDecoration(
	annotation: EpubAnnotation,
	theme: DecorationTheme = LIGHT_THEME,
	positions: Locator[] = [],
): Decoration | null {
	const toolkit = readerLocatorToToolkit(annotation.locator, positions)
	if (!toolkit) return null

	return {
		id: annotation.id,
		locator: toolkit,
		style: decorationStyleFor(annotation.style, theme),
		extras: {
			annotationText: annotation.annotationText ?? null,
			mediaId: annotation.mediaId,
		},
	}
}

export function annotationsToDecorations(
	annotations: EpubAnnotation[],
	theme: DecorationTheme = LIGHT_THEME,
	positions: Locator[] = [],
): Decoration[] {
	return annotations
		.map((annotation) => annotationToDecoration(annotation, theme, positions))
		.filter((d): d is Decoration => d != null)
}

export function readerLocatorToToolkit(
	locator: ReaderLocator,
	positions: Locator[] = [],
): Locator | null {
	if (!locator.href) return null

	let href = locator.href
	if (positions.length > 0) {
		const match = positions.find((p) => hrefsMatch(p.href, href))
		if (match) href = match.href
	}

	return new Locator({
		href,
		type: locator.type || 'application/xhtml+xml',
		title: locator.title ?? locator.chapterTitle ?? undefined,
		locations: new LocatorLocations({
			fragments: locator.locations?.fragments ? [...locator.locations.fragments] : undefined,
			position: locator.locations?.position ?? undefined,
			progression: safeNumber(locator.locations?.progression),
			totalProgression: safeNumber(locator.locations?.totalProgression),
		}),
		text: locator.text
			? new LocatorText({
					after: locator.text.after ?? undefined,
					before: locator.text.before ?? undefined,
					highlight: locator.text.highlight ?? undefined,
				})
			: undefined,
	})
}

function safeNumber(value: unknown): number | undefined {
	if (value == null) return undefined
	const num = Number(value)
	return Number.isFinite(num) ? num : undefined
}
