import { AnnotationStyle } from '@stump/graphql'
import { Contrast, Highlighter, type LucideIcon, Strikethrough, Underline } from 'lucide-react'

export type AnnotationStyleMeta = {
	style: AnnotationStyle
	/** The key that applies this style to the selection or the selected annotation */
	shortcut: string
	icon: LucideIcon
	/** i18n key of the style's name */
	labelKey: string
}

/** The annotation styles, in toolbar order. Highlight is the primary one */
export const ANNOTATION_STYLES: AnnotationStyleMeta[] = [
	{
		style: AnnotationStyle.Highlight,
		shortcut: 'h',
		icon: Highlighter,
		labelKey: 'shared.readerAnnotations.styles.highlight',
	},
	{
		style: AnnotationStyle.Underline,
		shortcut: 'u',
		icon: Underline,
		labelKey: 'shared.readerAnnotations.styles.underline',
	},
	{
		style: AnnotationStyle.Strikethrough,
		shortcut: 's',
		icon: Strikethrough,
		labelKey: 'shared.readerAnnotations.styles.strikethrough',
	},
	{
		style: AnnotationStyle.Invert,
		shortcut: 'i',
		icon: Contrast,
		labelKey: 'shared.readerAnnotations.styles.invert',
	},
]

export const NOTE_SHORTCUT = 'n'
export const BOOKMARK_SHORTCUT = 'b'
export const HELP_SHORTCUT = '?'

export function annotationStyleMeta(
	style: AnnotationStyle | null | undefined,
): AnnotationStyleMeta {
	return ANNOTATION_STYLES.find((meta) => meta.style === style) ?? ANNOTATION_STYLES[0]!
}

/**
 * The colours an annotation is drawn with, for a reader theme's text and background
 * colours. No colour picker: a highlight is yellow, and the other styles use the text
 * colour, so they follow the theme (light, sepia, dark, black).
 */
export function annotationTint(
	style: AnnotationStyle,
	theme: { text: string; background: string },
): string {
	switch (style) {
		case AnnotationStyle.Highlight:
			return HIGHLIGHT_TINT
		case AnnotationStyle.Invert:
		case AnnotationStyle.Underline:
		case AnnotationStyle.Strikethrough:
		default:
			return theme.text
	}
}

export const HIGHLIGHT_TINT = '#FACC15'
