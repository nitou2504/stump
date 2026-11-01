import type { AnnotationStyle } from '@stump/graphql'

/** A box on a fixed-layout page, in fractions (0-1) of the page's width and height */
export type ReaderLocatorRect = { x: number; y: number; width: number; height: number }

/**
 * A position in a book, shaped like a Readium locator. EPUBs locate text with `href`,
 * `locations.progression` and the `text` quote; fixed-layout books (PDF) use
 * `locations.position` for the page and `locations.rects` for the selected boxes.
 */
export type ReaderLocator = {
	href: string
	type: string
	title?: string
	chapterTitle?: string
	locations?: {
		fragments?: string[] | null
		progression?: number | null
		position?: number | null
		totalProgression?: number | null
		cssSelector?: string | null
		rects?: ReaderLocatorRect[] | null
	} | null
	text?: {
		after?: string | null
		before?: string | null
		highlight?: string | null
	} | null
}

/** A user's annotation in any reader: a styled range of text with an optional note */
export type ReaderAnnotation = {
	id: string
	mediaId: string
	userId: string
	annotationText?: string | null
	style: AnnotationStyle
	createdAt: string
	updatedAt: string
	locator: ReaderLocator
}

/** A box in viewport (CSS px) coordinates that a floating toolbar is placed next to */
export type ToolbarAnchor = { x: number; y: number; width: number; height: number }

/** The text a user selected in a reader, ready to become an annotation */
export type AnnotationSelection = {
	/** Viewport-relative bounding box of the selection, for the floating toolbar */
	rect: ToolbarAnchor
	/** Canonical locator ready for persistence / decoration */
	locator: ReaderLocator
	/** Selected plain text */
	text: string
}

/** What the floating annotation toolbar is acting on */
export type AnnotationTarget =
	| { kind: 'selection'; selection: AnnotationSelection }
	| { kind: 'annotation'; annotationId: string; rect: ToolbarAnchor }
