import type { ReaderLocator, ToolbarAnchor } from '../annotations'
import { type FractionRect, mergeLineRects, toPageFractions } from './geometry'
import { collapseWhitespace, type PageString, quoteContext } from './textLayout'

/** The attribute on a page's root element holding its 1-based page number */
export const PAGE_ATTRIBUTE = 'data-pdf-page'
/** The attribute on a text span holding the PDFium index of its first char */
export const CHAR_ATTRIBUTE = 'data-char'

/** A text selection on one PDF page, before it becomes a locator */
export type PdfSelection = {
	page: number
	/** `[start, end)` PDFium char range on the page */
	chars: [number, number]
	text: string
	rects: FractionRect[]
	/** Viewport box of the selection, for the floating toolbar */
	anchor: ToolbarAnchor
}

function closestElement(node: Node | null): Element | null {
	if (!node) return null
	return node.nodeType === Node.ELEMENT_NODE ? (node as Element) : node.parentElement
}

function pageElementOf(node: Node | null): HTMLElement | null {
	return closestElement(node)?.closest<HTMLElement>(`[${PAGE_ATTRIBUTE}]`) ?? null
}

function charSpanOf(node: Node | null): HTMLElement | null {
	return closestElement(node)?.closest<HTMLElement>(`[${CHAR_ATTRIBUTE}]`) ?? null
}

function spanStart(span: Element): number {
	return Number(span.getAttribute(CHAR_ATTRIBUTE))
}

function spanEnd(span: Element): number {
	return spanStart(span) + (span.textContent?.length ?? 0)
}

/** The text spans of a page, in DOM (reading) order */
function pageSpans(page: Element): HTMLElement[] {
	return Array.from(page.querySelectorAll<HTMLElement>(`[${CHAR_ATTRIBUTE}]`))
}

/**
 * The PDFium char index at a DOM boundary point of a selection. Inside a span it's the
 * span's first char plus the offset; between spans it's the next span's start (for the
 * range start) or the previous span's end (for the range end).
 */
export function boundaryToChar(node: Node, offset: number, edge: 'start' | 'end'): number | null {
	const span = charSpanOf(node)
	if (span) {
		if (node.nodeType === Node.TEXT_NODE) {
			return spanStart(span) + Math.min(offset, node.textContent?.length ?? 0)
		}
		// the span element itself: 0 = before its text, anything else = after
		return offset === 0 ? spanStart(span) : spanEnd(span)
	}

	// a boundary between spans (e.g. in the text layer container, or on a <br>)
	const page = pageElementOf(node)
	if (!page) return null
	const spans = pageSpans(page)
	if (!spans.length) return null

	const reference =
		node.nodeType === Node.ELEMENT_NODE && node.childNodes[offset] ? node.childNodes[offset]! : node
	const position = (span: Element) => {
		const relation = reference.compareDocumentPosition(span)
		return relation & Node.DOCUMENT_POSITION_FOLLOWING ? 1 : -1
	}

	if (edge === 'start') {
		const next = spans.find((span) => span === reference || position(span) > 0)
		return next ? spanStart(next) : spanEnd(spans[spans.length - 1]!)
	}
	const previous = [...spans].reverse().find((span) => span !== reference && position(span) < 0)
	return previous ? spanEnd(previous) : spanStart(spans[0]!)
}

/**
 * Turns the browser's selection into a selection on one PDF page. A selection running onto
 * the next pages is cut at the end of its first page. `null` when nothing is selected in a
 * text layer.
 */
export function readPdfSelection(selection: Selection | null): PdfSelection | null {
	if (!selection || selection.isCollapsed || selection.rangeCount === 0) return null
	const range = selection.getRangeAt(0)

	const pageElement = pageElementOf(range.startContainer) ?? pageElementOf(range.endContainer)
	if (!pageElement) return null
	const page = Number(pageElement.getAttribute(PAGE_ATTRIBUTE))
	if (!Number.isFinite(page) || page < 1) return null

	const clipped = range.cloneRange()
	if (pageElementOf(clipped.endContainer) !== pageElement) {
		const spans = pageSpans(pageElement)
		const last = spans[spans.length - 1]
		if (!last) return null
		clipped.setEndAfter(last)
	}
	if (pageElementOf(clipped.startContainer) !== pageElement) {
		const first = pageSpans(pageElement)[0]
		if (!first) return null
		clipped.setStartBefore(first)
	}

	const start = boundaryToChar(clipped.startContainer, clipped.startOffset, 'start')
	const end = boundaryToChar(clipped.endContainer, clipped.endOffset, 'end')
	if (start == null || end == null || end <= start) return null

	const text = collapseWhitespace(rangeText(clipped))
	if (!text) return null

	const pageBox = pageElement.getBoundingClientRect()
	const clientRects = Array.from(clipped.getClientRects()).filter(
		(rect) => rect.width > 0 && rect.height > 0,
	)
	const rects = mergeLineRects(
		toPageFractions(
			clientRects.map((rect) => ({
				left: rect.left,
				top: rect.top,
				width: rect.width,
				height: rect.height,
			})),
			{ left: pageBox.left, top: pageBox.top, width: pageBox.width, height: pageBox.height },
		),
	)
	if (!rects.length) return null

	const bounds = clipped.getBoundingClientRect()
	return {
		page,
		chars: [start, end],
		text,
		rects,
		anchor: { x: bounds.left, y: bounds.top, width: bounds.width, height: bounds.height },
	}
}

/**
 * A range's text the way it reads: its text nodes, with a line break for each `<br>` (the
 * text layer puts one after each line), which `Range.toString()` would drop.
 */
export function rangeText(range: Range): string {
	const root = range.commonAncestorContainer
	if (root.nodeType === Node.TEXT_NODE) {
		return (root.textContent ?? '').slice(range.startOffset, range.endOffset)
	}
	const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT)
	let text = ''
	for (let node = walker.nextNode(); node; node = walker.nextNode()) {
		if (!range.intersectsNode(node)) continue
		if (node.nodeType === Node.TEXT_NODE) {
			const value = node.textContent ?? ''
			const start = node === range.startContainer ? range.startOffset : 0
			const end = node === range.endContainer ? range.endOffset : value.length
			text += value.slice(start, end)
		} else if ((node as Element).tagName === 'BR') {
			text += '\n'
		}
	}
	return text
}

export type LocatorContext = {
	mediaId: string
	pageCount: number
	/** The page's label (e.g. "xix"), if the PDF defines labels */
	label?: string | null
	/** The outline entry containing the page */
	chapterTitle?: string | null
	/** The page's text, for the quote's context */
	pageString?: PageString | null
}

/** The locator stored with a PDF annotation (see the reader-annotations API notes) */
export function selectionToLocator(
	selection: PdfSelection,
	context: LocatorContext,
): ReaderLocator {
	const { page, chars, text, rects } = selection
	const progression = rects[0]?.y ?? 0
	const surrounding = context.pageString
		? quoteContext(context.pageString, chars[0], chars[1])
		: null
	return {
		href: `page/${page}`,
		type: 'application/pdf',
		title: context.label || String(page),
		chapterTitle: context.chapterTitle ?? '',
		locations: {
			position: page,
			progression,
			totalProgression: totalProgression(page, progression, context.pageCount),
			fragments: [`page=${page}`, `chars=${chars[0]}-${chars[1]}`],
			rects,
		},
		text: {
			highlight: text,
			before: surrounding?.before || undefined,
			after: surrounding?.after || undefined,
		},
	}
}

/** `(page - 1 + progression) / pageCount`, rounded, used to sort the annotations list */
export function totalProgression(page: number, progression: number, pageCount: number): number {
	if (pageCount <= 0) return 0
	const value = (page - 1 + Math.min(1, Math.max(0, progression))) / pageCount
	return Math.round(Math.min(1, Math.max(0, value)) * 1e6) / 1e6
}

/** The `chars=` range in a locator's fragments, if any */
export function locatorChars(fragments: string[] | null | undefined): [number, number] | null {
	const fragment = fragments?.find((value) => value.startsWith('chars='))
	const match = fragment?.match(/^chars=(\d+)-(\d+)$/)
	if (!match) return null
	return [Number(match[1]), Number(match[2])]
}
