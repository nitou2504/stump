import { DecorationStyleType } from '@readium/navigator'
import { AnnotationStyle } from '@stump/graphql'

import { HIGHLIGHT_TINT } from '../../../annotations/styles'
import {
	annotationsToDecorations,
	annotationToDecoration,
	decorationStyleFor,
} from '../decorations'
import type { EpubAnnotation } from '../types'

const SEPIA = { text: '#5b4636', background: '#f4ecd8' }

function buildAnnotation(overrides: Partial<EpubAnnotation> = {}): EpubAnnotation {
	return {
		id: 'annotation-1',
		mediaId: 'media-1',
		userId: 'user-1',
		annotationText: null,
		style: AnnotationStyle.Highlight,
		createdAt: '2026-01-01T00:00:00Z',
		updatedAt: '2026-01-01T00:00:00Z',
		locator: {
			href: '/resource/chapter-1.xhtml',
			type: 'application/xhtml+xml',
			chapterTitle: 'Chapter One',
			locations: { position: 1, progression: 0.1 },
			text: { highlight: 'highlighted text' },
		},
		...overrides,
	}
}

describe('annotationToDecoration', () => {
	it('maps the annotation id, style, and highlight text onto a Decoration', () => {
		const decoration = annotationToDecoration(buildAnnotation())

		expect(decoration?.id).toBe('annotation-1')
		expect(decoration?.style).toEqual({
			type: DecorationStyleType.Highlight,
			tint: HIGHLIGHT_TINT,
			enforceContrast: false,
		})
		expect(decoration?.locator.href).toBe('/resource/chapter-1.xhtml')
		expect(decoration?.locator.text?.highlight).toBe('highlighted text')
		expect(decoration?.extras).toEqual({ annotationText: null, mediaId: 'media-1' })
	})

	it('carries the note text through to decoration extras', () => {
		const decoration = annotationToDecoration(buildAnnotation({ annotationText: 'my note' }))
		expect(decoration?.extras).toMatchObject({ annotationText: 'my note' })
	})

	it('returns null when the locator has no href to anchor the decoration', () => {
		const decoration = annotationToDecoration(
			buildAnnotation({ locator: { href: '', type: 'application/xhtml+xml' } }),
		)
		expect(decoration).toBeNull()
	})
})

describe('decorationStyleFor', () => {
	it('draws a highlight in yellow whatever the theme', () => {
		expect(decorationStyleFor(AnnotationStyle.Highlight, SEPIA)).toEqual({
			type: DecorationStyleType.Highlight,
			tint: HIGHLIGHT_TINT,
			enforceContrast: false,
		})
	})

	it('draws underline and strikethrough in the theme text colour', () => {
		expect(decorationStyleFor(AnnotationStyle.Underline, SEPIA)).toEqual({
			type: DecorationStyleType.Underline,
			tint: SEPIA.text,
		})
		expect(decorationStyleFor(AnnotationStyle.Strikethrough, SEPIA)).toEqual({
			type: DecorationStyleType.Strikethrough,
			tint: SEPIA.text,
		})
	})

	it('draws invert as a highlight in the text colour, without contrast adjustment', () => {
		expect(decorationStyleFor(AnnotationStyle.Invert, SEPIA)).toEqual({
			type: DecorationStyleType.Highlight,
			tint: SEPIA.text,
			enforceContrast: false,
		})
	})
})

describe('annotationsToDecorations', () => {
	it('maps a list of annotations, skipping any without a resolvable locator', () => {
		const annotations = [
			buildAnnotation({ id: 'a' }),
			buildAnnotation({ id: 'b', locator: { href: '', type: 'application/xhtml+xml' } }),
			buildAnnotation({ id: 'c', style: AnnotationStyle.Invert }),
		]

		const decorations = annotationsToDecorations(annotations, SEPIA)

		expect(decorations.map((decoration) => decoration.id)).toEqual(['a', 'c'])
		expect(decorations[1]?.style).toMatchObject({ tint: SEPIA.text })
	})

	it('returns an empty array for an empty annotation list', () => {
		expect(annotationsToDecorations([])).toEqual([])
	})
})
