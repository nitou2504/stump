import { cn } from '@stump/components'
import { AnnotationStyle } from '@stump/graphql'
import { StickyNote } from 'lucide-react'
import { type CSSProperties, memo } from 'react'

import { HIGHLIGHT_TINT, type ReaderAnnotation } from '../annotations'
import { boundingRect, type FractionRect, fractionRectStyle } from './geometry'
import type { PdfThemeLook } from './pdfThemes'

type Props = {
	annotations: ReaderAnnotation[]
	look: PdfThemeLook
	/** The annotation whose toolbar is open, outlined */
	selectedId?: string | null
	/** Search hits to flash on this page */
	flashRects?: FractionRect[] | null
	onOpenAnnotation: (annotationId: string, anchor: DOMRect) => void
}

/**
 * Draws a page's annotations over its picture, in page fractions so they follow the zoom:
 * a yellow highlight, an underline or strikethrough in the theme's text colour, or an
 * inverted box. Annotations with a note get a small note glyph after their last line.
 * The layer doesn't take pointer events (the text under it stays selectable); taps on
 * annotations are hit-tested by the page.
 */
function PdfAnnotationLayer({
	annotations,
	look,
	selectedId,
	flashRects,
	onOpenAnnotation,
}: Props) {
	return (
		<>
			<div className="pdf-annotation-layer" aria-hidden>
				{annotations.map((annotation) => {
					const rects = annotation.locator.locations?.rects ?? []
					return rects.map((rect, index) => (
						<div
							key={`${annotation.id}-${index}`}
							className={cn('pdf-annotation', {
								'pdf-annotation--selected': annotation.id === selectedId,
							})}
							data-annotation-id={annotation.id}
							data-style={annotation.style}
							style={{ ...fractionRectStyle(rect), ...annotationStyle(annotation.style, look) }}
						/>
					))
				})}
				{flashRects?.map((rect, index) => (
					<div
						key={`flash-${index}`}
						className="pdf-search-flash"
						style={fractionRectStyle(rect)}
					/>
				))}
			</div>
			<div className="pdf-note-layer">
				{annotations
					.filter((annotation) => !!annotation.annotationText)
					.map((annotation) => {
						const rects = annotation.locator.locations?.rects ?? []
						const last = rects[rects.length - 1]
						const box = boundingRect(rects)
						if (!last || !box) return null
						return (
							<button
								key={annotation.id}
								type="button"
								className="pdf-note-glyph"
								title={annotation.annotationText ?? undefined}
								aria-label={annotation.annotationText ?? undefined}
								data-annotation-id={annotation.id}
								style={{
									left: `min(calc(${(last.x + last.width) * 100}% + 2px), calc(100% - 18px))`,
									top: `${last.y * 100}%`,
									color: look.text,
								}}
								onClick={(event) => {
									event.stopPropagation()
									onOpenAnnotation(annotation.id, event.currentTarget.getBoundingClientRect())
								}}
							>
								<StickyNote className="h-3.5 w-3.5" />
							</button>
						)
					})}
			</div>
		</>
	)
}

function annotationStyle(style: AnnotationStyle, look: PdfThemeLook): CSSProperties {
	switch (style) {
		case AnnotationStyle.Underline:
			return {
				background: `linear-gradient(to top, ${look.text} 0, ${look.text} 2px, transparent 2px)`,
			}
		case AnnotationStyle.Strikethrough:
			return {
				background: `linear-gradient(to bottom, transparent calc(55% - 1px), ${look.text} calc(55% - 1px), ${look.text} calc(55% + 1px), transparent calc(55% + 1px))`,
			}
		case AnnotationStyle.Invert:
			// `difference` with white inverts exactly the pixels under the box, in any theme
			return { background: '#ffffff', mixBlendMode: 'difference' }
		case AnnotationStyle.Highlight:
		default:
			return {
				background: HIGHLIGHT_TINT,
				opacity: look.highlightOpacity,
				mixBlendMode: look.highlightBlend,
			}
	}
}

export default memo(PdfAnnotationLayer)
